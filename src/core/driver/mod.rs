//! The frontend-neutral scheduler ("the driver"): everything the app must do
//! on a timer, extracted from the terminal event loop so any frontend can run
//! playback correctly by calling [`Driver::tick`] at its tick rate.
//!
//! One tick advances playback state (`App::update_on_tick` plus the debounced
//! seek/volume/state flushes), syncs OS presence (Discord, MPRIS, window
//! title), detects track changes (lyrics + cover art), auto-advances the native
//! queue and the Qobuz session, persists the session, and feeds the audio
//! visualizer.
//!
//! What stays with the frontend, injected through [`TickEnv`]: the visualizer
//! bar count (a function of frontend geometry) and the terminal image-protocol
//! probe behind `TickEnv::cover_art_full_image_support`.
//!
//! A frontend that stops ticking is a bug this module makes loud:
//! `App::playback_position_ms` reports stale once no tick has run for two
//! seconds, instead of leaving a silently frozen playbar.

mod plan;
mod presence;
#[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
mod viz_capture;

use crate::core::app::{App, RouteId};
#[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
use crate::infra::audio;
#[cfg(feature = "discord-rpc")]
use crate::infra::discord_rpc;
#[cfg(all(feature = "mpris", target_os = "linux"))]
use crate::infra::mpris;
use crate::infra::network::IoEvent;
#[cfg(feature = "scripting")]
use crate::infra::scripting::ScriptEngine;
#[cfg(feature = "scripting")]
use log::info;
use std::time::{Duration, Instant};

#[cfg(feature = "discord-rpc")]
pub type DiscordRpcHandle = Option<discord_rpc::DiscordRpcManager>;
#[cfg(not(feature = "discord-rpc"))]
pub type DiscordRpcHandle = Option<()>;

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub type MprisHandle = Option<std::sync::Arc<mpris::MprisManager>>;
#[cfg(not(all(feature = "mpris", target_os = "linux")))]
pub type MprisHandle = Option<()>;

/// Identity of the currently-playing track, used by the shared track-change
/// detector to fire lyrics + cover-art fetches exactly once per track (rather
/// than every tick). Title + artists + album + duration distinguishes tracks
/// across every source without depending on a source-specific id.
type TrackIdentity = (String, Vec<String>, String, u32);

fn track_identity(snapshot: &crate::infra::media_metadata::PlaybackSnapshot) -> TrackIdentity {
  (
    snapshot.metadata.title.clone(),
    snapshot.metadata.artists.clone(),
    snapshot.metadata.album.clone(),
    snapshot.metadata.duration_ms,
  )
}

/// Fetch a track's lyrics, or publish "not found" at once when the user turned lyrics off.
fn request_track_lyrics(app: &mut App, title: String, artists: Vec<String>, duration_secs: f64) {
  app.desired_lyrics_identity = Some((title.clone(), artists.join(", ")));
  if app.user_config.behavior.enable_lyrics {
    app.dispatch(IoEvent::GetLyrics(title, artists, duration_secs));
  } else {
    app.set_lyrics(crate::core::app::LyricsStatus::NotFound, None, false);
    app
      .plugin_data_generations
      .bump(crate::core::app::PluginDataKind::Lyrics);
  }
}

/// Resolve what cover art to fetch for the track described by `snapshot`;
/// `None` when it has no art to show.
#[cfg(feature = "art-decode")]
fn cover_art_request_for(
  snapshot: &crate::infra::media_metadata::PlaybackSnapshot,
) -> Option<crate::core::art::CoverArtRequest> {
  crate::infra::media_metadata::cover_fetch_url(snapshot)
    .map(crate::core::art::CoverArtRequest::Url)
}

/// Per-tick inputs only the frontend can supply.
pub struct TickEnv {
  /// Wall-clock instant of this tick; injected so the driver's throttling
  /// decisions stay deterministic under test.
  pub now: Instant,
  /// How many of cava's bands the visualizer fits, or `None` while no
  /// visualizer shows (which releases the capture).
  #[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
  pub viz_bars: Option<usize>,
  /// Whether the frontend renders real pixels for cover art (the terminal
  /// image-protocol probe); false in a decode-only build, where art is
  /// fetched solely for the adaptive theme.
  #[cfg(feature = "art-decode")]
  pub cover_art_full_image_support: bool,
}

