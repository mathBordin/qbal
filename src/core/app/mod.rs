use crate::core::action::LibraryTarget;
use crate::core::geometry::Viewport;
use crate::core::input::Key;
use crate::core::pagination::Paged;
use crate::core::pane::{Pane, PaneNode};
use crate::core::plugin_api::{PlaylistInfo, SavedAlbumInfo, ShowInfo, TrackInfo};
use crate::core::requirement::{availability, Availability, Capability, Requirement};
use crate::core::sort::{SortContext, SortField, SortState};
use crate::core::source::Source;
use crate::core::state::{PersistedRuntimeState, RuntimeState};
use crate::core::user_config::{
  color_to_string, key_to_config_string, normalize_tick_rate_milliseconds, UserConfig,
};
use crate::infra::history::{RecapPeriod, StatsData, StreakSummary};
use crate::infra::network::IoEvent;
use crate::infra::queue::QueueNowPlaying;
use anyhow::anyhow;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
// Bare `Arc` here is named by the MPRIS manager and the queue-slot accessors below.
use std::sync::Arc;
use std::{
  cmp::{max, min},
  collections::HashSet,
  time::{Duration, Instant},
};
// The test modules under `core/app/` take `SystemTime` through `use super::*`.
use unicode_width::UnicodeWidthStr;

use log::info;

use crate::infra::queue::RepeatMode;

mod album_theme;
mod audio_analysis;
mod construction;
mod help;
mod keep_awake;
mod keybindings;
mod layout;
mod library;
mod localization;
mod lyrics;
mod models;
mod persistence;
mod playback_routing;
mod playlist_pages;
mod playlists;
mod plugins;
mod queue;
mod queue_suspend;
mod recap;
mod route;
mod scrollable_pages;
mod seek;
mod session;
mod settings_apply;
mod settings_schema;
mod shuffle_repeat;
mod status;
mod tick;
mod transport;
mod view;
mod volume;
mod welcome;

#[cfg(test)]
mod test_support;

pub use help::*;
pub use keybindings::*;
pub use library::*;
pub use lyrics::*;
pub use models::*;
pub use plugins::*;
pub use route::*;
pub use scrollable_pages::*;
pub use session::*;
pub use settings_schema::*;
pub use status::*;
pub use view::*;

