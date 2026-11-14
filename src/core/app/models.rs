use super::*;

/// State backing the monthly recap popup.
#[derive(Clone, Debug)]
pub struct RecapPromptState {
  pub path: PathBuf,
  pub listens: usize,
}

// Is it possible to compose enums?
#[derive(PartialEq, Debug)]
pub enum TrackTableContext {
  QobuzPlaylist,
}

/// The five search result pages. Their cursors and focus are `App.view`'s
/// `search_*` fields.
#[derive(Default)]
pub struct SearchResult {
  pub albums: Option<crate::core::pagination::Paged<crate::core::plugin_api::AlbumInfo>>,
  pub artists: Option<crate::core::pagination::Paged<crate::core::plugin_api::ArtistInfo>>,
  pub playlists: Option<crate::core::pagination::Paged<crate::core::plugin_api::PlaylistInfo>>,
  pub tracks: Option<crate::core::pagination::Paged<crate::core::plugin_api::TrackInfo>>,
  pub shows: Option<crate::core::pagination::Paged<crate::core::plugin_api::ShowInfo>>,
}

/// The shared track table's rows and what they are. Its cursor is
/// `App.view.track_table_index`, its scroll offset `view.track_table_scroll_offset`.
#[derive(Default)]
pub struct TrackTable {
  pub tracks: Vec<TrackInfo>,
  pub context: Option<TrackTableContext>,
}

impl App {
  /// Show `tracks` in the shared table with the cursor on the top row.
  /// The rows are sorted by `playlist_sort`; the arrival order is kept for `Default`.
  pub(crate) fn set_track_table(&mut self, tracks: Vec<TrackInfo>, context: TrackTableContext) {
    self.track_table_epoch = self.track_table_epoch.wrapping_add(1);
    self.track_table_unsorted = tracks;
    self.track_table.context = Some(context);
    self.view.track_table_index = 0;
    let mut sorted = self.track_table_unsorted.clone();
    crate::core::sort::sort_tracks(&mut sorted, self.playlist_sort);
    self.track_table.tracks = sorted;
  }

  /// Re-sort the track table after the sort changed, keeping the cursor on its track.
  pub(crate) fn resort_track_table(&mut self) {
    if self.track_table.context.is_none() {
      return;
    }
    let selected = self
      .track_table
      .tracks
      .get(self.view.track_table_index)
      .and_then(|track| track.uri.clone());
    let mut sorted = self.track_table_unsorted.clone();
    crate::core::sort::sort_tracks(&mut sorted, self.playlist_sort);
    self.track_table.tracks = sorted;
    if let Some(index) = selected.and_then(|uri| {
      self
        .track_table
        .tracks
        .iter()
        .position(|track| track.uri.as_deref() == Some(uri.as_str()))
    }) {
      self.view.track_table_index = index;
    }
  }

  pub(crate) fn search_results(&self) -> &SearchResult {
    &self.search_results
  }
}
