//! Launching the terminal UI (`launch_ui`): OS media integrations, the
//! persisted-session restore, and the exit path.

use super::bootstrap::Boot;
use super::pump::start_tokio;
use crate::core::app::App;
use crate::core::driver::{Driver, MprisHandle};
use crate::core::user_config::{StartupBehavior, UserConfig};
#[cfg(feature = "discord-rpc")]
use crate::infra::discord_rpc;
#[cfg(all(feature = "mpris", target_os = "linux"))]
use crate::infra::mpris;
use crate::infra::network::{IoEvent, Network};
#[cfg(feature = "tui")]
use crate::tui::runner;
#[cfg(feature = "tui")]
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "discord-rpc")]
type DiscordRpcHandle = Option<discord_rpc::DiscordRpcManager>;
#[cfg(not(feature = "discord-rpc"))]
type DiscordRpcHandle = Option<()>;

#[cfg(feature = "discord-rpc")]
const DEFAULT_DISCORD_CLIENT_ID: &str = "1464235043462447166";

#[cfg(feature = "discord-rpc")]
fn resolve_discord_app_id(user_config: &UserConfig) -> Option<String> {
  std::env::var("QBAL_DISCORD_APP_ID")
    .ok()
    .filter(|value| !value.trim().is_empty())
    .or_else(|| user_config.behavior.discord_rpc_client_id.clone())
    .or_else(|| Some(DEFAULT_DISCORD_CLIENT_ID.to_string()))
}

/// What `prepare_frontend` hands the frontend that runs next.
pub(super) struct FrontendHandles {
  pub(super) app: Arc<Mutex<App>>,
  pub(super) user_config: UserConfig,
  pub(super) mpris: MprisHandle,
  pub(super) discord: DiscordRpcHandle,
  pub(super) history_collector: crate::infra::history::HistoryCollectorHandle,
}

/// OS media integrations, deferred streaming, session restore and the pump task, shared by every frontend.
pub(super) async fn prepare_frontend(boot: Boot) -> FrontendHandles {
  let app = boot.app;
  let sync_io_rx = boot.sync_io_rx;
  let user_config = boot.user_config;
  let restore_playback = boot.restore_playback;
  let restore_queue = boot.restore_queue;
  let initial_startup_behavior = boot.initial_startup_behavior;

  let history_collector = crate::infra::history::spawn_history_collector(Arc::clone(&app));

  // Initialize MPRIS D-Bus integration for desktop media control
  // This registers qbal as a controllable media player on the session bus
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let mpris_manager: Option<Arc<mpris::MprisManager>> = match mpris::MprisManager::new() {
    Ok(mgr) => {
      info!("mpris d-bus interface registered - media keys and playerctl enabled");
      Some(Arc::new(mgr))
    }
    Err(e) => {
      info!(
        "failed to initialize mpris: {} - media key control disabled",
        e
      );
      None
    }
  };

  // Store MPRIS manager reference in App for emitting Seeked signals from native seeks
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  {
    let mut app_mut = app.lock().await;
    app_mut.mpris_manager = mpris_manager.clone();
  }

  #[cfg(feature = "discord-rpc")]
  let discord_rpc_manager: DiscordRpcHandle = if user_config.behavior.enable_discord_rpc {
    match resolve_discord_app_id(&user_config)
      .and_then(|app_id| discord_rpc::DiscordRpcManager::new(app_id).ok())
    {
      Some(mgr) => {
        info!("discord rich presence enabled");
        Some(mgr)
      }
      None => {
        info!("discord rich presence failed to initialize");
        None
      }
    }
  } else {
    info!("discord rich presence disabled");
    None
  };
  #[cfg(not(feature = "discord-rpc"))]
  let discord_rpc_manager: DiscordRpcHandle = None;

  // Spawn MPRIS event handler to process external control requests (media keys, playerctl)
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  if let Some(ref mpris) = mpris_manager {
    if let Some(event_rx) = mpris.take_event_rx() {
      let mpris_for_seek = Arc::clone(mpris);
      let app_for_mpris = Arc::clone(&app);
      tokio::spawn(async move {
        handle_mpris_events(event_rx, mpris_for_seek, app_for_mpris).await;
      });
    }
  }

  // Clone MPRIS manager for UI loop (to update status on device changes)
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let mpris_for_ui = mpris_manager.clone();

  let cloned_app = Arc::clone(&app);
  info!("spawning the network event handler");
  tokio::spawn(async move {
    let mut network = Network::new(&app);

    // The saved-device startup decision moved into
    // `deferred_streaming_startup` (it needs the native player's device
    // name, which now materializes in the background).

    // Restore the persisted native queue into app state before the runner
    // starts, independent of whether a playlist is resumed.
    if !restore_queue.is_empty() {
      network.app.lock().await.native_queue = restore_queue;
    }
    if let Some(session) = restore_playback {
      // Resume off the event pump: the download must not stall the startup
      // events the UI's first render queues. The restore drives the Qobuz
      // start path, which serializes on the `App` lock like every other event,
      // so running it concurrently is safe. It honors `startup_behavior` for
      // its play/pause decision.
      let restore_app = Arc::clone(&network.app);
      tokio::spawn(async move {
        restore_playback_session(&restore_app, session, initial_startup_behavior).await;
      });
    }

    start_tokio(sync_io_rx, &mut network).await;
  });
  FrontendHandles {
    app: cloned_app,
    user_config,
    #[cfg(all(feature = "mpris", target_os = "linux"))]
    mpris: mpris_for_ui,
    #[cfg(not(all(feature = "mpris", target_os = "linux")))]
    mpris: None,
    discord: discord_rpc_manager,
    history_collector,
  }
}

