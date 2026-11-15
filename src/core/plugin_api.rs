//! Plugin-facing domain facade.
//!
//! Serde-serializable snapshots with string IDs/URIs only: the shapes Lua plugins
//! see, and the plain structs the rest of the app passes around.

#![allow(dead_code)]

use crate::core::app::App;
use crate::infra::media_metadata::current_playback_snapshot;
use serde::{Deserialize, Serialize};

/// The repeat mode of the playing context: off, one track, or the whole list.
/// Serialized as `off` / `track` / `context`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatState {
  #[default]
  Off,
  Track,
  Context,
}

pub const API_VERSION: u32 = 6;

/// A popup dialog produced by a plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginPopup {
  pub title: String,
  pub lines: Vec<PopupLine>,
}

/// A single line in a [`PluginPopup`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopupLine {
  pub text: String,
  pub fg: Option<crate::core::theme::Color>,
  pub bold: bool,
  pub italic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
  pub uri: Option<String>,
  pub name: String,
  /// Display artist names. Mirrors `artist_refs[*].name`; retained for the
  /// `api_version = 4` scripting contract (plugins read `track.artists`).
  pub artists: Vec<String>,
  /// Display album name. Retained for the scripting contract.
  pub album: String,
  pub duration_ms: u64,
  // --- Fields below were added later; they only ever ADD keys to the
  // serialized snapshot, so older plugins keep working. ---
  /// The source's track id, when known.
  #[serde(default)]
  pub id: Option<String>,
  /// The source's id of the track's album, when known.
  #[serde(default)]
  pub album_id: Option<String>,
  /// Structured, navigable artist references (id + name). Populated when the
  /// source provides per-artist data; empty when only a combined display string
  /// is available (e.g. native-playback snapshots).
  #[serde(default)]
  pub artist_refs: Vec<ArtistRef>,
  #[serde(default = "default_true")]
  pub is_playable: bool,
  #[serde(default)]
  pub is_local: bool,
  #[serde(default)]
  pub track_number: u32,
  #[serde(default)]
  pub explicit: bool,
  /// A directly-fetchable cover-art image URL for this track, when the source
  /// provides one.
  #[serde(default)]
  pub image_url: Option<String>,
  /// Release date of the track's album as the source reports it: `YYYY`,
  /// `YYYY-MM` or `YYYY-MM-DD`. `None` when the source has no date.
  #[serde(default)]
  pub release_date: Option<String>,
  // Identity of a generated title, retained when a queue is saved and restored.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub generated_name: Option<GeneratedTrackName>,
}

/// Generated track titles are translated at display time; real tags stay literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratedTrackName {
  LocalUnknown,
}

impl GeneratedTrackName {
  pub fn display_name<'a>(
    name: &'a str,
    generated: Option<Self>,
    strings: &crate::core::i18n::Strings,
  ) -> &'a str {
    match generated {
      Some(Self::LocalUnknown) => strings.status.infra.local_unknown_track,
      None => name,
    }
  }
}

impl TrackInfo {
  pub fn display_name<'a>(&'a self, strings: &crate::core::i18n::Strings) -> &'a str {
    GeneratedTrackName::display_name(&self.name, self.generated_name, strings)
  }

  pub fn displayed(&self, strings: &crate::core::i18n::Strings) -> Self {
    Self {
      name: self.display_name(strings).to_owned(),
      ..self.clone()
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
  pub track: Option<TrackInfo>,
  pub is_playing: bool,
  pub progress_ms: u64,
  pub shuffle: bool,
  /// One of `"off"`, `"track"`, or `"context"`.
  pub repeat: String,
  pub volume_percent: Option<u8>,
  pub device: Option<DeviceInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
  pub id: Option<String>,
  pub name: String,
  /// Lowercased DeviceType name, e.g. `"computer"`, `"smartphone"`, `"speaker"`.
  pub kind: String,
  pub is_active: bool,
  pub volume_percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistInfo {
  /// In-memory identity for generated labels; frontends receive the rendered name.
  #[serde(skip)]
  pub generated_name: Option<GeneratedPlaylistName>,
  pub uri: String,
  pub name: String,
  /// Owner display name (falls back to the owner id when no display name is set).
  pub owner: String,
  pub track_count: u32,
  // --- Fields added later; see the TrackInfo note above. ---
  /// The source's playlist id, when known.
  #[serde(default)]
  pub id: Option<String>,
  /// Owner's base62 user id, when known. Used for ownership checks (the display
  /// `owner` is not stable for comparison).
  #[serde(default)]
  pub owner_id: Option<String>,
  #[serde(default)]
  pub collaborative: bool,
  #[serde(default)]
  pub public: Option<bool>,
  #[serde(default)]
  pub image_url: Option<String>,
}

/// Synthetic playlist names are rendered from the current table, never matched by text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedPlaylistName {
  LocalRoot,
}

impl PlaylistInfo {
  pub fn display_name<'a>(&'a self, strings: &crate::core::i18n::Strings) -> &'a str {
    match self.generated_name {
      Some(GeneratedPlaylistName::LocalRoot) => strings.status.infra.local_root_folder,
      None => &self.name,
    }
  }
}

