#![cfg_attr(
  not(any(
    feature = "discord-rpc",
    all(feature = "mpris", target_os = "linux"),
    all(feature = "macos-media", target_os = "macos"),
    all(feature = "windows-media", target_os = "windows")
  )),
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
  /// True only for internet-radio live streams (no track duration, ICY metadata).
  pub is_live: bool,
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

/// Build a [`PlaybackSnapshot`] for whichever non-Spotify decoded source
/// currently owns playback (at most one `*_playback` is `Some`). Metadata comes
/// from the source's stored track info; progress and play-state are read live
/// from its player. Returns `None` when no such source is active.
fn source_playback_snapshot(
  app: &App,
  strings: Option<&crate::core::i18n::Strings>,
) -> Option<PlaybackSnapshot> {
  // The native queue slot playing a decoded track wins over every per-source
  // context: it is what is actually audible, and it drives the playbar / MPRIS /
  // cover art / lyrics via the shared track-change detector.
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
    // The native queue ignores the decoded shuffle/repeat modes (they belong to
    // the suspended source resumed once the queue drains), so don't advertise
    // them — mirroring the radio override and the playbar's hidden controls.
    snapshot.shuffle = false;
    snapshot.repeat = None;
    return Some(snapshot);
  }

  // A queued *Spotify* track owns the sink (a decoded one returned above): every
  // `*_playback` below is then a suspended context, not what is audible, so
  // reporting one would publish the wrong track *and* its stale shuffle/repeat.
  // Fall through to the Spotify snapshot, which describes the queued track and
  // blanks the modes for a queue slot. Mirrors the same filter the runner applies
  // to its dedicated local-playback path.
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

fn is_subsonic(uri: &str) -> bool {
  crate::core::queue::queue_item_source(uri) == crate::core::queue::QueueItemSource::Subsonic
}

/// The cover URL to fetch in this process; for Subsonic it carries the login, so it is not in the snapshot.
#[cfg(feature = "art-decode")]
pub fn cover_fetch_url(app: &App, snapshot: &PlaybackSnapshot) -> Option<String> {
  if snapshot.metadata.image_url.is_some() {
    return snapshot.metadata.image_url.clone();
  }
  let uri = snapshot
    .item_uri
    .as_deref()
    .filter(|uri| is_subsonic(uri))?;
  if let Some(crate::infra::queue::QueueNowPlaying::Decoded(d)) = app.queue_now.as_ref() {
    if d.track.uri.as_deref() == Some(uri) {
      return d.track.image_url.clone();
    }
  }
  None
}

/// Assemble a [`PlaybackSnapshot`] from a decoded source's fields. `image_url`
/// is a directly-fetchable cover-art URL when the source provides one (Subsonic
/// getCoverArt, YouTube thumbnail) and `None` otherwise (local files carry
/// embedded art fetched separately; radio has none). Sources are always treated
/// as a single track; shuffle/repeat come from the player-global decoded state
/// (the radio caller overrides them since a live stream has no queue).
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
  // The decoded repeat is source-neutral; translate to the Spotify-shaped
  // `RepeatState` here at the snapshot boundary.
  use crate::infra::queue::RepeatMode;
  let repeat = match app.decoded_repeat {
    RepeatMode::Off => RepeatState::Off,
    RepeatMode::Context => RepeatState::Context,
    RepeatMode::Track => RepeatState::Track,
  };
  // Discord, MPRIS, Now Playing, the desktop host and the cloud feed publish
  // this field, and a Subsonic URL carries the login (`u`, `t`, `s`).
  let image_url = image_url.filter(|_| !item_uri.as_deref().is_some_and(is_subsonic));
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
    is_live: false,
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
