use super::*;

/// Minimal source-agnostic snapshot of the signed-in user, used for playlist
/// ownership checks and market/country resolution. Holds only string fields so
/// no `rspotify::model` type leaks into [`App`] state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserInfo {
  pub id: String,
  pub display_name: Option<String>,
  /// ISO 3166-1 alpha-2 country code (e.g. `"US"`), when known.
  pub country: Option<String>,
}

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

/// The track table as the TrackTable revision last counted it.
#[derive(Default)]
pub struct TrackTableView {
  /// The list the rows belong to; `None` while it loads, or for a table that is not a playlist.
  pub uri: Option<String>,
  pub tracks: Vec<TrackInfo>,
  pub has_more: bool,
  /// Each row's position in its Spotify playlist, for a remove; `None` for any other table.
  pub positions: Option<Vec<usize>>,
  /// The seed track id when the rows are that track's recommendations.
  pub radio: Option<String>,
}

impl App {
  pub fn replace_track_table_tracks(&mut self, tracks: Vec<TrackInfo>) {
    self.track_table_epoch = self.track_table_epoch.wrapping_add(1);
    self.playlist_track_positions = None;

    let track_count = tracks.len();
    self.track_table.tracks = tracks;
    if track_count > 0 {
      self.clamp_track_table_cursor();
    } else {
      self.view.track_table_index = 0;
    }
  }

  /// Show `tracks` in the shared table with the cursor on the top row.
  pub(crate) fn set_track_table(&mut self, tracks: Vec<TrackInfo>, context: TrackTableContext) {
    // Another list replaced the Music one: its later pages must not land here.
    {}
    self.track_table_epoch = self.track_table_epoch.wrapping_add(1);
    self.track_table.tracks = tracks;
    self.track_table.context = Some(context);
    self.view.track_table_index = 0;
  }

  /// Keep the track table cursor inside the rows the table holds now.
  pub(crate) fn clamp_track_table_cursor(&mut self) {
    let max_index = self.track_table.tracks.len().saturating_sub(1);
    if self.view.track_table_index > max_index {
      self.view.track_table_index = max_index;
    }
  }

  /// The Spotify track ids among the search results that are in Liked Songs.
  pub(crate) fn search_liked_ids(&self) -> Vec<String> {
    self
      .search_results
      .tracks
      .iter()
      .flat_map(|page| &page.items)
      .filter(|track| track.uri.as_deref().is_some_and(|_uri| false))
      .filter_map(|track| track.id.clone())
      .filter(|id| self.liked_song_ids_set.contains(id))
      .collect()
  }

  /// Bump Search when the liked marks of its results change; they land after the results.
  pub(super) fn note_search_liked_changes(&mut self) {
    let liked = self.search_liked_ids();
    if liked != self.search_liked_view {
      self.search_liked_view = liked;
      self.display_revisions.bump(DisplayDomain::Search);
    }
  }

  /// The playlist the track table holds, once its rows landed.
  fn track_table_uri(&self) -> Option<String> {
    None
  }

  /// Bump TrackTable when its rows or their list change; many producers write the table.
  pub(super) fn note_track_table_changes(&mut self) {
    let uri = self.track_table_uri();
    let has_more = self.track_table_has_more_rows();
    let positions = None;
    let radio = None;
    let view = &self.track_table_view;
    if view.uri == uri
      && view.has_more == has_more
      && view.positions == positions
      && view.radio == radio
      && view.tracks == self.track_table.tracks
    {
      return;
    }
    self.track_table_view = TrackTableView {
      uri,
      tracks: self.track_table.tracks.clone(),
      has_more,
      positions,
      radio,
    };
    self.display_revisions.bump(DisplayDomain::TrackTable);
  }

  pub(crate) fn search_results(&self) -> &SearchResult {
    &self.search_results
  }

  fn track_table_has_more_rows(&self) -> bool {
    false
  }
}

#[cfg(test)]
mod tests {}
