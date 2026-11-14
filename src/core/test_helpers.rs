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
