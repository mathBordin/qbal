extern crate unicode_width;

use crate::core::action::Action;
use crate::core::app::{ActiveBlock, App, RouteId};
use crate::tui::event::Key;
use std::convert::TryInto;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Handle event when the search input block is active
pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Tab => app.cycle_search_mode(true),
    Key::BackTab => app.cycle_search_mode(false),
    Key::Ctrl('k') => {
      app
        .view
        .input
        .drain(app.view.input_idx..app.view.input.len());
    }
    Key::Ctrl('u') => {
      app.view.input.drain(..app.view.input_idx);
      app.view.input_idx = 0;
      app.view.input_cursor_position = 0;
    }
    Key::Ctrl('l') => {
      app.view.input = vec![];
      app.view.input_idx = 0;
      app.view.input_cursor_position = 0;
    }
    Key::Ctrl('w') => {
      if app.view.input_cursor_position == 0 {
        return;
      }
      let word_end = match app.view.input[..app.view.input_idx]
        .iter()
        .rposition(|&x| x != ' ')
      {
        Some(index) => index + 1,
        None => 0,
      };
      let word_start = match app.view.input[..word_end].iter().rposition(|&x| x == ' ') {
        Some(index) => index + 1,
        None => 0,
      };
      let deleted: String = app.view.input[word_start..app.view.input_idx]
        .iter()
        .collect();
      let deleted_len: u16 = UnicodeWidthStr::width(deleted.as_str()).try_into().unwrap();
      app.view.input.drain(word_start..app.view.input_idx);
      app.view.input_idx = word_start;
      app.view.input_cursor_position -= deleted_len;
    }
    Key::End | Key::Ctrl('e') => {
      app.view.input_idx = app.view.input.len();
      let input_string: String = app.view.input.iter().collect();
      app.view.input_cursor_position = UnicodeWidthStr::width(input_string.as_str())
        .try_into()
        .unwrap();
    }
    Key::Home | Key::Ctrl('a') => {
      app.view.input_idx = 0;
      app.view.input_cursor_position = 0;
    }
    Key::Left | Key::Ctrl('b') if !app.view.input.is_empty() && app.view.input_idx > 0 => {
      let last_c = app.view.input[app.view.input_idx - 1];
      app.view.input_idx -= 1;
      app.view.input_cursor_position -= compute_character_width(last_c);
    }
    Key::Right | Key::Ctrl('f') if app.view.input_idx < app.view.input.len() => {
      let next_c = app.view.input[app.view.input_idx];
      app.view.input_idx += 1;
      app.view.input_cursor_position += compute_character_width(next_c);
    }
    Key::Esc => {
      app.set_current_route_state(Some(ActiveBlock::Empty), Some(ActiveBlock::Library));
    }
    Key::Enter => {
      let input_str: String = app.view.input.iter().collect();

      process_input(app, input_str);
    }
    Key::Char(c) => {
      app.view.input.insert(app.view.input_idx, c);
      app.view.input_idx += 1;
      app.view.input_cursor_position += compute_character_width(c);
    }
    Key::Backspace | Key::Ctrl('h') if !app.view.input.is_empty() && app.view.input_idx > 0 => {
      let last_c = app.view.input.remove(app.view.input_idx - 1);
      app.view.input_idx -= 1;
      app.view.input_cursor_position -= compute_character_width(last_c);
    }
    Key::Delete | Key::Ctrl('d')
      if !app.view.input.is_empty() && app.view.input_idx < app.view.input.len() =>
    {
      app.view.input.remove(app.view.input_idx);
    }
    _ => {}
  }
}

fn process_input(app: &mut App, input: String) {
  // Don't do anything if there is no input
  if input.is_empty() {
    return;
  }

  // On searching for a track, clear the playlist selection
  app.view.selected_playlist_index = Some(0);

  // Default fallback behavior: treat the input as a raw search phrase, routed
  // to the active source's catalog.
  app.apply(Action::SearchActiveSource(input));
  app.push_navigation_stack(RouteId::Search, ActiveBlock::SearchResultBlock);
  // push_navigation_stack is a no-op when the Search route is already on top, which
  // otherwise leaves focus trapped in the input box. Force focus onto the results so
  // submitting a search always escapes the search box (#191).
  app.set_current_route_state(
    Some(ActiveBlock::SearchResultBlock),
    Some(ActiveBlock::SearchResultBlock),
  );
}