/// A navigable reference to an artist: optional id plus display name.
///
/// Reused by [`TrackInfo`], [`AlbumInfo`], and [`ArtistInfo`]. `id` is `None`
/// when the API omits it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArtistRef {
  pub id: Option<String>,
  pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArtistInfo {
  pub id: Option<String>,
  pub uri: Option<String>,
  pub name: String,
  #[serde(default)]
  pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AlbumInfo {
  pub id: Option<String>,
  pub uri: Option<String>,
  pub name: String,
  #[serde(default)]
  pub artists: Vec<ArtistRef>,
  /// One of `"album"`, `"single"`, `"compilation"` (lowercased), when known.
  #[serde(default)]
  pub album_type: Option<String>,
  #[serde(default)]
  pub release_date: Option<String>,
  #[serde(default)]
  pub total_tracks: Option<u32>,
  #[serde(default)]
  pub image_url: Option<String>,
  /// Populated when mapped from a full album; empty for simplified albums.
  #[serde(default)]
  pub tracks: Vec<TrackInfo>,
}

/// An album as it appears in the user's saved-albums library: the album itself
/// plus the saved-relationship metadata (`added_at`). `added_at` is a property
/// of the *save*, not the album, so it lives here rather than on [`AlbumInfo`]
/// (which is also reused for search results and discographies). Carried so the
/// "Date Added" sort on the saved-albums screen keeps working.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SavedAlbumInfo {
  pub album: AlbumInfo,
  /// RFC 3339 UTC timestamp; sorts lexicographically == chronologically.
  #[serde(default)]
  pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShowInfo {
  pub id: Option<String>,
  pub uri: Option<String>,
  pub name: String,
  #[serde(default)]
  pub description: String,
  /// Show publisher.
  #[serde(default)]
  pub publisher: String,
  #[serde(default)]
  pub image_url: Option<String>,
}

/// Where playback of an episode was last left off. Part of the plugin
/// snapshot shape; Qobuz has no episodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePointInfo {
  pub fully_played: bool,
  pub resume_position_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeInfo {
  pub id: Option<String>,
  pub uri: Option<String>,
  pub name: String,
  pub duration_ms: u64,
  /// Parent show name.
  #[serde(default)]
  pub show_name: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub release_date: String,
  #[serde(default = "default_true")]
  pub is_playable: bool,
  /// Resume/played state, when the source provides it.
  #[serde(default)]
  pub resume_point: Option<ResumePointInfo>,
  #[serde(default)]
  pub image_url: Option<String>,
}

/// A playable item in a queue or playlist: a music track, or an episode (kept
/// in the plugin snapshot shape; Qobuz has none).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayableInfo {
  Track(TrackInfo),
  Episode(EpisodeInfo),
}

/// Aggregated, source-agnostic search results. Sources without a given
/// capability simply leave the corresponding vector empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchResults {
  #[serde(default)]
  pub tracks: Vec<TrackInfo>,
  #[serde(default)]
  pub albums: Vec<AlbumInfo>,
  #[serde(default)]
  pub artists: Vec<ArtistInfo>,
  #[serde(default)]
  pub playlists: Vec<PlaylistInfo>,
  #[serde(default)]
  pub shows: Vec<ShowInfo>,
}

