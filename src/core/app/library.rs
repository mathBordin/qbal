use super::*;

/// Every sidebar row with what it needs; `App::library_rows` is the filtered list.
pub(crate) fn library_row_requirements() -> &'static [(LibraryTarget, Requirement)] {
  &[(LibraryTarget::Stats, Requirement::None)]
}

impl App {
  // Library display state. Every write goes through a `_mut` accessor, which bumps the Library revision.

  pub(crate) fn library(&self) -> &Library {
    &self.library
  }

  pub(crate) fn library_mut(&mut self) -> &mut Library {
    self.display_revisions.bump(DisplayDomain::Library);
    &mut self.library
  }

  pub(crate) fn all_playlists(&self) -> &Vec<PlaylistInfo> {
    &self.all_playlists
  }

  pub(crate) fn liked_song_ids_set(&self) -> &HashSet<String> {
    &self.liked_song_ids_set
  }

  pub(crate) fn followed_artist_ids_set(&self) -> &HashSet<String> {
    &self.followed_artist_ids_set
  }

  pub(crate) fn saved_album_ids_set(&self) -> &HashSet<String> {
    &self.saved_album_ids_set
  }

  pub(crate) fn saved_show_ids_set(&self) -> &HashSet<String> {
    &self.saved_show_ids_set
  }

  pub(crate) fn user(&self) -> &Option<UserInfo> {
    &self.user
  }
}

impl App {
  /// The sidebar rows the active source and the session can serve.
  pub(crate) fn library_rows(&self) -> Vec<LibraryTarget> {
    library_row_requirements()
      .iter()
      .filter(|(_, needs)| self.availability(*needs).is_available())
      .map(|(target, _)| *target)
      .collect()
  }

  /// Where the selected row sits among the rows offered now; the top row when it is hidden.
  pub(crate) fn library_cursor(&self) -> usize {
    self
      .library_rows()
      .iter()
      .position(|row| *row == self.view.library_selected)
      .unwrap_or(0)
  }
}

#[derive(Clone)]
pub struct Library {
  pub saved_tracks: ScrollableResultPages<Paged<TrackInfo>>,
  pub saved_albums: ScrollableResultPages<Paged<SavedAlbumInfo>>,
  pub saved_shows: ScrollableResultPages<Paged<ShowInfo>>,
  pub saved_artists: ScrollableResultPages<CursorPaged<ArtistInfo>>,
  pub show_episodes: ScrollableResultPages<Paged<EpisodeInfo>>,
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum EpisodeTableContext {
  Full,
}

#[derive(Clone)]
pub struct SelectedFullShow {
  pub show: ShowInfo,
}

impl App {
  /// Sort the recently-played track list in place per `recently_played_sort`.
  /// `Default` keeps the API's play order (a re-fetch restores it).
  pub fn sort_recently_played_items(&mut self) {
    let sort_state = self.recently_played_sort;
    if sort_state.field == SortField::Default {
      return;
    }
    if let Some(page) = self.recently_played.as_mut() {
      use crate::core::sort::sort_by_key_with_order;
      match sort_state.field {
        SortField::Name => {
          sort_by_key_with_order(&mut page.items, sort_state.order, |t| t.name.to_lowercase())
        }
        SortField::Artist => sort_by_key_with_order(&mut page.items, sort_state.order, |t| {
          t.artists.first().map(|s| s.to_lowercase())
        }),
        SortField::Album => sort_by_key_with_order(&mut page.items, sort_state.order, |t| {
          t.album.to_lowercase()
        }),
        _ => {}
      }
    }
  }

  /// Sort one list surface by a field, like the sort menu's Enter.
  pub(crate) fn apply_sort_field(&mut self, context: SortContext, field: SortField) {
    if context == SortContext::SidebarPlaylists && !context.available_fields().contains(&field) {
      return;
    }
    let mut state = self.sort_state(context);
    state.apply_field(field);
    self.set_sort_state(context, state);

    // Actually sort the data
    match context {
      SortContext::SidebarPlaylists => {}
      SortContext::PlaylistTracks => {}
      SortContext::SavedAlbums => self.sort_saved_albums(),
      SortContext::SavedArtists => self.sort_saved_artists(),
      SortContext::RecentlyPlayed => self.sort_recently_played_items(),
    }
  }

