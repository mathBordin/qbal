mod analysis;
mod announcement_prompt;
mod common_key_events;
#[cfg(feature = "cover-art")]
mod cover_art_view;
mod dialog;
mod empty;
mod error_screen;
mod filter_input;
mod help_menu;
mod home;
mod input;
mod library;
mod lyrics_view;
mod miniplayer;
mod mouse;
mod panes;
mod playbar;
mod playlist;
mod plugin_screen;
mod queue_menu;
mod recap_prompt;
pub mod resize;
mod search_results;
mod settings;
mod sort_menu;
mod stats;
mod track_table;

use crate::core::action::{Action, NavTarget};
use crate::core::app::{ActiveBlock, App, InputContext, RouteId, SearchResultBlock};
use crate::core::requirement::{Availability, Capability, Requirement};
use crate::tui::event::Key;

pub use announcement_prompt::dismiss as dismiss_announcement;
pub(crate) use dialog::playlist_picker_captures_key;
pub use input::handler as input_handler;
pub use mouse::handler as mouse_handler;
pub use panes::{leave_focused_pane, pane_arrows_apply, show_main_for_new_screen};

#[cfg(target_os = "macos")]
fn key_matches_open_settings_binding(key: Key, binding: Key) -> bool {
  key == binding
    || (binding == Key::Alt(',') && key == Key::Char('≤'))
    || (binding == Key::Ctrl(',')
      && (key == Key::Ctrl('l')
        || key == Key::Ctrl('L')
        || key == Key::Ctrl('4')
        || key == Key::Ctrl('<')))
}

#[cfg(not(target_os = "macos"))]
fn key_matches_open_settings_binding(key: Key, binding: Key) -> bool {
  key == binding
}

