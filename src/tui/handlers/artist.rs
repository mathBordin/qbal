//! The artist page: the artist's popular tracks, or the discography grouped as
//! albums, EPs & singles and other releases. Tab switches between the two.
//!
//! On the tracks tab Enter plays the list from the selected track and the queue
//! key queues it; on the albums tab Enter plays the selected release whole and
//! `o` opens its track list instead.

use super::common_key_events;
use crate::core::action::{Action, OpenTarget};
use crate::core::app::App;
use crate::core::artist::{discography_rows, step_release_row, DiscographyRow};
use crate::tui::event::Key;

/// The selected release's URI on the albums tab.
fn selected_release_uri(app: &App) -> Option<String> {
  let page = app.artist_page()?;
  let rows = discography_rows(&page.releases);
  match rows.get(app.view.artist_release_row)? {
    DiscographyRow::Release(index) => Some(page.releases.get(*index)?.uri.clone()),
    DiscographyRow::Heading(_) => None,
  }
}

fn track_count(app: &App) -> usize {
  app.artist_page().map_or(0, |page| page.top_tracks.len())
}

/// Move the cursor: `step` rows (`1`/`-1`), or to the top (`Top`) or bottom
/// (`Bottom`) of the shown list.
enum Move {
  Step(isize),
  Top,
  Middle,
  Bottom,
}

fn move_cursor(app: &mut App, movement: Move) {
  if app.artist_shows_albums() {
    let Some(page) = app.artist_page() else {
      return;
    };
    let rows = discography_rows(&page.releases);
    let releases: Vec<usize> = rows
      .iter()
      .enumerate()
      .filter(|(_, row)| matches!(row, DiscographyRow::Release(_)))
      .map(|(index, _)| index)
      .collect();
    let current = app.view.artist_release_row;
    app.view.artist_release_row = match movement {
      Move::Step(step) => step_release_row(&rows, current, step),
      Move::Top => releases.first().copied().unwrap_or(0),
      Move::Middle => releases.get(releases.len() / 2).copied().unwrap_or(0),
      Move::Bottom => releases.last().copied().unwrap_or(0),
    };
  } else {
    let len = track_count(app);
    let current = app.view.artist_track_index;
    app.view.artist_track_index = match movement {
      Move::Step(1) => (current + 1).min(len.saturating_sub(1)),
      Move::Step(_) => current.saturating_sub(1),
      Move::Top => 0,
      Move::Middle => len / 2,
      Move::Bottom => len.saturating_sub(1),
    };
  }
}

fn activate(app: &mut App) {
  if app.artist_shows_albums() {
    if let Some(uri) = selected_release_uri(app) {
      app.apply(Action::PlayAlbum { uri });
    }
    return;
  }
  let Some(page) = app.artist_page() else {
    return;
  };
  if page.top_tracks.is_empty() {
    return;
  }
  let (uris, offset) = common_key_events::uri_playback_request(
    page.top_tracks.iter().map(|track| track.uri.clone()),
    app.view.artist_track_index,
  );
  app.apply(Action::PlayUris { uris, offset });
}