  /// Flip the recorded direction without re-sorting loaded rows (the sort
  /// menu's uppercase shortcut, quirk kept).
  pub(crate) fn toggle_sort_order(&mut self, context: SortContext) {
    // The sidebar offers three fixed orders, without descending variants.
    if context == SortContext::SidebarPlaylists {
      return;
    }
    let mut state = self.sort_state(context);
    state.order = state.order.toggle();
    self.set_sort_state(context, state);
  }

  pub(crate) fn sort_state(&self, context: SortContext) -> SortState {
    match context {
      SortContext::SidebarPlaylists => SortState {
        field: self.runtime_state.playlist_sidebar_order.field(),
        order: crate::core::sort::SortOrder::Ascending,
      },
      SortContext::PlaylistTracks => self.playlist_sort,
      SortContext::SavedAlbums => self.album_sort,
      SortContext::SavedArtists => self.artist_sort,
      SortContext::RecentlyPlayed => self.recently_played_sort,
    }
  }

  fn set_sort_state(&mut self, context: SortContext, state: SortState) {
    match context {
      SortContext::SidebarPlaylists => {
        if let Some(_order) = SidebarPlaylistOrder::from_field(state.field) {}
      }
      SortContext::PlaylistTracks => self.playlist_sort = state,
      SortContext::SavedAlbums => self.album_sort = state,
      SortContext::SavedArtists => self.artist_sort = state,
      SortContext::RecentlyPlayed => self.recently_played_sort = state,
    }
  }

  fn sort_saved_albums(&mut self) {
    use crate::core::sort::{sort_by_key_with_order, SortOrder};

    let sort_state = self.album_sort;

    // Sort library.saved_albums pages
    for page in &mut self.library_mut().saved_albums.pages {
      match sort_state.field {
        SortField::Name => sort_by_key_with_order(&mut page.items, sort_state.order, |a| {
          a.album.name.to_lowercase()
        }),
        SortField::Artist => sort_by_key_with_order(&mut page.items, sort_state.order, |a| {
          a.album
            .artists
            .first()
            .map(|artist| artist.name.to_lowercase())
            .unwrap_or_default()
        }),
        SortField::DateAdded => {
          sort_by_key_with_order(&mut page.items, sort_state.order, |a| a.added_at.clone())
        }
        // Spotify dates are `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, so the strings
        // order chronologically; an album without one sorts first.
        SortField::ReleaseDate => sort_by_key_with_order(&mut page.items, sort_state.order, |a| {
          a.album.release_date.clone()
        }),
        // The library arrives newest saved first, whatever order the user
        // picked since, so Default is always descending by `added_at`.
        SortField::Default => sort_by_key_with_order(&mut page.items, SortOrder::Descending, |a| {
          a.added_at.clone()
        }),
        _ => {}
      }
    }
  }

  fn sort_saved_artists(&mut self) {
    use crate::core::sort::sort_by_key_with_order;

    let sort_state = self.artist_sort;
    if sort_state.field != SortField::Name {
      return;
    }

    // Sort library.saved_artists pages
    for page in &mut self.library_mut().saved_artists.pages {
      sort_by_key_with_order(&mut page.items, sort_state.order, |a| a.name.to_lowercase());
    }

    // Also sort the app.artists vec
    sort_by_key_with_order(&mut self.artists, sort_state.order, |a| {
      a.name.to_lowercase()
    });
  }

  pub(crate) fn reload_stats(&mut self) {
    self.stats_loading = true;
    self.display_revisions.bump(DisplayDomain::Stats);
    self.dispatch(IoEvent::LoadListeningStats(self.stats_period));
  }

  pub(crate) fn stats_loading(&self) -> bool {
    self.stats_loading
  }

