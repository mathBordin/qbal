//! Sidebar labels and messages shared by frontends.

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct SidebarStrings {
  pub library: &'static str,
  pub discover: &'static str,
  pub recently_played: &'static str,
  pub stats: &'static str,
  pub playlist_sync: &'static str,
  pub liked_songs: &'static str,
  pub albums: &'static str,
  pub artists: &'static str,
  pub podcasts: &'static str,
  pub radio_stations: &'static str,
  pub youtube_playlists: &'static str,
  pub unknown: &'static str,
  pub add_playlist: &'static str,
  pub playlists: &'static str,
  pub favorite_tracks: &'static str,
  pub spotify: &'static str,
  pub subsonic: &'static str,
  pub youtube: &'static str,
  pub qobuz: &'static str,
  pub apple_music: &'static str,
}

pub const EN: SidebarStrings = SidebarStrings {
  library: "Library",
  discover: "Discover",
  recently_played: "Recently Played",
  stats: "Stats",
  playlist_sync: "Playlist sync",
  liked_songs: "Liked Songs",
  albums: "Albums",
  artists: "Artists",
  podcasts: "Podcasts",
  radio_stations: "Radio Stations",
  youtube_playlists: "YouTube Playlists",
  unknown: "Unknown",
  add_playlist: "+ Add Playlist",
  playlists: "Playlists",
  favorite_tracks: "Favorite tracks",
  spotify: "Spotify",
  subsonic: "Subsonic",
  youtube: "YouTube",
  qobuz: "Qobuz",
  apple_music: "Apple Music",
};

pub const PT_BR: SidebarStrings = SidebarStrings {
  library: "Biblioteca",
  discover: "Descobrir",
  recently_played: "Tocadas recentemente",
  stats: "Estatísticas",
  playlist_sync: "Sincronizar playlists",
  liked_songs: "Músicas curtidas",
  albums: "Álbuns",
  artists: "Artistas",
  podcasts: "Podcasts",
  radio_stations: "Rádios",
  youtube_playlists: "Playlists do YouTube",
  unknown: "Desconhecido",
  add_playlist: "+ Adicionar playlist",
  playlists: "Playlists",
  favorite_tracks: "Faixas favoritas",
  spotify: "Spotify",
  subsonic: "Subsonic",
  youtube: "YouTube",
  qobuz: "Qobuz",
  apple_music: "Apple Music",
};
pub const FR: SidebarStrings = SidebarStrings {
  library: "Bibliothèque",
  discover: "Découvrir",
  recently_played: "Écoutés récemment",
  stats: "Statistiques",
  playlist_sync: "Synchronisation des playlists",
  liked_songs: "Titres likés",
  albums: "Albums",
  artists: "Artistes",
  podcasts: "Podcasts",
  radio_stations: "Stations de radio",
  youtube_playlists: "Playlists YouTube",
  unknown: "Inconnu",
  add_playlist: "+ Ajouter une playlist",
  playlists: "Playlists",
  favorite_tracks: "Titres favoris",
  spotify: "Spotify",
  subsonic: "Subsonic",
  youtube: "YouTube",
  qobuz: "Qobuz",
  apple_music: "Apple Music",
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
