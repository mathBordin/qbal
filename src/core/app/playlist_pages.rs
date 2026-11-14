use super::*;

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
  pub(crate) fn show_source_search_tracks(&mut self, _query: &str, tracks: Vec<TrackInfo>) {
    let total = tracks.len() as u32;
    // No cursor clamp: the search starts again at the top row.
    self.search_results = SearchResult {
      tracks: Some(Paged {
        items: tracks,
        total,
        ..Default::default()
      }),
      ..Default::default()
    };
    self.view.search_selected_tracks_index = Some(0);
    self.view.search_hovered_block = SearchResultBlock::SongSearch;
    self.view.search_selected_block = SearchResultBlock::Empty;
  }

  pub fn is_playlist_track_table_context(&self) -> bool {
    false
  }
}

#[cfg(test)]
mod tests {}