  /// Cycling periods quickly can race two loads; only the response for the
  /// currently selected period lands.
  pub(crate) fn land_listening_stats(&mut self, period: RecapPeriod, stats: StatsData) {
    if self.stats_period != period {
      return;
    }
    self.stats_data = Some(stats);
    self.stats_loading = false;
    self.display_revisions.bump(DisplayDomain::Stats);
  }

  pub(crate) fn fail_listening_stats(&mut self) {
    self.stats_loading = false;
    self.display_revisions.bump(DisplayDomain::Stats);
  }

  /// The period is written before the fetch: the result handler only accepts
  /// data for the current period.
  pub(crate) fn set_stats_period(&mut self, period: RecapPeriod) {
    self.stats_period = period;
    self.stats_data = None;
    self.reload_stats();
  }

  pub(crate) fn cycle_stats_period(&mut self, forward: bool) {
    let period = if forward {
      self.stats_period.next()
    } else {
      self.stats_period.prev()
    };
    self.set_stats_period(period);
  }

  /// The saved tracks cached from offset 0 up to the first gap, the index of that
  /// run's last page, and whether a page follows it.
  pub(crate) fn saved_tracks_prefix(&self) -> (Vec<TrackInfo>, usize, bool) {
    let mut tracks = Vec::new();
    let mut expected_offset = 0;
    let mut seen_offsets = HashSet::new();
    let mut active_index = 0;
    let mut has_more = false;

    for (page_index, page) in self.library.saved_tracks.pages.iter().enumerate() {
      if page.offset != expected_offset || !seen_offsets.insert(page.offset) {
        break;
      }

      tracks.extend(page.items.iter().cloned());
      expected_offset = expected_offset.saturating_add(page.limit);
      active_index = page_index;
      has_more = page.next.is_some();

      if !has_more {
        break;
      }
    }

    (tracks, active_index, has_more)
  }

  pub fn set_saved_tracks_to_table_continuous(&mut self) {
    if self.active_saved_track_filter.is_some() {
      return;
    }

    let (tracks, active_index, _) = self.saved_tracks_prefix();
    self.library.saved_tracks.index = active_index;
    self.replace_track_table_tracks(tracks);
  }

  /// Open a library sidebar section: the whole Enter consequence of a
  /// library row (fetches, route pushes, bookkeeping), moved verbatim from
  /// the library handler so every frontend fires the same sequence through
  /// `Action::OpenLibrary`.
  pub fn open_library_section(&mut self, target: LibraryTarget) {
    match target {
      LibraryTarget::Stats => {
        self.reload_stats();
        self.push_navigation_stack(RouteId::Stats, ActiveBlock::Stats);
      }
    }
  }

  pub fn clear_saved_track_filter(&mut self) {
    self.active_saved_track_filter = None;
    self.view.input_context = InputContext::GlobalSearch;

    if self.library.saved_tracks.pages.is_empty() {
      self.track_table.tracks.clear();
      self.view.track_table_index = 0;
      return;
    }
    self.set_saved_tracks_to_table_continuous();
  }

  pub fn apply_saved_track_search_results(&mut self, query: String, matches: Vec<TrackInfo>) {
    self.active_saved_track_filter = Some(query);
    self.view.track_table_index = 0;
    self.track_table.tracks = matches;
  }

  pub(crate) fn try_search_saved_tracks_from_cached_pages(&mut self, query: &str) -> bool {
    let terms: Vec<String> = query.split_whitespace().map(|s| s.to_lowercase()).collect();

    let mut matches = Vec::new();
    for page in &self.library.saved_tracks.pages {
      for track in &page.items {
        // If track has an ID and the user unliked it in this session, skip it!
        if let Some(id) = &track.id {
          if !self.liked_song_ids_set().contains(id) {
            continue;
          }
        }

        if playlist_track_info_matches_terms(track, &terms) {
          matches.push(track.clone());
        }
      }
    }

    self.apply_saved_track_search_results(query.to_string(), matches);
    true
  }

  pub fn is_saved_track_filter_active(&self) -> bool {
    false
  }

