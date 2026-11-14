use super::common_key_events;
use super::filter_input::{self, FilterEdit};
use crate::core::app::{ActiveBlock, App, DialogContext};
use crate::tui::event::Key;

pub fn handler(key: Key, app: &mut App) {
  let dialog_context = match app.get_current_route().active_block {
    ActiveBlock::Dialog(context) => context,
    _ => return,
  };

  match dialog_context {
    DialogContext::AddTrackToPlaylistPicker => handle_add_to_playlist_picker(key, app),
    DialogContext::PlaylistSearch | DialogContext::PersistKeybindingFallback => {
      handle_confirmation_dialog(key, app, dialog_context)
    }
  }
}

fn handle_confirmation_dialog(key: Key, app: &mut App, dialog_context: DialogContext) {
  match key {
    Key::Enter => {
      if app.view.confirm {
        match dialog_context {
          DialogContext::PlaylistSearch => handle_playlist_search_dialog(app),
          DialogContext::PersistKeybindingFallback => {
            app.persist_open_settings_fallback();
          }
          DialogContext::AddTrackToPlaylistPicker => {}
        }
      } else if dialog_context == DialogContext::PersistKeybindingFallback {
        app.set_status_message(app.strings().popups.fallback_session_only, 4);
      }
      close_dialog(app);
    }
    Key::Char('q') => {
      if dialog_context == DialogContext::PersistKeybindingFallback {
        app.set_status_message(app.strings().popups.fallback_session_only, 4);
      }
      close_dialog(app);
    }
    k if common_key_events::right_event(k, &app.user_config.keys) => {
      app.view.confirm = !app.view.confirm
    }
    k if common_key_events::left_event(k, &app.user_config.keys) => {
      app.view.confirm = !app.view.confirm
    }
    _ => {}
  }
}

/// Whether the add-to-playlist picker's search must see `key` before the
/// global bindings and the runner's back key claim it.
pub(crate) fn playlist_picker_captures_key(app: &App, key: Key) -> bool {
  app.get_current_route().active_block
    == ActiveBlock::Dialog(DialogContext::AddTrackToPlaylistPicker)
    && (app.view.playlist_picker_filter_editing
      || key == app.user_config.keys.search
      || (key == Key::Esc && !app.view.playlist_picker_filter.is_empty()))
}

fn clear_playlist_picker_filter(app: &mut App) {
  app.view.playlist_picker_filter.clear();
  app.view.playlist_picker_filter_editing = false;
  app.view.playlist_picker_selected_index = 0;
}

fn handle_playlist_picker_filter_input(key: Key, app: &mut App) {
  {}
  match filter_input::apply(key, &mut app.view.playlist_picker_filter) {
    FilterEdit::Cancel => clear_playlist_picker_filter(app),
    FilterEdit::Confirm => {
      if app.view.playlist_picker_filter.trim().is_empty() {
        clear_playlist_picker_filter(app);
      } else {
        app.view.playlist_picker_filter_editing = false;
      }
    }
    FilterEdit::Changed => app.view.playlist_picker_selected_index = 0,
    FilterEdit::Ignored => {}
  }
}

fn handle_add_to_playlist_picker(key: Key, app: &mut App) {
  if app.view.playlist_picker_filter_editing {
    handle_playlist_picker_filter_input(key, app);
    return;
  }
  if key == app.user_config.keys.search {
    app.view.playlist_picker_filter.clear();
    app.view.playlist_picker_filter_editing = true;
    app.view.playlist_picker_selected_index = 0;
    return;
  }
  if key == Key::Esc && !app.view.playlist_picker_filter.is_empty() {
    clear_playlist_picker_filter(app);
    return;
  }

  match key {
    Key::Enter => {}
    Key::Char('q') => {
      close_dialog(app);
    }
    _ => {}
  }
}

fn handle_playlist_search_dialog(app: &mut App) {
  if let Some(_playlist_id) = app.selected_search_result_playlist_id() {}
}

fn close_dialog(app: &mut App) {
  app.pop_navigation_stack();
  app.clear_dialog_state();
}

#[cfg(test)]
mod tests {}