pub struct App {
  navigation_stack: Vec<Route>,
  pub spectrum_data: Option<crate::infra::audio::SpectrumData>,
  pub audio_capture_active: bool,
  /// Whether the last tick saw a source play.
  playing_last_tick: bool,
  /// The format the sound card runs at, refreshed by the driver; `None` when
  /// no card is playing or the kernel does not say.
  hardware_output: Option<crate::core::stream_format::HardwareFormat>,
  /// Presentation state of the terminal frontend (cursors, scroll offsets, edit
  /// buffers, focus, popups, terminal geometry). See [`ViewState`]: handlers and
  /// draw functions write it freely; nothing outside `tui/` and these `App`
  /// methods should.
  pub view: ViewState,
  pub user_config: UserConfig,
  strings: &'static crate::core::i18n::Strings,
  system_language: crate::core::i18n::Language,
  localize_messages: bool,
  pub runtime_state: RuntimeState,
  pub state_path: Option<PathBuf>,
  /// The live error recorded by [`App::handle_error`], or empty when there is
  /// none. The `RouteId::Error` frame raised alongside it is a presentation
  /// *hint*, not the state: a frontend may draw it full-screen, render this
  /// string as a toast, or ignore it. Dismiss via [`App::clear_api_error`].
  api_error: String,
  /// When the live `api_error` stops being current. Stamped by `handle_error`,
  /// consumed by `App::expire_api_error` on the tick. `None` means none is live.
  api_error_expires_at: Option<Instant>,
  /// The native playback queue (FIFO).
  pub native_queue: Vec<TrackInfo>,
  /// How to resume the underlying per-source context after the native queue
  /// drains. Populated when a track is queued over an active context.
  pub queue_suspended: Option<crate::core::queue::SuspendedContext>,
  /// What the native queue's playback slot is currently playing, if anything.
  /// Overlays `qobuz_playback` without mutating it (that is the context to
  /// resume); reads go through [`Self::queue_owns_playback`].
  pub queue_now: Option<crate::infra::queue::QueueNowPlaying>,
  /// Whether the queue slot should be playing. Set on a user skip, flipped by
  /// the slot's pause/resume transport arms, cleared by a device removal; read
  /// by the paths that stage the next queued item and by the suspended
  /// context's resume. It survives a player teardown, so a recovery replay of
  /// the slot can honor a user's pause.
  pub queue_slot_desired_playing: bool,
  /// Decoded cover art for the current track plus its load status. The TUI
  /// renderer (`tui::cover_art`) caches its terminal protocols on this
  /// store's key.
  #[cfg(feature = "art-decode")]
  pub cover_art: crate::core::art::CoverArtStore,
  /// Image key currently desired by the UI. Detached decode results for older
  /// keys are discarded.
  #[cfg(feature = "art-decode")]
  pub desired_cover_art_key: Option<String>,
  /// Accent colors extracted from the current cover art. Stored even while
  /// Adaptive Theme is off, so toggling it on recolors immediately without a
  /// refetch. Cleared together with the art itself.
  #[cfg(feature = "art-decode")]
  cover_art_palette: Option<crate::core::cover_theme::AlbumPalette>,
  /// Whether an album-derived theme is applied, and the user theme to restore.
  #[cfg(feature = "art-decode")]
  cover_theme_state: crate::core::cover_theme::CoverThemeState,
  /// In-flight fade of the live theme, advanced by `update_on_tick`.
  #[cfg(feature = "art-decode")]
  theme_transition: Option<crate::core::cover_theme::ThemeTransition>,
  library: Library,
  pub active_playlist_track_filter: Option<String>,
  pub pending_playlist_track_search: Option<String>,
  /// The Recently Played page. Its cursor is `view.recently_played_index`.
  pub recently_played:
    Option<crate::core::pagination::CursorPaged<crate::core::plugin_api::TrackInfo>>,
  /// Moves on every write that opens other rows in the track table.
  track_table_epoch: u64,
  search_results: SearchResult,
  pub song_progress_ms: u128,
  /// When `update_on_tick` last ran. Every frontend must drive the tick (see
  /// `core::driver`); `playback_position_ms` reports stale past 2s so one
  /// that stops ticking shows a loud error instead of a frozen playbar.
  last_tick_at: Instant,
  pub seek_ms: Option<u128>,
  /// Last time a decoded-source seek was dispatched (for throttling drags)
  last_source_seek: Option<Instant>,
  /// Pending decoded-source seek position (throttled; drags coalesce to the last target)
  pending_source_seek: Option<u32>,
  pub track_table: TrackTable,
  /// The Qobuz sidebar rows (favorites, playlists, albums) shown by the Qobuz
  /// browser. Populated by `GetQobuzPlaylists` dispatch.
  qobuz_playlists: Vec<PlaylistInfo>,
  /// The source the UI is currently scoped to (sidebar, search, capability
  /// gating). Browse-scope only — never changes playback routing.
  pub active_source: Source,
  pub is_loading: bool,
  io_tx: Option<Sender<IoEvent>>,

