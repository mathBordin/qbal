//! `App::apply`: the single write path from an [`Action`] into `App`.
//!
//! Ported from the scripting engine's effect drain, which this replaces.
//! Every arm routes through the same `App` method as the equivalent
//! keybinding, so native-streaming fast paths and throttling/coalescing are
//! automatically honoured.

use crate::core::app::{ActiveBlock, App, RouteId};
use crate::core::plugin_api;
use crate::infra::network::IoEvent;

use super::{Action, ActionOutcome, NavTarget, RepeatSetting};
use crate::core::plugin_api::RepeatState;

impl App {
  /// Apply one frontend-neutral action. See the module doc for the rules
  /// every arm follows.
  pub fn apply(&mut self, action: Action) -> ActionOutcome {
    let outcome = self.apply_action(action);
    self.note_display_changes();
    outcome
  }

  fn apply_action(&mut self, action: Action) -> ActionOutcome {
    match action {
      Action::Play => {
        if !effective_is_playing(self) {
          self.toggle_playback();
        }
      }
      Action::Pause => {
        if effective_is_playing(self) {
          self.toggle_playback();
        }
      }
      Action::TogglePlayback => self.toggle_playback(),
      Action::NextTrack => self.next_track(),
      Action::PreviousTrack => self.previous_track(),
      Action::ForcePreviousTrack => self.force_previous_track(),
      Action::SeekTo(ms) => self.seek_to(ms),
      Action::SeekForward => self.seek_forwards(),
      Action::SeekBackward => self.seek_backwards(),
      Action::SetVolume(v) => self.set_volume_percent(v),
      Action::VolumeUp => self.increase_volume(),
      Action::VolumeDown => self.decrease_volume(),
      Action::SetShuffle(desired) => {
        let current = plugin_api::playback_state(self)
          .map(|p| p.shuffle)
          .unwrap_or(false);
        if current != desired {
          self.shuffle();
        }
      }
      Action::ToggleShuffle => self.shuffle(),
      Action::CycleRepeat => self.repeat(),
      Action::SetRepeat(setting) => {
        let _state = match setting {
          RepeatSetting::Off => RepeatState::Off,
          RepeatSetting::Track => RepeatState::Track,
          RepeatSetting::Context => RepeatState::Context,
        };
        self.report_nothing_playing();
      }
      Action::PlayUris { uris, offset } => self.start_playback_uris(uris, offset),
      Action::PlayContext { uri, offset } => self.start_playback_context(uri, offset),
      Action::PlayTrackInContext { context, track } => {
        self.start_playback_track_in_context(context, track);
      }
      Action::QueueTrack(track) => self.add_track_to_native_queue(track),
      Action::PlayQueueItem { uri, position } => self.play_queue_item(&uri, position),
      Action::RemoveFromQueue { uri, position } => {
        self.remove_queue_item(&uri, position);
      }
      Action::MoveQueueItem { uri, from, to } => {
        self.move_queue_item(&uri, from, to);
      }
      Action::ClearQueue => self.clear_native_queue(),
      Action::SearchActiveSource(query) => match self.active_source {
        crate::core::source::Source::Qobuz => {
          self.dispatch(IoEvent::GetQobuzSearchResults(query));
        }
      },
      Action::Notify(msg, ttl) => self.set_status_message(msg, ttl),
      Action::NotifyError(msg, ttl) => self.set_error_status_message(msg, ttl),
      Action::Navigate(target) => apply_navigate(self, target),
      Action::Back => {
        self.pop_navigation_stack();
      }
      Action::LoadMore(target) => match target {
        super::ListTarget::PlaylistTracks => self.get_playlist_tracks_next(),
        super::ListTarget::SavedTracks => self.get_current_user_saved_tracks_next(),
        super::ListTarget::SavedAlbums => self.get_current_user_saved_albums_next(),
        super::ListTarget::SavedShows => self.get_current_user_saved_shows_next(),
        super::ListTarget::SavedArtists => self.get_current_user_saved_artists_next(),
        super::ListTarget::ShowEpisodes => self.get_episode_table_next(),
      },
      Action::Sort { context, field } => self.apply_sort_field(context, field),
      Action::ToggleSortOrder(context) => self.toggle_sort_order(context),
      Action::Open(target) => match target {
        super::OpenTarget::SourcePlaylist(uri) => self.open_source_playlist_tracks(uri),
      },
      Action::OpenLibrary(target) => self.open_library_section(target),
      Action::SelectSource(source) => {
        self.set_active_source(source);
        self.load_source_sidebar(source);
      }
      Action::GenerateRecap => self.generate_recap(),
      Action::GenerateStatsRecap => self.generate_stats_recap(),
      Action::CycleStatsPeriod { forward } => self.cycle_stats_period(forward),
      Action::SetPlaybarSegment { plugin, text } => match text {
        Some(t) => {
          self.plugin_playbar_segments.insert(plugin, t);
        }
        None => {
          self.plugin_playbar_segments.remove(&plugin);
        }
      },
      Action::ShowPopup(popup) => self.show_plugin_popup(popup),
      Action::ClosePopup => self.close_plugin_popup(),
      Action::SetTheme(pairs) => self.set_theme_colors(pairs),
      Action::SetSetting { id, value } => {
        return ActionOutcome::SettingsSaved {
          saved: self.set_setting(&id, value),
        };
      }
      Action::SaveSettings => {
        return ActionOutcome::SettingsSaved {
          saved: self.save_settings_from_items(),
        };
      }
      Action::PressSettingButton { id } => self.press_setting_button(&id),
      Action::CycleVisualizerStyle => self.cycle_visualizer_style(),
      Action::OpenRecap => self.open_recap(),
      Action::DismissRecapPrompt => self.dismiss_recap_prompt(),
      Action::DisableRecapPrompt => self.disable_recap_prompt(),
      Action::DismissAnnouncement { id } => self.dismiss_announcement(&id),
      Action::OpenAnnouncementUrl { id } => self.open_announcement_url(&id),
      Action::OpenChangelogLink { url } => self.open_changelog_link(&url),
      Action::SetScreenContent { name, content } => {
        self.plugin_screens.insert(name, content);
      }
      Action::ShowScreen(name) => self.open_plugin_screen(name),
      Action::CloseScreen(name) => {
        if self.get_current_route().id == RouteId::PluginScreen(name) {
          self.pop_navigation_stack();
        }
      }
    }
    ActionOutcome::Applied
  }
}

