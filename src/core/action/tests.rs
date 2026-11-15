//! Tests for `App::apply`.

use super::{Action, NavTarget, RepeatSetting};
use crate::core::app::App;
use crate::core::theme::ThemeField;
use crate::infra::queue::RepeatMode;

#[test]
fn theme_field_names_round_trip() {
  for field in ThemeField::ALL {
    assert_eq!(ThemeField::from_name(field.name()), Some(field));
  }
  assert_eq!(ThemeField::from_name("not_a_field"), None);
}

#[test]
fn set_repeat_with_nothing_playing_leaves_the_mode_alone() {
  let mut app = App::default();
  app.apply(Action::SetRepeat(RepeatSetting::Track));
  assert_eq!(app.decoded_repeat, RepeatMode::Off);
}

#[test]
fn nav_target_names_round_trip() {
  for target in NavTarget::ALL {
    assert_eq!(NavTarget::from_name(target.name()), Some(target));
  }
  assert_eq!(NavTarget::from_name("nowhere"), None);
}

#[test]
fn play_album_asks_for_the_album_by_uri() {
  use crate::infra::network::IoEvent;
  let (tx, rx) = std::sync::mpsc::channel();
  let mut app = App::new_with_state(
    tx,
    crate::core::user_config::UserConfig::new(),
    crate::core::state::RuntimeState::default(),
    None,
    crate::core::i18n::Language::En,
  );
  app.apply(Action::PlayAlbum {
    uri: "qobuz:album:42".into(),
  });
  assert!(matches!(
    rx.try_recv(),
    Ok(IoEvent::PlayQobuzAlbum(uri)) if uri == "qobuz:album:42"
  ));
}
