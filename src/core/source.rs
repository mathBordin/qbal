//! Capability-based media-source traits — the seam for the multi-source refactor.
//!
//! A source is addressed by its URI **scheme** (`spotify:`, `file:`, `subsonic:`),
//! mirroring Mopidy's proven dispatch model. [`MediaSource`] is the required
//! minimum every source implements; the remaining traits are optional
//! capabilities discovered at runtime, so the UI can light up per source (e.g.
//! a source without [`LibraryProvider`] shows no "liked songs" tab).
//!
//! These are definitions only — implementations land in later slices
//! (`SpotifySource` over the existing `Network`, then `infra/local/` and
//! `infra/subsonic/`). All methods speak the domain types from
//! [`crate::core::plugin_api`]; rspotify types never appear here.
//!
//! **Dispatch.** These use native `async fn` in traits (matching the existing
//! `PlaybackNetwork` convention), which is *not* `dyn`-compatible. The planned
//! by-scheme routing therefore dispatches over a **closed enum** of concrete
//! sources (one variant per backend), matching on `scheme()` — not
//! `Box<dyn MediaSource>`. If open/plugin-provided sources are ever needed, add
//! the `async-trait` crate at that point rather than reaching for `dyn` here.

// No implementors in the binary yet; the multi-source slices wire these up.
#![allow(dead_code)]

use crate::core::plugin_api::{AlbumInfo, ArtistInfo, PlaylistInfo, SearchResults, TrackInfo};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// The source the UI is currently scoped to: which catalog the sidebar, search
/// and capability gating reflect.
///
/// This is a **browse-scope** marker only: it never changes playback routing
/// (that stays URI-scheme driven via `route_local_event` + `App::local_playback`),
/// so switching the active source never interrupts what is currently playing.
///
/// The enum is deliberately unconditional: every variant compiles in every
/// build (including the slim `telemetry`-only CI build) so handlers and UI code
/// never need `#[cfg]`. Only the per-source *data loading* is gated behind that
/// source's feature (`local-files`, `subsonic`).
///
/// The serde derives are the action-vocabulary wire shape; the derived unit-
/// variant strings coincide with the `to_config_str()` tokens, while the
/// `state.yml` persistence keeps its own lenient hand-written helpers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Source {
  #[default]
  Qobuz,
}

impl Source {
  /// Every source, in display order. Add new sources here.
  pub const ALL: [Source; 1] = [Source::Qobuz];

  /// The sources the `d` picker offers.
  pub fn picker_sources() -> &'static [Source] {
    &Self::ALL
  }

  /// Stable English name for CLI output, logs and protocol tokens.
  /// Interfaces use the active table's `sidebar.source_label` instead.
  pub fn label(&self) -> &'static str {
    crate::core::i18n::EN.sidebar.source_label(*self)
  }

  /// Config-file token used to persist the active source.
  pub fn to_config_str(self) -> &'static str {
    match self {
      Source::Qobuz => "Qobuz",
    }
  }

  /// Parse the config-file token back to a `Source`. Every token (including
  /// the ones older spotatui configs wrote) maps to the only source, Qobuz.
  pub fn from_config_str(_s: &str) -> Self {
    Source::Qobuz
  }

  /// Whether this source can search its catalog (implements [`Searcher`]).
  pub fn supports_search(&self) -> bool {
    true
  }

  /// Whether this source exposes a saved library (implements [`LibraryProvider`]).
  pub fn supports_library(&self) -> bool {
    false
  }

  /// Whether this source can mutate playlists.
  pub fn supports_playlist_write(&self) -> bool {
    false
  }

  /// Whether this source supports liking / saving the currently-playing track.
  pub fn supports_like(&self) -> bool {
    false
  }
}

/// The required minimum every media source implements: browse playlists and the
/// tracks within them.
pub trait MediaSource {
  /// Human-readable source name shown in the UI (e.g. `"Spotify"`, `"Navidrome"`).
  fn name(&self) -> &str;

  /// URI scheme this source owns, without the colon (e.g. `"spotify"`, `"file"`,
  /// `"subsonic"`). Used to route a URI to the source that can handle it.
  fn scheme(&self) -> &str;

  /// The user's playlists for this source.
  async fn playlists(&self) -> Result<Vec<PlaylistInfo>>;

  /// The tracks of a playlist, identified by its source-native URI.
  async fn tracks(&self, playlist_uri: &str) -> Result<Vec<TrackInfo>>;
}

/// Optional capability: search the source's catalog.
pub trait Searcher {
  async fn search(&self, query: &str) -> Result<SearchResults>;
}

/// Optional capability: the user's saved library (liked tracks, saved albums,
/// followed artists).
pub trait LibraryProvider {
  async fn saved_tracks(&self) -> Result<Vec<TrackInfo>>;
  async fn saved_albums(&self) -> Result<Vec<AlbumInfo>>;
  async fn saved_artists(&self) -> Result<Vec<ArtistInfo>>;
}

/// Optional capability: mutate playlists (add/remove tracks by URI).
pub trait PlaylistWriter {
  async fn add_tracks(&self, playlist_uri: &str, track_uris: &[String]) -> Result<()>;
  async fn remove_tracks(&self, playlist_uri: &str, track_uris: &[String]) -> Result<()>;
}

/// Optional capability: produce a playable audio stream for a URI and route it
/// into the shared rodio sink (so the visualizer and volume control work
/// uniformly across sources).
///
/// The concrete stream/handle return type is defined in the local-files slice
/// (Phase 3), when the symphonia → rodio pipeline is wired. Until then this is
/// the marker seam that lets the dispatch layer ask "can this source stream?".
pub trait Streamer {
  /// Begin streaming the given URI. Returns once playback has started (or
  /// errors if the URI is not streamable by this source).
  async fn stream(&self, uri: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn serde_tokens_match_the_config_tokens() {
    for source in Source::ALL {
      let wire = serde_json::to_string(&source).unwrap();
      assert_eq!(wire, format!("\"{}\"", source.to_config_str()));
      assert_eq!(serde_json::from_str::<Source>(&wire).unwrap(), source);
    }
  }

  #[test]
  fn qobuz_supports_search_only() {
    // `catalog/search` is searchable; favorites and playlists are read through
    // the sidebar, and like/playlist writes are follow-ups.
    assert!(Source::Qobuz.supports_search());
    assert!(!Source::Qobuz.supports_library());
    assert!(!Source::Qobuz.supports_playlist_write());
    assert!(!Source::Qobuz.supports_like());
  }

  #[test]
  fn config_str_round_trips_every_source() {
    for source in Source::ALL {
      assert_eq!(Source::from_config_str(source.to_config_str()), source);
    }
  }
}