/// Returns `true` when the current playback state indicates active playback.
fn effective_is_playing(app: &App) -> bool {
  plugin_api::playback_state(app)
    .map(|p| p.is_playing)
    .unwrap_or(false)
}

/// Replicate the matching keybinding for each nav target.
fn apply_navigate(app: &mut App, target: NavTarget) {
  match target {
    NavTarget::Home => app.push_navigation_stack(RouteId::Home, ActiveBlock::Empty),
    NavTarget::Queue => {
      app.push_navigation_stack(RouteId::Queue, ActiveBlock::Queue);
    }
    NavTarget::Settings => app.open_settings_screen(),
    // The help KEY also clears the help filter first (`help_menu::open`); that
    // reset needs the TUI's filtered-docs count, so it cannot live here.
    NavTarget::Help => app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::HelpMenu),
    NavTarget::Lyrics => app.push_navigation_stack(RouteId::LyricsView, ActiveBlock::LyricsView),
    NavTarget::Analysis => app.get_audio_analysis(),
    NavTarget::MiniPlayer => {
      if app.get_current_route().id == RouteId::MiniPlayer {
        app.pop_navigation_stack();
      } else {
        app.push_navigation_stack(RouteId::MiniPlayer, ActiveBlock::MiniPlayer);
      }
    }
  }
}