/// The scheduler's own state: everything that used to live as locals of the
/// terminal event loop, one instance per frontend session.
pub struct Driver {
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  mpris_manager: MprisHandle,
  #[cfg(feature = "discord-rpc")]
  discord_rpc_manager: DiscordRpcHandle,
  #[cfg(feature = "scripting")]
  script_engine: Option<ScriptEngine>,
  #[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
  audio_capture: viz_capture::VizCapture<audio::AudioCaptureManager>,
  #[cfg(feature = "discord-rpc")]
  discord_presence: presence::DiscordPresenceState,
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  mpris_state: presence::MprisState,
  window_title: presence::WindowTitleState,
  /// Last track the shared detector fired lyrics for, so the lookup re-fires
  /// only on an actual track change rather than every tick.
  last_track_identity: Option<TrackIdentity>,
  /// The item and position the last tick saw, so "Up next" refreshes on every
  /// playback transition, a repeated track included.
  last_playback_mark: plan::PlaybackMark,
  /// Cache key (URL / file URI) of the cover art last requested, so the
  /// per-tick cover-art evaluation dispatches a fetch only when the resolved
  /// art changes.
  #[cfg(feature = "art-decode")]
  last_cover_art_key: Option<String>,
  /// Throttle state for `last_session.yml` (see `plan::session_persist_action`):
  /// `last_session_save` spaces the periodic writes; `session_was_present`
  /// lets a Some -> None transition (the queue ended, playback stopped) clear
  /// the file so a stale session is never resurrected.
  last_session_save: Option<Instant>,
  session_was_present: bool,
  /// Reads the sound card's running format off-thread.
  hardware_probe: crate::infra::audio::HardwareProbe,
}

/// The startup route's data fetch, gated on what the route needs.
fn fetch_startup_route(app: &mut App) {
  let route = app.get_current_route().id.clone();
  if !app.availability(route.startup_requirement()).is_available() {
    return;
  }
  if route == RouteId::Stats {
    app.reload_stats()
  }
}

impl Driver {
  pub fn new(mpris_manager: MprisHandle, discord_rpc_manager: DiscordRpcHandle) -> Self {
    #[cfg(not(all(feature = "mpris", target_os = "linux")))]
    let _ = mpris_manager;
    #[cfg(not(feature = "discord-rpc"))]
    let _ = discord_rpc_manager;

    // The Lua VM (plus its HTTP client) is only constructed when the user
    // actually has script files; a zero-plugin install skips the engine — and
    // its per-tick `on_tick` dispatch — entirely.
    #[cfg(feature = "scripting")]
    let script_engine: Option<ScriptEngine> = {
      let config_dir = crate::core::user_config::default_app_config_dir();
      match config_dir {
        Some(config_dir) if ScriptEngine::has_user_scripts(&config_dir) => {
          match ScriptEngine::new() {
            Ok(mut engine) => {
              let loaded = engine.load_user_scripts(&config_dir);
              info!("loaded {loaded} lua plugin file(s)");
              Some(engine)
            }
            Err(e) => {
              log::error!("failed to initialize lua scripting engine: {e}");
              None
            }
          }
        }
        _ => {
          info!("no lua plugin files found; scripting engine not started");
          None
        }
      }
    };

    Driver {
      #[cfg(all(feature = "mpris", target_os = "linux"))]
      mpris_manager,
      #[cfg(feature = "discord-rpc")]
      discord_rpc_manager,
      #[cfg(feature = "scripting")]
      script_engine,
      #[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
      audio_capture: viz_capture::VizCapture::new(),
      #[cfg(feature = "discord-rpc")]
      discord_presence: presence::DiscordPresenceState::default(),
      #[cfg(all(feature = "mpris", target_os = "linux"))]
      mpris_state: presence::MprisState::default(),
      window_title: presence::WindowTitleState::default(),
      last_track_identity: None,
      last_playback_mark: plan::PlaybackMark::default(),
      #[cfg(feature = "art-decode")]
      last_cover_art_key: None,
      last_session_save: None,
      session_was_present: false,
      hardware_probe: crate::infra::audio::HardwareProbe::start(),
    }
  }

