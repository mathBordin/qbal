//! Native queue playback routing.
//!
//! [`route_queue_event`] is wired **first** in the runtime IoEvent pump (before
//! the per-source dispatchers). It owns [`IoEvent::AdvanceNativeQueue`] and the
//! transport controls for the queue slot's player, and it relinquishes the queue
//! slot when the user starts an unrelated playback.
//!
//! An item that is not a Qobuz track is skipped with a status message.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::core::app::App;
use crate::core::plugin_api::TrackInfo;
use crate::core::queue::is_qobuz_uri;
use crate::core::source::Source;
use crate::infra::network::IoEvent;
use crate::infra::queue::QueueEnd;

use crate::infra::audio::LocalPlayer;
use std::time::Duration;

/// Intercept queue-owned events before the per-source dispatchers.
///
/// Returns `true` when the event was consumed and must **not** be forwarded,
/// `false` to let the normal dispatch run. [`IoEvent::StartPlayback`] variants
/// return `false` (the per-source teardowns/starts still run) but first clear
/// the queue slot so a new play cleanly takes over.
pub async fn route_queue_event(app: &Arc<Mutex<App>>, event: &IoEvent) -> bool {
  // Music owns playback: the queue slot was cleared by the claim, and an
  // advance must not start a second player under it.

  if let IoEvent::AdvanceNativeQueue = event {
    advance_native_queue(app).await;
    return true;
  }

  // The slot is done, with the rest of the queue left where it is: the driver's
  // tick sends this when the slot's device died and would not reopen.
  if let IoEvent::FinishNativeQueue = event {
    resume_or_finish(app, QueueEnd::DeviceLost).await;
    return true;
  }

  // Transport for the queue slot's own player (Pause / Seek / Volume / Next /
  // bare-resume). Only meaningful when a decoded queued track owns the sink;
  // compiles out entirely without a queueable decoded source.
  if let Some(handled) = route_queue_transport(app, event).await {
    return handled;
  }

  // An explicit new-playback start relinquishes the queue slot (keeping the
  // queued items) so the per-source teardowns/starts run against a clean state.
  if matches!(
    event,
    IoEvent::StartPlayback(Some(_), _, _) | IoEvent::StartPlayback(_, Some(_), _)
  ) {
    clear_queue_playback(app).await;
  }
  false
}

/// Transport controls for the queue slot's player, when a decoded queued track
/// owns the sink. Returns `Some(true)` when consumed, `None` when this event is
/// not a queue-slot transport control (so the caller falls through).
async fn route_queue_transport(app: &Arc<Mutex<App>>, event: &IoEvent) -> Option<bool> {
  let player = {
    let guard = app.lock().await;
    guard.queue_now_decoded_player().map(Arc::clone)
  }?;
  match event {
    IoEvent::PausePlayback => {
      player.pause();
      app.lock().await.queue_slot_desired_playing = false;
      Some(true)
    }
    // Bare "resume current" while the queue owns playback resumes the queue slot.
    IoEvent::StartPlayback(None, None, None) => {
      player.resume();
      app.lock().await.queue_slot_desired_playing = true;
      Some(true)
    }
    IoEvent::Seek(position_ms) => {
      let _ = player.seek(Duration::from_millis(*position_ms as u64));
      Some(true)
    }
    IoEvent::ChangeVolume(volume) => {
      player.set_volume(*volume);
      let mut app = app.lock().await;
      app.runtime_state.volume_percent = *volume;
      app.schedule_state_save(crate::core::state::PersistedRuntimeState::volume_percent(
        *volume,
      ));
      Some(true)
    }
    // Skip the queued track: advance to the next queued item (or resume). A
    // skip while paused plays the next item.
    IoEvent::NextTrack => {
      drop(player);
      app.lock().await.queue_slot_desired_playing = true;
      advance_native_queue(app).await;
      Some(true)
    }
    // A forward-only queue has no "previous"; restart the current queued track.
    IoEvent::PreviousTrack | IoEvent::ForcePreviousTrack => {
      let _ = player.seek(Duration::from_millis(0));
      Some(true)
    }
    _ => None,
  }
}

/// Drop the queue slot (stopping its player) and forget any suspended context,
/// but keep the queued items. Called when the user starts an unrelated playback.
async fn clear_queue_playback(app: &Arc<Mutex<App>>) {
  let player = {
    let mut guard = app.lock().await;
    guard.queue_suspended = None;
    // The slot's desired play state belongs to the queue run that just ended.
    guard.queue_slot_desired_playing = true;
    guard.take_queue_now_decoded_player()
  };
  if let Some(player) = player {
    player.stop();
  }
}

// ---------------------------------------------------------------------------
// Advance
// ---------------------------------------------------------------------------

