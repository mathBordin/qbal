//! Sidebar labels and messages shared by frontends.

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct SidebarStrings {
  pub library: &'static str,
  pub stats: &'static str,
  pub artists: &'static str,
  pub playlists: &'static str,
  pub favorite_tracks: &'static str,
  pub qobuz: &'static str,
}

pub const EN: SidebarStrings = SidebarStrings {
  library: "Library",
  stats: "Stats",
  artists: "Artists",
  playlists: "Playlists",
  favorite_tracks: "Favorite tracks",
  qobuz: "Qobuz",
};

pub const PT_BR: SidebarStrings = SidebarStrings {
  library: "Biblioteca",
  stats: "Estatísticas",
  artists: "Artistas",
  playlists: "Playlists",
  favorite_tracks: "Faixas favoritas",
  qobuz: "Qobuz",
};
pub const FR: SidebarStrings = SidebarStrings {
  library: "Bibliothèque",
  stats: "Statistiques",
  artists: "Artistes",
  playlists: "Playlists",
  favorite_tracks: "Titres favoris",
  qobuz: "Qobuz",
};

impl SidebarStrings {
  /// Resolve only reserved synthetic rows; a user playlist with the same name
  /// remains untouched. URI literals also work in builds without that source.
  pub fn playlist_label<'a>(
    &self,
    source: crate::core::source::Source,
    uri: &str,
    name: &'a str,
  ) -> &'a str {
    use crate::core::source::Source;
    match source {
      Source::Qobuz if uri == "qobuz:favorites:tracks" => self.favorite_tracks,
      Source::Qobuz => name,
    }
  }

  pub fn source_label(&self, source: crate::core::source::Source) -> &'static str {
    use crate::core::source::Source;
    match source {
      Source::Qobuz => self.qobuz,
    }
  }
}