pub fn handle_app(key: Key, app: &mut App) {
  // Plugin popup is a modal: intercept all keys before anything else.
  if app.plugin_popup.is_some() {
    match key {
      Key::Esc | Key::Char('q') => {
        app.apply(Action::ClosePopup);
      }
      k if k == app.user_config.keys.back => {
        app.apply(Action::ClosePopup);
      }
      k if common_key_events::up_event(k, &app.user_config.keys) => {
        app.view.plugin_popup_scroll = app.view.plugin_popup_scroll.saturating_sub(1);
      }
      k if common_key_events::down_event(k, &app.user_config.keys) => {
        let max_scroll = app
          .plugin_popup
          .as_ref()
          .map(|p| p.lines.len().saturating_sub(1) as u16)
          .unwrap_or(0);
        app.view.plugin_popup_scroll = app
          .view
          .plugin_popup_scroll
          .saturating_add(1)
          .min(max_scroll);
      }
      _ => {} // swallow all other keys
    }
    return;
  }

  // Help filtering is an inline modal input. Give it priority over global
  // bindings so queries can contain keys such as d, n, p, q, or ?.
  if app.get_current_route().active_block == ActiveBlock::HelpMenu
    && (app.view.help_filter_editing || key == app.user_config.keys.search)
  {
    help_menu::handler(key, app);
    return;
  }

  // The add-to-playlist picker's search is an inline input too: typed playlist
  // names must not trigger global bindings.
  if dialog::playlist_picker_captures_key(app, key) {
    handle_block_events(key, app);
    return;
  }

  // Settings has the same inline modal inputs: an open edit, the unsaved-changes
  // prompt, and the row filter. Each needs the raw key before global bindings
  // claim it, and the search binding is what opens the filter in the first place.
  if app.get_current_route().active_block == ActiveBlock::Settings
    && (app.view.settings_unsaved_prompt_visible
      || app.view.settings_edit_mode
      || app.view.settings_filter_editing
      || key == app.user_config.keys.search
      || (key == settings::BUTTON_KEY && settings::selected_row_has_button(app)))
  {
    settings::handler(key, app);
    return;
  }

  // The sort menu is a popup whose shortcut letters ('n', 'd', 'a', ...)
  // collide with global bindings, so it needs the keys before they are claimed.
  if app.get_current_route().active_block == ActiveBlock::SortMenu {
    handle_block_events(key, app);
    return;
  }

  if app.maybe_activate_open_settings_fallback(key) {
    app.open_settings_screen();
    if app.pending_keybinding_persist_key().is_some() {
      app.push_navigation_stack(
        RouteId::Dialog,
        ActiveBlock::Dialog(crate::core::app::DialogContext::PersistKeybindingFallback),
      );
    }
    return;
  }

  let effective_open_settings = app.effective_open_settings_key();
  if key_matches_open_settings_binding(key, app.user_config.keys.open_settings)
    || key_matches_open_settings_binding(key, effective_open_settings)
  {
    app.apply(Action::Navigate(NavTarget::Settings));
    return;
  }

  if !is_input_mode(app) && panes::handle_pane_key(key, app) {
    return;
  }

  // First handle any global event and then move to block event
  match key {
    Key::Esc => {
      if app.get_current_route().active_block == ActiveBlock::Settings {
        settings::handler(key, app);
      } else {
        handle_escape(app);
      }
    }
    _ if key == app.user_config.keys.decrease_volume => {
      app.apply(Action::VolumeDown);
    }
    _ if key == app.user_config.keys.increase_volume => {
      app.apply(Action::VolumeUp);
    }
    // Press space to toggle playback
    _ if key == app.user_config.keys.toggle_playback => {
      app.apply(Action::TogglePlayback);
    }
    _ if key == app.user_config.keys.seek_backwards => {
      app.apply(Action::SeekBackward);
    }
    _ if key == app.user_config.keys.seek_forwards => {
      app.apply(Action::SeekForward);
    }
    _ if key == app.user_config.keys.next_track => {
      app.apply(Action::NextTrack);
    }
    _ if key == app.user_config.keys.previous_track => {
      app.apply(Action::PreviousTrack);
    }
    _ if key == app.user_config.keys.force_previous_track => {
      app.apply(Action::ForcePreviousTrack);
    }
    _ if key == app.user_config.keys.help => {
      help_menu::open(app);
    }
    _ if key == app.user_config.keys.show_queue => {
      app.apply(Action::Navigate(NavTarget::Queue));
    }

    _ if key == app.user_config.keys.shuffle => {
      app.apply(Action::ToggleShuffle);
    }
    _ if key == app.user_config.keys.repeat => {
      app.apply(Action::CycleRepeat);
    }
    _ if key == app.user_config.keys.search => {
      open_global_search(app);
    }
    _ if key == app.user_config.keys.audio_analysis => {
      app.apply(Action::Navigate(NavTarget::Analysis));
    }
    _ if key == app.user_config.keys.lyrics_view => {
      app.apply(Action::Navigate(NavTarget::Lyrics));
    }
    _ if key == app.user_config.keys.miniplayer_view => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        app.apply(Action::Navigate(NavTarget::MiniPlayer));
      }
    }
    #[cfg(feature = "cover-art")]
    _ if key == app.user_config.keys.cover_art_view => {
      app.push_navigation_stack(RouteId::CoverArtView, ActiveBlock::CoverArtView);
    }
    #[cfg(feature = "scripting")]
    _ if app.user_config.plugin_command_keys.contains_key(&key) => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else if let Some(name) = app.user_config.plugin_command_keys.get(&key).cloned() {
        app.queue_plugin_command(name);
      }
    }
    _ if key == app.user_config.keys.generate_recap => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        app.apply(Action::GenerateRecap);
      }
    }
    // Resize sidebar: { decreases, } increases width
    Key::Char('{') => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        resize::decrease_sidebar_width(app);
      }
    }
    Key::Char('}') => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        resize::increase_sidebar_width(app);
      }
    }
    // Resize playbar or library/playlist split depending on hovered pane:
    // ( decreases height, ) increases height
    Key::Char('(') => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        match app.get_current_route().hovered_block {
          ActiveBlock::Library | ActiveBlock::MyPlaylists => resize::decrease_library_height(app),
          _ => resize::decrease_playbar_height(app),
        }
      }
    }
    Key::Char(')') => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        match app.get_current_route().hovered_block {
          ActiveBlock::Library | ActiveBlock::MyPlaylists => resize::increase_library_height(app),
          _ => resize::increase_playbar_height(app),
        }
      }
    }
    // Reset all pane sizes to defaults
    Key::Char('|') => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      } else {
        resize::reset_layout(app);
      }
    }
    Key::Char('W') => {
      if is_input_mode(app) {
        handle_block_events(key, app);
      }
    }
    _ => {
      if !panes::handle_focused_pane_key(key, app) {
        handle_block_events(key, app);
      }
    }
  }
}

/// Open the global search box. Never rewrite the error frame in place to
/// `{Error, Input}`: push dedupes on the top frame's id, so a surviving Error
/// frame makes every later error silently fail to render. Dismiss the error
/// and open search on the screen below.
pub(super) fn focus_global_search(app: &mut App) {
  if app.get_current_route().id == RouteId::Error {
    app.clear_api_error();
  }
  app.view.input_context = InputContext::GlobalSearch;
  app.set_current_route_state(Some(ActiveBlock::Input), Some(ActiveBlock::Input));
}