/// Launch the terminal UI on the shared frontend runtime.
#[cfg(feature = "tui")]
pub(super) async fn launch_ui(boot: Boot) -> Result<()> {
  info!("launching interactive terminal ui");
  let FrontendHandles {
    app,
    user_config,
    mpris,
    discord,
    history_collector,
  } = prepare_frontend(boot).await;
  // The UI must run in the "main" thread
  info!("starting terminal ui event loop");
  info!("ui thread initialized");
  // The driver owns everything the app must do on a timer (see
  // `core::driver`); the terminal loop's job shrinks to drawing frames,
  // reading events, and calling `driver.tick` at the configured tick rate.
  let mut driver = Driver::new(mpris, discord);
  let ui_result = match runner::start_ui(user_config, &app, &mut driver).await {
    Ok(session) => {
      shutdown_frontend(&app, &mut driver, history_collector, |driver| {
        session.restore(driver)
      })
      .await;
      Ok(())
    }
    Err(e) => Err(e),
  };
  drop(driver);
  if ui_result.is_err() {
    let mut app = app.lock().await;
    app.flush_state_save(true);
  }
  ui_result?;

  Ok(())
}

/// The exit path after a clean quit; `restore_frontend` runs where the terminal restore always ran.
pub(super) async fn shutdown_frontend(
  app: &Arc<Mutex<App>>,
  driver: &mut Driver,
  history_collector: crate::infra::history::HistoryCollectorHandle,
  restore_frontend: impl FnOnce(&mut Driver),
) {
  // Capture the exact final position of the session on a graceful
  // quit (the throttled in-loop save is up to a few seconds stale). Done
  // synchronously before teardown so the player is still alive to read from.
  {
    let session = app.lock().await.current_persisted_session();
    if let Some(session) = session {
      if let Ok(path) = crate::core::persisted_playback::default_session_path() {
        if let Err(e) = crate::core::persisted_playback::save(&path, &session) {
          log::warn!("[session] failed to persist playback session on exit: {e}");
        }
      }
    }
  }

  {
    let mut app = app.lock().await;
    driver.on_quit(&mut app);
  }

  // A volume/resize/shuffle change may still be inside its debounce window;
  // persist it before the process exits.
  {
    let mut app = app.lock().await;
    app.flush_state_save(true);
  }

  // Stop the collector and all network work it owns before the final sync and
  // clear. In particular, a pause-triggered now-playing push must not race the
  // exit clear and recreate a stale public widget.
  history_collector.shutdown().await;

  // Restore the frontend before the exit network calls: there is no reason to
  // hold the alternate screen and raw mode while waiting on HTTP.
  restore_frontend(driver);

  driver.clear_presence();
}