  /// One tick. The frontend calls this at its tick rate with the `App` lock
  /// held; everything slow (file I/O, network) is dispatched off-lock. Must
  /// run inside a Tokio runtime: session persistence goes through
  /// `tokio::task::spawn_blocking`, which panics without one.
  pub fn tick(&mut self, app: &mut App, elapsed: Duration, env: TickEnv) {
    app.update_on_tick(elapsed);

    app.flush_pending_source_seek();
    app.flush_state_save(false);

    #[cfg(feature = "scripting")]
    if let Some(engine) = self.script_engine.as_mut() {
      engine.on_tick(app);
    }

    #[cfg(feature = "discord-rpc")]
    if let Some(ref manager) = self.discord_rpc_manager {
      presence::update_discord_presence(manager, &mut self.discord_presence, app);
    }

    #[cfg(all(feature = "mpris", target_os = "linux"))]
    if let Some(ref mpris) = self.mpris_manager {
      presence::update_mpris_state(mpris, &mut self.mpris_state, app);
    }

    // Shared track-change detector. One place decides "the playing track
    // changed" off the playback snapshot, then drives both lyrics and cover
    // art (cover-art feature).
    let playing = {
      let snapshot = crate::infra::media_metadata::tracking_playback_snapshot(app);

      // Lyrics fire once per track (identity latch): their inputs — title,
      // artist, duration — ARE the identity, so they are correct at the
      // instant the identity changes.
      let identity = snapshot.as_ref().map(track_identity);
      if identity != self.last_track_identity {
        self.last_track_identity = identity;
        app.view.lyrics_view.reset();
        match snapshot.as_ref() {
          Some(snapshot) => {
            use crate::infra::media_metadata::PlaybackItemKind;
            // LRCLIB lookup by title + artist + duration. A non-track item
            // has no lyrics, so skip the lookup and show the not-found
            // message rather than stale lyrics.
            if snapshot.item_kind == PlaybackItemKind::Track {
              let title = snapshot.metadata.title.clone();
              // The identity latch keys on the joined display credit, but the
              // LRCLIB lookup needs the structured artist list so it can fall
              // back to the primary artist alone for collaborations (#410).
              let artists = snapshot.metadata.artists.clone();
              request_track_lyrics(
                app,
                title,
                artists,
                snapshot.metadata.duration_ms as f64 / 1000.0,
              );
            } else {
              app.desired_lyrics_identity = None;
              app.set_lyrics(crate::core::app::LyricsStatus::NotFound, None, false);
              app
                .plugin_data_generations
                .bump(crate::core::app::PluginDataKind::Lyrics);
            }
          }
          None => {
            app.desired_lyrics_identity = None;
            // Nothing is playing: reset so no stale lyrics linger.
            app.set_lyrics(crate::core::app::LyricsStatus::NotStarted, None, false);
            app
              .plugin_data_generations
              .bump(crate::core::app::PluginDataKind::Lyrics);
          }
        }
      }

      // "Up next" follows every playback transition, including the same
      // track starting again, which leaves the lyrics identity unchanged.
      let mark = snapshot
        .as_ref()
        .map_or_else(plan::PlaybackMark::default, |snapshot| plan::PlaybackMark {
          uri: snapshot.item_uri.clone(),
          progress_ms: snapshot.progress_ms,
        });
      plan::playback_transition(&self.last_playback_mark, &mark);
      self.last_playback_mark = mark;

      // Cover art is re-evaluated every tick against the desired image key,
      // not latched to the identity change, so art that resolves after the
      // track flips is still fetched. Comparing against `last_cover_art_key`
      // keeps this a no-op on quiet ticks and fires exactly once whenever the
      // resolved art actually changes.
      #[cfg(feature = "art-decode")]
      {
        use crate::core::art::CoverArtStatus;
        let enabled = app
          .user_config
          .needs_cover_art(env.cover_art_full_image_support);
        let desired = if enabled {
          snapshot.as_ref().and_then(cover_art_request_for)
        } else {
          None
        };
        match plan::cover_art_action(
          desired.as_ref().map(|request| request.key()),
          self.last_cover_art_key.as_deref(),
          app.cover_art.available(),
          enabled,
          snapshot.is_some(),
        ) {
          plan::CoverArtAction::Fetch => {
            let request = desired.expect("Fetch is only planned with a desired request");
            app.desired_cover_art_key = Some(request.key().to_string());
            self.last_cover_art_key = Some(request.key().to_string());
            // Keep the previous image on screen until the new one
            // resolves (smooth swap); the fetch runs off-lock.
            app.cover_art.status = CoverArtStatus::Loading;
            app.dispatch(IoEvent::FetchCoverArt(request));
          }
          plan::CoverArtAction::Keep => {
            app.desired_cover_art_key = desired.map(|request| request.key().to_string());
          }
          plan::CoverArtAction::Drop { clear, status } => {
            app.desired_cover_art_key = None;
            self.last_cover_art_key = None;
            // No art to show (art disabled, nothing playing): drop
            // any stale image once, so the pane shows the placeholder.
            if clear {
              app.clear_cover_art();
            }
            app.cover_art.status = status;
          }
        }
      }

      snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.is_playing)
    };