/// Retained content of a plugin custom screen. Screens are retained-mode by
/// design: draw runs with `&App` only (the engine is unreachable there), so
/// plugins publish content via effects and the renderer just reads it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PluginScreenContent {
  pub title: String,
  pub widgets: Vec<PluginWidget>,
}

/// A size along one axis: an absolute cell count, or a percentage of the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginLength {
  Cells(u16),
  /// 1..=100 (validated at the API layer).
  Percent(u16),
}

/// How a [`PluginWidgetKind::CoverArt`] image is scaled into its area.
///
/// `Crop` (CSS `cover`) is deliberately not offered: the terminal image
/// renderer documents that cropping overdraws characters over graphics and
/// that some terminals misbehave when it does (Alacritty's sixel branch is
/// named explicitly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PluginCoverArtFit {
  /// Fit inside the area, keeping the aspect ratio, never upscaling.
  #[default]
  Contain,
  /// Like `Contain` but upscales an image smaller than the area.
  Scale,
}

/// A widget in a [`PluginScreenContent`].
///
/// `width`/`height` are per-axis size hints; only the one matching the parent
/// container's axis is used (the top level is an implicit vertical stack).
/// `None` means "share the remaining space evenly".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginWidget {
  pub kind: PluginWidgetKind,
  pub width: Option<PluginLength>,
  pub height: Option<PluginLength>,
}

impl PluginWidget {
  /// A widget with no size hints on either axis.
  pub fn new(kind: PluginWidgetKind) -> Self {
    Self {
      kind,
      width: None,
      height: None,
    }
  }
}

/// The content of a [`PluginWidget`]. `Row`/`Column` are pure layout containers:
/// they draw no border and no title, they only split their area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginWidgetKind {
  Paragraph {
    lines: Vec<PopupLine>,
    /// Whether `PageUp`/`PageDown` scrolling applies to this paragraph.
    /// Defaults to `true`, matching the `api_version = 5` behavior where every
    /// paragraph on a screen scrolled in lockstep.
    scroll: bool,
  },
  List {
    title: Option<String>,
    items: Vec<PopupLine>,
    /// 0-based index of the highlighted item.
    selected: Option<usize>,
  },
  Gauge {
    /// 0.0..=1.0 (clamped at the API layer).
    ratio: f64,
    label: Option<String>,
  },
  /// The current track's cover art, rendered from the app's already-decoded
  /// in-memory image. At most one may appear per screen.
  CoverArt {
    fit: PluginCoverArtFit,
  },
  Row {
    children: Vec<PluginWidget>,
  },
  Column {
    children: Vec<PluginWidget>,
  },
}

/// A queue item as exposed to plugins. [`PlayableInfo`]'s externally-tagged
/// serde shape (`{ Track = {...} }`) is hostile to Lua, so queue items are
/// flattened into an explicit `kind` plus at most one populated payload field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItemSnapshot {
  /// `"track"` (or `"episode"`, which Qobuz never produces).
  pub kind: String,
  #[serde(default)]
  pub track: Option<TrackInfo>,
  #[serde(default)]
  pub episode: Option<EpisodeInfo>,
}

impl QueueItemSnapshot {
  pub fn from_playable(item: &PlayableInfo) -> Self {
    match item {
      PlayableInfo::Track(t) => QueueItemSnapshot {
        kind: "track".to_string(),
        track: Some(t.clone()),
        episode: None,
      },
      PlayableInfo::Episode(e) => QueueItemSnapshot {
        kind: "episode".to_string(),
        track: None,
        episode: Some(e.clone()),
      },
    }
  }
}

/// The playback queue as exposed to plugins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QueueSnapshot {
  #[serde(default)]
  pub currently_playing: Option<QueueItemSnapshot>,
  #[serde(default)]
  pub items: Vec<QueueItemSnapshot>,
}

/// A single timestamped lyrics line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsLineSnapshot {
  pub time_ms: u64,
  pub text: String,
}

