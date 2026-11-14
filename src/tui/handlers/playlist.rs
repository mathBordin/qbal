use super::common_key_events;
use crate::core::action::{Action, OpenTarget};
use crate::core::app::App;
use crate::core::app::{ActiveBlock, SidebarBlock};
use crate::core::source::Source;
use crate::tui::event::Key;

/// Total rows in the sidebar Playlists panel. For Spotify this is the leading
/// "+ Add Playlist" row + playlists/folders; for Local it is the folder count
/// (no write capability, so no "Add Playlist").
pub(crate) fn total_display_count(app: &App) -> usize {
  match app.active_source {
    Source::Qobuz => app.qobuz_playlists().len(),
  }
}

/// Open a non-Spotify playlist's tracks in the shared track table.
pub(super) fn open_source_playlist(app: &mut App, uri: Option<String>) {
  if let Some(uri) = uri {
    app.apply(Action::Open(OpenTarget::SourcePlaylist(uri)));
  }
}

/// Qobuz: open the highlighted row (favorites, playlist, or album) in the
/// shared track table.
fn open_qobuz_folder(app: &mut App) {
  let uri = app
    .view
    .selected_playlist_index
    .and_then(|idx| app.qobuz_playlists().get(idx))
    .map(|playlist| playlist.uri.clone());
  open_source_playlist(app, uri);
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::toward_content_event(k, &app.user_config) => {
      common_key_events::focus_content(app)
    }
    k if common_key_events::down_event(k, &app.user_config.keys) => select_next(app),
    k if common_key_events::up_event(k, &app.user_config.keys) => select_previous(app),
    k if common_key_events::high_event(k) && total_display_count(app) > 0 => {
      app.view.selected_playlist_index = Some(0);
    }
    k if common_key_events::middle_event(k) => {
      let count = total_display_count(app);
      if count > 0 {
        let next_index = if count.is_multiple_of(2) {
          count.saturating_sub(1) / 2
        } else {
          count / 2
        };
        app.view.selected_playlist_index = Some(next_index);
      }
    }
    k if common_key_events::low_event(k) => {
      let count = total_display_count(app);
      if count > 0 {
        app.view.selected_playlist_index = Some(count - 1);
      }
    }
    Key::Enter => {
      common_key_events::remember_sidebar_block(app, ActiveBlock::MyPlaylists);
      activate_selected(app);
    }
    _ => {}
  }
}

/// The down key's cursor move, named for the mouse wheel.
pub(super) fn select_next(app: &mut App) {
  let count = total_display_count(app);
  if count > 0 {
    let current = app.view.selected_playlist_index.unwrap_or(0);
    app.view.selected_playlist_index = Some((current + 1) % count);
  }
}

pub(super) fn select_previous(app: &mut App) {
  let count = total_display_count(app);
  if count > 0 {
    let current = app.view.selected_playlist_index.unwrap_or(0);
    app.view.selected_playlist_index = Some(if current == 0 { count - 1 } else { current - 1 });
  }
}

/// The Enter consequence, routed by the active browse source.
pub(super) fn activate_selected(app: &mut App) {
  app.view.last_sidebar_block = SidebarBlock::Playlists;
  match app.active_source {
    Source::Qobuz => open_qobuz_folder(app),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn opening_a_playlist_row_remembers_the_playlist_panel() {
    let mut app = App::default();
    app.view.last_sidebar_block = SidebarBlock::Library;
    activate_selected(&mut app);
    assert_eq!(app.view.last_sidebar_block, SidebarBlock::Playlists);
  }
}
