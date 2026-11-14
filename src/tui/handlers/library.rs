use super::common_key_events;
use crate::core::app::App;
use crate::core::{action::Action, app::SidebarBlock};
use crate::tui::event::Key;

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::toward_content_event(k, &app.user_config) => {
      common_key_events::focus_content(app)
    }
    k if common_key_events::down_event(k, &app.user_config.keys) => select_next(app),
    k if common_key_events::up_event(k, &app.user_config.keys) => select_previous(app),
    k if common_key_events::high_event(k) => {
      select_row(app, common_key_events::on_high_press_handler());
    }
    k if common_key_events::middle_event(k) => {
      let next_index = common_key_events::on_middle_press_handler(&app.library_rows());
      select_row(app, next_index);
    }
    k if common_key_events::low_event(k) => {
      let next_index = common_key_events::on_low_press_handler(&app.library_rows());
      select_row(app, next_index);
    }
    Key::Enter => activate_selected(app),
    _ => (),
  };
}

/// Moves the highlight to the row at `index` among the rows offered now.
pub(super) fn select_row(app: &mut App, index: usize) {
  if let Some(target) = app.library_rows().get(index) {
    app.view.library_selected = *target;
  }
}

/// The down key's cursor move, named for the mouse wheel.
pub(super) fn select_next(app: &mut App) {
  let index = app.library_cursor();
  let next_index = common_key_events::on_down_press_handler(&app.library_rows(), Some(index));
  select_row(app, next_index);
}

pub(super) fn select_previous(app: &mut App) {
  let index = app.library_cursor();
  let next_index = common_key_events::on_up_press_handler(&app.library_rows(), Some(index));
  select_row(app, next_index);
}

pub(super) fn activate_selected(app: &mut App) {
  app.view.last_sidebar_block = SidebarBlock::Library;
  let Some(target) = app.library_rows().get(app.library_cursor()).copied() else {
    return;
  };

  app.apply(Action::OpenLibrary(target));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn opening_a_library_row_remembers_the_library_panel() {
    let mut app = App::default();
    app.view.last_sidebar_block = SidebarBlock::Playlists;
    activate_selected(&mut app);
    assert_eq!(app.view.last_sidebar_block, SidebarBlock::Library);
  }
}