/// Lyrics for the current track as exposed to plugins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LyricsSnapshot {
  /// One of `"not_started"`, `"loading"`, `"found"`, `"not_found"`.
  pub status: String,
  #[serde(default)]
  pub lines: Vec<LyricsLineSnapshot>,
}

/// Safe, plugin-visible behavior settings. Credentials and service settings
/// (the Discord client id, the announcement feed) are deliberately excluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BehaviorSnapshot {
  pub seek_milliseconds: u32,
  pub volume_increment: u8,
  pub tick_rate_milliseconds: u64,
  pub animation_tick_rate_milliseconds: u64,
  pub enable_text_emphasis: bool,
  pub show_loading_indicator: bool,
  pub enforce_wide_search_bar: bool,
  pub playing_icon: String,
  pub set_window_title: bool,
  pub shuffle_enabled: bool,
  pub sidebar_width_percent: u8,
  pub playbar_height_rows: u16,
  pub library_height_percent: u8,
  /// Lowercased active source name: `"qobuz"`.
  pub active_source: String,
}

/// User configuration as exposed to plugins: theme colors as config-file
/// strings plus safe behavior scalars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ConfigSnapshot {
  pub theme: std::collections::BTreeMap<String, String>,
  pub behavior: BehaviorSnapshot,
}

/// Build a [`ConfigSnapshot`] from the live user config and runtime state.
/// Theme colors use the same string forms `parse_theme_item` accepts (named or
/// `"r, g, b"`).
pub fn config_snapshot(
  config: &crate::core::user_config::UserConfig,
  runtime_state: &crate::core::state::RuntimeState,
) -> ConfigSnapshot {
  use crate::core::user_config::color_to_string;
  let t = &config.theme;
  let theme: std::collections::BTreeMap<String, String> = [
    ("active", t.active),
    ("analysis_bar", t.analysis_bar),
    ("analysis_bar_text", t.analysis_bar_text),
    ("banner", t.banner),
    ("background", t.background),
    ("error_border", t.error_border),
    ("error_text", t.error_text),
    ("header", t.header),
    ("highlighted_lyrics", t.highlighted_lyrics),
    ("hint", t.hint),
    ("hovered", t.hovered),
    ("inactive", t.inactive),
    ("playbar_background", t.playbar_background),
    ("playbar_progress", t.playbar_progress),
    ("playbar_progress_text", t.playbar_progress_text),
    ("playbar_text", t.playbar_text),
    ("selected", t.selected),
    ("text", t.text),
  ]
  .into_iter()
  .map(|(name, color)| (name.to_string(), color_to_string(color)))
  .collect();

  let b = &config.behavior;
  ConfigSnapshot {
    theme,
    behavior: BehaviorSnapshot {
      seek_milliseconds: b.seek_milliseconds,
      volume_increment: b.volume_increment,
      tick_rate_milliseconds: b.tick_rate_milliseconds,
      animation_tick_rate_milliseconds: b.animation_tick_rate_milliseconds,
      enable_text_emphasis: b.enable_text_emphasis,
      show_loading_indicator: b.show_loading_indicator,
      enforce_wide_search_bar: b.enforce_wide_search_bar,
      playing_icon: b.playing_icon.clone(),
      set_window_title: b.set_window_title,
      shuffle_enabled: runtime_state.shuffle_enabled,
      sidebar_width_percent: runtime_state.sidebar_width_percent,
      playbar_height_rows: runtime_state.playbar_height_rows,
      library_height_percent: runtime_state.library_height_percent,
      active_source: format!("{:?}", runtime_state.active_source).to_lowercase(),
    },
  }
}

