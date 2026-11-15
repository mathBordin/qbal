use super::*;
use crate::core::plugin_api::AlbumInfo;

impl App {
  /// Open a decoded source's playlist or folder in the shared track table,
  /// routed by URI scheme.
  pub(crate) fn open_source_playlist_tracks(&mut self, uri: String) {
    let (context, event) = if uri.starts_with("qobuz:") {
      (
        TrackTableContext::QobuzPlaylist,
        IoEvent::GetQobuzTracks(uri),
      )
    } else {
      // Unknown scheme: silent no-op, like the other opening paths.
      return;
    };
    self.dispatch(event);
    self.source_table_uri = None;
    self.show_tracks_in_table(Vec::new(), context);
  }

  pub(super) fn show_tracks_in_table(
    &mut self,
    tracks: Vec<TrackInfo>,
    context: TrackTableContext,
  ) {
    self.set_track_table(tracks, context);
    self.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
  }

  /// Show the rows of the decoded-source list `uri` in the shared track table.
  pub(crate) fn set_source_track_table(
    &mut self,
    uri: &str,
    tracks: Vec<TrackInfo>,
    context: TrackTableContext,
  ) {
    self.set_track_table(tracks, context);
    self.source_table_uri = Some(uri.to_string());
  }

  /// Store a search's songs, albums and artists and put every cursor on its
  /// top row. Focus leaves the lists, and the hover lands on the shown tab.
  pub(crate) fn show_source_search_results(
    &mut self,
    _query: &str,
    tracks: Vec<TrackInfo>,
    albums: Vec<AlbumInfo>,
    artists: Vec<crate::core::plugin_api::ArtistInfo>,
  ) {
    fn paged<T>(items: Vec<T>) -> Option<Paged<T>> {
      let total = items.len() as u32;
      Some(Paged {
        items,
        total,
        ..Default::default()
      })
    }
    // No cursor clamp: the search starts again at the top row.
    self.search_results = SearchResult {
      tracks: paged(tracks),
      albums: paged(albums),
      artists: paged(artists),
      ..Default::default()
    };
    self.view.search_selected_tracks_index = Some(0);
    self.view.search_selected_albums_index = Some(0);
    self.view.search_selected_artists_index = Some(0);
    self.view.search_hovered_block = self.view.search_mode.block();
    self.view.search_selected_block = SearchResultBlock::Empty;
  }

  /// Move the search screen to its next tab (or the previous one). Focus
  /// leaves the list, so the next Enter selects the new one. Choosing songs
  /// or albums also sets the mode artist pages open in.
  pub(crate) fn cycle_search_mode(&mut self, forward: bool) {
    use crate::core::artist::ArtistTab;
    self.view.search_mode = self.view.search_mode.cycled(forward);
    match self.view.search_mode {
      SearchMode::Songs => self.view.artist_tab = ArtistTab::TopTracks,
      SearchMode::Albums => self.view.artist_tab = ArtistTab::Albums,
      SearchMode::Artists => {}
    }
    self.view.search_hovered_block = self.view.search_mode.block();
    self.view.search_selected_block = SearchResultBlock::Empty;
  }

  /// Play the album at `uri` from its first track.
  pub(crate) fn play_album(&mut self, uri: String) {
    self.dispatch(IoEvent::PlayQobuzAlbum(uri));
  }

  pub fn is_playlist_track_table_context(&self) -> bool {
    false
  }
}

#[cfg(test)]
mod tests {}
