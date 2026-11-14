//! Native playback queue: item checks and suspension records.
//!
//! The queue itself is a `Vec<TrackInfo>` on [`App`](crate::core::app::App); this
//! module holds the small value types that check a queue item and record how to
//! resume the underlying playlist once the queue drains.

/// Whether a queued URI is a Qobuz track, the only kind Qbal plays. A queue
/// restored from an older session can still hold other URIs.
pub fn is_qobuz_uri(uri: &str) -> bool {
  uri.starts_with("qobuz:")
}

/// How to resume the underlying per-source context after the native queue
/// drains. Recorded when a track is queued over an active context.
///
/// `resume_index: None` means the context was exhausted, so it should be torn
/// down rather than resumed.
#[derive(Debug, Clone)]
pub enum SuspendedContext {
  Qobuz {
    resume_index: Option<usize>,
    resume_position_ms: u64,
  },
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_qobuz_uris_are_playable_queue_items() {
    assert!(is_qobuz_uri("qobuz:track:123"));
    assert!(!is_qobuz_uri("spotify:track:0DiWol3AO6WpXZgp0goxAV"));
    assert!(!is_qobuz_uri(""));
  }
}