/// A character's display width; control characters (which a paste can carry)
/// have none and take no columns.
fn compute_character_width(character: char) -> u16 {
  UnicodeWidthChar::width(character)
    .and_then(|width| u16::try_from(width).ok())
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
  #[test]
  fn a_control_character_has_no_width_instead_of_crashing() {
    assert_eq!(super::compute_character_width('\u{7}'), 0);
    assert_eq!(super::compute_character_width('a'), 1);
    assert_eq!(super::compute_character_width('\u{1F3B5}'), 2);
  }

  use super::*;

  fn str_to_vec_char(s: &str) -> Vec<char> {
    String::from(s).chars().collect()
  }

  #[test]
  fn test_compute_character_width_with_multiple_characters() {
    assert_eq!(1, compute_character_width('a'));
    assert_eq!(1, compute_character_width('ß'));
    assert_eq!(1, compute_character_width('ç'));
  }

  #[test]
  fn test_input_handler_clear_input_on_ctrl_l() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");

    handler(Key::Ctrl('l'), &mut app);

    assert_eq!(app.view.input, str_to_vec_char(""));
  }

  #[test]
  fn test_input_handler_ctrl_u() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");

    handler(Key::Ctrl('u'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("My text"));

    app.view.input_cursor_position = 3;
    app.view.input_idx = 3;
    handler(Key::Ctrl('u'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("text"));
  }

  #[test]
  fn test_input_handler_ctrl_k() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");

    handler(Key::Ctrl('k'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char(""));

    app.view.input = str_to_vec_char("My text");
    app.view.input_cursor_position = 2;
    app.view.input_idx = 2;
    handler(Key::Ctrl('k'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("My"));

    handler(Key::Ctrl('k'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("My"));
  }

  #[test]
  fn test_input_handler_ctrl_w() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");

    handler(Key::Ctrl('w'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("My text"));

    app.view.input_cursor_position = 3;
    app.view.input_idx = 3;
    handler(Key::Ctrl('w'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("text"));
    assert_eq!(app.view.input_cursor_position, 0);
    assert_eq!(app.view.input_idx, 0);

    app.view.input = str_to_vec_char("    ");
    app.view.input_cursor_position = 3;
    app.view.input_idx = 3;
    handler(Key::Ctrl('w'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char(" "));
    assert_eq!(app.view.input_cursor_position, 0);
    assert_eq!(app.view.input_idx, 0);
    app.view.input_cursor_position = 1;
    app.view.input_idx = 1;
    handler(Key::Ctrl('w'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char(""));
    assert_eq!(app.view.input_cursor_position, 0);
    assert_eq!(app.view.input_idx, 0);

    app.view.input = str_to_vec_char("Hello there  ");
    app.view.input_cursor_position = 13;
    app.view.input_idx = 13;
    handler(Key::Ctrl('w'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("Hello "));
    assert_eq!(app.view.input_cursor_position, 6);
    assert_eq!(app.view.input_idx, 6);
  }

  #[test]
  fn test_input_handler_esc_back_to_playlist() {
    let mut app = App::default();

    app.set_current_route_state(Some(ActiveBlock::MyPlaylists), None);
    handler(Key::Esc, &mut app);

    let current_route = app.get_current_route();
    assert_eq!(current_route.active_block, ActiveBlock::Empty);
  }

  #[test]
  fn test_input_handler_on_enter_text() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My tex");
    app.view.input_cursor_position = app.view.input.len().try_into().unwrap();
    app.view.input_idx = app.view.input.len();

    handler(Key::Char('t'), &mut app);

    assert_eq!(app.view.input, str_to_vec_char("My text"));
  }

  #[test]
  fn test_input_handler_backspace() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");
    app.view.input_cursor_position = app.view.input.len().try_into().unwrap();
    app.view.input_idx = app.view.input.len();

    handler(Key::Backspace, &mut app);
    assert_eq!(app.view.input, str_to_vec_char("My tex"));

    // Test that backspace deletes from the cursor position
    app.view.input_idx = 2;
    app.view.input_cursor_position = 2;

    handler(Key::Backspace, &mut app);
    assert_eq!(app.view.input, str_to_vec_char("M tex"));

    app.view.input_idx = 1;
    app.view.input_cursor_position = 1;

    handler(Key::Ctrl('h'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char(" tex"));
  }

  #[test]
  fn test_input_handler_delete() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");
    app.view.input_idx = 3;
    app.view.input_cursor_position = 3;

    handler(Key::Delete, &mut app);
    assert_eq!(app.view.input, str_to_vec_char("My ext"));

    app.view.input = str_to_vec_char("ラスト");
    app.view.input_idx = 1;
    app.view.input_cursor_position = 1;

    handler(Key::Delete, &mut app);
    assert_eq!(app.view.input, str_to_vec_char("ラト"));

    app.view.input = str_to_vec_char("Rust");
    app.view.input_idx = 2;
    app.view.input_cursor_position = 2;

    handler(Key::Ctrl('d'), &mut app);
    assert_eq!(app.view.input, str_to_vec_char("Rut"));
  }

  #[test]
  fn test_input_handler_left_event() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("My text");
    let input_len = app.view.input.len().try_into().unwrap();
    app.view.input_idx = app.view.input.len();
    app.view.input_cursor_position = input_len;

    handler(Key::Left, &mut app);
    assert_eq!(app.view.input_cursor_position, input_len - 1);
    handler(Key::Left, &mut app);
    assert_eq!(app.view.input_cursor_position, input_len - 2);
    handler(Key::Left, &mut app);
    assert_eq!(app.view.input_cursor_position, input_len - 3);
    handler(Key::Ctrl('b'), &mut app);
    assert_eq!(app.view.input_cursor_position, input_len - 4);
    handler(Key::Ctrl('b'), &mut app);
    assert_eq!(app.view.input_cursor_position, input_len - 5);

    // Pretend to smash the left event to test the we have no out-of-bounds crash
    for _ in 0..20 {
      handler(Key::Left, &mut app);
    }

    assert_eq!(app.view.input_cursor_position, 0);
  }

  #[test]
  fn test_input_handler_on_enter_text_non_english_char() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("ыа");
    app.view.input_cursor_position = app.view.input.len().try_into().unwrap();
    app.view.input_idx = app.view.input.len();

    handler(Key::Char('ы'), &mut app);

    assert_eq!(app.view.input, str_to_vec_char("ыаы"));
  }

  #[test]
  fn test_input_handler_on_enter_text_wide_char() {
    let mut app = App::default();

    app.view.input = str_to_vec_char("你");
    app.view.input_cursor_position = 2; // 你 is 2 char wide
    app.view.input_idx = 1; // 1 char

    handler(Key::Char('好'), &mut app);

    assert_eq!(app.view.input, str_to_vec_char("你好"));
    assert_eq!(app.view.input_idx, 2);
    assert_eq!(app.view.input_cursor_position, 4);
  }
}
