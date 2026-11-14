//! The shared action vocabulary: one enum of frontend-neutral state changes,
//! applied by [`App::apply`](crate::core::app::App).
//!
//! Every producer that is not the network layer's own result handling routes
//! its `App` mutations through here: the Lua scripting engine drains its
//! queued actions into `apply`, the mutating DJ/MCP tools call `apply`
//! directly, and the terminal handlers adopt it screen by screen as the
//! conversion sub-PRs land. Each arm delegates to the same ownership-aware
//! `App` method the equivalent keybinding uses, so the
//! `queue_owns_playback()` / `active_decoded_source()` /
//! native-streaming predicate order is evaluated in one place and the
//! native fast paths and throttling/coalescing are honoured automatically.
//!
//! Design rules (enforced by review plus the gates in `src/gates.rs`):
//!
//! - No rspotify type appears in [`Action`]; payloads are strings, scalars,
//!   and the `core::plugin_api` snapshot types.
//! - No raw `IoEvent` payload either: a producer that can smuggle an
//!   arbitrary event is outside the vocabulary. Arms may dispatch events,
//!   but the variant set is the contract.
//! - Playback starts go through `App::start_playback_uris` /
//!   `App::start_playback_context`; no arm builds a `StartPlayback` event
//!   by hand.
//! - Matches in this module are exhaustive: the deny below refuses a
//!   catch-all arm, and the `wildcard_arms_in_action_tree` gate keeps the
//!   count at zero, tests included.
//! - Address by identity (URIs, ids, names), never by list ordinal.
//!
//! The serde derives are the wire shape for future frontend codegen; they
//! are deliberately in place before any second frontend consumes them.

// The compiler half of the catch-all gate: a new variant must be placed.
#![deny(
  clippy::wildcard_enum_match_arm,
  clippy::match_wildcard_for_single_variants
)]

use serde::{Deserialize, Serialize};

use crate::core::plugin_api::{PluginPopup, PluginScreenContent, TrackInfo};
use crate::core::sort::{SortContext, SortField};
use crate::core::source::Source;
use crate::core::theme::{Color, ThemeField};

mod apply;
#[cfg(test)]
mod tests;

