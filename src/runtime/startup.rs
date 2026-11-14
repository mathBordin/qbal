//! Launching an interactive frontend: OS media integrations, the deferred
//! native-streaming startup, persisted-session restore, and the exit path,
//! shared by the terminal UI (`launch_ui`) and the browser frontend.

use super::bootstrap::Boot;
use super::pump::start_tokio;
use crate::core::app::App;
use crate::core::driver::{Driver, MprisHandle};
use crate::core::user_config::{StartupBehavior, UserConfig};
#[cfg(feature = "discord-rpc")]
use crate::infra::discord_rpc;
#[cfg(all(feature = "macos-media", target_os = "macos"))]
use crate::infra::macos_media;
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

#[cfg(all(feature = "macos-media", target_os = "macos"))]
#[derive(Default, PartialEq)]
struct MacosMetadata {
  title: String,
  artists: Vec<String>,
  album: String,
  duration_ms: u32,
  art_url: Option<String>,
}

#[cfg(all(feature = "windows-media", target_os = "windows"))]
#[derive(Default, PartialEq)]
struct WindowsMetadata {
  title: String,
  artists: Vec<String>,
  album: String,
  duration: u64,
  art_url: Option<String>,
}

#[cfg(feature = "discord-rpc")]
fn resolve_discord_app_id(user_config: &UserConfig) -> Option<String> {
  std::env::var("SPOTATUI_DISCORD_APP_ID")
    .ok()
    .filter(|value| !value.trim().is_empty())
    .or_else(|| user_config.behavior.discord_rpc_client_id.clone())
    .or_else(|| Some(DEFAULT_DISCORD_CLIENT_ID.to_string()))
}

#[cfg(all(feature = "macos-media", target_os = "macos"))]
fn update_macos_metadata(
  manager: &macos_media::MacMediaManager,
  last_metadata: &mut Option<MacosMetadata>,
  app: &App,
) {
  // Music publishes its own entry while it owns playback.

  if let Some(snapshot) = crate::infra::media_metadata::current_playback_snapshot(app) {
    let new_metadata = MacosMetadata {
      title: snapshot.metadata.title.clone(),
      artists: snapshot.metadata.artists.clone(),
      album: snapshot.metadata.album.clone(),
      duration_ms: snapshot.metadata.duration_ms,
      art_url: snapshot.metadata.image_url.clone(),
    };

    // Only update if metadata changed to avoid repeated artwork fetches.
    if last_metadata.as_ref() != Some(&new_metadata) {
      manager.set_metadata(
        &snapshot.metadata.title,
        &snapshot.metadata.artists,
        &snapshot.metadata.album,
        snapshot.metadata.duration_ms,
        snapshot.metadata.image_url,
      );
      *last_metadata = Some(new_metadata);
    }
  } else if last_metadata.is_some() {
    *last_metadata = None;
  }
}

