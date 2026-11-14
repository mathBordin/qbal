use crate::core::action::Action;
use crate::core::app::App;
use crate::tui::event::Key;

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Enter | Key::Esc | Key::Char('q') | Key::Char(' ') => dismiss(app),
    _ => {}
  }
}

/// Dismiss the active announcement, or close a prompt that has none.
pub fn dismiss(app: &mut App) {
  match app.active_announcement.as_ref().map(|a| a.id.clone()) {
    Some(id) => {
      app.apply(Action::DismissAnnouncement { id });
    }
    None => {
      app.pop_navigation_stack();
    }
  }
}

#[cfg(test)]
mod tests {}