    // Output device lost mid-track (Bluetooth headphones disconnected, a USB
    // DAC unplugged). cpal pauses the stream and reports it, after which no
    // audio callback runs again: `LocalPlayer` then refuses the transport calls
    // that would wait on one, so nothing hangs, but the session sits silent and
    // still looks like it is playing (`is_finished` stays false — the source is
    // queued on a sink that will never drain it). That is why this runs before
    // every advance block below: a dead device must not read as end-of-track.
    //
    // Rebuild the output on the new default device (with retries: a Bluetooth
    // output takes seconds to come back) and restage the track there, at the
    // position it stopped. Whether it stays paused depends on which of the
    // two shapes this was: cpal reporting the device *removed* (headphones
    // out) pauses — nobody wants the music jumping to the laptop speakers. A default that merely moved (headphones *in*) keeps
    // whatever play/pause state the session had. The seek and the pause ride
    // on the session as `resume_at`, applied by the path that stages the
    // track, never as events queued behind it.
    macro_rules! decoded_device_recovery {
      ($app:ident, $playback:ident) => {
        // Not while the queue owns the sink: the session below is suspended, and
        // its player is the *same* handle the queue slot is using (see
        // `suspended_context_player`). The queue branch further down recovers it.
        if !$app.queue_owns_playback()
          && $app
            .$playback
            .as_ref()
            .is_some_and(|s| s.player.device_lost())
        {
          use crate::infra::audio::Reopen;
          // Read before the reopen: it swaps in a fresh sink that is paused,
          // empty, and reports no removal. A change already in flight stages a
          // different track, so its position is the top.
          let (position_ms, paused) = $app.$playback.as_ref().map_or((0, false), |s| {
            (
              if s.advancing {
                0
              } else {
                s.player.position().as_millis() as u64
              },
              s.player.device_removed() || s.player.is_paused(),
            )
          });
          match $app.$playback.as_ref().map(|s| s.player.recover_device()) {
            Some(Reopen::Reopened) => {
              // The fresh sink is empty, so `is_finished()` is true from here
              // on: hold the advance latch or the block below would read that
              // as end-of-track and skip a song while the replay is in flight.
              if let Some(s) = $app.$playback.as_mut() {
                s.advancing = true;
                s.resume_at = Some(crate::infra::queue::ResumePoint {
                  position_ms,
                  paused,
                });
              }
              $app.dispatch(IoEvent::ReplayCurrentTrack);
              $app.set_status_message(
                if paused {
                  $app.strings().status.app.audio_device_paused
                } else {
                  $app.strings().status.app.audio_device_moved
                },
                8,
              );
            }
            Some(Reopen::Retrying) => {
              $app.set_status_message($app.strings().status.app.audio_device_waiting, 8);
            }
            Some(Reopen::GaveUp) => {
              $app.$playback = None;
              $app.set_status_message($app.strings().status.infra.audio_disconnected, 8);
            }
            Some(Reopen::Waiting) | None => {}
          }
        }
      };
    }
    decoded_device_recovery!(app, qobuz_playback);

    // The native queue slot's own sink. There is no "replay this queued item"
    // event, so recovery here reopens the device and lets the advance block
    // below take it from the top of the next item: the interrupted queued track
    // is skipped rather than restaged. Same split as the context path above:
    // a removal clears the slot's desired play state, which every path that
    // stages the next item (and the suspended context's resume) reads, so the
    // queue does not play out loud on a device the user just left.
    {
      use crate::infra::audio::Reopen;
      use crate::infra::queue::QueueNowPlaying;
      let lost = match app.queue_now.as_ref() {
        Some(QueueNowPlaying::Decoded(d)) if d.player.device_lost() => Some((
          std::sync::Arc::clone(&d.player),
          d.player.device_removed() || d.player.is_paused(),
        )),
        _ => None,
      };
      if let Some((player, paused)) = lost {
        match player.recover_device() {
          Reopen::Reopened => {
            if paused {
              app.queue_slot_desired_playing = false;
            }
            app.set_status_message(
              if paused {
                app.strings().status.app.audio_device_paused
              } else {
                app.strings().status.app.audio_device_moved
              },
              8,
            );
          }
          Reopen::Retrying => {
            app.set_status_message(app.strings().status.app.audio_device_waiting, 8);
          }
          // No device to play on. Hand the slot to the same teardown a drained
          // queue uses: clearing `queue_now` here would strand the suspended
          // context (latched `advancing`) and the remaining queued items.
          Reopen::GaveUp => {
            if paused {
              app.queue_slot_desired_playing = false;
            }
            app.dispatch(IoEvent::FinishNativeQueue);
            app.set_status_message(app.strings().status.infra.audio_disconnected, 8);
          }
          Reopen::Waiting => {}
        }
      }
    }

