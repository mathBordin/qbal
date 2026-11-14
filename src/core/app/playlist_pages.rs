use super::*;

impl App {
  pub fn set_playlist_tracks_to_table_continuous(&mut self) {
    // A late page must not replace the filtered rows; clearing the filter
    // shows the pages again.
    if self.is_playlist_track_filter_active() {
      return;
    }
    let mut tracks: Vec<TrackInfo> = Vec::new();
    let mut track_ids: Vec<String> = Vec::new();
    let mut positions: Vec<usize> = Vec::new();
    let mut expected_offset = 0;
    let mut seen_offsets = HashSet::new();
    let mut active_index = 0;
    let mut active_page = None;

    for (page_index, page) in self.playlist_track_pages.pages.iter().enumerate() {
      if page.offset != expected_offset || !seen_offsets.insert(page.offset) {
        break;
      }

      for (position, item) in page.items.iter() {
        if let PlayableInfo::Track(track) = item {
          if let Some(id) = track.id.as_ref() {
            track_ids.push(id.clone());
          }
          tracks.push(track.clone());
          positions.push(*position as usize);
        }
      }

      expected_offset = expected_offset.saturating_add(page.limit);
      active_index = page_index;
      active_page = Some(page.clone());

      if page.next.is_none() {
        break;
      }
    }

    self.playlist_track_pages.index = active_index;
    self.playlist_tracks = active_page;
    self.playlist_offset = 0;
    self.replace_track_table_tracks(tracks);
    self.playlist_track_positions = Some(positions);
  }

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

  /// Show a decoded source's search hits as a songs-only result set with the
  /// songs block focused. The album/artist blocks stay empty: their Enter
  /// paths dispatch Spotify-bound events.
  pub(crate) fn show_source_search_tracks(&mut self, _query: &str, tracks: Vec<TrackInfo>) {
    let total = tracks.len() as u32;
    // No cursor clamp here, unlike set_search_results: the hidden blocks keep
    // their cursors for the next Spotify search, as before.
    self.search_results = SearchResult {
      tracks: Some(Paged {
        items: tracks,
        total,
        ..Default::default()
      }),
      ..Default::default()
    };
    self.display_revisions.bump(DisplayDomain::Search);
    self.view.search_selected_tracks_index = Some(0);
    self.view.search_hovered_block = SearchResultBlock::SongSearch;
    self.view.search_selected_block = SearchResultBlock::Empty;
  }

  pub fn is_playlist_track_filter_active(&self) -> bool {
    self.active_playlist_track_filter.is_some()
  }

  pub fn clear_playlist_track_filter(&mut self) {
    self.active_playlist_track_filter = None;
    self.pending_playlist_track_search = None;
    self.view.input_context = InputContext::GlobalSearch;
    if self.playlist_track_pages.pages.is_empty() {
      self.track_table.tracks.clear();
      self.view.track_table_index = 0;
      self.playlist_track_positions = None;
      return;
    }
    // An add or remove may have left the cached pages behind the playlist.
    // Restoring them would show the rows from before that edit, the removed
    // track included, so walk Spotify again from the first page.
    if self.playlist_track_pages.stale {
      self.refetch_stale_playlist_tracks();
      return;
    }
    self.set_playlist_tracks_to_table_continuous();
  }

  /// Rebuild the page cache and the visible table from the first page after an
  /// add or remove left the cached pages stale. Reuses the open-playlist reset,
  /// so the fetch responses rebuild the table through the one existing
  /// `set_playlist_tracks_to_table_continuous` restore path.
  fn refetch_stale_playlist_tracks(&mut self) {
    let Some(_context) = self.track_table.context.take() else {
      return;
    };
    self.set_status_message(self.strings().status.app.loading_playlist, 3);
  }

  pub fn is_playlist_track_table_context(&self) -> bool {
    false
  }

  pub fn next_missing_playlist_tracks_offset_continuous(&self) -> Option<u32> {
    let playlist_tracks_page = self
      .playlist_track_pages
      .get_results(Some(self.playlist_track_pages.index))?;
    playlist_tracks_page.next.as_ref()?;
    Some(playlist_tracks_page.offset + playlist_tracks_page.limit)
  }

  pub fn current_playlist_has_more_tracks(&self) -> bool {
    if self.is_playlist_track_filter_active() {
      return false;
    }

    self
      .playlist_tracks
      .as_ref()
      .is_some_and(|playlist_tracks| playlist_tracks.next.is_some())
  }

  pub fn get_playlist_tracks_next(&mut self) {
    if self.is_playlist_track_filter_active() {
      return;
    }

    if !self.current_playlist_has_more_tracks() {
      return;
    }

    if let Some(next_offset) = self.next_missing_playlist_tracks_offset_continuous() {
      if self
        .playlist_track_pages
        .page_index_for_offset(next_offset)
        .is_some()
      {
        self.set_playlist_tracks_to_table_continuous();
      } else if !self
        .playlist_tracks_prefetch_in_flight
        .contains(&next_offset)
      {
        self.playlist_tracks_prefetch_in_flight.insert(next_offset);
      }
    }
  }
}

#[cfg(test)]
mod tests {}
