use super::*;

impl App {
  pub(super) fn apply_seek(&mut self, _seek_ms: u32) {}

  pub fn seek_forwards(&mut self) {
    info!(
      "seeking forwards by {} ms",
      self.user_config.behavior.seek_milliseconds
    );
    // A decoded source owns the session: seek relative to *its* live position,
    // never from the stale/foreign Spotify progress. The source player clamps
    // to the track duration internally, so no upper clamp is needed (and we
    // must not read the stale Spotify context duration). Radio has no position
    // (not seekable): stop there instead of repositioning the paused Spotify
    // player underneath it.
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
    // A decoded source owns the session: seek relative to *its* live position,
    // never from the stale/foreign Spotify progress. Radio has no position (not
    // seekable): stop there instead of repositioning the paused Spotify player
    // underneath it.
    if self.active_decoded_source() {
      if let Some(pos) = self.active_source_position_ms() {
        let new_progress = (pos as u32).saturating_sub(self.user_config.behavior.seek_milliseconds);
        self.song_progress_ms = new_progress as u128;
        self.seek_ms = None;
        self.dispatch(IoEvent::Seek(new_progress));
      }
      return;
    }
    let old_progress = match self.seek_ms {
      Some(seek_ms) => seek_ms,
      None => self.song_progress_ms,
    };
    let new_progress =
      (old_progress as u32).saturating_sub(self.user_config.behavior.seek_milliseconds);
    self.seek_ms = Some(new_progress as u128);

    // Fallback: API-based seek for external devices (with throttling)
    self.queue_api_seek(new_progress);
  }

  /// Seek to an absolute position within the current track (e.g. from clicking or
  /// dragging on the playbar progress line). The target is clamped to the track
  /// duration. Mirrors the dispatch logic of [`Self::seek_forwards`].
  pub fn seek_to(&mut self, position_ms: u32) {
    // A decoded source owns the session: seek it to the absolute target directly
    // (the source player clamps to the track duration internally). Never read
    // the stale Spotify context duration for a source. Radio has no position
    // (not seekable): stop there instead of repositioning the paused Spotify
    // player underneath it.
    if self.active_decoded_source() && self.active_source_position_ms().is_some() {
      // Decoded `.seek()` can re-decode forward for many codecs, so a mouse
      // drag must not dispatch one seek per drag event; coalesce to the last
      // target with the same throttle-and-flush pattern as the other backends.
      self.queue_source_seek(position_ms);
    }
  }

  /// Queue a decoded-source (local/Subsonic/YouTube) seek with throttling
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

  /// Execute a decoded-source seek and update tracking state
  fn execute_source_seek(&mut self, position_ms: u32) {
    self.pending_source_seek = None;
    self.last_source_seek = Some(Instant::now());
    self.dispatch(IoEvent::Seek(position_ms));
  }

  /// Flush any pending decoded-source seek (called from tick loop)
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

  /// Queue an API-based seek with throttling (for external device control)
  fn queue_api_seek(&mut self, position_ms: u32) {
    // Always update UI immediately
    self.song_progress_ms = position_ms as u128;
    self.seek_ms = None;

    // Start the ignore window immediately when the user requests a seek
    // This prevents position updates from overwriting our target while waiting
    let now = Instant::now();

    // Mark poll data as stale so resync won't happen after ignore window
    self.instant_since_last_current_playback_poll = now;

    // Throttle API calls (max ~5/sec to respect rate limits)
    const API_SEEK_THROTTLE_MS: u128 = 200;
    let should_seek_now = self
      .last_api_seek
      .is_none_or(|t| t.elapsed().as_millis() >= API_SEEK_THROTTLE_MS);

    // Update last_api_seek for BOTH the ignore window AND throttling
    // This ensures the ignore window starts immediately on any seek request
    self.last_api_seek = Some(now);

    if should_seek_now {
      self.execute_api_seek(position_ms);
    } else {
      // Queue the seek - will be flushed by tick loop
      self.pending_api_seek = Some(position_ms);
    }
  }

  /// Execute an API-based seek
  fn execute_api_seek(&mut self, position_ms: u32) {
    self.pending_api_seek = None;
    self.apply_seek(position_ms);
  }

  /// Flush any pending API seek (called from tick loop)
  pub fn flush_pending_api_seek(&mut self) {
    // Queued for an external Connect device; under any other owner the Web
    // API seek would land on a device that is not the audible one.
    if self.pending_api_seek.is_some() {
      self.pending_api_seek = None;
      return;
    }
    if let Some(position) = self.pending_api_seek {
      const API_SEEK_THROTTLE_MS: u128 = 200;
      let should_flush = self
        .last_api_seek
        .is_none_or(|t| t.elapsed().as_millis() >= API_SEEK_THROTTLE_MS);

      if should_flush {
        self.execute_api_seek(position);
      }
    }
  }
}
