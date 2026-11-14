use super::*;

/// Grace window after the last tick before playback-position reads go stale.
const STALE_TICK_AFTER: Duration = Duration::from_secs(2);

// The window must cover at least two of the slowest allowed ticks, so one
// missed frame can never read as stale. The tick rate is bounded on every
// path (config load rejects out-of-range values, the settings screen clamps),
// and this assertion keeps the two constants coupled if either ever moves.
const _: () = assert!(
  STALE_TICK_AFTER.as_millis() >= 2 * crate::core::user_config::MAX_TICK_RATE_MILLISECONDS as u128
);

impl App {
  /// Milliseconds into the current track, or `None` when no tick has run for
  /// over [`STALE_TICK_AFTER`]. A frontend that stops driving
  /// `core::driver::Driver::tick` (or never starts) would otherwise show a
  /// silently frozen playbar; `None` is the loud version of that failure,
  /// rendered as an explicit stall by the playbar.
  pub fn playback_position_ms(&self) -> Option<u128> {
    (self.last_tick_at.elapsed() <= STALE_TICK_AFTER).then_some(self.song_progress_ms)
  }

  pub fn update_on_tick(&mut self, elapsed: Duration) {
    self.last_tick_at = Instant::now();

    // Increment global animation tick (wraps after ~9.4 quintillion ticks, effectively never)
    self.view.animation_tick = self.view.animation_tick.wrapping_add(1);

    // Advance an adaptive-theme fade. Real elapsed time, not tick count, so
    // the fade speed is independent of the configured tick rates.
    #[cfg(feature = "art-decode")]
    if let Some(transition) = self.theme_transition.as_mut() {
      transition.advance(elapsed);
      let current = transition.current();
      if self.user_config.theme != current {
        self.user_config.theme = current;
      }
      if transition.is_complete() {
        self.theme_transition = None;
        // A finished fade-out means the user's own theme is back in place.
        if let crate::core::cover_theme::CoverThemeState::Restoring { .. } = self.cover_theme_state
        {
          self.cover_theme_state = crate::core::cover_theme::CoverThemeState::Inactive;
        }
      }
    }

    if let Some(expires_at) = self.status_message_expires_at {
      if Instant::now() >= expires_at {
        self.clear_status_message();
      }
    }

    // Must stay above the early returns further down this fn, or the backstop
    // would silently stop running during playback.
    self.expire_api_error();

    if let Some(frame) = self.view.liked_song_animation_frame {
      if frame > 0 {
        self.view.liked_song_animation_frame = Some(frame - 1);
      } else {
        self.view.liked_song_animation_frame = None;
      }
    }

    self.advance_lyrics_scroll(elapsed);

    let playing_now =
      self.user_config.behavior.keepawake_enabled && self.decoded_playing_state().unwrap_or(false);
    self.keepawake.request(playing_now);

    // Music's last reported position, advanced while it plays.
  }
}

#[cfg(test)]
mod tests {}
