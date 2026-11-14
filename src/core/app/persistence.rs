use super::*;

/// Debounce window for coalescing hot-path runtime-state saves (volume,
/// shuffle) into one disk write, and the retry delay after a failed save.
const STATE_SAVE_DEBOUNCE_MS: u64 = 500;

impl App {
  /// Snapshot the currently-playing non-Spotify session for persistence, or
  /// `None` when Spotify (or nothing) owns playback. Reads the live position and
  /// pause state straight from whichever source's player is active, mirroring
  /// the source-ownership order the runner tick uses. Spotify playback is not
  /// persisted here — its resume is handled by `startup_behavior` device logic.
  /// The resume point (`resume_index`, `resume_position_ms`) recorded when the
  /// active decoded context was suspended under the native queue, or `None` when
  /// no decoded context is suspended. Only one context is ever active, so this
  /// unambiguously describes it.
  fn suspended_resume(&self) -> Option<(Option<usize>, u64)> {
    match self.queue_suspended.as_ref()? {
      crate::core::queue::SuspendedContext::Qobuz {
        resume_index,
        resume_position_ms,
      } => Some((*resume_index, *resume_position_ms)),
      #[allow(unreachable_patterns)]
      _ => None,
    }
  }

  pub fn current_persisted_playback(
    &self,
  ) -> Option<crate::core::persisted_playback::PersistedPlayback> {
    if !self.has_persistable_playback() {
      return None;
    }
    use crate::core::persisted_playback::PersistedPlayback;
    if let Some(s) = self.qobuz_playback.as_ref() {
      match self.suspended_resume() {
        Some((Some(index), position_ms)) => {
          return Some(PersistedPlayback::Qobuz {
            tracks: s.tracks.clone(),
            index,
            position_ms,
            paused: false,
            repeat: self.decoded_repeat,
            shuffle_on: self.decoded_shuffle,
            shuffle: s.shuffle_backup.clone(),
          });
        }
        Some((None, _)) => {}
        None => {
          return Some(PersistedPlayback::Qobuz {
            tracks: s.tracks.clone(),
            index: s.index,
            position_ms: s.player.position().as_millis() as u64,
            paused: s.player.is_paused(),
            repeat: self.decoded_repeat,
            shuffle_on: self.decoded_shuffle,
            shuffle: s.shuffle_backup.clone(),
          });
        }
      }
    }
    None
  }

  /// Snapshot the full session to persist: the active non-Spotify playback (if
  /// any) plus the native queue. Returns `None` only when there is nothing to
  /// save (no active source *and* an empty queue), so the caller clears the
  /// session file — preserving the existing Some→None clear semantics.
  pub fn current_persisted_session(
    &self,
  ) -> Option<crate::core::persisted_playback::PersistedSession> {
    let playback = self.current_persisted_playback();
    let queue_now_track = self.queue_now_track();
    if playback.is_none() && self.native_queue.is_empty() && queue_now_track.is_none() {
      return None;
    }
    // A track playing through the queue slot has been popped off `native_queue`;
    // prepend it so a mid-queue quit resumes it (it re-enters the queue on the
    // next launch).
    let mut queue = self.native_queue.clone();
    if let Some(track) = queue_now_track {
      queue.insert(0, track.clone());
    }
    Some(crate::core::persisted_playback::PersistedSession { playback, queue })
  }

  /// Cheap presence check for `current_persisted_session`: same decision
  /// logic, no snapshot allocation. Lets the tick loop skip building the
  /// snapshot (which clones the whole native queue) unless a save is due.
  pub fn has_persistable_session(&self) -> bool {
    self.has_persistable_playback()
      || !self.native_queue.is_empty()
      || self.queue_now_track().is_some()
  }

  fn has_persistable_playback(&self) -> bool {
    // Mirrors `current_persisted_playback`: a decoded context suspended past
    // its end (resume_index == None) does not persist.
    {
      let context_resumable = !matches!(self.suspended_resume(), Some((None, _)));
      if self.qobuz_playback.is_some() && context_resumable {
        return true;
      }
    }
    {
      false
    }
  }

  /// Schedule a debounced state save. Hot paths (volume, shuffle) call this
  /// instead of `save_runtime_state()` so a held key doesn't pay
  /// disk + YAML work on every auto-repeat; the save lands once, shortly
  /// after the last change.
  pub fn schedule_state_save(&mut self, patch: PersistedRuntimeState) {
    if patch.is_empty() {
      return;
    }
    self.pending_state_save_patch.merge_patch(&patch);
    self.state_save_due = Some(Instant::now() + Duration::from_millis(STATE_SAVE_DEBOUNCE_MS));
  }

  pub fn save_runtime_state(&self, patch: &PersistedRuntimeState) -> anyhow::Result<()> {
    if patch.is_empty() {
      return Ok(());
    }
    let path = match &self.state_path {
      Some(path) => path.clone(),
      None => crate::core::state::default_state_path()?,
    };
    crate::core::state::save(&path, patch)
  }

  /// Flush a scheduled state save once its debounce window has passed, or
  /// immediately when `force` is set (shutdown).
  pub fn flush_state_save(&mut self, force: bool) {
    let Some(due) = self.state_save_due else {
      return;
    };
    if force || Instant::now() >= due {
      let patch = std::mem::take(&mut self.pending_state_save_patch);
      self.state_save_due = None;
      match self.save_runtime_state(&patch) {
        Ok(()) => self.state_save_error_reported = false,
        Err(e) => {
          self.pending_state_save_patch.merge_patch(&patch);
          // Keep the save scheduled: with the deadline cleared the retained patch
          // would never be retried (this fn returns early on a `None` deadline),
          // and the forced shutdown flush would silently drop it.
          self.state_save_due =
            Some(Instant::now() + Duration::from_millis(STATE_SAVE_DEBOUNCE_MS));
          // Reported to the status bar, not as an error page: the tick calls
          // this every frame and the retry above re-arms twice a second, so as
          // a modal an unwritable state dir is an unusable app, and as an
          // `api_error` it is one no frontend can clear because every retry
          // restamps the lifetime. Latched to one report per failure run, or
          // the same rate would pin the status bar into error mode and drop
          // every ordinary message for the rest of the session.
          if !self.state_save_error_reported {
            self.state_save_error_reported = true;
            self.set_error_status_message(
              self.strings().status.app.state_save_failed(&e.to_string()),
              8,
            );
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {}
