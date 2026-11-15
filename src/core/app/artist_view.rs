//! The artist page and the playing album's details.

use super::*;
use crate::core::artist::{
  discography_rows, first_release_row, AlbumDetails, ArtistPage, ArtistTab,
};

impl App {
  /// Open an artist's page: show it loading at once, and fetch the popular
  /// tracks and the discography. It opens on the tab of the mode last chosen
  /// on the search screen.
  pub(crate) fn open_artist(&mut self, uri: String, name: String) {
    if !uri.starts_with("qobuz:artist:") {
      return;
    }
    self.artist_page = Some(ArtistPage {
      uri: uri.clone(),
      name: name.clone(),
      loading: true,
      ..ArtistPage::default()
    });
    self.view.artist_track_index = 0;
    self.view.artist_release_row = 0;
    self.view.artist_tracks_scroll_offset.set(0);
    self.view.artist_releases_scroll_offset.set(0);
    self.push_navigation_stack(RouteId::Artist, ActiveBlock::Artist);
    self.set_current_route_state(Some(ActiveBlock::Artist), Some(ActiveBlock::Artist));
    self.dispatch(IoEvent::GetQobuzArtist(uri, name));
  }

  /// Store a fetched artist page, unless another artist was opened meanwhile.
  pub(crate) fn show_artist_page(&mut self, page: ArtistPage) {
    if self.artist_page.as_ref().map(|p| p.uri.as_str()) != Some(page.uri.as_str()) {
      return;
    }
    self.view.artist_track_index = 0;
    self.view.artist_release_row = first_release_row(&discography_rows(&page.releases));
    self.artist_page = Some(page);
  }

  /// The fetch for the artist at `uri` failed: stop showing it as loading.
  pub(crate) fn artist_page_failed(&mut self, uri: &str) {
    if let Some(page) = self.artist_page.as_mut().filter(|p| p.uri == uri) {
      page.loading = false;
    }
  }

  /// The open artist page.
  pub fn artist_page(&self) -> Option<&ArtistPage> {
    self.artist_page.as_ref()
  }

  /// Switch the artist page between popular tracks and albums.
  pub(crate) fn toggle_artist_tab(&mut self) {
    self.view.artist_tab = self.view.artist_tab.toggled();
  }

  /// Whether the artist page shows its albums.
  pub fn artist_shows_albums(&self) -> bool {
    self.view.artist_tab == ArtistTab::Albums
  }

  /// Ask for the details of the album `album_id`, once per album.
  pub(crate) fn request_album_details(&mut self, album_id: &str) {
    if self.album_details.as_ref().map(|d| d.album_id.as_str()) == Some(album_id) {
      return;
    }
    // A placeholder stops the next tick from asking again while the fetch runs.
    self.album_details = Some(AlbumDetails {
      album_id: album_id.to_string(),
      ..AlbumDetails::default()
    });
    self.dispatch_without_spinner(IoEvent::GetQobuzAlbumDetails(album_id.to_string()));
  }

  /// Store fetched album details, unless the album changed meanwhile.
  pub(crate) fn set_album_details(&mut self, details: AlbumDetails) {
    if self.album_details.as_ref().map(|d| d.album_id.as_str()) == Some(details.album_id.as_str()) {
      self.album_details = Some(details);
    }
  }

  /// The playing album's details, once fetched (the placeholder has no tracks).
  pub fn album_details(&self) -> Option<&AlbumDetails> {
    self.album_details.as_ref().filter(|d| !d.tracks.is_empty())
  }

  /// The track playing now: the native queue slot's, else the Qobuz session's.
  pub fn playing_track(&self) -> Option<&TrackInfo> {
    if let Some(crate::infra::queue::QueueNowPlaying::Decoded(d)) = self.queue_now.as_ref() {
      return Some(&d.track);
    }
    self.qobuz_playback.as_ref()?.current()
  }

  /// Open the playing track's album in the track table.
  pub(crate) fn go_to_playing_album(&mut self) {
    let album = self
      .playing_track()
      .and_then(|track| track.album_id.clone());
    match album {
      Some(id) => self.open_source_playlist_tracks(format!("qobuz:album:{id}")),
      None => self.report_nothing_playing(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::artist::Release;

  fn app_with_events() -> (App, std::sync::mpsc::Receiver<IoEvent>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let app = App::new_with_state(
      tx,
      crate::core::user_config::UserConfig::new(),
      crate::core::state::RuntimeState::default(),
      None,
      crate::core::i18n::Language::En,
    );
    (app, rx)
  }

  #[test]
  fn opening_an_artist_shows_it_loading_and_fetches_it() {
    let (mut app, rx) = app_with_events();
    app.open_artist("qobuz:artist:1".into(), "Daft Punk".into());
    assert_eq!(app.get_current_route().id, RouteId::Artist);
    assert!(app.artist_page().unwrap().loading);
    assert!(matches!(
      rx.try_recv(),
      Ok(IoEvent::GetQobuzArtist(uri, name)) if uri == "qobuz:artist:1" && name == "Daft Punk"
    ));
  }

  #[test]
  fn a_late_page_for_another_artist_is_dropped() {
    let (mut app, _rx) = app_with_events();
    app.open_artist("qobuz:artist:2".into(), "Justice".into());
    app.show_artist_page(ArtistPage {
      uri: "qobuz:artist:1".into(),
      name: "Daft Punk".into(),
      ..ArtistPage::default()
    });
    assert_eq!(app.artist_page().unwrap().name, "Justice");

    let release = Release {
      uri: "qobuz:album:a".into(),
      title: "Cross".into(),
      version: None,
      release_date: None,
      tracks_count: 12,
      kind: crate::core::artist::ReleaseKind::Album,
      bit_depth: None,
      sample_rate_khz: None,
    };
    app.show_artist_page(ArtistPage {
      uri: "qobuz:artist:2".into(),
      name: "Justice".into(),
      releases: vec![release],
      ..ArtistPage::default()
    });
    let page = app.artist_page().unwrap();
    assert!(!page.loading);
    assert_eq!(
      app.view.artist_release_row, 1,
      "the cursor starts below the heading"
    );
  }

  #[test]
  fn album_details_are_asked_once_per_album_and_kept_only_for_it() {
    let (mut app, rx) = app_with_events();
    app.request_album_details("a1");
    app.request_album_details("a1");
    assert!(matches!(rx.try_recv(), Ok(IoEvent::GetQobuzAlbumDetails(id)) if id == "a1"));
    assert!(rx.try_recv().is_err(), "asked once");
    assert!(
      app.album_details().is_none(),
      "nothing to show until it arrives"
    );

    app.set_album_details(AlbumDetails {
      album_id: "zz".into(),
      tracks: vec![("t".into(), 1)],
      ..AlbumDetails::default()
    });
    assert!(
      app.album_details().is_none(),
      "another album's details are dropped"
    );
    app.set_album_details(AlbumDetails {
      album_id: "a1".into(),
      tracks: vec![("t".into(), 1)],
      ..AlbumDetails::default()
    });
    assert_eq!(app.album_details().unwrap().album_id, "a1");
  }
}