  pub fn next_missing_saved_tracks_offset_continuous(&self) -> Option<u32> {
    let saved_tracks_page = self
      .library
      .saved_tracks
      .get_results(Some(self.library.saved_tracks.index))?;
    saved_tracks_page.next.as_ref()?;
    Some(saved_tracks_page.offset + saved_tracks_page.limit)
  }

  pub fn current_saved_tracks_has_more_tracks(&self) -> bool {
    self
      .library
      .saved_tracks
      .get_results(Some(self.library.saved_tracks.index))
      .is_some_and(|saved_tracks| saved_tracks.next.is_some())
  }

  /// Check whether the last cached page has a next page available.
  pub fn current_saved_artists_has_more(&self) -> bool {
    self
      .library
      .saved_artists
      .pages
      .last()
      .is_some_and(|page| page.has_next())
  }

  /// Get the `after` cursor for fetching the next page (ID of the last loaded artist).
  pub fn next_saved_artists_cursor(&self) -> Option<String> {
    if !self.current_saved_artists_has_more() {
      return None;
    }
    self
      .library
      .saved_artists
      .pages
      .last()
      .and_then(|page| page.cursor_after.clone())
  }

  pub fn get_current_user_saved_artists_next(&mut self) {
    if !self.current_saved_artists_has_more() {
      return;
    }

    if let Some(cursor) = self.next_saved_artists_cursor() {
      // If this cursor is already fetching, ignore
      if self.saved_artists_prefetch_in_flight.as_deref() == Some(&cursor) {
        return;
      }

      self.saved_artists_prefetch_in_flight = Some(cursor.clone());
    }
  }

  pub fn get_current_user_saved_tracks_next(&mut self) {
    if !self.current_saved_tracks_has_more_tracks() {
      return;
    }

    if let Some(next_offset) = self.next_missing_saved_tracks_offset_continuous() {
      if self
        .library
        .saved_tracks
        .page_index_for_offset(next_offset)
        .is_some()
      {
        self.set_saved_tracks_to_table_continuous();
      } else if !self.saved_tracks_prefetch_in_flight.contains(&next_offset) {
        self.saved_tracks_prefetch_in_flight.insert(next_offset);
      }
    }
  }

  pub fn get_current_user_saved_albums_next(&mut self) {
    let Some(current) = self.library.saved_albums.get_results(None) else {
      return;
    };
    if !current.has_next() {
      return;
    }
    let _next_offset = current.offset + current.limit;
    if self
      .library
      .saved_albums
      .get_results(Some(self.library.saved_albums.index + 1))
      .is_some()
    {
      self.library_mut().saved_albums.index += 1;
    }
    // Back to the top row, where the next page opens.
    self.view.album_list_index = 0;
  }

  pub fn get_current_user_saved_shows_next(&mut self) {
    match self
      .library
      .saved_shows
      .get_results(Some(self.library.saved_shows.index + 1))
      .cloned()
    {
      Some(_) => self.library_mut().saved_shows.index += 1,
      None => {
        if let Some(saved_shows) = &self.library.saved_shows.get_results(None) {
          let _offset = Some(saved_shows.offset + saved_shows.limit);
        }
      }
    }
  }

  /// Next page of the open show's episodes; a no-op when no show is open.
  pub(crate) fn get_episode_table_next(&mut self) {
    let Some(_show_id) = self.selected_episode_show_id() else {
      return;
    };
    match self
      .library
      .show_episodes
      .get_results(Some(self.library.show_episodes.index + 1))
      .cloned()
    {
      Some(_) => self.library_mut().show_episodes.index += 1,
      None => {
        if let Some(show_episodes) = &self.library.show_episodes.get_results(None) {
          let _offset = Some(show_episodes.offset + show_episodes.limit);
        }
      }
    }
  }

  /// Resolve the currently selected show's id/URI (from the episode-table
  /// context). Returns `None` if the stored domain show has no id.
  pub(crate) fn selected_episode_show_id(&self) -> Option<String> {
    match self.episode_table_context {
      EpisodeTableContext::Full => self
        .selected_show_full
        .as_ref()
        .and_then(|s| s.show.id.clone()),
    }
  }
}

#[cfg(test)]
mod tests {}
