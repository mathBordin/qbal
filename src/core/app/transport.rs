use super::*;

impl App {
  pub fn toggle_playback(&mut self) {
    // The native queue slot owns the sink: toggle its player directly (covers the
    // idle-app case where no per-source context is set).
    if let Some(player) = self.queue_now_decoded_player() {
      if player.is_paused() {
        player.resume();
      } else {
        player.pause();
      }
      return;
    }

    // Qobuz playback owns the session: toggle its sink directly. The playbar
    // reads pause state live from the player, so nothing else to update.
    if let Some(qobuz) = &self.qobuz_playback {
      if qobuz.player.is_paused() {
        qobuz.player.resume();
      } else {
        qobuz.player.pause();
      }
      return;
    }

    // A decoded start in flight, or a source that lost its sink: nothing to
    // toggle yet.
    self.report_nothing_playing();
  }

  pub fn previous_track(&mut self) {
    info!("playing previous track or restarting current track");
    // The native queue owns playback: a forward-only queue has no "previous",
    // so restart the current queued track. The queue router intercepts the
    // dispatched event for both decoded and Spotify queue slots.
    if self.queue_owns_playback() {
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::PreviousTrack);
      return;
    }
    // A decoded source owns the session: route to its dispatcher, never to the
    // paused librespot. Preserve the ">= 3s restarts current, else previous"
    // semantics (radio no-ops both Seek and PreviousTrack).
    if self.active_decoded_source() {
      if self.song_progress_ms >= 3_000 {
        self.dispatch(IoEvent::Seek(0));
      } else {
        self.dispatch(IoEvent::PreviousTrack);
      }
      self.song_progress_ms = 0;
      return;
    }
    self.report_nothing_playing();
  }

  pub fn force_previous_track(&mut self) {
    info!("force skipping to previous track");
    // The native queue owns playback: restart the current queued track (the
    // queue router intercepts the event for both slot kinds).
    if self.queue_owns_playback() {
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::ForcePreviousTrack);
      return;
    }
    // A decoded source owns the session: route to its dispatcher, never to the
    // paused librespot. The source handles or no-ops ForcePreviousTrack.
    if self.active_decoded_source() {
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::ForcePreviousTrack);
      return;
    }

    self.song_progress_ms = 0;
    self.report_nothing_playing();
  }

  pub fn next_track(&mut self) {
    info!("skipping to next track");
    // The native queue owns playback: skip to the next queued item (or resume
    // the suspended context when the queue drains).
    if self.queue_owns_playback() {
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::AdvanceNativeQueue);
      return;
    }
    // A decoded context is playing with items waiting in the queue: suspend it
    // (skip semantics — resume at the context's next track) and start the queue.
    // An explicit Next advances the context even under Repeat One, matching the
    // per-source skip paths: repeat-one only replays on *auto* advance.
    if self.active_decoded_source() && !self.native_queue.is_empty() {
      self.suspend_active_decoded_context_for_skip(crate::infra::queue::SuspendCause::ManualSkip);
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::AdvanceNativeQueue);
      return;
    }
    // A decoded source (local/subsonic/radio/youtube) owns the session: route to
    // its dispatcher, never to the paused librespot. The source handles or
    // no-ops NextTrack (radio has no queue).
    if self.active_decoded_source() {
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::NextTrack);
      return;
    }

    // Fallback for external devices
    self.report_nothing_playing();
  }

  /// Start playback of an explicit list of playable URIs, optionally from an
  /// offset into that list. The single place the shared action vocabulary
  /// builds a URI-list `StartPlayback`; ownership routing happens in the
  /// pump's source routers, exactly as for the equivalent keybinding.
  #[cfg_attr(not(feature = "scripting"), allow(dead_code))]
  pub(crate) fn start_playback_uris(&mut self, uris: Vec<String>, offset: Option<usize>) {
    // No URIs: nothing to start, and an empty start would only tear the current player down.
    if uris.is_empty() {
      return;
    }
    // One Music track at a time: there is no cross-source queue for Apple Music.
    if uris.iter().all(|_uri| false) {
      let offset = offset.unwrap_or(0);
      if let Some(uri) = uris.get(offset) {
        let _occurrence = uris[..offset].iter().filter(|u| *u == uri).count();
      }
      return;
    }
    self.dispatch(IoEvent::StartPlayback(None, Some(uris), offset));
  }

  /// Start playback of a Spotify context URI, optionally from a 0-based track
  /// offset. The context twin of [`Self::start_playback_uris`].
  #[cfg_attr(not(feature = "scripting"), allow(dead_code))]
  pub(crate) fn start_playback_context(&mut self, context_uri: String, offset: Option<usize>) {
    self.dispatch(IoEvent::StartPlayback(Some(context_uri), None, offset));
  }

  /// Start playback of one track URI as the first track inside a Spotify
  /// context URI. The third start shape beside the two twins above: the
  /// network layer deliberately does not trim the uri list when a context is
  /// present, which is what keeps the selected track first under shuffle.
  #[cfg_attr(not(feature = "scripting"), allow(dead_code))]
  pub(crate) fn start_playback_track_in_context(&mut self, context_uri: String, track_uri: String) {
    self.dispatch(IoEvent::StartPlayback(
      Some(context_uri),
      Some(vec![track_uri]),
      Some(0),
    ));
  }
}

#[cfg(test)]
mod devices_tests {}
