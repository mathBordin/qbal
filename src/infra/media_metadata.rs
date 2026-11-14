#![cfg_attr(
  not(any(feature = "discord-rpc", all(feature = "mpris", target_os = "linux"))),
  allow(dead_code)
)]

use crate::core::app::App;
use crate::core::plugin_api::RepeatState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackItemKind {
  Track,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackSource {
  ExternalDevice,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackMetadata {
  pub title: String,
  pub artists: Vec<String>,
  pub album: String,
  pub image_url: Option<String>,
  pub duration_ms: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackSnapshot {
  pub metadata: PlaybackMetadata,
  pub item_kind: PlaybackItemKind,
  pub item_id: Option<String>,
  pub item_uri: Option<String>,
  pub context_uri: Option<String>,
  pub source: PlaybackSource,
  pub progress_ms: u128,
  pub is_playing: bool,
  pub shuffle: bool,
  pub repeat: Option<RepeatState>,
}

impl PlaybackSnapshot {
  pub fn primary_artist(&self) -> String {
    self.metadata.artists.join(", ")
  }
}

pub fn current_playback_snapshot(app: &App) -> Option<PlaybackSnapshot> {
  playback_snapshot_with(app, Some(app.strings()))
}

/// History and track-change detection use stored metadata, so changing the
/// interface language cannot start another listen or restart a lyrics lookup.
pub fn tracking_playback_snapshot(app: &App) -> Option<PlaybackSnapshot> {
  playback_snapshot_with(app, None)
}

fn playback_snapshot_with(
  app: &App,
  strings: Option<&crate::core::i18n::Strings>,
) -> Option<PlaybackSnapshot> {
  // The decoded source (Qobuz, or a queued track) owns the audio sink while its
  // `*_playback` field is `Some`; progress and play-state are read live from
  // its player.
  source_playback_snapshot(app, strings)
}

fn snapshot_track_title(
  stored_name: &str,
  generated_name: Option<crate::core::plugin_api::GeneratedTrackName>,
  strings: Option<&crate::core::i18n::Strings>,
) -> String {
  match strings {
    Some(strings) => crate::core::plugin_api::GeneratedTrackName::display_name(
      stored_name,
      generated_name,
      strings,
    ),
    None => stored_name,
  }
  .to_owned()
}

/// Build a [`PlaybackSnapshot`] for whatever owns playback: the queue slot or
/// the Qobuz session. Metadata comes from the stored track info; progress and
/// play-state are read live from the player. Returns `None` when nothing plays.
fn source_playback_snapshot(
  app: &App,
  strings: Option<&crate::core::i18n::Strings>,
) -> Option<PlaybackSnapshot> {
  // The native queue slot wins over the playlist session: it is what is
  // actually audible, and it drives the playbar / MPRIS / cover art / lyrics
  // via the shared track-change detector.
  if let Some(crate::infra::queue::QueueNowPlaying::Decoded(d)) = app.queue_now.as_ref() {
    let mut snapshot = source_snapshot(
      snapshot_track_title(&d.track.name, d.track.generated_name, strings),
      d.track.artists.clone(),
      d.track.album.clone(),
      d.track.duration_ms as u32,
      d.track.uri.clone(),
      d.track.image_url.clone(),
      d.player.position().as_millis(),
      !d.player.is_paused(),
      app,
    );
    // The native queue ignores the shuffle/repeat modes (they belong to the
    // suspended playlist resumed once the queue drains), so don't advertise
    // them — mirroring the playbar's hidden controls.
    snapshot.shuffle = false;
    snapshot.repeat = None;
    return Some(snapshot);
  }

  // The queue owns the sink but its track is not published yet: the playlist
  // below is suspended, not audible, so reporting it would publish the wrong
  // track and its stale shuffle/repeat.
  if app.queue_owns_playback() {
    return None;
  }

  if let Some(qobuz) = app.qobuz_playback.as_ref() {
    let track = qobuz.tracks.get(qobuz.index)?;
    return Some(source_snapshot(
      track.name.clone(),
      track.artists.clone(),
      track.album.clone(),
      track.duration_ms as u32,
      track.uri.clone(),
      track.image_url.clone(),
      qobuz.player.position().as_millis(),
      // Until the new stream commits, the sink still holds the old track.
      !qobuz.player.is_paused() && !qobuz.advancing,
      app,
    ));
  }

  None
}

/// The cover URL to fetch for the snapshot's track.
#[cfg(feature = "art-decode")]
pub fn cover_fetch_url(snapshot: &PlaybackSnapshot) -> Option<String> {
  snapshot.metadata.image_url.clone()
}

/// Assemble a [`PlaybackSnapshot`] from a track's fields. `image_url` is the
/// cover URL when Qobuz provides one. Shuffle/repeat come from the
/// player-global state (the queue-slot caller blanks them).
#[allow(clippy::too_many_arguments)]
fn source_snapshot(
  title: String,
  artists: Vec<String>,
  album: String,
  duration_ms: u32,
  item_uri: Option<String>,
  image_url: Option<String>,
  progress_ms: u128,
  is_playing: bool,
  app: &App,
) -> PlaybackSnapshot {
  // Translate the player's repeat mode to the plugin-facing `RepeatState`
  // here at the snapshot boundary.
  use crate::infra::queue::RepeatMode;
  let repeat = match app.decoded_repeat {
    RepeatMode::Off => RepeatState::Off,
    RepeatMode::Context => RepeatState::Context,
    RepeatMode::Track => RepeatState::Track,
  };
  PlaybackSnapshot {
    metadata: PlaybackMetadata {
      title,
      artists,
      album,
      image_url,
      duration_ms,
    },
    item_kind: PlaybackItemKind::Track,
    item_id: None,
    item_uri,
    context_uri: None,
    source: PlaybackSource::ExternalDevice,
    progress_ms,
    is_playing,
    shuffle: app.decoded_shuffle,
    repeat: Some(repeat),
  }
}

#[cfg(test)]
pub(crate) mod tests {
  #[test]
  fn generated_display_titles_change_without_changing_tracking_metadata() {
    use crate::core::i18n::EN;
    use crate::core::plugin_api::GeneratedTrackName;
    let mut alternate = EN;
    alternate.status.infra.local_unknown_track = "Alternate unknown title";
    let generated = Some(GeneratedTrackName::LocalUnknown);
    let stored = EN.status.infra.local_unknown_track;
    assert_eq!(snapshot_track_title(stored, generated, Some(&EN)), stored);
    assert_eq!(
      snapshot_track_title(stored, generated, Some(&alternate)),
      "Alternate unknown title"
    );
    assert_eq!(snapshot_track_title(stored, generated, None), stored);
    assert_eq!(snapshot_track_title(stored, None, Some(&alternate)), stored);
  }

  use super::*;
}
