use super::*;

impl App {
  pub fn seek_forwards(&mut self) {
    info!(
      "seeking forwards by {} ms",
      self.user_config.behavior.seek_milliseconds
    );
    // Seek relative to the player's live position. The player clamps to the
    // track duration internally, so no upper clamp is needed.
    if self.active_decoded_source() {
      if let Some(pos) = self.active_source_position_ms() {
        let new_progress = (pos as u32).saturating_add(self.user_config.behavior.seek_milliseconds);
        self.song_progress_ms = new_progress as u128;
        self.seek_ms = None;
        self.dispatch(IoEvent::Seek(new_progress));
      }
    }
  }

  pub fn seek_backwards(&mut self) {
    info!(
      "seeking backwards by {} ms",
      self.user_config.behavior.seek_milliseconds
    );
    // Seek relative to the player's live position.
    if self.active_decoded_source() {
      if let Some(pos) = self.active_source_position_ms() {
        let new_progress = (pos as u32).saturating_sub(self.user_config.behavior.seek_milliseconds);
        self.song_progress_ms = new_progress as u128;
        self.seek_ms = None;
        self.dispatch(IoEvent::Seek(new_progress));
      }
    }
  }

  /// Seek to an absolute position within the current track (e.g. from clicking or
  /// dragging on the playbar progress line). The target is clamped to the track
  /// duration by the player.
  pub fn seek_to(&mut self, position_ms: u32) {
    if self.active_decoded_source() && self.active_source_position_ms().is_some() {
      // Decoded `.seek()` can re-decode forward for many codecs, so a mouse
      // drag must not dispatch one seek per drag event; coalesce to the last
      // target with the same throttle-and-flush pattern as the other backends.
      self.queue_source_seek(position_ms);
    }
  }

  /// Queue a seek with throttling
  fn queue_source_seek(&mut self, position_ms: u32) {
    // Always update UI immediately
    self.song_progress_ms = position_ms as u128;
    self.seek_ms = None;

    const SOURCE_SEEK_THROTTLE_MS: u128 = 50;
    let should_seek_now = self
      .last_source_seek
      .is_none_or(|t| t.elapsed().as_millis() >= SOURCE_SEEK_THROTTLE_MS);

    if should_seek_now {
      self.execute_source_seek(position_ms);
    } else {
      // Queue the seek - will be flushed by tick loop or next seek
      self.pending_source_seek = Some(position_ms);
    }
  }

  /// Execute a seek and update tracking state
  fn execute_source_seek(&mut self, position_ms: u32) {
    self.pending_source_seek = None;
    self.last_source_seek = Some(Instant::now());
    self.dispatch(IoEvent::Seek(position_ms));
  }

  /// Flush any pending seek (called from tick loop)
  pub fn flush_pending_source_seek(&mut self) {
    // Queued for a decoded owner; once that owner is gone the value would
    // reach whoever holds the sink now.

    if self.pending_source_seek.is_some() && !self.active_decoded_source() {
      self.pending_source_seek = None;
      return;
    }
    if let Some(position) = self.pending_source_seek {
      const SOURCE_SEEK_THROTTLE_MS: u128 = 50;
      let should_flush = self
        .last_source_seek
        .is_none_or(|t| t.elapsed().as_millis() >= SOURCE_SEEK_THROTTLE_MS);

      if should_flush {
        self.execute_source_seek(position);
      }
    }
  }
}