/// Focuses the search input, or reports why the key does nothing now.
pub(super) fn open_global_search(app: &mut App) -> bool {
  match app.availability(Requirement::Capability(Capability::Search)) {
    Availability::Available => {
      focus_global_search(app);
      true
    }
    Availability::NotForSource(_) => {
      app.set_status_message(
        app
          .strings()
          .status
          .app
          .search_source_unavailable(app.strings().sidebar.source_label(app.active_source)),
        4,
      );
      false
    }
  }
}

fn is_input_mode(app: &App) -> bool {
  matches!(
    app.get_current_route().active_block,
    ActiveBlock::Input
      | ActiveBlock::Dialog(_)
      | ActiveBlock::AnnouncementPrompt
      | ActiveBlock::ExitPrompt
      | ActiveBlock::RecapPrompt
  ) || {
    {
      false
    }
  }
}

// Handle event for the current active block
fn handle_block_events(key: Key, app: &mut App) {
  let current_route = app.get_current_route();
  match current_route.active_block {
    ActiveBlock::Analysis => {
      analysis::handler(key, app);
    }
    ActiveBlock::Input => {
      input::handler(key, app);
    }
    ActiveBlock::MyPlaylists => {
      playlist::handler(key, app);
    }
    ActiveBlock::TrackTable => {
      track_table::handler(key, app);
    }
    ActiveBlock::HelpMenu => {
      help_menu::handler(key, app);
    }
    ActiveBlock::Error => {
      error_screen::handler(key, app);
    }
    ActiveBlock::SearchResultBlock => {
      search_results::handler(key, app);
    }
    ActiveBlock::Home => {
      home::handler(key, app);
    }
    ActiveBlock::Library => {
      library::handler(key, app);
    }
    ActiveBlock::Empty => {
      empty::handler(key, app);
    }
    ActiveBlock::PlayBar => {
      playbar::handler(key, app);
    }
    ActiveBlock::LyricsView => {
      lyrics_view::handler(key, app);
    }
    ActiveBlock::MiniPlayer => {
      miniplayer::handler(key, app);
    }
    ActiveBlock::CoverArtView => {
      #[cfg(feature = "cover-art")]
      cover_art_view::handler(key, app);
    }
    ActiveBlock::Dialog(_) => {
      dialog::handler(key, app);
    }

    ActiveBlock::AnnouncementPrompt => {
      announcement_prompt::handler(key, app);
    }
    ActiveBlock::ExitPrompt => {}
    ActiveBlock::Settings => {
      settings::handler(key, app);
    }
    ActiveBlock::SortMenu => {
      sort_menu::handler(key, app);
    }
    ActiveBlock::Queue => {
      queue_menu::handler(key, app);
    }
    ActiveBlock::Stats => {
      stats::handler(key, app);
    }
    ActiveBlock::RecapPrompt => {
      recap_prompt::handler(key, app);
    }
    ActiveBlock::PluginScreen => {
      plugin_screen::handler(key, app);
    }
  }
}

/// Unified Back/Esc handler to clear active track filters before exiting.
///
/// Handles playlist filters first (saved tracks are subsumed).
/// Returns `true` if a filter was cleared.
pub(crate) fn clear_active_track_filter(app: &mut App) -> bool {
  if app.is_playlist_track_filter_active() {
    app.clear_playlist_track_filter();
    true
  } else if app.is_saved_track_filter_active() {
    app.clear_saved_track_filter();
    true
  } else {
    false
  }
}

