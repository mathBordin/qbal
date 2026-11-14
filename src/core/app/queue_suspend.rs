use super::*;

impl App {
  /// Suspend the active decoded context with **skip** semantics (resume at the
  /// context's *next* track, position 0) and latch its `advancing` guard so the
  /// runner tick leaves it alone. Radio is torn down (a live stream can't share
  /// the sink) and its station stashed for reconnect. A no-op when no decoded
  /// context is active. Called before handing the sink to the native queue.
  ///
  /// `cause` decides where the context resumes under Repeat One: an auto-advance
  /// handoff replays the repeated track, a manual Next advances past it. See
  /// [`crate::infra::queue::resume_index_after_queue`].
  pub(crate) fn suspend_active_decoded_context_for_skip(
    &mut self,
    cause: crate::infra::queue::SuspendCause,
  ) {
    // `resume_index_after_queue` applies the per-mode wrap/clamp: Repeat All
    // wraps at the end (last -> first) so a skip at the final track resumes the
    // first one rather than reading as exhausted (`None`); Repeat One resumes
    // the *same* track on an auto-advance (a queued song must not consume the
    // repeat) but advances on a manual skip; Off clamps to `None` at the boundary.
    let repeat = self.decoded_repeat;
    #[allow(clippy::needless_return)]
    if let Some(s) = self.qobuz_playback.as_mut() {
      let resume_index =
        crate::infra::queue::resume_index_after_queue(s.index, s.tracks.len(), repeat, cause);
      // The queue takes the sink: a download in flight must not commit onto it.
      if let Some(fetch) = s.fetch.take() {
        fetch.abort();
      }
      s.advancing = true;
      self.queue_suspended = Some(crate::core::queue::SuspendedContext::Qobuz {
        resume_index,
        resume_position_ms: 0,
      });
      return;
    }
  }

  /// Suspend the active decoded context with **mid-track** semantics (resume the
  /// *same* track at its live position) — the Enter-jump path. Radio has no
  /// seekable position, so it is stashed for reconnect like the skip path.
  pub(crate) fn suspend_active_decoded_context_mid_track(&mut self) {
    #[allow(clippy::needless_return)]
    if let Some(s) = self.qobuz_playback.as_mut() {
      let position_ms = s.player.position().as_millis() as u64;
      let index = s.index;
      // The queue takes the sink: a download in flight must not commit onto it.
      if let Some(fetch) = s.fetch.take() {
        fetch.abort();
      }
      s.advancing = true;
      self.queue_suspended = Some(crate::core::queue::SuspendedContext::Qobuz {
        resume_index: Some(index),
        resume_position_ms: position_ms,
      });
      return;
    }
  }
}