/// Resume a persisted playback session at launch.
///
/// Drives the Qobuz start path (seeding the browse table with the persisted
/// metadata so the snapshot resolves), and applies the saved position and the
/// play/pause decision. That decision follows `startup_behavior`: `Play` forces
/// playing, `Pause` forces paused, and `Continue` restores the exact state the
/// session had when it was saved.
///
/// A failed start (a removed track, no network) publishes no session, so
/// startup is never blocked or crashed by a stale session.
#[allow(unused_variables)]
async fn restore_playback_session(
  app: &Arc<Mutex<App>>,
  session: crate::core::persisted_playback::PersistedPlayback,
  startup_behavior: StartupBehavior,
) {
  use crate::core::persisted_playback::PersistedPlayback;

  // Resolve whether the restored track should end up paused.
  let resolve_paused = |saved_paused: bool| match startup_behavior {
    StartupBehavior::Play => false,
    StartupBehavior::Pause => true,
    StartupBehavior::Continue => saved_paused,
  };

  let PersistedPlayback::Qobuz {
    tracks,
    index,
    position_ms,
    paused,
    repeat,
    shuffle_on,
    shuffle,
  } = session;
  let uris: Vec<String> = tracks.iter().filter_map(|t| t.uri.clone()).collect();
  if uris.is_empty() {
    return;
  }
  // Seed the browse table so the start path's snapshot resolves metadata.
  app.lock().await.track_table.tracks = tracks;
  // The download runs off the pump; the seek and pause apply when it plays.
  let resume = crate::infra::qobuz::ResumePoint {
    position_ms,
    paused: resolve_paused(paused),
  };
  crate::infra::qobuz::dispatch::start_qobuz_queue(app, &uris, index, Some(resume)).await;
  let mut guard = app.lock().await;
  if guard.qobuz_playback.is_some() {
    guard.decoded_repeat = repeat;
    guard.decoded_shuffle = shuffle_on;
  }
  if let Some(s) = guard.qobuz_playback.as_mut() {
    s.shuffle_backup = shuffle;
  }
}

/// Handle MPRIS events from external clients (media keys, playerctl, desktop
/// widgets). While a decoded owner (Qobuz or the queue slot) holds the sink,
/// `route_decoded_mpris_event` sends the event through the same `IoEvent`s the
/// keyboard uses; otherwise nothing is playing and the event is answered here.
#[cfg(all(feature = "mpris", target_os = "linux"))]
async fn handle_mpris_events(
  mut event_rx: tokio::sync::mpsc::UnboundedReceiver<mpris::MprisEvent>,
  mpris_manager: Arc<mpris::MprisManager>,
  app: Arc<Mutex<App>>,
) {
  use mpris::MprisEvent;

  while let Some(event) = event_rx.recv().await {
    if !app.lock().await.user_config.behavior.enable_media_keys {
      continue;
    }
    if route_decoded_mpris_event(&event, &app, &mpris_manager).await {
      continue;
    }

    let mut app_lock = app.lock().await;
    match event {
      // The same toggle as the TUI's play/pause, so it follows the owner.
      MprisEvent::PlayPause => app_lock.toggle_playback(),
      MprisEvent::Next => app_lock.next_track(),
      MprisEvent::Previous => app_lock.previous_track(),
      MprisEvent::SetVolume(volume_percent) => app_lock.set_volume_percent(volume_percent),
      // Nothing owns the sink: correct the client's optimistic flip back to
      // what the snapshot reports, then say so.
      MprisEvent::SetShuffle(_) => {
        drop(app_lock);
        mpris_manager.set_shuffle(false);
      }
      MprisEvent::SetLoopStatus(_) => {
        drop(app_lock);
        mpris_manager.set_loop_status(mpris::LoopStatusEvent::None);
      }
      MprisEvent::Play
      | MprisEvent::Pause
      | MprisEvent::Stop
      | MprisEvent::Seek(_)
      | MprisEvent::SetPosition(_) => app_lock.report_nothing_playing(),
    }
  }
}