/// Pop the head of the native queue and play it, skipping unplayable items with
/// a status message (bounded by the queue length — never unbounded recursion).
/// When the queue drains, resume the suspended context (or finish).
async fn advance_native_queue(app: &Arc<Mutex<App>>) {
  loop {
    let track = {
      let mut guard = app.lock().await;
      if guard.native_queue.is_empty() {
        None
      } else {
        Some(guard.native_queue.remove(0))
      }
    };
    let Some(track) = track else {
      resume_or_finish(app, QueueEnd::Drained).await;
      return;
    };
    if try_play_queued(app, &track).await {
      return; // now playing this track
    }
    // Unplayable / skipped — loop to the next item.
  }
}

/// Try to play one queued track. Returns `true` if it is now playing, `false`
/// if it was skipped (feature off, no URI, download/decode error) — the caller
/// then advances to the next item.
async fn try_play_queued(app: &Arc<Mutex<App>>, track: &TrackInfo) -> bool {
  let Some(uri) = track.uri.clone() else {
    set_status(
      app,
      crate::infra::app_strings(app)
        .await
        .status
        .infra
        .queue_no_uri
        .to_string(),
    )
    .await;
    return false;
  };
  if !is_qobuz_uri(&uri) {
    let message = crate::infra::app_strings(app)
      .await
      .status
      .infra
      .queue_not_qobuz
      .to_string();
    set_status(app, message).await;
    return false;
  }
  play_queued_qobuz(app, track, &uri).await
}

// ---------------------------------------------------------------------------
// Per-source queue playback
// ---------------------------------------------------------------------------

async fn play_queued_qobuz(app: &Arc<Mutex<App>>, track: &TrackInfo, uri: &str) -> bool {
  claim_sink_for(app, Source::Qobuz).await;
  let Some(source) = crate::infra::qobuz::dispatch::build_playback_source(app).await else {
    return false; // build_playback_source surfaced its own status
  };
  let Some(player) = acquire_queue_player(app).await else {
    return false;
  };
  let fetch_id = publish_pending_decoded(app, &player, track).await;
  let quality = app.lock().await.user_config.behavior.qobuz_quality;
  // Fetch off the IoEvent pump: a Qobuz track is a long download.
  let app_for_spawn = Arc::clone(app);
  let uri = uri.to_string();
  let name = track.name.clone();
  let handle = tokio::spawn(async move {
    let result = crate::infra::qobuz::dispatch::download_for_queue(&source, &uri, quality)
      .await
      .map(|(tmp, label)| (tmp, Some(label)));
    finish_decoded_fetch(&app_for_spawn, fetch_id, result, &name).await;
  });

  attach_abort_handle(app, fetch_id, handle.abort_handle()).await;
  true
}