/// A frontend-neutral state change, applied by `App::apply`.
///
/// Variant payloads are fully resolved except where a value only `App` knows
/// is needed ([`Action::Search`] resolves the user country and
/// [`Action::UnfollowPlaylist`] the user id at apply time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
  /// Start playback if it is not already playing (intent, not a toggle).
  Play,
  /// Pause playback if it is playing (intent, not a toggle).
  Pause,
  /// Toggle play/pause, matching the toggle-playback key (and MPRIS
  /// PlayPause) exactly.
  TogglePlayback,
  NextTrack,
  PreviousTrack,
  /// Always go to the previous track (or restart the current queue slot),
  /// skipping the ">= 3s restarts the current track" rule of
  /// [`Action::PreviousTrack`].
  ForcePreviousTrack,
  /// Seek to an absolute position in the current track, in milliseconds.
  SeekTo(u32),
  /// Seek forwards by the user's configured seek step.
  SeekForward,
  /// Seek backwards by the user's configured seek step.
  SeekBackward,
  /// Set the volume to an absolute percentage (0-100).
  SetVolume(u8),
  /// Raise the volume by the user's configured increment.
  VolumeUp,
  /// Lower the volume by the user's configured increment.
  VolumeDown,
  /// Ensure shuffle matches the given state (intent, not a toggle).
  SetShuffle(bool),
  /// Toggle shuffle, matching the shuffle key exactly (source-gated: decoded
  /// queues reorder in place, radio and the native queue slot no-op).
  ToggleShuffle,
  /// Cycle repeat off, then context, then track, matching the repeat key.
  CycleRepeat,
  /// Set repeat to an absolute mode. Mirrors the historic Lua `set_repeat`:
  /// this is the Web API path only; an ownership-aware absolute setter does
  /// not exist yet.
  SetRepeat(RepeatSetting),
  /// Play an explicit list of playable URIs, optionally from an offset into
  /// that list.
  PlayUris {
    uris: Vec<String>,
    offset: Option<usize>,
  },
  /// Play a Spotify context URI (album/playlist/artist/show), optionally
  /// from a 0-based track offset.
  PlayContext {
    uri: String,
    offset: Option<usize>,
  },
  /// Play one track URI as the first track inside a Spotify context URI:
  /// the playlist Enter-start that keeps the selected track first even with
  /// shuffle on (the network layer deliberately does not trim the uri list
  /// when a context is present).
  PlayTrackInContext {
    context: String,
    track: String,
  },
  /// Add one track to the native cross-source queue (the queue key). Distinct
  /// from [`Action::AddToQueue`], the Spotify Web API queue.
  QueueTrack(TrackInfo),
  /// Jump to one native-queue item, addressed by URI with the 0-based position
  /// as the tie-breaker for duplicates.
  PlayQueueItem {
    uri: String,
    position: usize,
  },
  /// Addressed like [`Action::PlayQueueItem`].
  RemoveFromQueue {
    uri: String,
    position: usize,
  },
  /// Move a native-queue item to an absolute index; addressed like
  /// [`Action::PlayQueueItem`], out of range is a no-op.
  MoveQueueItem {
    uri: String,
    from: usize,
    to: usize,
  },
  /// Empty the native queue; the track playing now keeps playing.
  ClearQueue,
  /// Run a search against the active browse source's catalog; falls back to
  /// the Spotify catalog when the active source has no own search (the
  /// Local-files scope behaves this way too).
  SearchActiveSource(String),
  /// message, ttl_secs
  Notify(String, u64),
  /// Error message, ttl_secs. Always shown; blocks normal message
  /// overwrites until it expires.
  NotifyError(String, u64),
  /// Navigate to a top-level surface; apply mirrors the matching keybinding
  /// exactly.
  Navigate(NavTarget),
  /// Pop the navigation stack (same as the back key).
  Back,
  /// Fetch the next page of a paginated list surface - the shared "hit the
  /// end of the list" consequence that GUI infinite scroll also fires.
  /// Self-guarding: a no-op when no next page exists. Page-flipped surfaces
  /// (saved albums, saved shows, show episodes) move the visible page when it
  /// is cached.
  LoadMore(ListTarget),
  /// Sort a list surface by a field, like the sort menu: the field already
  /// in effect flips the direction instead.
  Sort {
    context: SortContext,
    field: SortField,
  },
  /// Flip the recorded sort direction without re-sorting loaded rows.
  ToggleSortOrder(SortContext),
  /// Open a resource page (album/artist/show/playlist) by id. Never starts
  /// playback; Open and Play stay disjoint.
  Open(OpenTarget),
  /// Open a library sidebar section and fetch its data, mirroring the
  /// library row's Enter consequence exactly.
  OpenLibrary(LibraryTarget),
  /// Switch the active browse source (and persist the choice), then fetch
  /// the sidebar data that source needs. Browse scope only: never
  /// interrupts playback.
  SelectSource(Source),
  /// Generate a listening recap. The period is resolved at apply time: the
  /// selected period on the Stats screen when that screen is current, 30
  /// days anywhere else.
  GenerateRecap,
  /// Generate a listening recap for the selected Stats period, whatever the route.
  GenerateStatsRecap,
  /// Step the Stats period through its ring and reload; relative because the
  /// period type is not part of the wire shape.
  CycleStatsPeriod {
    forward: bool,
  },
  /// Set or clear a playbar segment for a plugin (keyed by plugin name).
  SetPlaybarSegment {
    plugin: String,
    text: Option<String>,
  },
  /// Show a plugin popup dialog.
  ShowPopup(PluginPopup),
  /// Dismiss the plugin popup, if one is shown.
  ClosePopup,
  /// Apply theme color overrides at runtime.
  SetTheme(Vec<(ThemeField, Color)>),
  /// Commit the Settings screen's staged rows and write `config.yml`; the
  /// outcome says whether the write succeeded.
  SaveSettings,
  /// Set one settings row and write `config.yml`; the outcome says whether
  /// the write succeeded. An unknown id or a value of the wrong kind changes nothing.
  SetSetting {
    id: String,
    value: SettingInput,
  },
  /// Press the button of one settings row (Soloist Path: download from
  /// Spotify). A row without a button changes nothing.
  PressSettingButton {
    id: String,
  },
  /// Cycle the visualizer style and persist it.
  CycleVisualizerStyle,
  /// Open the monthly recap in the browser and close its popup.
  OpenRecap,
  /// Close the recap popup without opening the recap.
  DismissRecapPrompt,
  /// Close the recap popup and switch the monthly prompt off in `config.yml`.
  DisableRecapPrompt,
  /// Dismiss the active announcement if it has this id, persist it as seen,
  /// and show the next pending one.
  DismissAnnouncement {
    id: String,
  },
  /// Open the link of the active or a fetched announcement with this id in the
  /// system browser; the URL comes from the feed, never the producer.
  OpenAnnouncementUrl {
    id: String,
  },
  /// Open a Welcome page link in the system browser: the Discord invite or a
  /// link target in the embedded changelog, any other URL is ignored.
  OpenChangelogLink {
    url: String,
  },
  /// Publish (retained) content for a registered plugin screen.
  SetScreenContent {
    name: String,
    content: PluginScreenContent,
  },
  /// Navigate to a registered plugin screen.
  ShowScreen(String),
  /// Pop the named plugin screen if it is the current route.
  CloseScreen(String),
}