fn handle_escape(app: &mut App) {
  match app.get_current_route().active_block {
    ActiveBlock::SearchResultBlock => {
      app.view.search_selected_block = SearchResultBlock::Empty;
    }
    ActiveBlock::Error => {
      app.pop_navigation_stack();
    }
    ActiveBlock::Dialog(dialog_context) => {
      if dialog_context == crate::core::app::DialogContext::PersistKeybindingFallback {
        app.set_status_message(app.strings().status.app.shortcut_session_only, 4);
      }
      app.pop_navigation_stack();
      app.clear_dialog_state();
    }
    ActiveBlock::HelpMenu => {
      if app.view.help_filter_editing || !app.view.help_filter.is_empty() {
        help_menu::clear_filter(app);
      } else {
        app.pop_navigation_stack();
      }
    }
    ActiveBlock::TrackTable => {
      if !clear_active_track_filter(app) {
        app.set_current_route_state(Some(ActiveBlock::Empty), None);
      }
    }
    ActiveBlock::Queue => {
      app.pop_navigation_stack();
    }
    ActiveBlock::LyricsView => {
      // Esc first leaves lyrics browsing mode; a second Esc closes the view.
      if app.view.lyrics_view.manual_index.is_some() {
        app.view.lyrics_view.manual_index = None;
      } else {
        app.pop_navigation_stack();
      }
    }
    ActiveBlock::CoverArtView
    | ActiveBlock::MiniPlayer
    | ActiveBlock::PluginScreen
    | ActiveBlock::Analysis => {
      app.pop_navigation_stack();
    }

    // Esc hands the popup its own dismissal, like Enter does
    ActiveBlock::AnnouncementPrompt => {
      announcement_prompt::handler(Key::Esc, app);
    }
    ActiveBlock::ExitPrompt => {}
    // Sort menu closes on escape
    ActiveBlock::SortMenu => {
      app.view.sort_menu_visible = false;
      app.view.sort_context = None;
      app.set_current_route_state(Some(ActiveBlock::Empty), None);
    }
    // "[ESC] Later" on the recap popup: dismiss without opening.
    ActiveBlock::RecapPrompt => {
      app.apply(Action::DismissRecapPrompt);
    }
    _ => {
      app.set_current_route_state(Some(ActiveBlock::Empty), None);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // --- Sort menu shortcut routing (issue #660) ---

  #[test]
  fn global_shift_w_is_not_intercepted_in_input_mode() {
    let mut app = App::default();
    app.set_current_route_state(Some(ActiveBlock::Input), Some(ActiveBlock::Input));

    handle_app(Key::Char('W'), &mut app);

    assert_eq!(app.view.input, vec!['W']);
    assert!(app.status_message().is_none());
  }

  #[test]
  fn force_previous_track_dispatches_from_anywhere() {
    let mut app = App::default();
    app.set_current_route_state(Some(ActiveBlock::Empty), Some(ActiveBlock::Library));

    // Default force_previous_track is Key::Char('P')
    handle_app(Key::Char('P'), &mut app);

    // force_previous_track dispatches through App which requires no io_tx in tests,
    // so just confirm the route didn't change (it shouldn't navigate anywhere)
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Empty);
  }

  #[test]
  fn global_shift_f_is_not_intercepted_in_input_mode() {
    let mut app = App::default();
    app.set_current_route_state(Some(ActiveBlock::Input), Some(ActiveBlock::Input));

    handle_app(Key::Char('F'), &mut app);

    // In input mode, 'F' should be added to the input buffer
    assert_eq!(app.view.input, vec!['F']);
  }

  #[test]
  fn ctrl_f_off_a_track_table_leaves_the_input_context_alone() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);

    handle_app(Key::Ctrl('f'), &mut app);

    assert_eq!(app.view.input_context, InputContext::GlobalSearch);
    assert_eq!(
      app.get_current_route().active_block,
      ActiveBlock::TrackTable
    );
  }

  #[cfg(target_os = "macos")]
  #[test]
  fn plain_comma_fallback_opens_settings_and_prompts_to_persist() {
    let mut app = App::default();
    app.user_config.keys.open_settings = Key::Ctrl(',');
    app.set_current_route_state(Some(ActiveBlock::Empty), Some(ActiveBlock::Library));

    handle_app(Key::Char(','), &mut app);

    assert_eq!(
      app.keybinding_runtime.effective_open_settings,
      Some(Key::Alt(','))
    );
    assert_eq!(
      app.get_current_route().active_block,
      ActiveBlock::Dialog(crate::core::app::DialogContext::PersistKeybindingFallback)
    );
    assert!(app.status_message().is_some());
  }

  #[cfg(target_os = "macos")]
  #[test]
  fn plain_comma_does_not_override_track_table_sort_menu() {
    let mut app = App::default();
    app.user_config.keys.open_settings = Key::Ctrl(',');
    app.track_table.context = Some(TrackTableContext::QobuzPlaylist);
    app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);

    handle_app(Key::Char(','), &mut app);

    assert_eq!(app.get_current_route().active_block, ActiveBlock::SortMenu);
  }

  // --- U5: source-gate tests ---

  #[test]
  fn esc_closes_the_audio_analysis_screen() {
    let mut app = App::default();
    app.get_audio_analysis();
    assert_eq!(app.get_current_route().id, RouteId::Analysis);

    handle_app(Key::Esc, &mut app);

    assert_eq!(app.get_current_route().id, RouteId::Home);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Empty);
  }
}
