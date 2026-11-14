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

  pub(crate) fn all_playlists(&self) -> &Vec<PlaylistInfo> {
    &self.all_playlists
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
}

impl App {
  /// Sort one list surface by a field, like the sort menu's Enter.
  pub(crate) fn apply_sort_field(&mut self, context: SortContext, field: SortField) {
    let mut state = self.sort_state(context);
    state.apply_field(field);
    self.set_sort_state(context, state);
  }

  /// Flip the direction of the current sort (the sort menu's uppercase shortcut).
  pub(crate) fn toggle_sort_order(&mut self, context: SortContext) {
    let mut state = self.sort_state(context);
    state.order = state.order.toggle();
    self.set_sort_state(context, state);
  }

  pub(crate) fn sort_state(&self, context: SortContext) -> SortState {
    match context {
      SortContext::PlaylistTracks => self.playlist_sort,
    }
  }

  fn set_sort_state(&mut self, context: SortContext, state: SortState) {
    match context {
      SortContext::PlaylistTracks => {
        self.playlist_sort = state;
        self.resort_track_table();
      }
    }
  }

  pub(crate) fn reload_stats(&mut self) {
    self.stats_loading = true;
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
  }

  pub(crate) fn fail_listening_stats(&mut self) {
    self.stats_loading = false;
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
}

#[cfg(test)]
mod tests {}
