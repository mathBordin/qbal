//! The search screen: songs, albums or artists from the Qobuz catalog, one tab
//! at a time.
//!
//! The list is either *hovered* (focus is on the screen, `search_selected_block`
//! is `Empty`) or *selected* (a row cursor moves inside it). Enter selects the
//! hovered list, then acts on the selected row: a song starts its list at that
//! song, an album plays whole from its first track, an artist opens the
//! artist's page. Tab and Shift-Tab move between the tabs.

use super::common_key_events;
use crate::core::action::{Action, OpenTarget};
use crate::core::app::{App, SearchMode, SearchResultBlock};
use crate::core::plugin_api::{AlbumInfo, ArtistInfo, TrackInfo};
use crate::tui::event::Key;

fn songs(app: &App) -> &[TrackInfo] {
  app
    .search_results()
    .tracks
    .as_ref()
    .map_or(&[], |paged| paged.items.as_slice())
}

fn albums(app: &App) -> &[AlbumInfo] {
  app
    .search_results()
    .albums
    .as_ref()
    .map_or(&[], |paged| paged.items.as_slice())
}

fn artists(app: &App) -> &[ArtistInfo] {
  app
    .search_results()
    .artists
    .as_ref()
    .map_or(&[], |paged| paged.items.as_slice())
}

fn is_selected(app: &App) -> bool {
  app.view.search_selected_block == app.view.search_mode.block()
}

/// The shown list's length and cursor row.
fn cursor(app: &App) -> (usize, Option<usize>) {
  match app.view.search_mode {
    SearchMode::Songs => (songs(app).len(), app.view.search_selected_tracks_index),
    SearchMode::Albums => (albums(app).len(), app.view.search_selected_albums_index),
    SearchMode::Artists => (artists(app).len(), app.view.search_selected_artists_index),
  }
}

fn set_cursor(app: &mut App, index: usize) {
  match app.view.search_mode {
    SearchMode::Songs => app.view.search_selected_tracks_index = Some(index),
    SearchMode::Albums => app.view.search_selected_albums_index = Some(index),
    SearchMode::Artists => app.view.search_selected_artists_index = Some(index),
  }
}

/// Move the cursor with one of the shared list-navigation functions, which
/// only look at the list's length.
fn move_cursor(app: &mut App, next: impl FnOnce(&[()], Option<usize>) -> usize) {
  let (len, index) = cursor(app);
  // A `Vec` of a zero-sized type holds its length without allocating.
  let rows = vec![(); len];
  set_cursor(app, next(&rows, index));
}

/// Act on the selected row: play the songs list from that song, play the
/// album whole, or open the artist's page.
fn play_selected(app: &mut App) {
  match app.view.search_mode {
    SearchMode::Songs => play_selected_song(app),
    SearchMode::Albums => {
      if let Some(uri) = selected_album_uri(app) {
        app.apply(Action::PlayAlbum { uri });
      }
    }
    SearchMode::Artists => {
      let artist = app
        .view
        .search_selected_artists_index
        .and_then(|index| artists(app).get(index))
        .and_then(|artist| Some((artist.uri.clone()?, artist.name.clone())));
      if let Some((uri, name)) = artist {
        app.apply(Action::Open(OpenTarget::Artist { uri, name }));
      }
    }
  }
}

