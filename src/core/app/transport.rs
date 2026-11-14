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
    // dispatched event.
    if self.queue_owns_playback() {
      self.song_progress_ms = 0;
      self.dispatch(IoEvent::PreviousTrack);
      return;
    }
    // The Qobuz session owns playback: route to its dispatcher, keeping the
    // ">= 3s restarts the current track, else previous" semantics.
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
    // The Qobuz session owns playback: route to its dispatcher.
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
    // The Qobuz session owns playback: route to its dispatcher.
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
    self.dispatch(IoEvent::StartPlayback(None, Some(uris), offset));
  }
}

#[cfg(test)]
mod devices_tests {}