/// Route an MPRIS event through the standard dispatch path while a decoded
/// owner holds the sink. Returns `true` if the event was consumed.
/// Play/pause/next/previous/stop/seek map onto the same `IoEvent`s the keyboard
/// uses; the queue and Qobuz routers in the pump take them from there.
#[cfg(all(feature = "mpris", target_os = "linux"))]
async fn route_decoded_mpris_event(
  event: &mpris::MprisEvent,
  app: &Arc<Mutex<App>>,
  mpris_manager: &Arc<mpris::MprisManager>,
) -> bool {
  use mpris::MprisEvent;

  let mut app_lock = app.lock().await;

  // The native queue slot owns playback: shuffle/repeat mean nothing over an
  // explicit queue, so reject them, as the keyboard's `App::shuffle` /
  // `App::repeat` do. The corrective push matches what the snapshot reports for
  // a queue slot, so the runner's dedup cache converges instead of stranding
  // the property.
  if app_lock.queue_owns_playback() {
    match event {
      MprisEvent::SetShuffle(_) => {
        drop(app_lock);
        mpris_manager.set_shuffle(false);
        return true;
      }
      MprisEvent::SetLoopStatus(_) => {
        drop(app_lock);
        mpris_manager.set_loop_status(mpris::LoopStatusEvent::None);
        return true;
      }
      _ => {}
    }
  }

  // Read the live source-player state up front, then drop the borrow so the
  // immutable read does not conflict with the `&mut self` dispatch calls below.
  // A decoded claim with no player (a start in flight, a lost output device)
  // consumes the event: nothing can serve it yet. Volume still falls through
  // to `set_volume_percent`, which is owner-aware.
  let Some(player) = app_lock.active_decoded_player() else {
    return !matches!(event, MprisEvent::SetVolume(_)) && app_lock.active_decoded_source();
  };
  let is_paused = player.is_paused();
  let position_ms = player.position().as_millis() as i64;

  match event {
    MprisEvent::PlayPause => {
      if is_paused {
        app_lock.dispatch(IoEvent::StartPlayback(None, None, None));
      } else {
        app_lock.dispatch(IoEvent::PausePlayback);
      }
      true
    }
    MprisEvent::Play => {
      app_lock.dispatch(IoEvent::StartPlayback(None, None, None));
      true
    }
    MprisEvent::Pause | MprisEvent::Stop => {
      app_lock.dispatch(IoEvent::PausePlayback);
      true
    }
    MprisEvent::Next => {
      app_lock.dispatch(IoEvent::NextTrack);
      true
    }
    MprisEvent::Previous => {
      app_lock.dispatch(IoEvent::PreviousTrack);
      true
    }
    MprisEvent::Seek(offset_micros) => {
      let offset_ms = offset_micros / 1000;
      let new_position_ms = (position_ms + offset_ms).max(0) as u32;
      app_lock.dispatch(IoEvent::Seek(new_position_ms));
      drop(app_lock);
      mpris_manager.emit_seeked(new_position_ms as u64);
      true
    }
    MprisEvent::SetPosition(position_micros) => {
      let new_position_ms = (position_micros / 1000).max(0) as u32;
      app_lock.dispatch(IoEvent::Seek(new_position_ms));
      drop(app_lock);
      mpris_manager.emit_seeked(new_position_ms as u64);
      true
    }
    // Shuffle/repeat drive the player-global decoded state. A Qobuz session
    // honours them; an owner that cannot is rejected below with a corrective
    // push. Volume is handled by the top-level `set_volume_percent`.
    MprisEvent::SetShuffle(on) => {
      if app_lock.set_decoded_shuffle(*on) {
        drop(app_lock);
        mpris_manager.set_shuffle(*on);
      } else {
        // The owner has no shuffle of its own (the queue slot plays an
        // explicit list). Push the true value back so the client's optimistic
        // flip is corrected:
        // the runner re-pushes only when the snapshot *changes* against its own
        // cache, and it never sees this out-of-band write, so leaving the client
        // at the rejected value would strand the property there for the session.
        drop(app_lock);
        mpris_manager.set_shuffle(false);
      }
      true
    }
    MprisEvent::SetLoopStatus(status) => {
      use crate::infra::queue::RepeatMode;
      let mode = match status {
        mpris::LoopStatusEvent::None => RepeatMode::Off,
        mpris::LoopStatusEvent::Track => RepeatMode::Track,
        mpris::LoopStatusEvent::Playlist => RepeatMode::Context,
      };
      if app_lock.set_decoded_repeat(mode) {
        drop(app_lock);
        mpris_manager.set_loop_status(*status);
      } else {
        // Rejected for the same reason as `SetShuffle` above, and corrected back
        // to the `None` the snapshot reports for these sources.
        drop(app_lock);
        mpris_manager.set_loop_status(mpris::LoopStatusEvent::None);
      }
      true
    }
    MprisEvent::SetVolume(_) => false,
  }
}
