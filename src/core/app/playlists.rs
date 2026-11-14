use super::*;

#[derive(Clone)]
pub struct PendingPlaylistTrackAdd {
  pub track_name: String,
}

#[derive(Clone)]
pub struct PendingPlaylistTrackRemoval {}

impl App {
  // The per-source sidebar lists, part of the Library display state.

  pub(crate) fn qobuz_playlists(&self) -> &Vec<PlaylistInfo> {
    &self.qobuz_playlists
  }

  pub(crate) fn qobuz_playlists_mut(&mut self) -> &mut Vec<PlaylistInfo> {
    self.display_revisions.bump(DisplayDomain::Library);
    &mut self.qobuz_playlists
  }
}

impl App {
  pub fn clear_playlist_track_dialog_state(&mut self) {
    self.pending_playlist_track_add = None;
    self.pending_playlist_track_removal = None;
    self.view.playlist_picker_selected_index = 0;
    self.view.playlist_picker_folder_id = 0;
    self.view.playlist_picker_filter.clear();
    self.view.playlist_picker_filter_editing = false;
  }

  pub fn clear_dialog_state(&mut self) {
    self.view.dialog = None;
    self.view.confirm = false;
    self.pending_keybinding_persist = None;
    self.pending_playlist_sync_remove = None;
    self.view.playlist_sync_picker_index = 0;
    self.clear_playlist_track_dialog_state();
  }

  /// The highlighted search-result playlist's Spotify id.
  pub fn selected_search_result_playlist_id(&self) -> Option<String> {
    let playlists = self.search_results.playlists.as_ref()?;
    let selected_index = self.view.search_selected_playlists_index?;
    playlists.items.get(selected_index)?.id.clone()
  }
}

pub fn playlist_track_info_matches_terms(track: &TrackInfo, terms: &[String]) -> bool {
  let haystack =
    format!("{} {} {}", track.name, track.album, track.artists.join(" "),).to_lowercase();
  terms.iter().all(|term| haystack.contains(term))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn playlist_track_matcher_matches_case_insensitively_across_fields() {
    let track = TrackInfo {
      generated_name: None,
      name: "Never Gonna Give You Up".into(),
      artists: vec!["Rick Astley".into()],
      album: "Whenever You Need Somebody".into(),
      duration_ms: 213_000,
      id: Some("rick123".into()),
      uri: None,
      album_id: None,
      artist_refs: vec![],
      is_playable: true,
      is_local: false,
      track_number: 1,
      explicit: false,
      image_url: None,
      release_date: None,
    };

    // Match on track title
    let terms = vec!["never".to_string(), "gonna".to_string()];
    assert!(playlist_track_info_matches_terms(&track, &terms));

    // Match across artist and album
    let terms = vec!["astley".to_string(), "whenever".to_string()];
    assert!(playlist_track_info_matches_terms(&track, &terms));

    // Miss on nonexistent term
    let terms = vec!["astley".to_string(), "metallica".to_string()];
    assert!(!playlist_track_info_matches_terms(&track, &terms));
  }
}