fn play_selected_song(app: &mut App) {
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

fn selected_album_uri(app: &App) -> Option<String> {
  let index = app.view.search_selected_albums_index?;
  albums(app).get(index)?.uri.clone()
}

/// Show the selected album's tracks in the track table.
fn open_selected_album(app: &mut App) {
  if app.view.search_mode != SearchMode::Albums || !is_selected(app) {
    return;
  }
  if let Some(uri) = selected_album_uri(app) {
    app.apply(Action::Open(OpenTarget::SourcePlaylist(uri)));
  }
}

fn queue_selected(app: &mut App) {
  if app.view.search_mode != SearchMode::Songs || !is_selected(app) {
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
    Key::Tab => app.cycle_search_mode(true),
    Key::BackTab => app.cycle_search_mode(false),
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
      move_cursor(app, |rows, _| {
        common_key_events::on_middle_press_handler(rows)
      });
    }
    k if common_key_events::low_event(k) && is_selected(app) => {
      move_cursor(app, |rows, _| common_key_events::on_low_press_handler(rows));
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
      let (_, index) = cursor(app);
      set_cursor(app, index.unwrap_or(0));
      app.view.search_selected_block = app.view.search_mode.block();
    }
    Key::Char('o') => open_selected_album(app),
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
    app.show_source_search_results(
      "q",
      (1..=count).map(track).collect(),
      Vec::new(),
      Vec::new(),
    );
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

  fn artist(n: u32) -> ArtistInfo {
    ArtistInfo {
      id: Some(n.to_string()),
      uri: Some(format!("qobuz:artist:{n}")),
      name: format!("Artist {n}"),
      image_url: None,
    }
  }

  fn album(n: u32) -> AlbumInfo {
    AlbumInfo {
      id: Some(n.to_string()),
      uri: Some(format!("qobuz:album:{n}")),
      name: format!("Album {n}"),
      total_tracks: Some(10),
      ..AlbumInfo::default()
    }
  }

  /// An app on the search screen with `count` songs and albums, whose
  /// dispatched events the returned receiver collects.
  fn app_with_results(
    count: u32,
  ) -> (
    App,
    std::sync::mpsc::Receiver<crate::infra::network::IoEvent>,
  ) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = App::new_with_state(
      tx,
      crate::core::user_config::UserConfig::new(),
      crate::core::state::RuntimeState::default(),
      None,
      crate::core::i18n::Language::En,
    );
    app.show_source_search_results(
      "q",
      (1..=count).map(track).collect(),
      (1..=count).map(album).collect(),
      (1..=count).map(artist).collect(),
    );
    app.set_current_route_state(
      Some(ActiveBlock::SearchResultBlock),
      Some(ActiveBlock::SearchResultBlock),
    );
    (app, rx)
  }

  #[test]
  fn tab_cycles_songs_albums_artists_and_leaves_the_list() {
    let (mut app, _rx) = app_with_results(3);
    handler(Key::Enter, &mut app);
    assert_eq!(app.view.search_mode, SearchMode::Songs);
    assert!(is_selected(&app));

    handler(Key::Tab, &mut app);
    assert_eq!(app.view.search_mode, SearchMode::Albums);
    assert_eq!(app.view.search_selected_block, SearchResultBlock::Empty);
    assert_eq!(
      app.view.search_hovered_block,
      SearchResultBlock::AlbumSearch
    );

    handler(Key::Tab, &mut app);
    assert_eq!(app.view.search_mode, SearchMode::Artists);
    handler(Key::Tab, &mut app);
    assert_eq!(app.view.search_mode, SearchMode::Songs);
    handler(Key::BackTab, &mut app);
    assert_eq!(app.view.search_mode, SearchMode::Artists);
  }

  #[test]
  fn songs_and_albums_set_the_mode_artist_pages_open_in() {
    use crate::core::artist::ArtistTab;
    let (mut app, _rx) = app_with_results(1);
    handler(Key::Tab, &mut app);
    assert_eq!(app.view.artist_tab, ArtistTab::Albums);
    handler(Key::Tab, &mut app);
    assert_eq!(
      app.view.artist_tab,
      ArtistTab::Albums,
      "the artists tab keeps it"
    );
    handler(Key::Tab, &mut app);
    assert_eq!(app.view.artist_tab, ArtistTab::TopTracks);
  }

  #[test]
  fn enter_on_an_artist_opens_the_artist_page() {
    use crate::core::app::RouteId;
    let (mut app, rx) = app_with_results(2);
    handler(Key::BackTab, &mut app);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    handler(Key::Enter, &mut app);
    assert_eq!(next_event(&rx).as_deref(), Some("open artist Artist 2"));
    assert_eq!(app.get_current_route().id, RouteId::Artist);
  }

  #[test]
  fn each_mode_keeps_its_own_cursor() {
    let (mut app, _rx) = app_with_results(4);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    handler(Key::Down, &mut app);
    assert_eq!(app.view.search_selected_tracks_index, Some(2));

    handler(Key::Tab, &mut app);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    assert_eq!(app.view.search_selected_albums_index, Some(1));
    assert_eq!(app.view.search_selected_tracks_index, Some(2));
  }

  /// The next dispatched event, in words.
  fn next_event(rx: &std::sync::mpsc::Receiver<crate::infra::network::IoEvent>) -> Option<String> {
    rx.try_recv()
      .ok()
      .map(|event| crate::core::test_helpers::describe_event(&event))
  }

  #[test]
  fn enter_on_an_album_plays_the_whole_album() {
    let (mut app, rx) = app_with_results(3);
    handler(Key::Tab, &mut app);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    handler(Key::Enter, &mut app);
    assert_eq!(next_event(&rx).as_deref(), Some("play album qobuz:album:2"));
    assert!(
      rx.try_recv().is_err(),
      "nothing else is dispatched for an album"
    );
  }

  #[test]
  fn o_opens_the_selected_album_s_track_list() {
    let (mut app, rx) = app_with_results(2);
    handler(Key::Tab, &mut app);
    handler(Key::Enter, &mut app);
    handler(Key::Char('o'), &mut app);
    assert_eq!(next_event(&rx).as_deref(), Some("open qobuz:album:1"));
  }

  #[test]
  fn o_and_the_queue_key_do_nothing_in_the_wrong_mode() {
    let (mut app, rx) = app_with_results(2);
    handler(Key::Enter, &mut app);
    handler(Key::Char('o'), &mut app);
    assert!(rx.try_recv().is_err(), "o only opens albums");

    handler(Key::Tab, &mut app);
    handler(Key::Enter, &mut app);
    let queue_key = app.user_config.keys.add_item_to_queue;
    handler(queue_key, &mut app);
    assert!(rx.try_recv().is_err(), "the queue key adds songs only");
  }

  #[test]
  fn enter_on_a_song_still_plays_the_song_list_from_that_row() {
    let (mut app, rx) = app_with_results(3);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    handler(Key::Enter, &mut app);
    assert_eq!(
      next_event(&rx).as_deref(),
      Some("play 3 songs from Some(1)")
    );
  }

  #[test]
  fn a_new_search_resets_both_cursors_and_keeps_the_mode() {
    let (mut app, _rx) = app_with_results(3);
    handler(Key::Tab, &mut app);
    handler(Key::Enter, &mut app);
    handler(Key::Down, &mut app);
    app.show_source_search_results("again", vec![track(9)], vec![album(9)], Vec::new());
    assert_eq!(app.view.search_mode, SearchMode::Albums);
    assert_eq!(app.view.search_selected_albums_index, Some(0));
    assert_eq!(app.view.search_selected_block, SearchResultBlock::Empty);
    assert_eq!(
      app.view.search_hovered_block,
      SearchResultBlock::AlbumSearch
    );
  }
}