/// What applying an [`Action`] produced, beyond the state change itself.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionOutcome {
  /// The action was applied (or routed); nothing further to report.
  Applied,
  /// [`Action::SaveSettings`]: whether `config.yml` was written.
  SettingsSaved { saved: bool },
}

/// A settings row's new value from a frontend; a choice row takes `Text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingInput {
  Bool(bool),
  Number(i64),
  Text(String),
}

/// An absolute repeat mode, mirroring Spotify's three states without the
/// rspotify type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatSetting {
  Off,
  Track,
  Context,
}

/// Surfaces reachable through [`Action::Navigate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavTarget {
  Home,
  Queue,
  Settings,
  Help,
  Lyrics,
  Analysis,
  MiniPlayer,
}

impl NavTarget {
  /// Every target, in the order the Lua API has always advertised them.
  pub const ALL: [NavTarget; 7] = [
    NavTarget::Home,
    NavTarget::Queue,
    NavTarget::Settings,
    NavTarget::Help,
    NavTarget::Lyrics,
    NavTarget::Analysis,
    NavTarget::MiniPlayer,
  ];

  /// The name used by `spotatui.navigate(name)`.
  pub fn name(self) -> &'static str {
    match self {
      NavTarget::Home => "home",
      NavTarget::Queue => "queue",
      NavTarget::Settings => "settings",
      NavTarget::Help => "help",
      NavTarget::Lyrics => "lyrics",
      NavTarget::Analysis => "analysis",
      NavTarget::MiniPlayer => "miniplayer",
    }
  }

  /// Look up a target by name; `None` for unknown names.
  pub fn from_name(name: &str) -> Option<NavTarget> {
    NavTarget::ALL.into_iter().find(|t| t.name() == name)
  }
}

/// A paginated list surface [`Action::LoadMore`] can advance. Grows
/// additively as later screens adopt continuous pagination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListTarget {
  /// The open playlist's track table (MyPlaylists / PlaylistSearch).
  PlaylistTracks,
  /// The liked-songs (saved tracks) table.
  SavedTracks,
  /// The saved-albums list (page-flipped, not continuous).
  SavedAlbums,
  /// The saved-podcasts list (page-flipped, not continuous).
  SavedShows,
  /// The followed-artists list (cursor-paged, continuous).
  SavedArtists,
  /// The open show's episode list; the show is resolved at apply time.
  ShowEpisodes,
}

/// A resource deep-link [`Action::Open`] can address, by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenTarget {
  /// A decoded source's playlist or folder, routed by URI scheme (`file:`,
  /// `subsonic:`, `youtube:playlist:`, `qobuz:`); an unknown scheme is a no-op.
  SourcePlaylist(String),
}

/// Library sidebar sections reachable through [`Action::OpenLibrary`].
/// Deliberately NOT advertised through the Lua `navigate()` API (unlike
/// [`NavTarget`]): adding rows there would expand the published plugin
/// surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LibraryTarget {
  #[default]
  Stats,
}

impl LibraryTarget {
  /// The sidebar label in the selected language.
  pub fn localized_name(self, strings: &crate::core::i18n::Strings) -> &'static str {
    let sidebar = &strings.sidebar;
    match self {
      LibraryTarget::Stats => sidebar.stats,
    }
  }
}
