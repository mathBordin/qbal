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

  /// Show the search hits as a songs-only result set with the songs block
  /// focused.
  /// Store a search's songs and albums and put both cursors on their top rows.
  /// Focus leaves the lists, and the hover lands on the one the mode shows.
  pub(crate) fn show_source_search_results(
    &mut self,
    _query: &str,
    tracks: Vec<TrackInfo>,
    albums: Vec<AlbumInfo>,
  ) {
    let track_total = tracks.len() as u32;
    let album_total = albums.len() as u32;
    // No cursor clamp: the search starts again at the top row.
    self.search_results = SearchResult {
      tracks: Some(Paged {
        items: tracks,
        total: track_total,
        ..Default::default()
      }),
      albums: Some(Paged {
        items: albums,
        total: album_total,
        ..Default::default()
      }),
      ..Default::default()
    };
    self.view.search_selected_tracks_index = Some(0);
    self.view.search_selected_albums_index = Some(0);
    self.view.search_hovered_block = self.view.search_mode.block();
    self.view.search_selected_block = SearchResultBlock::Empty;
  }

  /// Switch the search screen between songs and albums. Focus leaves the list,
  /// so the next Enter selects the new one.
  pub(crate) fn toggle_search_mode(&mut self) {
    self.view.search_mode = self.view.search_mode.toggled();
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