  pub active_announcement: Option<Announcement>,
  pending_announcements: Vec<Announcement>,
  /// Every announcement of the last fetched feed inside its time window, seen or not.
  announcement_feed: Vec<Announcement>,
  /// How many announcements of the batch on screen were dismissed.
  announcement_batch_dismissed: usize,
  lyrics: Option<Vec<(u128, String)>>,
  lyrics_status: LyricsStatus,
  /// Title/artist pair whose lyrics response is currently desired. Detached
  /// service responses must match this before mutating visible state.
  pub desired_lyrics_identity: Option<(String, String)>,
  /// Whether the current `lyrics` carry real LRC timestamps rather than
  /// synthesized evenly-spaced ones derived from plain lyrics.
  lyrics_synced: bool,
  // Settings screen state
  pub settings_items: Vec<SettingItem>,
  settings_saved_items: Vec<SettingItem>,
  /// Prevent idle/sleep during playback
  keepawake: keep_awake::KeepAwake,
  /// The liked marks the Search revision last counted.
  /// The decoded-source list whose rows landed in the track table.
  source_table_uri: Option<String>,
  /// Period shown on the Stats screen
  pub stats_period: RecapPeriod,
  /// Whether we're currently loading stats data
  stats_loading: bool,
  /// Aggregated listening stats for the Stats screen
  pub stats_data: Option<StatsData>,
  /// Cached listening streak summary (Home strip + Stats screen)
  pub listening_streaks: Option<StreakSummary>,
  /// The plays finished since this process started, oldest first.
  session_plays: Vec<SessionPlay>,
  /// Pending monthly recap popup (path + listen count)
  recap_prompt: Option<RecapPromptState>,
  /// Current sort state per context
  pub playlist_sort: SortState,
  /// The track table rows in the order they arrived, before `playlist_sort`.
  track_table_unsorted: Vec<TrackInfo>,
  /// Ephemeral status message shown in the playbar. Read through
  /// [`App::status_message`]; every writer is a setter in `status.rs`.
  status_message: Option<String>,
  /// When to clear the status message
  status_message_expires_at: Option<Instant>,
  /// True when the current status message is an error (blocks normal message overwrites)
  status_message_is_error: bool,
  /// Full flat list of all user playlists (all pages combined)
  all_playlists: Vec<PlaylistInfo>,
  /// Deadline for a debounced state save scheduled by auto-repeating runtime
  /// state changes such as volume and shuffle.
  state_save_due: Option<Instant>,
  /// Runtime-state patch waiting for the next debounced save.
  pending_state_save_patch: PersistedRuntimeState,
  /// Whether the current run of state-save failures has already been reported.
  /// A failed flush re-arms its own deadline, so the tick retries twice a
  /// second; without this latch the report would re-fire at that rate and pin
  /// the status bar into error mode, where `set_status_message` refuses to
  /// overwrite it and every ordinary message is dropped for the rest of the run.
  state_save_error_reported: bool,
  /// Player-global repeat mode for the Qobuz session. Consulted by the
  /// auto-advance / skip logic and rendered in the playbar. The native queue
  /// ignores it.
  pub decoded_repeat: RepeatMode,
  /// Player-global shuffle flag for the Qobuz session. When set, the playlist's
  /// track list is reordered in place (see `ShuffleBackup`), keeping the
  /// current track playing.
  pub decoded_shuffle: bool,
  /// The active Qobuz playback session, or `None` when another backend owns
  /// playback. Same decoupling contract as [`local_playback`](Self::local_playback).
  pub qobuz_playback: Option<crate::infra::qobuz::QobuzPlaybackState>,
  /// The source that claimed the audio sink: set when its start path begins,
  /// kept when the session dies with nothing to replace it (a failed start, a
  /// lost output device), cleared when the queue runs out. Covers the window
  /// in which `qobuz_playback` is `None` for a playlist the user asked for.
  decoded_sink_claim: Option<Source>,
  /// Reference to MPRIS manager for emitting Seeked signals after native seeks
  #[cfg(all(feature = "mpris", target_os = "linux"))]
  pub mpris_manager: Option<Arc<crate::infra::mpris::MprisManager>>,

  /// Commands queued by keybindings for the scripting engine to run.
  pub pending_plugin_commands: Vec<String>,
  /// Per-domain write counters driving async plugin data reads (see
  /// [`PluginDataKind`]). Ungated: the network layer bumps them in every build;
  /// only the scripting engine reads them.
  pub plugin_data_generations: PluginDataGenerations,
  /// Retained content of plugin-registered custom screens, keyed by screen
  /// name. Written by script effects; read by the draw loop.
  pub plugin_screens:
    std::collections::BTreeMap<String, crate::core::plugin_api::PluginScreenContent>,
  /// Keys pressed while a plugin screen was focused: `(screen_name, key_string)`,
  /// drained by the script engine after each key event.
  pub pending_plugin_screen_keys: Vec<(String, String)>,
  /// Per-plugin playbar segments, keyed by plugin name (BTreeMap for deterministic order).
  pub plugin_playbar_segments: std::collections::BTreeMap<String, String>,
  /// Currently displayed plugin popup, if any.
  pub plugin_popup: Option<crate::core::plugin_api::PluginPopup>,
  /// Where this run's log file is being written, resolved once here so draw
  /// code can show it without doing the environment lookup every frame.
  pub log_path: String,
}

impl App {
  /// Clone the IoEvent sender so a spawned task (e.g. the Qobuz login callback
  /// server) can dispatch events back into the pump without holding the `App`
  /// lock. `None` before the sender is wired up or after teardown.
  pub fn io_tx_clone(&self) -> Option<Sender<IoEvent>> {
    self.io_tx.clone()
  }

  pub fn dispatch(&mut self, action: IoEvent) {
    // `is_loading` will be set to false again after the async action has finished in network.rs
    self.is_loading = true;
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action) {
        self.is_loading = false;
        log::error!("Error from dispatch {}", e);
        // TODO: handle error
      };
    }
  }

  // Close the IO channel to allow the network thread to exit gracefully
  pub fn close_io_channel(&mut self) {
    self.io_tx = None;
  }
}