    // Native queue slot: when the queued track finishes, advance the queue
    // (play the next queued item, or resume the suspended context). Runs
    // before the per-source blocks so it takes precedence over them.
    {
      use crate::infra::queue::QueueNowPlaying;
      let advance = match app.queue_now.as_mut() {
        Some(QueueNowPlaying::Decoded(d))
          if plan::native_queue_advance_due(d.player.is_finished(), d.advancing) =>
        {
          d.advancing = true; // atomic check-and-set: one dispatch only
          true
        }
        _ => false,
      };
      if advance {
        app.dispatch(IoEvent::AdvanceNativeQueue);
      }
    }

    // Qobuz auto-advance. The session reads its progress live from the
    // player at render time; the only state self-managed here is
    // end-of-track. When the sink drains and no track change is already in
    // flight (`!advancing`), `plan::decoded_advance` picks the move: advance /
    // replay (repeat-one) / suspend to the native queue / tear down.
    //
    // Decide under one borrow, then act after the borrow ends. `advancing`
    // is set *synchronously here* (atomic check-and-set, before
    // dispatching) because the sink stays empty for the whole download and
    // decode window; without it the next tick would re-dispatch and skip
    // several tracks per advance.
    macro_rules! decoded_auto_advance {
      ($app:ident, $playback:ident, $queue:ident) => {
        if !$app.queue_owns_playback() {
          use crate::infra::queue::next_index;
          let queue_len = $app.native_queue.len();
          let repeat = $app.decoded_repeat;
          let advance = $app.$playback.as_ref().map(|s| {
            plan::decoded_advance(
              s.player.is_finished(),
              s.advancing,
              next_index(s.index, s.$queue.len()).is_some(),
              queue_len,
              repeat,
            )
          });
          match advance {
            Some(plan::DecodedAdvance::Dispatch { replay }) => {
              if let Some(s) = $app.$playback.as_mut() {
                s.advancing = true; // atomic check-and-set: one dispatch only
              }
              $app.dispatch(if replay {
                IoEvent::ReplayCurrentTrack
              } else {
                IoEvent::NextTrack
              });
            }
            Some(plan::DecodedAdvance::SuspendToQueue) => {
              // End-of-track handoff: under Repeat One the context resumes
              // the same track, so a queued song can't consume the repeat.
              $app.suspend_active_decoded_context_for_skip(
                crate::infra::queue::SuspendCause::AutoAdvance,
              );
              $app.dispatch(IoEvent::AdvanceNativeQueue);
            }
            Some(plan::DecodedAdvance::Teardown) => {
              $app.$playback = None;
              $app.release_decoded_sink_claim();
            }
            Some(plan::DecodedAdvance::None) | None => {}
          }
        }
      };
    }
    decoded_auto_advance!(app, qobuz_playback, tracks);

    // Apply any shuffle toggle that was deferred while a track change was in
    // flight, now that the advance may have committed (a cheap no-op when the
    // queue order already matches the decoded shuffle state).
    app.reconcile_decoded_shuffle();

    // The Qobuz session owns the sink while `qobuz_playback` is `Some`: drive
    // `song_progress_ms` from its live player.
    #[allow(unused_mut)]
    let mut source_owns_playback = false;
    // The native queue slot owns the sink when playing a decoded track; read
    // progress from its player first (it may share the suspended context's
    // player, in which case a per-source block below reads the same value).
    if let Some(crate::infra::queue::QueueNowPlaying::Decoded(d)) = app.queue_now.as_ref() {
      source_owns_playback = true;
      app.song_progress_ms = d.player.position().as_millis();
    }
    if let Some(qobuz) = app.qobuz_playback.as_ref() {
      source_owns_playback = true;
      app.song_progress_ms = qobuz.player.position().as_millis();
    }