/// Returns the snapshot's play state and position, `None` with no snapshot.
#[cfg(all(feature = "windows-media", target_os = "windows"))]
fn update_windows_metadata(
  manager: &smtc_tokio::WindowsMediaManager,
  last_metadata: &mut Option<WindowsMetadata>,
  app: &App,
) -> Option<(bool, u64)> {
  if let Some(snapshot) = crate::infra::media_metadata::current_playback_snapshot(app) {
    let new_metadata = WindowsMetadata {
      title: snapshot.metadata.title.clone(),
      artists: snapshot.metadata.artists.clone(),
      album: snapshot.metadata.album.clone(),
      duration: snapshot.metadata.duration_ms as u64,
      art_url: snapshot.metadata.image_url.clone(),
    };

    if last_metadata.as_ref() != Some(&new_metadata) {
      manager.set_metadata(
        &snapshot.metadata.title,
        &snapshot.metadata.artists,
        &snapshot.metadata.album,
        snapshot.metadata.duration_ms as u64,
        snapshot.metadata.image_url,
      );
      *last_metadata = Some(new_metadata);
    }
    Some((snapshot.is_playing, snapshot.progress_ms as u64))
  } else {
    if last_metadata.is_some() {
      *last_metadata = None;
    }
    None
  }
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
  let _initial_shuffle_enabled = boot.initial_shuffle_enabled;
  let initial_startup_behavior = boot.initial_startup_behavior;

  // Never displaces a message something else just set (they are more urgent
  // than this notice). A Spotify browse scope with no session comes second:
  // say so once, before the first key press reaches the auth gate.
  {
    let app_mut = app.lock().await;
    if app_mut.status_message().is_none() {}
  }

  let history_collector = crate::infra::history::spawn_history_collector(Arc::clone(&app));

  // Create shared atomic for real-time position updates from native player
  // This avoids lock contention - the player event handler can update position
  // without needing to acquire the app mutex
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let shared_position = Arc::new(std::sync::atomic::AtomicU64::new(0));

  // Create shared atomic for playing state (lock-free for MPRIS toggle)
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let shared_is_playing = Arc::new(std::sync::atomic::AtomicBool::new(false));
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let shared_is_playing_for_mpris = Arc::clone(&shared_is_playing);
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let shared_position_for_mpris = Arc::clone(&shared_position);
  #[cfg(all(feature = "macos-media", target_os = "macos"))]
  let shared_is_playing_for_macos = Arc::clone(&shared_is_playing);

  // Initialize MPRIS D-Bus integration for desktop media control
  // This registers spotatui as a controllable media player on the session bus
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

  // Initialize macOS Now Playing integration for media key control
  // This registers with MPRemoteCommandCenter for media key events
  // Registered without a Spotify session, like MPRIS and SMTC, so decoded
  // sources get media keys too; the handlers no-op while nothing plays.
  #[cfg(all(feature = "macos-media", target_os = "macos"))]
  let macos_media_manager: Option<Arc<macos_media::MacMediaManager>> =
    match macos_media::MacMediaManager::new() {
      Ok(mgr) => {
        info!("macos now playing interface registered - media keys enabled");
        Some(Arc::new(mgr))
      }
      Err(e) => {
        info!(
          "failed to initialize macos media control: {} - media keys disabled",
          e
        );
        None
      }
    };

  // Registered without a Spotify session, like MPRIS, so decoded sources get media keys too.
  #[cfg(all(feature = "windows-media", target_os = "windows"))]
  let windows_media_manager: Option<Arc<smtc_tokio::WindowsMediaManager>> =
    match smtc_tokio::WindowsMediaManager::new() {
      Ok(mgr) => {
        info!("windows smtc com registered - media keys enabled");
        Some(Arc::new(mgr))
      }
      Err(e) => {
        info!(
          "failed to initialize windows smtc com: {} - media keys disabled",
          e
        );
        None
      }
    };

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
        handle_mpris_events(
          event_rx,
          shared_is_playing_for_mpris,
          shared_position_for_mpris,
          mpris_for_seek,
          app_for_mpris,
        )
        .await;
      });
    }
  }

  // Spawn macOS media event handler to process external control requests (media keys, Control Center)
  #[cfg(all(feature = "macos-media", target_os = "macos"))]
  if let Some(ref macos_media) = macos_media_manager {
    if let Some(event_rx) = macos_media.take_event_rx() {
      let app_for_macos = Arc::clone(&app);
      tokio::spawn(async move {
        handle_macos_media_events(event_rx, app_for_macos, shared_is_playing_for_macos).await;
      });
    }
  }

  // Keep Now Playing metadata (including artwork URL from Web API playback state)
  // synchronized with Control Center.
  #[cfg(all(feature = "macos-media", target_os = "macos"))]
  if let Some(ref macos_media) = macos_media_manager {
    let macos_media_for_metadata = Arc::clone(macos_media);
    let app_for_macos_metadata = Arc::clone(&app);
    tokio::spawn(async move {
      let mut last_metadata: Option<MacosMetadata> = None;
      let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

      loop {
        interval.tick().await;
        if let Ok(app) = app_for_macos_metadata.try_lock() {
          update_macos_metadata(&macos_media_for_metadata, &mut last_metadata, &app);
        }
      }
    });
  }

  #[cfg(all(feature = "windows-media", target_os = "windows"))]
  if let Some(ref windows_media) = windows_media_manager {
    if let Some(event_rx) = windows_media.take_event_rx() {
      let app_for_windows = Arc::clone(&app);
      tokio::spawn(async move {
        handle_windows_media_events(event_rx, app_for_windows).await;
      });
    }
  }

  #[cfg(all(feature = "windows-media", target_os = "windows"))]
  if let Some(ref windows_media) = windows_media_manager {
    let windows_media_for_metadata = Arc::clone(windows_media);
    let app_for_windows_metadata = Arc::clone(&app);
    tokio::spawn(async move {
      let mut last_metadata: Option<WindowsMetadata> = None;
      let mut last_playing: Option<bool> = None; // Track play state
      let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

      loop {
        interval.tick().await;
        if let Ok(app) = app_for_windows_metadata.try_lock() {
          let snapshot_state =
            update_windows_metadata(&windows_media_for_metadata, &mut last_metadata, &app);
          // Native playback pushes its own state from the player events; an
          // external device or a decoded source (over paused librespot) is
          // polled here from the snapshot.
          let (is_playing, position_ms) =
            snapshot_state.unwrap_or((false, app.song_progress_ms as u64));
          if last_playing != Some(is_playing) {
            windows_media_for_metadata.set_playback_status(is_playing);
            last_playing = Some(is_playing);
          }
          windows_media_for_metadata.set_position(position_ms);
        }
      }
    });
  }

  // Clone MPRIS manager for UI loop (to update status on device changes)
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  let mpris_for_ui = mpris_manager.clone();

  let cloned_app = Arc::clone(&app);
  info!("spawning spotify network event handler");
  tokio::spawn(async move {
    let mut network = Network::new(&app);

    // The saved-device startup decision moved into
    // `deferred_streaming_startup` (it needs the native player's device
    // name, which now materializes in the background).

    // Resume a persisted non-Spotify session if there is one; it honors the
    // startup behavior for its own play/pause decision. Otherwise fall back to
    // the Spotify startup play behavior. Continue is passive and must not
    // transfer devices, change shuffle, or otherwise activate Spotatui.
    // Restore the persisted native queue into app state before the runner
    // starts, independent of whether a source playback is resumed.
    if !restore_queue.is_empty() {
      network.app.lock().await.native_queue = restore_queue;
    }
    if let Some(session) = restore_playback {
      // Resume off the event pump: a slow source (yt-dlp download, remote
      // fetch) must not stall the Spotify startup events the UI's first render
      // queues (user, playlists, current playback). The restore drives the
      // source's own start path, which serializes on the `App` lock like every
      // other event, so running it concurrently is safe.
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
  // Capture the exact final position of a non-Spotify session on a graceful
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

/// Resume a persisted non-Spotify playback session at launch.
///
/// Drives the source's existing, tested start path (seeding the browse table
/// with the persisted metadata so the snapshot resolves, then dispatching the
/// same `StartPlayback` the keyboard would), and afterwards applies the saved
/// position and the play/pause decision. That decision follows `startup_behavior`:
/// `Play` forces playing, `Pause` forces paused, and `Continue` restores the
/// exact state the session had when it was saved.
///
/// A failed start (removed video, dead network, macOS local playback, missing
/// yt-dlp) publishes no session, so the resume step simply finds nothing and
/// no-ops — startup is never blocked or crashed by a stale session. Any variant
/// whose source feature is disabled in this build is a no-op.
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

  if let PersistedPlayback::Qobuz {
    tracks,
    index,
    position_ms,
    paused,
    repeat,
    shuffle_on,
    shuffle,
  } = session
  {
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
}

/// Handle MPRIS events from external clients (media keys, playerctl, etc.)
/// Routes to native streaming player when available, or dispatches IoEvents as fallback
#[cfg(all(feature = "mpris", target_os = "linux"))]
async fn handle_mpris_events(
  mut event_rx: tokio::sync::mpsc::UnboundedReceiver<mpris::MprisEvent>,
  _shared_is_playing: Arc<std::sync::atomic::AtomicBool>,
  _shared_position: Arc<std::sync::atomic::AtomicU64>,
  mpris_manager: Arc<mpris::MprisManager>,
  app: Arc<Mutex<App>>,
) {
  use mpris::MprisEvent;

  while let Some(event) = event_rx.recv().await {
    if !app.lock().await.user_config.behavior.enable_media_keys {
      continue;
    }

    // A decoded source (local file, Subsonic, radio, or YouTube) owns the
    // session: route transport through the same IoEvents the keyboard uses
    // (intercepted by the per-source route_*_event dispatchers before the
    // Spotify network) so media keys follow the audible source instead of
    // librespot. This must run *before* the streaming-player branches below,
    // since librespot is initialized even while a decoded source is playing.
    if route_decoded_mpris_event(&event, &app, &mpris_manager).await {
      continue;
    }

    let play_key = match &event {
      MprisEvent::PlayPause => Some(None),
      MprisEvent::Play => Some(Some(true)),
      MprisEvent::Pause => Some(Some(false)),
      _ => None,
    };
    if let Some(_play) = play_key {}

    match event {
      MprisEvent::PlayPause => {
        // Fallback: the same toggle as the TUI's play/pause. It follows the playback
        // owner; `native_is_playing` still describes the idle local player while
        // another device plays, so reading it here would resume instead of pause.
        app.lock().await.toggle_playback();
      }
      MprisEvent::Play => {
        let mut app_lock = app.lock().await;
        app_lock.report_nothing_playing();
      }
      MprisEvent::Pause => {
        let mut app_lock = app.lock().await;
        app_lock.report_nothing_playing();
      }
      MprisEvent::Next => {
        app.lock().await.next_track();
      }
      MprisEvent::Previous => {
        app.lock().await.previous_track();
      }
      MprisEvent::Stop => {
        let mut app_lock = app.lock().await;
        app_lock.report_nothing_playing();
      }
      MprisEvent::Seek(offset_micros) => {
        // Fallback: read current position from app, dispatch Seek IoEvent
        let mut app_lock = app.lock().await;
        let current_ms = app_lock.song_progress_ms as i64;
        let offset_ms = offset_micros / 1000;
        let new_position_ms = (current_ms + offset_ms).max(0) as u32;
        app_lock.song_progress_ms = new_position_ms as u128;
        app_lock.report_nothing_playing();
        drop(app_lock);
        mpris_manager.emit_seeked(new_position_ms as u64);
      }
      MprisEvent::SetPosition(position_micros) => {
        // MPRIS SetPosition sends absolute position in microseconds
        let new_position_ms = (position_micros / 1000).max(0) as u32;
        // Fallback: dispatch Seek IoEvent
        let mut app_lock = app.lock().await;
        app_lock.song_progress_ms = new_position_ms as u128;
        app_lock.report_nothing_playing();
        drop(app_lock);
        mpris_manager.emit_seeked(new_position_ms as u64);
      }
      MprisEvent::SetShuffle(shuffle) => {
        // Fallback: dispatch Shuffle IoEvent
        mpris_manager.set_shuffle(shuffle);
        let mut app_lock = app.lock().await;
        app_lock.runtime_state.shuffle_enabled = shuffle;
        app_lock.schedule_state_save(crate::core::state::PersistedRuntimeState::shuffle_enabled(
          shuffle,
        ));
        app_lock.report_nothing_playing();
      }
      MprisEvent::SetLoopStatus(loop_status) => {
        use crate::core::plugin_api::RepeatState;
        use mpris::LoopStatusEvent;

        let _repeat_state = match loop_status {
          LoopStatusEvent::None => RepeatState::Off,
          LoopStatusEvent::Track => RepeatState::Track,
          LoopStatusEvent::Playlist => RepeatState::Context,
        };
        // Fallback: dispatch Repeat IoEvent
        mpris_manager.set_loop_status(loop_status);
        let mut app_lock = app.lock().await;
        app_lock.report_nothing_playing();
      }
      MprisEvent::SetVolume(volume_percent) => {
        let mut app_lock = app.lock().await;
        app_lock.set_volume_percent(volume_percent);
      }
    }
  }
}

/// Route an MPRIS transport event through the standard dispatch path when any
/// decoded source (local file, Subsonic, internet radio, or YouTube) owns the
/// session.
///
/// Returns `true` if the event was consumed (and the caller must skip the
/// Spotify/librespot branches). Play/pause/next/previous/stop/seek map onto the
/// same `IoEvent`s the keyboard uses; the per-source `route_*_event` dispatchers
/// intercept them before the Spotify network, so the control lands on whichever
/// source is actually audible instead of the paused librespot session.
/// Non-transport events (shuffle/loop) return `false` so existing behaviour is
/// preserved.
#[cfg(all(feature = "mpris", target_os = "linux"))]
async fn route_decoded_mpris_event(
  event: &mpris::MprisEvent,
  app: &Arc<Mutex<App>>,
  mpris_manager: &Arc<mpris::MprisManager>,
) -> bool {
  use mpris::MprisEvent;

  let mut app_lock = app.lock().await;

  // The native queue slot owns playback: shuffle/repeat mean nothing over an
  // explicit queue, whatever it suspended, so reject them here rather than in the
  // match below. A queued *Spotify* track has no decoded player and would
  // otherwise take the early return underneath and let the Spotify handler mutate
  // the suspended context — the one state where MPRIS and the keyboard
  // (`App::shuffle` / `App::repeat`, which no-op) would disagree. The corrective
  // push matches what the snapshot reports for a queue slot, so the runner's
  // dedup cache converges instead of stranding the property.
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
  // consumes the event: nothing can serve it, and librespot must not. Volume
  // still falls through to `set_volume_percent`, which is owner-aware.
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
    // Shuffle/repeat drive the player-global decoded state so an external
    // controller changes the audible decoded source, not the stale Spotify
    // context. A queueable source (Local/Subsonic/YouTube) consumes them; the
    // sources that cannot honour them reject them (below) rather than falling
    // through. Only Spotify (no decoded player at all, so this function returned
    // early) reaches the Spotify handling.
    // Volume is handled by the top-level `set_volume_percent`.
    MprisEvent::SetShuffle(on) => {
      if app_lock.set_decoded_shuffle(*on) {
        drop(app_lock);
        mpris_manager.set_shuffle(*on);
      } else {
        // A decoded source owns playback but has no shuffle of its own: radio is
        // an endless stream, and the queue slot plays an explicit list over a
        // suspended context. Consume the event instead of falling through to the
        // Spotify handler, which would flip shuffle on the user's real device for
        // a source they are not listening to (and persist it to their config).
        // Push the true value back so the client's optimistic flip is corrected:
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

/// Handle macOS media events from external sources (media keys, Control Center, AirPods, etc.)
/// Routes control requests through the same transport the keyboard uses.
#[cfg(all(feature = "macos-media", target_os = "macos"))]
async fn handle_macos_media_events(
  mut event_rx: tokio::sync::mpsc::UnboundedReceiver<macos_media::MacMediaEvent>,
  app: Arc<Mutex<App>>,
  _shared_is_playing: Arc<std::sync::atomic::AtomicBool>,
) {
  use macos_media::MacMediaEvent;

  while let Some(event) = event_rx.recv().await {
    if !app.lock().await.user_config.behavior.enable_media_keys {
      continue;
    }
    // The decoded source (Qobuz, or the queue slot) owns the session: route
    // transport through the same IoEvents the keyboard uses.
    if route_decoded_macos_event(&event, &app).await {
      continue;
    }
    let mut app = app.lock().await;
    match event {
      MacMediaEvent::PlayPause => app.toggle_playback(),
      MacMediaEvent::Next => app.next_track(),
      MacMediaEvent::Previous => app.previous_track(),
      MacMediaEvent::Play | MacMediaEvent::Pause | MacMediaEvent::Stop => {
        app.report_nothing_playing()
      }
    }
  }
}

/// Route a macOS media transport event through the standard dispatch path when
/// any decoded source (local file, Subsonic, internet radio, or YouTube) owns
/// the session.
///
/// Returns `true` if the event was consumed (and the caller must skip the
/// streaming-player branches). Play/pause/next/previous/stop map onto the same
/// `IoEvent`s the keyboard uses; the per-source `route_*_event` dispatchers
/// intercept them before the Spotify network, so the control lands on whichever
/// source is actually audible instead of the paused librespot session.
#[cfg(all(feature = "macos-media", target_os = "macos"))]
async fn route_decoded_macos_event(
  event: &macos_media::MacMediaEvent,
  app: &Arc<Mutex<App>>,
) -> bool {
  use macos_media::MacMediaEvent;

  let mut app_lock = app.lock().await;
  // Read the live source-player state up front, then drop the borrow so the
  // immutable read does not conflict with the `&mut self` dispatch calls below.
  // A decoded claim with no player (a start in flight, a lost output device)
  // consumes the event: nothing can serve it, and librespot must not.
  let Some(player) = app_lock.active_decoded_player() else {
    return app_lock.active_decoded_source();
  };
  let is_paused = player.is_paused();

  match event {
    MacMediaEvent::PlayPause => {
      if is_paused {
        app_lock.dispatch(IoEvent::StartPlayback(None, None, None));
      } else {
        app_lock.dispatch(IoEvent::PausePlayback);
      }
    }
    MacMediaEvent::Play => {
      app_lock.dispatch(IoEvent::StartPlayback(None, None, None));
    }
    MacMediaEvent::Pause | MacMediaEvent::Stop => {
      app_lock.dispatch(IoEvent::PausePlayback);
    }
    MacMediaEvent::Next => {
      app_lock.dispatch(IoEvent::NextTrack);
    }
    MacMediaEvent::Previous => {
      app_lock.dispatch(IoEvent::PreviousTrack);
    }
  }
  true
}

#[cfg(all(feature = "windows-media", target_os = "windows"))]
async fn handle_windows_media_events(
  mut event_rx: tokio::sync::mpsc::UnboundedReceiver<smtc_tokio::WindowsMediaEvent>,
  app: Arc<Mutex<App>>,
) {
  use smtc_tokio::WindowsMediaEvent;

  while let Some(event) = event_rx.recv().await {
    if !app.lock().await.user_config.behavior.enable_media_keys {
      continue;
    }

    // A decoded source (local file, Subsonic, radio, or YouTube) owns the
    // session: route transport through the same IoEvents the keyboard uses
    // (intercepted by the per-source route_*_event dispatchers before the
    // Spotify network) so SMTC controls follow the audible source instead of
    // librespot. This must run *before* the streaming-player branches below,
    // since a paused librespot stays active while a decoded source is playing.
    if route_decoded_windows_event(&event, &app).await {
      continue;
    }

    let mut app = app.lock().await;
    match event {
      WindowsMediaEvent::Next => app.next_track(),
      WindowsMediaEvent::Previous => app.previous_track(),
      WindowsMediaEvent::Play
      | WindowsMediaEvent::Pause
      | WindowsMediaEvent::Stop
      | WindowsMediaEvent::SetPosition(_) => app.report_nothing_playing(),
    }
  }
}

/// Route a Windows SMTC media transport event through the standard dispatch
/// path when any decoded source (local file, Subsonic, internet radio, or
/// YouTube) owns the session.
///
/// Returns `true` if the event was consumed (and the caller must skip the
/// streaming-player branches). Play/pause/next/previous/stop/seek map onto the
/// same `IoEvent`s the keyboard uses; the per-source `route_*_event` dispatchers
/// intercept them before the Spotify network, so the control lands on whichever
/// source is actually audible instead of the paused librespot session.
#[cfg(all(feature = "windows-media", target_os = "windows"))]
async fn route_decoded_windows_event(
  event: &smtc_tokio::WindowsMediaEvent,
  app: &Arc<Mutex<App>>,
) -> bool {
  use smtc_tokio::WindowsMediaEvent;

  let mut app_lock = app.lock().await;
  // Only consume the event while a decoded source owns the session; otherwise
  // fall through to the streaming-player branches for Spotify/librespot.
  // A decoded claim with no player (a start in flight, a lost output device)
  // consumes the event: nothing can serve it, and librespot must not.
  if app_lock.active_decoded_player().is_none() {
    return app_lock.active_decoded_source();
  }

  match event {
    WindowsMediaEvent::Play => {
      app_lock.dispatch(IoEvent::StartPlayback(None, None, None));
    }
    WindowsMediaEvent::Pause | WindowsMediaEvent::Stop => {
      app_lock.dispatch(IoEvent::PausePlayback);
    }
    WindowsMediaEvent::Next => {
      app_lock.dispatch(IoEvent::NextTrack);
    }
    WindowsMediaEvent::Previous => {
      app_lock.dispatch(IoEvent::PreviousTrack);
    }
    WindowsMediaEvent::SetPosition(pos) => {
      app_lock.song_progress_ms = *pos as u128;
      app_lock.dispatch(IoEvent::Seek(*pos as u32));
    }
  }
  true
}
