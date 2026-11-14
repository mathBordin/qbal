use super::*;

impl App {
  // The per-source sidebar lists, part of the Library display state.

  pub(crate) fn qobuz_playlists(&self) -> &Vec<PlaylistInfo> {
    &self.qobuz_playlists
  }

  pub(crate) fn qobuz_playlists_mut(&mut self) -> &mut Vec<PlaylistInfo> {
    &mut self.qobuz_playlists
  }
}
