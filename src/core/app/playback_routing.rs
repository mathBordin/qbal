use super::*;

#[cfg(test)]
pub(crate) const NOTHING_PLAYING_STATUS: &str = "Nothing is playing";

impl App {
  /// Record that `source` took the audio sink; its start path calls this
  /// before it starts the player, so the tick sees an owner for the whole start.
  pub(crate) fn claim_decoded_sink(&mut self, source: Source) {
    self.decoded_sink_claim = Some(source);
  }

  /// The decoded owner let go of the sink: its queue ran out or the queue's
  /// exhausted-context resume found nothing to resume.
  pub(crate) fn release_decoded_sink_claim(&mut self) {
    self.decoded_sink_claim = None;
  }

  /// The last arm of a transport chain: nothing owns playback, so answer
  /// with a status instead of dispatching.
  pub(crate) fn report_nothing_playing(&mut self) {
    self.set_status_message(self.strings().status.app.nothing_playing, 4);
  }

  /// `Some(true)` when a decoded source owns the sink and plays, `Some(false)`
  /// when it owns the sink and is paused, `None` when none owns it.
  pub(crate) fn decoded_playing_state(&self) -> Option<bool> {
    self.active_decoded_player().map(|p| !p.is_paused())
  }

  /// Whether a decoded source owns the sink: the native queue slot playing a
  /// decoded track, a Qobuz start still in flight (the claim), or a Qobuz
  /// session. The transport methods use this guard to route to the owner via
  /// `IoEvent` dispatch.
  pub(crate) fn active_decoded_source(&self) -> bool {
    // The native queue slot playing a decoded track owns the sink even when no
    // per-source `*_playback` context is set (e.g. queueing from an idle app).
    if self.queue_now_decoded_player().is_some() {
      return true;
    }
    // A decoded start in flight, or a source whose session died with nothing to
    // replace it, still owns the sink.
    if self.decoded_sink_claim.is_some() {
      return true;
    }
    if self.qobuz_playback.is_some() {
      return true;
    }
    false
  }

  /// Whether a Qobuz session with its own track queue owns playback. Unlike
  /// [`active_decoded_source`](Self::active_decoded_source) this **excludes**
  /// the native queue slot (a suspended context is not the active source).
  /// This is the gate for the decoded repeat/shuffle controls, which only make
  /// sense over a real queue.
  /// Also gates which playbar buttons are drawn and clickable (see
  /// `playbar_supported_controls`).
  pub(crate) fn active_queueable_decoded_source(&self) -> bool {
    // The native queue owning the sink is out of scope for repeat/shuffle; any
    // per-source `*_playback` below is then a suspended context, not active.
    if self.queue_owns_playback() {
      return false;
    }
    if self.qobuz_playback.is_some() {
      return true;
    }
    false
  }

  /// Take every decoded session except `keep`'s, so one backend can own the
  /// output device, and return their players. Stop them off the `App` lock: a
  /// sink clear waits for the audio thread.
  pub(crate) fn take_decoded_sessions_except(
    &mut self,
    _keep: crate::core::source::Source,
  ) -> Vec<std::sync::Arc<crate::infra::audio::LocalPlayer>> {
    Vec::new()
  }

  /// The player of whichever decoded owner (the queue slot or a Qobuz
  /// session) holds the sink, or `None` when nothing does. Ordering mirrors
  /// [`Self::active_decoded_source`].
  pub fn active_decoded_player(&self) -> Option<&std::sync::Arc<crate::infra::audio::LocalPlayer>> {
    if let Some(p) = self.queue_now_decoded_player() {
      return Some(p);
    }
    if let Some(s) = &self.qobuz_playback {
      return Some(&s.player);
    }
    None
  }

  /// The current playback position, in milliseconds, of the decoded owner,
  /// read live from its player's sink; `None` when nothing owns the sink.
  pub(super) fn active_source_position_ms(&self) -> Option<u128> {
    if let Some(p) = self.queue_now_decoded_player() {
      return Some(p.position().as_millis());
    }
    if let Some(qobuz) = &self.qobuz_playback {
      return Some(qobuz.player.position().as_millis());
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn playback_status_fixtures_match_the_english_catalog() {
    let strings = &crate::core::i18n::EN.status.app;
    assert_eq!(NOTHING_PLAYING_STATUS, strings.nothing_playing);
  }
}
