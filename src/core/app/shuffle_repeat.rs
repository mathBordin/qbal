use super::*;

impl App {
  /// Apply the current [`decoded_shuffle`](Self::decoded_shuffle) flag to whatever
  /// queueable decoded source owns playback: reorder its queue when turning on,
  /// or restore the original order when turning off. The currently-playing track
  /// is unaffected (it stays at the front), so audio continues uninterrupted.
  /// Call only when [`active_queueable_decoded_source`](Self::active_queueable_decoded_source)
  /// holds, so exactly one branch applies.
  fn apply_decoded_shuffle(&mut self) {
    let on = self.decoded_shuffle;
    if let Some(s) = self.qobuz_playback.as_mut() {
      if !s.advancing {
        s.set_shuffle(on);
      }
    }
  }

  /// Re-sync the active queueable source's queue order to `decoded_shuffle` when
  /// a shuffle toggle was deferred by [`apply_decoded_shuffle`](Self::apply_decoded_shuffle)
  /// because a track change was in flight. Driven by the runner tick, so it lands
  /// as soon as the advance commits (`advancing` clears). A no-op whenever the
  /// order already matches `decoded_shuffle`, so it is cheap to call every tick.
  pub(crate) fn reconcile_decoded_shuffle(&mut self) {
    // The native queue owns the sink: any per-source struct is a suspended
    // context whose order is reconciled when it resumes, not now.
    if self.queue_owns_playback() {
      return;
    }
    let on = self.decoded_shuffle;
    if let Some(s) = self.qobuz_playback.as_mut() {
      if !s.advancing && s.shuffle_backup.is_some() != on {
        s.set_shuffle(on);
      }
    }
  }

  /// Set the shuffle to an explicit value from an external media controller
  /// (MPRIS), reordering the playlist. Returns whether the Qobuz session
  /// consumed it; on `false` (the queue slot plays) the caller rejects the
  /// request and corrects the client's property.
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  pub fn set_decoded_shuffle(&mut self, on: bool) -> bool {
    if !self.active_queueable_decoded_source() {
      return false;
    }
    if self.decoded_shuffle != on {
      self.decoded_shuffle = on;
      self.apply_decoded_shuffle();
    }
    true
  }

  /// Set the decoded repeat mode to an explicit value from an external media
  /// controller (MPRIS). Returns whether a queueable decoded source consumed it
  /// (see [`set_decoded_shuffle`](Self::set_decoded_shuffle)).
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  pub fn set_decoded_repeat(&mut self, mode: RepeatMode) -> bool {
    if !self.active_queueable_decoded_source() {
      return false;
    }
    self.decoded_repeat = mode;
    true
  }

  pub fn shuffle(&mut self) {
    // Toggle the player-global shuffle and reorder the playlist in place (the
    // current track stays at the front, so playback continues uninterrupted).
    if self.active_queueable_decoded_source() {
      self.decoded_shuffle = !self.decoded_shuffle;
      self.apply_decoded_shuffle();
      let label = if self.decoded_shuffle {
        self.strings().status.app.shuffle_on
      } else {
        self.strings().status.app.shuffle_off
      };
      self.set_status_message(label, 2);
      return;
    }

    // The native queue slot plays an explicit list over a suspended playlist:
    // it hides the shuffle button and blanks shuffle in the MPRIS snapshot, so
    // the key reports that shuffle does not apply.
    if self.active_decoded_source() || self.queue_owns_playback() {
      self.set_status_message(self.strings().status.app.shuffle_unsupported, 2);
    }
  }

  pub fn repeat(&mut self) {
    // Cycle the player-global repeat: Off -> Repeat All -> Repeat One -> Off.
    if self.active_queueable_decoded_source() {
      self.decoded_repeat = self.decoded_repeat.next();
      let label = match self.decoded_repeat {
        RepeatMode::Off => self.strings().status.app.repeat_off,
        RepeatMode::Context => self.strings().status.app.repeat_all,
        RepeatMode::Track => self.strings().status.app.repeat_one,
      };
      self.set_status_message(label, 2);
      return;
    }

    // See `shuffle`: the queue slot has no repeat of its own.
    if self.active_decoded_source() || self.queue_owns_playback() {
      self.set_status_message(self.strings().status.app.repeat_unsupported, 2);
    }
  }
}