/// Default for serde `is_playable` fields: a track is assumed playable unless
/// the API explicitly says otherwise.
fn default_true() -> bool {
  true
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

impl PlaybackState {
  pub fn repeat_from(state: RepeatState) -> String {
    match state {
      RepeatState::Off => "off".to_string(),
      RepeatState::Track => "track".to_string(),
      RepeatState::Context => "context".to_string(),
    }
  }
}

impl DeviceInfo {}

impl PlaylistInfo {}

/// Build a [`PlaybackState`] from the current [`App`] state.
///
/// Returns `None` only when there is no playback context at all (both
/// `current_playback_snapshot` and `app.current_playback_context` are absent).
pub fn playback_state(app: &App) -> Option<PlaybackState> {
  let snapshot = current_playback_snapshot(app);
  // Nothing playing: plugins see `nil`, as before the source reduction.
  snapshot.as_ref()?;

  let track = snapshot.as_ref().map(|s| TrackInfo {
    generated_name: None,
    uri: s.item_uri.clone(),
    name: s.metadata.title.clone(),
    artists: s.metadata.artists.clone(),
    album: s.metadata.album.clone(),
    duration_ms: s.metadata.duration_ms as u64,
    id: s.item_id.clone(),
    album_id: None,
    // The native-playback snapshot carries a single combined artist display
    // string (see `media_metadata`), not structured per-artist data, so there
    // are no navigable refs to populate here.
    artist_refs: Vec::new(),
    is_playable: true,
    is_local: false,
    track_number: 0,
    explicit: false,
    image_url: None,
    release_date: None,
  });

  let (is_playing, shuffle, repeat, device) = if let Some(s) = &snapshot {
    let repeat_str = s
      .repeat
      .map(PlaybackState::repeat_from)
      .unwrap_or_else(|| "off".to_string());
    (s.is_playing, s.shuffle, repeat_str, None::<DeviceInfo>)
  } else {
    (false, false, "off".to_string(), None)
  };

  let volume_percent = device.as_ref().and_then(|d| d.volume_percent);

  let progress_ms = snapshot.as_ref().map(|s| s.progress_ms as u64).unwrap_or(0);

  Some(PlaybackState {
    track,
    is_playing,
    progress_ms,
    shuffle,
    repeat,
    volume_percent,
    device,
  })
}

/// Return a list of available devices from [`App`]'s cached device payload.
pub fn device_list(_app: &App) -> Vec<DeviceInfo> {
  Vec::new()
}

/// Stable plugin-facing name of a route. Exhaustive on purpose (no `_` arm):
/// adding a `RouteId` without naming it here is a compile error, keeping the
/// scripting contract in sync.
pub fn route_name(route: &crate::core::app::Route) -> String {
  use crate::core::app::RouteId;
  match &route.id {
    RouteId::Analysis => "analysis",
    RouteId::LyricsView => "lyrics",
    RouteId::CoverArtView => "cover_art",
    RouteId::MiniPlayer => "miniplayer",
    RouteId::Error => "error",
    RouteId::Home => "home",
    RouteId::Search => "search",
    RouteId::TrackTable => "track_table",
    RouteId::AnnouncementPrompt => "announcement",
    RouteId::RecapPrompt => "recap_prompt",
    RouteId::ExitPrompt => "exit_prompt",
    RouteId::Settings => "settings",
    RouteId::HelpMenu => "help",
    RouteId::Queue => "queue",
    RouteId::Stats => "stats",
    RouteId::Artist => "artist",
    RouteId::PluginScreen(name) => return format!("plugin:{name}"),
  }
  .to_string()
}

// ---------------------------------------------------------------------------
// Snapshot serializers for plugin data reads
// ---------------------------------------------------------------------------

/// The user's playlists (full list, folder structure flattened away).
pub fn playlists_snapshot(app: &App) -> Vec<PlaylistInfo> {
  app.all_playlists().clone()
}

/// Saved ("liked") tracks fetched so far, in library order.
pub fn saved_tracks_snapshot(app: &App) -> Vec<TrackInfo> {
  app
    .library()
    .saved_tracks
    .pages
    .iter()
    .flat_map(|page| page.items.iter().cloned())
    .collect()
}

/// Saved albums fetched so far, in library order.
pub fn saved_albums_snapshot(app: &App) -> Vec<SavedAlbumInfo> {
  app
    .library()
    .saved_albums
    .pages
    .iter()
    .flat_map(|page| page.items.iter().cloned())
    .collect()
}

/// Saved shows fetched so far, in library order.
pub fn saved_shows_snapshot(app: &App) -> Vec<ShowInfo> {
  app
    .library()
    .saved_shows
    .pages
    .iter()
    .flat_map(|page| page.items.iter().cloned())
    .collect()
}

/// Recently played tracks (most recent first, as returned by the API).
pub fn recently_played_snapshot(app: &App) -> Vec<TrackInfo> {
  app
    .recently_played
    .as_ref()
    .map(|page| page.items.clone())
    .unwrap_or_default()
}

/// Qbal's native queue: the queued track playing now, if any, and the rest.
pub fn queue_snapshot(app: &App) -> QueueSnapshot {
  let item =
    |track: &TrackInfo| QueueItemSnapshot::from_playable(&PlayableInfo::Track(track.clone()));
  QueueSnapshot {
    currently_playing: app.queue_now_track().map(item),
    items: app.native_queue.iter().map(item).collect(),
  }
}

pub fn search_results_snapshot(app: &App) -> SearchResults {
  SearchResults {
    tracks: app
      .search_results()
      .tracks
      .as_ref()
      .map(|p| p.items.clone())
      .unwrap_or_default(),
    albums: app
      .search_results()
      .albums
      .as_ref()
      .map(|p| p.items.clone())
      .unwrap_or_default(),
    artists: app
      .search_results()
      .artists
      .as_ref()
      .map(|p| p.items.clone())
      .unwrap_or_default(),
    playlists: app
      .search_results()
      .playlists
      .as_ref()
      .map(|p| p.items.clone())
      .unwrap_or_default(),
    shows: app
      .search_results()
      .shows
      .as_ref()
      .map(|p| p.items.clone())
      .unwrap_or_default(),
  }
}

/// Whether `app.lyrics`/`app.lyrics_status` describe the item playing *right now*.
///
/// The driver's shared track-change detector runs **after** the script engine's
/// tick, so on the tick a track changes the lyrics state still belongs to the
/// previous track. Without this check, `qbal.get_lyrics` called from a
/// `track_change` handler - the obvious place to call it - sees a terminal
/// status, resolves synchronously, and hands the plugin the *previous* track's
/// lyrics; the one-shot callback is then spent by the time the real ones land.
///
/// `desired_lyrics_identity` is the detector's record of which item it last
/// published lyrics state for, keyed on the tracking snapshot, so comparing it
/// against the same snapshot detects exactly that window.
pub fn lyrics_state_is_current(app: &App) -> bool {
  use crate::infra::media_metadata::{tracking_playback_snapshot, PlaybackItemKind};
  match tracking_playback_snapshot(app) {
    // Nothing playing: the detector clears the identity.
    None => app.desired_lyrics_identity.is_none(),
    Some(snapshot) => match snapshot.item_kind {
      PlaybackItemKind::Track => {
        app.desired_lyrics_identity
          == Some((snapshot.metadata.title.clone(), snapshot.primary_artist()))
      }
    },
  }
}

/// Lyrics for the current track, with the fetch status spelled out.
pub fn lyrics_snapshot(app: &App) -> LyricsSnapshot {
  use crate::core::app::LyricsStatus;
  let status = match app.lyrics_status() {
    LyricsStatus::NotStarted => "not_started",
    LyricsStatus::Loading => "loading",
    LyricsStatus::Found => "found",
    LyricsStatus::NotFound => "not_found",
  };
  LyricsSnapshot {
    status: status.to_string(),
    lines: app
      .lyrics()
      .map(|lines| {
        lines
          .iter()
          .map(|(time_ms, text)| LyricsLineSnapshot {
            time_ms: *time_ms as u64,
            text: text.clone(),
          })
          .collect()
      })
      .unwrap_or_default(),
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  // --- repeat_from ---

  #[test]
  fn repeat_off_maps_to_string() {
    assert_eq!(PlaybackState::repeat_from(RepeatState::Off), "off");
  }

  #[test]
  fn repeat_track_maps_to_string() {
    assert_eq!(PlaybackState::repeat_from(RepeatState::Track), "track");
  }

  #[test]
  fn repeat_context_maps_to_string() {
    assert_eq!(PlaybackState::repeat_from(RepeatState::Context), "context");
  }

  // --- playback_state ---

  // --- config_snapshot ---

  // --- PlaylistInfo::from_simplified ---
}