    // Persist the session so it resumes on next launch. Throttled to avoid
    // churning the file every tick; a Some -> None transition (the queue
    // ended, playback stopped) clears it instead.
    let has_session = app.has_persistable_session();
    match plan::session_persist_action(
      has_session,
      self.session_was_present,
      self.last_session_save,
      env.now,
    ) {
      plan::SessionPersist::Save => {
        self.last_session_save = Some(env.now);
        // Snapshot (clones the queue) only when a save is actually due,
        // not on every tick.
        if let Some(session) = app.current_persisted_session() {
          // Fire-and-forget on the blocking pool: file I/O never blocks the
          // UI tick, and a dropped handle still runs to completion.
          tokio::task::spawn_blocking(move || {
            if let Ok(path) = crate::core::persisted_playback::default_session_path() {
              if let Err(e) = crate::core::persisted_playback::save(&path, &session) {
                log::warn!("[session] failed to persist playback session: {e}");
              }
            }
          });
        }
      }
      plan::SessionPersist::Clear => {
        self.last_session_save = None;
        tokio::task::spawn_blocking(|| {
          if let Ok(path) = crate::core::persisted_playback::default_session_path() {
            if let Err(e) = crate::core::persisted_playback::clear(&path) {
              log::warn!("[session] failed to clear playback session: {e}");
            }
          }
        });
      }
      plan::SessionPersist::None => {}
    }
    self.session_was_present = has_session;

    // Only the native position below reads the flag.
    let _ = source_owns_playback;
    app.note_playing(playing);
    // The card format only means something while qbal is the one playing.
    app.set_hardware_output(playing.then(|| self.hardware_probe.latest()).flatten());

    #[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
    match env.viz_bars {
      Some(bars) => {
        // Built at the count we are about to ask for, so the first frame
        // does not immediately throw the fresh cavacore plan away.
        let capture = self
          .audio_capture
          .poll(app, move || audio::AudioCaptureManager::new(bars));
        match capture {
          Some(capture) => {
            if let Some(spectrum) = capture.get_spectrum(bars) {
              app.spectrum_data = Some(spectrum);
            }
            // Kept outside the spectrum arm: a dead stream must drop the
            // "Capturing audio" status instead of freezing it on.
            app.audio_capture_active = capture.is_active();
          }
          None => app.audio_capture_active = false,
        }
      }
      None => {
        if self.audio_capture.close() {
          app.audio_capture_active = false;
          app.spectrum_data = None;
        }
      }
    }
  }

  /// The one-shot startup dispatch, fired by the frontend right after its
  /// first frame: the startup route's data, the source's sidebar data, and
  /// the announcement feed.
  pub fn dispatch_startup(&mut self, app: &mut App) {
    // startup_route seeds the nav stack directly (App::new), bypassing the
    // handlers that normally fetch a screen's data on navigation — kick
    // off that fetch here or the screen renders empty until re-entered.
    // (Home needs nothing extra.)
    fetch_startup_route(app);
    // The Qobuz sidebar (favorites and playlists) loads at startup.
    app.load_source_sidebar(app.active_source);
    app.dispatch(IoEvent::FetchAnnouncements);

    #[cfg(feature = "scripting")]
    if let Some(engine) = self.script_engine.as_mut() {
      engine.on_start(app);
    }
  }

  /// Run any plugin commands a keypress queued; the frontend calls this after
  /// each key event. No-op without the `scripting` feature.
  pub fn run_pending_script_commands(&mut self, app: &mut App) {
    #[cfg(feature = "scripting")]
    if let Some(engine) = self.script_engine.as_mut() {
      engine.run_pending_commands(app);
    }
    #[cfg(not(feature = "scripting"))]
    let _ = app;
  }

  /// The window title the frontend should apply now, or `None` when it is
  /// unchanged. Called once per frame (cheaper than a tick, and a title
  /// change should not wait for one).
  pub fn next_window_title(&mut self, app: &App) -> Option<String> {
    presence::next_window_title(&mut self.window_title, app)
  }

  /// On teardown: the default title to restore if this session changed it.
  pub fn window_title_reset(&mut self) -> Option<&'static str> {
    presence::window_title_reset(&mut self.window_title)
  }

  /// The scripting quit hook. No-op without the `scripting` feature.
  pub fn on_quit(&mut self, app: &mut App) {
    #[cfg(feature = "scripting")]
    if let Some(engine) = self.script_engine.as_mut() {
      engine.on_quit(app);
    }
    #[cfg(not(feature = "scripting"))]
    let _ = app;
  }

  /// Clear any published rich presence on the way out.
  pub fn clear_presence(&self) {
    #[cfg(feature = "discord-rpc")]
    if let Some(ref manager) = self.discord_rpc_manager {
      manager.clear();
    }
  }
}