fn queue_selected(app: &mut App) {
  if app.artist_shows_albums() {
    return;
  }
  let track = app
    .artist_page()
    .and_then(|page| page.top_tracks.get(app.view.artist_track_index).cloned());
  if let Some(track) = track {
    app.apply(Action::QueueTrack(track));
  }
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::toward_sidebar_event(k, &app.user_config) => {
      common_key_events::focus_sidebar(app)
    }
    Key::Tab | Key::BackTab => app.toggle_artist_tab(),
    k if common_key_events::down_event(k, &app.user_config.keys) => move_cursor(app, Move::Step(1)),
    k if common_key_events::up_event(k, &app.user_config.keys) => move_cursor(app, Move::Step(-1)),
    k if common_key_events::high_event(k) => move_cursor(app, Move::Top),
    k if common_key_events::middle_event(k) => move_cursor(app, Move::Middle),
    k if common_key_events::low_event(k) => move_cursor(app, Move::Bottom),
    Key::Enter => activate(app),
    Key::Char('o') if app.artist_shows_albums() => {
      if let Some(uri) = selected_release_uri(app) {
        app.apply(Action::Open(OpenTarget::SourcePlaylist(uri)));
      }
    }
    _ if key == app.user_config.keys.add_item_to_queue => queue_selected(app),
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::app::{ActiveBlock, RouteId};
  use crate::core::artist::{ArtistPage, ArtistTab, Release, ReleaseKind};
  use crate::core::plugin_api::TrackInfo;
  use crate::infra::network::IoEvent;

  fn track(n: u32) -> TrackInfo {
    TrackInfo {
      generated_name: None,
      uri: Some(format!("qobuz:track:{n}")),
      name: format!("Track {n}"),
      artists: vec!["Daft Punk".into()],
      album: "Album".into(),
      duration_ms: 200_000,
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

  fn release(id: &str, kind: ReleaseKind, date: &str) -> Release {
    Release {
      uri: format!("qobuz:album:{id}"),
      title: id.into(),
      version: None,
      release_date: Some(date.into()),
      tracks_count: 10,
      kind,
      bit_depth: None,
      sample_rate_khz: None,
    }
  }

  fn app_on_artist(tab: ArtistTab) -> (App, std::sync::mpsc::Receiver<IoEvent>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = App::new_with_state(
      tx,
      crate::core::user_config::UserConfig::new(),
      crate::core::state::RuntimeState::default(),
      None,
      crate::core::i18n::Language::En,
    );
    app.view.artist_tab = tab;
    app.open_artist("qobuz:artist:1".into(), "Daft Punk".into());
    let _ = rx.try_recv();
    app.show_artist_page(ArtistPage {
      uri: "qobuz:artist:1".into(),
      name: "Daft Punk".into(),
      loading: false,
      top_tracks: (1..=3).map(track).collect(),
      releases: crate::core::artist::arrange_releases(vec![
        release("ram", ReleaseKind::Album, "2013-05-17"),
        release("disco", ReleaseKind::Album, "2001-03-12"),
        release("lucky", ReleaseKind::EpSingle, "2013-04-19"),
      ]),
    });
    assert_eq!(app.get_current_route().id, RouteId::Artist);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Artist);
    (app, rx)
  }

  fn next_event(rx: &std::sync::mpsc::Receiver<IoEvent>) -> Option<String> {
    rx.try_recv()
      .ok()
      .map(|event| crate::core::test_helpers::describe_event(&event))
  }

  #[test]
  fn enter_on_a_popular_track_plays_the_list_from_it() {
    let (mut app, rx) = app_on_artist(ArtistTab::TopTracks);
    handler(Key::Down, &mut app);
    handler(Key::Enter, &mut app);
    assert_eq!(
      next_event(&rx).as_deref(),
      Some("play 3 songs from Some(1)")
    );
  }

  #[test]
  fn the_albums_tab_skips_headings_plays_whole_releases_and_opens_with_o() {
    let (mut app, rx) = app_on_artist(ArtistTab::Albums);
    handler(Key::Enter, &mut app);
    assert_eq!(
      next_event(&rx).as_deref(),
      Some("play album qobuz:album:ram"),
      "the newest album comes first"
    );
    handler(Key::Down, &mut app);
    handler(Key::Down, &mut app);
    handler(Key::Char('o'), &mut app);
    assert_eq!(
      next_event(&rx).as_deref(),
      Some("open qobuz:album:lucky"),
      "down moved past the EPs & singles heading"
    );
  }

  #[test]
  fn tab_switches_between_tracks_and_albums() {
    let (mut app, _rx) = app_on_artist(ArtistTab::TopTracks);
    assert!(!app.artist_shows_albums());
    handler(Key::Tab, &mut app);
    assert!(app.artist_shows_albums());
    handler(Key::BackTab, &mut app);
    assert!(!app.artist_shows_albums());
  }

  #[test]
  fn o_and_the_queue_key_only_act_on_their_own_tab() {
    let (mut app, rx) = app_on_artist(ArtistTab::TopTracks);
    handler(Key::Char('o'), &mut app);
    assert!(next_event(&rx).is_none(), "o opens releases only");
    let queue_key = app.user_config.keys.add_item_to_queue;
    handler(queue_key, &mut app);
    assert_eq!(app.native_queue.len(), 1);
    assert_eq!(app.native_queue[0].name, "Track 1");

    handler(Key::Tab, &mut app);
    handler(queue_key, &mut app);
    assert_eq!(app.native_queue.len(), 1, "albums are not queued");
  }
}
