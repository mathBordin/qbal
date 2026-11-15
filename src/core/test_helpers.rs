#![cfg(test)]

#[cfg(feature = "tui")]
use crate::core::app::App;

#[cfg(feature = "tui")]
pub fn app_with_sidebar(position: &str) -> App {
  let mut app = App::default();
  app.user_config.behavior.sidebar_position = position.to_string();
  app
}

#[cfg(feature = "tui")]
pub fn app_with_playbar(position: &str) -> App {
  let mut app = App::default();
  app.user_config.behavior.playbar_position = position.to_string();
  app
}

/// A dispatched event in words, for tests under `tui/`, which may not name
/// `IoEvent` variants (the `ioevent_refs_in_tui` gate).
pub fn describe_event(event: &crate::infra::network::IoEvent) -> String {
  use crate::infra::network::IoEvent;
  match event {
    IoEvent::PlayQobuzAlbum(uri) => format!("play album {uri}"),
    IoEvent::GetQobuzTracks(uri) => format!("open {uri}"),
    IoEvent::GetQobuzArtist(_, name) => format!("open artist {name}"),
    IoEvent::StartPlayback(_, Some(uris), offset) => {
      format!("play {} songs from {offset:?}", uris.len())
    }
    _ => "other".to_string(),
  }
}
