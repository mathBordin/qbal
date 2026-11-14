//! The search screen: one list of songs from the Qobuz catalog.
//!
//! The list is either *hovered* (focus is on the screen, `search_selected_block`
//! is `Empty`) or *selected* (a row cursor moves inside it). Enter selects the
//! hovered list, then plays from the selected row.

use super::common_key_events;
use crate::core::action::Action;
use crate::core::app::{App, SearchResultBlock};
use crate::core::plugin_api::TrackInfo;
use crate::tui::event::Key;

fn songs(app: &App) -> &[TrackInfo] {
  app
    .search_results()
    .tracks
    .as_ref()
    .map_or(&[], |paged| paged.items.as_slice())
}

fn is_selected(app: &App) -> bool {
  app.view.search_selected_block == SearchResultBlock::SongSearch
}

fn move_cursor(app: &mut App, next: impl FnOnce(&[TrackInfo], Option<usize>) -> usize) {
  let index = next(songs(app), app.view.search_selected_tracks_index);
  app.view.search_selected_tracks_index = Some(index);
}

/// Play the result list from the selected row.
fn play_selected(app: &mut App) {
  let Some(paged) = app.search_results().tracks.as_ref() else {
    return;
  };
  // No selection: an out-of-range index yields `offset: None`.
  let selected = app
    .view
    .search_selected_tracks_index
    .unwrap_or(paged.items.len());
  let (uris, offset) = common_key_events::uri_playback_request(
    paged.items.iter().map(|track| track.uri.clone()),
    selected,
  );
  app.apply(Action::PlayUris { uris, offset });
}

fn queue_selected(app: &mut App) {
  if !is_selected(app) {
    return;
  }
  let track = app
    .view
    .search_selected_tracks_index
    .and_then(|index| songs(app).get(index).cloned());
  if let Some(track) = track {
    app.apply(Action::QueueTrack(track));
  }
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => {
      app.view.search_selected_block = SearchResultBlock::Empty;
    }
    k if common_key_events::down_event(k, &app.user_config.keys) && is_selected(app) => {
      move_cursor(app, common_key_events::on_down_press_handler);
    }
    k if common_key_events::up_event(k, &app.user_config.keys) && is_selected(app) => {
      move_cursor(app, common_key_events::on_up_press_handler);
    }
    k if common_key_events::high_event(k) && is_selected(app) => {
      move_cursor(app, |_, _| common_key_events::on_high_press_handler());
    }
    k if common_key_events::middle_event(k) && is_selected(app) => {
      move_cursor(app, |items, _| {
        common_key_events::on_middle_press_handler(items)
      });
    }
    k if common_key_events::low_event(k) && is_selected(app) => {
      move_cursor(app, |items, _| {
        common_key_events::on_low_press_handler(items)
      });
    }
    k if common_key_events::left_event(k, &app.user_config.keys)
      && !common_key_events::sidebar_on_right(&app.user_config) =>
    {
      app.view.search_selected_block = SearchResultBlock::Empty;
      common_key_events::focus_sidebar(app);
    }
    k if common_key_events::right_event(k, &app.user_config.keys)
      && common_key_events::sidebar_on_right(&app.user_config) =>
    {
      app.view.search_selected_block = SearchResultBlock::Empty;
      common_key_events::focus_sidebar(app);
    }
    Key::Enter if is_selected(app) => play_selected(app),
    Key::Enter => {
      let index = app.view.search_selected_tracks_index.unwrap_or(0);
      app.view.search_selected_tracks_index = Some(index);
      app.view.search_selected_block = SearchResultBlock::SongSearch;
    }
    _ if key == app.user_config.keys.add_item_to_queue => queue_selected(app),
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::app::ActiveBlock;

  fn track(n: u32) -> TrackInfo {
    TrackInfo {
      generated_name: None,
      uri: Some(format!("qobuz:track:{n}")),
      name: format!("Song {n}"),
      artists: vec!["Artist".to_string()],
      album: "Album".to_string(),
      duration_ms: 180_000,
      id: Some(n.to_string()),
      album_id: None,
      artist_refs: Vec::new(),
      is_playable: true,
      is_local: false,
      track_number: n,
      explicit: false,
      image_url: None,
      release_date: None,
    }
  }

  fn app_with_songs(count: u32) -> App {
    let mut app = App::default();
    app.show_source_search_tracks("q", (1..=count).map(track).collect());
    app.set_current_route_state(
      Some(ActiveBlock::SearchResultBlock),
      Some(ActiveBlock::SearchResultBlock),
    );
    app
  }

  #[test]
  fn enter_selects_the_song_list_then_down_moves_the_cursor() {
    let mut app = app_with_songs(3);
    assert_eq!(app.view.search_selected_block, SearchResultBlock::Empty);

    handler(Key::Enter, &mut app);
    assert_eq!(
      app.view.search_selected_block,
      SearchResultBlock::SongSearch
    );
    assert_eq!(app.view.search_selected_tracks_index, Some(0));

    handler(Key::Down, &mut app);
    assert_eq!(app.view.search_selected_tracks_index, Some(1));
  }

  #[test]
  fn arrow_keys_do_nothing_until_the_list_is_selected() {
    let mut app = app_with_songs(3);
    handler(Key::Down, &mut app);
    assert_eq!(app.view.search_selected_tracks_index, Some(0));
    assert_eq!(app.view.search_selected_block, SearchResultBlock::Empty);
  }

  #[test]
  fn esc_leaves_the_list_without_losing_the_cursor() {
    let mut app = app_with_songs(3);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    handler(Key::Esc, &mut app);
    assert_eq!(app.view.search_selected_block, SearchResultBlock::Empty);
    assert_eq!(app.view.search_selected_tracks_index, Some(1));
  }
}