/// Monotonic source for [`DecodedQueuePlayback::fetch_id`] stamps.
static QUEUE_FETCH_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_fetch_id() -> u64 {
  QUEUE_FETCH_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Publish the queue slot for `track` *before* its (possibly multi-second)
/// download or decode, marked `advancing` so the runner tick doesn't read the
/// still-empty sink as end-of-track. From this instant `queue_owns_playback()`
/// is true, so the transport paths (skip, pause, playbar) see the queued track
/// as current during the fetch. Without this, a skip in the silent download
/// window fell through to the "context playing with items waiting" branch,
/// which re-suspended the context and dispatched a second advance — dropping
/// one queued item on the floor. Returns the slot's fetch stamp, which a
/// background fetch passes back to [`finish_decoded_fetch`].
async fn publish_pending_decoded(
  app: &Arc<Mutex<App>>,
  player: &Arc<LocalPlayer>,
  track: &TrackInfo,
) -> u64 {
  use crate::infra::queue::{DecodedQueuePlayback, QueueNowPlaying};
  let fetch_id = next_fetch_id();
  let mut guard = app.lock().await;
  guard.queue_now = Some(QueueNowPlaying::Decoded(DecodedQueuePlayback {
    player: Arc::clone(player),
    track: track.clone(),
    advancing: true,
    fetch_id,
    tempfile: None,
    abort_handle: None,
    quality: None,
  }));
  fetch_id
}

/// Complete a background queue fetch: if the slot still carries `fetch_id`,
/// stage the downloaded file and finalize the slot (with the delivered format
/// label, when the source reports one); otherwise (skipped, torn down, or
/// replaced meanwhile) drop the result silently. On a download or decode
/// failure the queue advances past the item, exactly like the old inline
/// path. The stage runs off the `App` lock (its clear waits on the audio
/// thread, and the runner takes the lock on every frame); the slot is
/// re-checked before the finalize, and a stage that a newer fetch superseded
/// is left paused in the sink for that fetch's own stage to clear.
async fn finish_decoded_fetch(
  app: &Arc<Mutex<App>>,
  fetch_id: u64,
  result: anyhow::Result<(tempfile::NamedTempFile, Option<String>)>,
  track_name: &str,
) {
  use crate::infra::queue::QueueNowPlaying;
  let mut guard = app.lock().await;
  let player = match guard.queue_now.as_ref() {
    Some(QueueNowPlaying::Decoded(d)) if d.fetch_id == fetch_id => Arc::clone(&d.player),
    _ => return, // superseded — the tempfile drops here
  };
  let (tmp, quality) = match result {
    Ok(fetched) => fetched,
    Err(e) => {
      {
        let strings = guard.strings();
        guard.set_status_message(
          strings
            .status
            .infra
            .queue_play_failed(track_name, &e.to_string()),
          4,
        )
      };
      guard.dispatch(IoEvent::AdvanceNativeQueue);
      return;
    }
  };
  let path = tmp.path().to_path_buf();
  let decode_player = Arc::clone(&player);
  drop(guard);
  let staged = tokio::task::spawn_blocking(move || decode_player.stage_file(&path))
    .await
    .map(|r| r.map_err(|e| e.to_string()))
    .unwrap_or_else(|e| Err(e.to_string()));
  let mut guard = app.lock().await;
  if !matches!(
    guard.queue_now.as_ref(),
    Some(QueueNowPlaying::Decoded(d)) if d.fetch_id == fetch_id
  ) {
    return;
  }
  if let Err(e) = staged {
    {
      let strings = guard.strings();
      guard.set_status_message(
        strings
          .status
          .infra
          .queue_play_failed(track_name, &e.to_string()),
        4,
      )
    };
    guard.dispatch(IoEvent::AdvanceNativeQueue);
    return;
  }
  player.set_volume(guard.runtime_state.volume_percent);
  // A user pause, or a device removal, holds the next item paused.
  if guard.queue_slot_desired_playing {
    player.resume();
  }
  if let Some(QueueNowPlaying::Decoded(d)) = guard.queue_now.as_mut() {
    d.tempfile = Some(tmp);
    d.quality = quality;
    d.advancing = false;
  }
  {
    let strings = guard.strings();
    guard.set_status_message(strings.status.infra.queue_playing(track_name), 4)
  };
}

/// Acquire an output-device player for the queue slot, in priority order:
/// 1. reuse the queue slot's own player (advancing within the queue);
/// 2. reuse the suspended decoded context's player (device-handoff-free);
/// 3. open a fresh device.
///
/// Callers must [`claim_sink_for`] *before* acquiring, so the tick sees an
/// owner for the whole download window.
async fn acquire_queue_player(app: &Arc<Mutex<App>>) -> Option<Arc<LocalPlayer>> {
  if let Some(p) = {
    let guard = app.lock().await;
    guard.queue_now_decoded_player().map(Arc::clone)
  } {
    return Some(p);
  }
  if let Some(p) = suspended_context_player(app).await {
    return Some(p);
  }
  match tokio::task::spawn_blocking(LocalPlayer::new).await {
    Ok(Ok(p)) => Some(Arc::new(p)),
    Ok(Err(e)) => {
      set_status(
        app,
        crate::infra::app_strings(app)
          .await
          .status
          .infra
          .queue_audio_unavailable(&e.to_string()),
      )
      .await;
      None
    }
    Err(e) => {
      set_status(
        app,
        crate::infra::app_strings(app)
          .await
          .status
          .infra
          .audio_init_failed(&e.to_string()),
      )
      .await;
      None
    }
  }
}

/// The player of the playlist suspended under the queue, so the queue slot can
/// reuse its output device.
async fn suspended_context_player(app: &Arc<Mutex<App>>) -> Option<Arc<LocalPlayer>> {
  let guard = app.lock().await;
  if let Some(s) = guard.qobuz_playback.as_ref() {
    return Some(Arc::clone(&s.player));
  }
  None
}

async fn attach_abort_handle(
  app: &Arc<Mutex<App>>,
  fetch_id: u64,
  abort_handle: tokio::task::AbortHandle,
) {
  let mut guard = app.lock().await;
  if let Some(crate::infra::queue::QueueNowPlaying::Decoded(ref mut d)) = guard.queue_now {
    if d.fetch_id == fetch_id {
      d.abort_handle = Some(crate::infra::queue::DownloadAbortHandle(abort_handle));
      return;
    }
  }
  abort_handle.abort();
}

/// Hand the sink to `source` before a decoded queue item takes over.
async fn claim_sink_for(app: &Arc<Mutex<App>>, source: Source) {
  app.lock().await.claim_decoded_sink(source);
}

// ---------------------------------------------------------------------------
// Resume
// ---------------------------------------------------------------------------

/// Queue run over: resume the suspended context, or finish if nothing was
/// suspended. A `DeviceLost` end resumes nothing. The queue slot's player is
/// stopped only when it is **not** shared with the context being resumed
/// (`Arc::ptr_eq`).
async fn resume_or_finish(app: &Arc<Mutex<App>>, end: QueueEnd) {
  use crate::core::queue::SuspendedContext;

  let suspended = { app.lock().await.queue_suspended.take() };
  // The slot's desired play state carries into the resume, then resets: a
  // device removal that paused the slot keeps the context paused, and the
  // next queue run starts playing.
  let playing = std::mem::replace(&mut app.lock().await.queue_slot_desired_playing, true);

  // Take the queue slot's player so we can decide whether to stop it.
  let queue_player = { app.lock().await.take_queue_now_decoded_player() };

  // The slot gave its device up (see the driver's tick): a decoded context
  // sharing that player cannot restage onto it either. End it with the device
  // error instead of a failed track.
  if let Some(dead) = queue_player.as_ref().filter(|p| p.device_lost()) {
    let mut guard = app.lock().await;
    if drop_context_sharing(&mut guard, suspended.as_ref(), dead) {
      {
        let strings = guard.strings();
        guard.set_status_message(strings.status.infra.audio_disconnected.to_string(), 8)
      };
      return;
    }
  }

  // The device is gone, not the queue. A resume would load the context onto
  // the output the OS now calls default, the one the user just left. The
  // driver already reported the device.
  if matches!(end, QueueEnd::DeviceLost) {
    app.lock().await.release_decoded_sink_claim();
    return;
  }

  match suspended {
    None => {
      // Nothing was suspended: the queue was playing over an idle app (or a
      // context finished before the queue started). Stop the slot and note it.
      if let Some(player) = queue_player {
        player.stop();
        {
          let mut guard = app.lock().await;
          let message = guard.strings().status.infra.queue_finished;
          guard.set_status_message(message, 3);
        };
      }
      // No decoded context resumes, so the queue's hold on the sink ends here.
      app.lock().await.release_decoded_sink_claim();
    }
    Some(SuspendedContext::Qobuz {
      resume_index,
      resume_position_ms,
    }) => resume_qobuz(app, resume_index, resume_position_ms, queue_player, playing).await,
    // Nothing was suspended.
    #[allow(unreachable_patterns)]
    _ => {
      if let Some(player) = queue_player {
        player.stop();
      }
    }
  }
}

/// Drop the suspended decoded context whose player is `dead` (the queue slot
/// shared it), returning whether there was one.
fn drop_context_sharing(
  app: &mut App,
  suspended: Option<&crate::core::queue::SuspendedContext>,
  dead: &Arc<LocalPlayer>,
) -> bool {
  use crate::core::queue::SuspendedContext;
  match suspended {
    Some(SuspendedContext::Qobuz { .. }) => app
      .qobuz_playback
      .take_if(|s| Arc::ptr_eq(&s.player, dead))
      .is_some(),
    _ => false,
  }
}

async fn resume_qobuz(
  app: &Arc<Mutex<App>>,
  resume_index: Option<usize>,
  resume_position_ms: u64,
  queue_player: Option<Arc<LocalPlayer>>,
  playing: bool,
) {
  let Some(index) = resume_index else {
    // Take under the lock, stop off it: a sink clear waits for the audio thread.
    let session = {
      let mut guard = app.lock().await;
      guard.release_decoded_sink_claim();
      guard.qobuz_playback.take()
    };
    if let Some(s) = session {
      Arc::clone(&s.player).stop_detached_holding(s);
    }
    if let Some(player) = queue_player {
      player.stop();
    }
    return;
  };
  let resume = super::ResumePoint {
    position_ms: resume_position_ms,
    paused: !playing,
  };
  // Same track with its whole file still on disk: replay it and seek, with no
  // second download. Otherwise re-download the target through play_index.
  let replay = {
    let mut guard = app.lock().await;
    match guard.qobuz_playback.as_mut() {
      Some(s) if index == s.index && s.finished_file().is_some() => {
        s.advancing = true;
        s.finished_file()
          .map(|path| (Arc::clone(&s.player), path.to_path_buf()))
      }
      Some(_) => None,
      None => return,
    }
  };
  // The queue slot shares the context player, so it is never stopped here.
  let _ = queue_player;
  match replay {
    Some((player, path)) => {
      super::replay_file(player, path, Some(resume)).await;
      if let Some(s) = app.lock().await.qobuz_playback.as_mut() {
        s.advancing = false;
      }
    }
    None => crate::infra::qobuz::dispatch::play_index(app, index, Some(resume)).await,
  }
}

async fn set_status(app: &Arc<Mutex<App>>, message: String) {
  app.lock().await.set_status_message(message, 4);
}

#[cfg(test)]
mod tests {}
