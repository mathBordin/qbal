//! Native cross-source playback queue: source identity and suspension records.
//!
//! The queue itself is a `Vec<TrackInfo>` on [`App`](crate::core::app::App); this
//! module holds the small value types that classify a queue item by its source
//! (URI scheme) and record how to resume the underlying per-source context once
//! the queue drains.

/// Which source a queue item plays through, derived from its URI scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueItemSource {
  Spotify,
  // Constructed by the terminal UI's queue badges only.
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  LocalFile,
  Subsonic,
  Qobuz,
}

/// Classify a queue item by its URI scheme. Anything that is not a local file,
/// Subsonic, YouTube, or Qobuz URI is treated as Spotify (the `spotify:track:`
/// scheme). Radio URIs (`radio:`) are never queued, so they are rejected before
/// reaching this function.
pub fn queue_item_source(uri: &str) -> QueueItemSource {
  if uri.starts_with("qobuz:") {
    QueueItemSource::Qobuz
  } else {
    QueueItemSource::Spotify
  }
}

/// A short, human-readable tag for a queue item's source, shown in the Queue
/// screen next to each row.
pub fn source_label(source: QueueItemSource) -> &'static str {
  match source {
    QueueItemSource::Spotify => "Spotify",
    QueueItemSource::LocalFile => "Local",
    QueueItemSource::Subsonic => "Subsonic",
    QueueItemSource::Qobuz => "Qobuz",
  }
}

/// Whether this build can actually play a queue item from the given source.
/// A slim build (no source features) can only play Spotify tracks via native
/// streaming; each alternative source is gated on its own Cargo feature. The queue
/// consults this to skip unplayable items with a status message instead of
/// stalling the queue.
pub fn source_available(source: QueueItemSource) -> bool {
  match source {
    QueueItemSource::Spotify => false,
    QueueItemSource::LocalFile => false,
    QueueItemSource::Subsonic => false,
    QueueItemSource::Qobuz => true,
  }
}

/// How to resume the underlying per-source context after the native queue
/// drains. Recorded when a track is queued over an active context.
///
/// `resume_index: None` means the context was exhausted, so it should be torn
/// down rather than resumed. In a slim build (no source features) this enum has
/// zero variants — the `App` field is `Option<SuspendedContext>`, so that is a
/// valid, always-`None` type.
#[derive(Debug, Clone)]
pub enum SuspendedContext {
  Qobuz {
    resume_index: Option<usize>,
    resume_position_ms: u64,
  },
}

#[cfg(test)]
mod tests {}
