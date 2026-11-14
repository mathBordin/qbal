//! The terminal's key surface: the help rows, each with the [`Requirement`]
//! it needs and filtered by the active source and the session, and the
//! registry naming the key or gesture that produces each [`Action`].

#![deny(
  clippy::wildcard_enum_match_arm,
  clippy::match_wildcard_for_single_variants
)]

use crate::core::app::App;
use crate::core::input::Key;
use crate::core::requirement::{Capability, Requirement};
use crate::core::user_config::KeyBindings;

/// How a help row names its key.
pub enum HelpKey {
  /// A rebindable key from `config.yml`; an unmet row stays listed, marked.
  Binding(fn(&KeyBindings) -> Key),
  /// A key the screen hard-codes; an unmet row is hidden.
  Literal(&'static str),
  /// A rebindable key expression built from the running app; treated like
  /// `Binding`.
  Custom(fn(&App) -> String),
}

pub struct HelpEntry {
  pub description: &'static str,
  pub key: HelpKey,
  pub context: &'static str,
  pub needs: Requirement,
}

fn row(description: &'static str, key: HelpKey, context: &'static str) -> HelpEntry {
  HelpEntry {
    description,
    key,
    context,
    needs: Requirement::None,
  }
}

/// A pane key with its fixed arrow alias, for example `<Alt+h> | <Alt+Left>`.
fn with_arrow_alias(app: &App, key: Key, modifiers: &str, arrow: &str) -> String {
  format!(
    "{} | <{}+{}>",
    key,
    modifiers,
    app.strings().help.arrow_key(arrow)
  )
}

impl HelpEntry {
  fn needs(mut self, needs: Requirement) -> Self {
    self.needs = needs;
    self
  }
}

const SEARCH: Requirement = Requirement::Capability(Capability::Search);

/// Every help row using the selected language, before the availability filter.
pub fn help_entries_for(strings: &crate::core::i18n::Strings) -> Vec<HelpEntry> {
  let help = &strings.help;
  use HelpKey::{Binding, Custom, Literal};
  vec![
    row(
      help.next_page,
      Binding(|k| k.next_page),
      help.context_pagination,
    ),
    row(
      help.previous_page,
      Binding(|k| k.previous_page),
      help.context_pagination,
    ),
    row(
      help.playlist_start,
      Binding(|k| k.jump_to_start),
      help.context_pagination,
    ),
    row(
      help.playlist_end,
      Binding(|k| k.jump_to_end),
      help.context_pagination,
    ),
    row(
      help.volume_up,
      Binding(|k| k.increase_volume),
      help.context_general,
    ),
    row(
      help.volume_down,
      Binding(|k| k.decrease_volume),
      help.context_general,
    ),
    row(
      help.next_track,
      Binding(|k| k.next_track),
      help.context_general,
    ),
    row(
      help.previous_track,
      Binding(|k| k.previous_track),
      help.context_general,
    ),
    row(
      help.force_previous_track,
      Binding(|k| k.force_previous_track),
      help.context_general,
    ),
    row(
      help.seek_backward,
      Binding(|k| k.seek_backwards),
      help.context_general,
    ),
    row(
      help.seek_forward,
      Binding(|k| k.seek_forwards),
      help.context_general,
    ),
    row(
      help.toggle_shuffle,
      Binding(|k| k.shuffle),
      help.context_general,
    ),
    row(
      help.cycle_repeat,
      Binding(|k| k.repeat),
      help.context_general,
    ),
    row(
      help.move_left,
      Custom(|app| {
        format!(
          "{} | <{}> | <Ctrl+b>",
          app.user_config.keys.move_left,
          app.strings().help.arrow_key(app.strings().help.arrow_left)
        )
      }),
      help.context_general,
    ),
    row(
      help.move_down,
      Custom(|app| {
        format!(
          "{} | <{}> | <Ctrl+n>",
          app.user_config.keys.move_down,
          app.strings().help.arrow_key(app.strings().help.arrow_down)
        )
      }),
      help.context_general,
    ),
    row(
      help.move_up,
      Custom(|app| {
        format!(
          "{} | <{}>",
          app.user_config.keys.move_up,
          app.strings().help.arrow_key(app.strings().help.arrow_up)
        )
      }),
      help.context_general,
    ),
    row(
      help.move_right,
      Custom(|app| {
        format!(
          "{} | <{}> | <Ctrl+f>",
          app.user_config.keys.move_right,
          app.strings().help.arrow_key(app.strings().help.arrow_right)
        )
      }),
      help.context_general,
    ),
    row(help.list_top, Literal("H"), help.context_general),
    row(help.list_middle, Literal("M"), help.context_general),
    row(help.list_bottom, Literal("L"), help.context_general),
    row(help.search, Binding(|k| k.search), help.context_general).needs(SEARCH),
    row(help.filter_help, Binding(|k| k.search), help.context_help),
    row(
      help.filter_settings,
      Binding(|k| k.search),
      help.context_settings,
    ),
    row(
      help.toggle_playback,
      Binding(|k| k.toggle_playback),
      help.context_general,
    ),
    row(help.active_mode, Literal("<Enter>"), help.context_general),
    row(
      help.audio_analysis,
      Binding(|k| k.audio_analysis),
      help.context_general,
    ),
    row(help.visualizer_style, Literal("V"), help.context_general),
    row(
      help.lyrics_view,
      Binding(|k| k.lyrics_view),
      help.context_general,
    ),
    row(
      help.lyrics_scroll,
      Custom(|app| {
        format!(
          "{}/{} | <Up>/<Down> | <Ctrl+n>",
          app.user_config.keys.move_up, app.user_config.keys.move_down
        )
      }),
      help.context_lyrics,
    ),
    row(
      help.lyrics_follow,
      Custom(|app| app.strings().help.key_alternative("f", "<Esc>")),
      help.context_lyrics,
    ),
    row(
      help.lyrics_timing,
      Custom(|app| {
        format!(
          "{}/{} | <Right>/<Left> | <Ctrl+f>/<Ctrl+b>",
          app.user_config.keys.move_right, app.user_config.keys.move_left
        )
      }),
      help.context_lyrics,
    ),
    row(
      help.lyrics_seek,
      Custom(|app| {
        let submit = app.user_config.keys.submit;
        if submit == Key::Enter {
          "<Enter>".to_string()
        } else {
          format!("{submit} | <Enter>")
        }
      }),
      help.context_lyrics,
    ),
    row(
      help.miniplayer,
      Binding(|k| k.miniplayer_view),
      help.context_general,
    ),
    #[cfg(feature = "cover-art")]
    row(
      help.cover_art,
      Binding(|k| k.cover_art_view),
      help.context_general,
    ),
    row(help.back, Binding(|k| k.back), help.context_general),
    row(
      help.settings_open,
      Custom(|app| app.effective_open_settings_key().to_string()),
      help.context_general,
    ),
    row(
      help.settings_save,
      Custom(|app| app.effective_save_settings_key().to_string()),
      help.context_settings,
    ),
    row(
      help.hover_mode,
      Literal("<Esc>"),
      help.context_selected_block,
    ),
    row(
      help.now_playing_panel,
      Literal("<Enter>"),
      help.context_playbar,
    ),
    row(help.sidebar_narrower, Literal("{"), help.context_layout),
    row(help.sidebar_wider, Literal("}"), help.context_layout),
    row(help.panels_shorter, Literal("("), help.context_layout),
    row(help.panels_taller, Literal(")"), help.context_layout),
    row(help.layout_reset, Literal("|"), help.context_layout),
    row(
      help.pane_detach,
      Binding(|k| k.detach_pane),
      help.context_layout,
    ),
    row(
      help.pane_close,
      Binding(|k| k.close_pane),
      help.context_layout,
    ),
    row(
      help.pane_focus_left,
      Custom(|app| with_arrow_alias(app, app.user_config.keys.focus_pane_left, "Alt", "Left")),
      help.context_layout,
    ),
    row(
      help.pane_focus_down,
      Custom(|app| with_arrow_alias(app, app.user_config.keys.focus_pane_down, "Alt", "Down")),
      help.context_layout,
    ),
    row(
      help.pane_focus_up,
      Custom(|app| with_arrow_alias(app, app.user_config.keys.focus_pane_up, "Alt", "Up")),
      help.context_layout,
    ),
    row(
      help.pane_focus_right,
      Custom(|app| with_arrow_alias(app, app.user_config.keys.focus_pane_right, "Alt", "Right")),
      help.context_layout,
    ),
    row(
      help.pane_move_left,
      Custom(|app| {
        with_arrow_alias(
          app,
          app.user_config.keys.move_pane_left,
          "Alt+Shift",
          "Left",
        )
      }),
      help.context_layout,
    ),
    row(
      help.pane_move_down,
      Custom(|app| {
        with_arrow_alias(
          app,
          app.user_config.keys.move_pane_down,
          "Alt+Shift",
          "Down",
        )
      }),
      help.context_layout,
    ),
    row(
      help.pane_move_up,
      Custom(|app| with_arrow_alias(app, app.user_config.keys.move_pane_up, "Alt+Shift", "Up")),
      help.context_layout,
    ),
    row(
      help.pane_move_right,
      Custom(|app| {
        with_arrow_alias(
          app,
          app.user_config.keys.move_pane_right,
          "Alt+Shift",
          "Right",
        )
      }),
      help.context_layout,
    ),
    row(
      help.pane_rotate,
      Binding(|k| k.rotate_pane),
      help.context_layout,
    ),
    row(
      help.pane_grow,
      Binding(|k| k.grow_pane),
      help.context_layout,
    ),
    row(
      help.pane_shrink,
      Binding(|k| k.shrink_pane),
      help.context_layout,
    ),
    row(
      help.pane_reset,
      Binding(|k| k.reset_panes),
      help.context_layout,
    ),
    row(
      help.play_selection,
      Binding(|k| k.submit),
      help.context_selected_block,
    ),
    row(
      help.search_submit,
      Literal("<Enter>"),
      help.context_search_input,
    )
    .needs(SEARCH),
    row(
      help.cursor_left,
      Custom(|app| {
        format!(
          "<{}>",
          app.strings().help.arrow_key(app.strings().help.arrow_left)
        )
      }),
      help.context_search_input,
    ),
    row(
      help.cursor_right,
      Custom(|app| {
        format!(
          "<{}>",
          app.strings().help.arrow_key(app.strings().help.arrow_right)
        )
      }),
      help.context_search_input,
    ),
    row(
      help.input_clear,
      Literal("<Ctrl+l>"),
      help.context_search_input,
    ),
    row(
      help.input_delete_before,
      Literal("<Ctrl+u>"),
      help.context_search_input,
    ),
    row(
      help.input_delete_after,
      Literal("<Ctrl+k>"),
      help.context_search_input,
    ),
    row(
      help.input_delete_word,
      Literal("<Ctrl+w>"),
      help.context_search_input,
    ),
    row(
      help.input_start,
      Literal("<Ctrl+a>"),
      help.context_search_input,
    ),
    row(
      help.input_end,
      Literal("<Ctrl+e>"),
      help.context_search_input,
    ),
    row(
      help.input_escape,
      Literal("<Esc>"),
      help.context_search_input,
    ),
    row(
      help.playlist_random,
      Literal("S"),
      help.context_selected_playlist,
    ),
    row(
      help.queue_add,
      Binding(|k| k.add_item_to_queue),
      help.context_hovered_track,
    ),
    row(
      help.queue_show,
      Binding(|k| k.show_queue),
      help.context_general,
    ),
    row(
      help.queue_remove,
      Binding(|k| k.remove_from_queue),
      help.context_queue,
    ),
    row(help.queue_move, Literal("J / K"), help.context_queue),
    row(help.queue_play, Literal("<Enter>"), help.context_queue),
    row(
      help.listening_recap,
      Binding(|k| k.generate_recap),
      help.context_general,
    ),
    row(
      help.stats_open,
      Literal(help.library_sidebar),
      help.context_stats,
    ),
    row(help.stats_period, Literal("[ / ]"), help.context_stats),
    row(help.sort_open, Literal(","), help.context_sortable_lists),
    // Append new rows here so the pinned English table keeps its indices.
  ]
}

/// The help rows the app can serve now, as `[description, key, context]`;
/// an unmet rebindable row stays, marked with the reason.
pub fn help_rows(app: &App) -> Vec<Vec<String>> {
  let keys = &app.user_config.keys;
  help_entries_for(app.strings())
    .into_iter()
    .filter_map(|entry| {
      let availability = app.availability(entry.needs);
      let description = if availability.is_available() {
        entry.description.to_string()
      } else if matches!(entry.key, HelpKey::Literal(_)) {
        return None;
      } else {
        app.strings().help.unavailable_description(
          entry.description,
          &availability.localized_hint(app.strings())?,
        )
      };
      let key = match entry.key {
        HelpKey::Binding(read) => read(keys).to_string(),
        HelpKey::Literal(text) => text.to_string(),
        HelpKey::Custom(build) => build(app),
      };
      Some(vec![description, key, entry.context.to_string()])
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeSet;

  use super::*;

  #[test]
  fn help_events_translate_words_without_changing_literal_keys() {
    use crate::core::i18n::Language;
    let mut app = App::default();
    for language in [Language::En, Language::PtBr, Language::Fr] {
      app.user_config.behavior.language = language;
      app.refresh_language();
      let help = &app.strings().help;
      let rows = help_rows(&app);
      let event = |description| rows.iter().find(|row| row[0] == description).unwrap()[1].clone();
      assert_eq!(
        event(help.move_left),
        format!(
          "{} | <{}> | <Ctrl+b>",
          app.user_config.keys.move_left,
          help.arrow_key(help.arrow_left)
        )
      );
      assert_eq!(
        event(help.move_down),
        format!(
          "{} | <{}> | <Ctrl+n>",
          app.user_config.keys.move_down,
          help.arrow_key(help.arrow_down)
        )
      );
      assert_eq!(
        event(help.move_up),
        format!(
          "{} | <{}>",
          app.user_config.keys.move_up,
          help.arrow_key(help.arrow_up)
        )
      );
      assert_eq!(
        event(help.move_right),
        format!(
          "{} | <{}> | <Ctrl+f>",
          app.user_config.keys.move_right,
          help.arrow_key(help.arrow_right)
        )
      );
      assert_eq!(
        event(help.lyrics_follow),
        help.key_alternative("f", "<Esc>")
      );
      assert_eq!(event(help.lyrics_seek), "<Enter>");
    }
  }

  /// `source` without its test items and comment lines: a gated block item
  /// runs to its column-0 `}` (rustfmt's shape), a braceless one to its `;`.
  fn production_half(source: &str) -> String {
    let mut out = String::new();
    let mut gated = false;
    let mut in_block = false;
    let mut in_item = false;
    for line in source.lines() {
      if in_block {
        in_block = line != "}";
        continue;
      }
      if in_item {
        in_item = !line.ends_with(';');
        continue;
      }
      if gated {
        gated = line.starts_with("#[");
        if !gated {
          in_block = line.ends_with('{');
          in_item = !in_block && !line.ends_with(';') && !line.ends_with('}');
        }
        continue;
      }
      if line.starts_with("#[cfg(test)]") || line.starts_with("#[cfg(all(test") {
        gated = true;
        continue;
      }
      if line.trim_start().starts_with("//") {
        continue;
      }
      out.push_str(line);
      out.push('\n');
    }
    out
  }

  fn leading_ident(text: &str) -> &str {
    let end = text
      .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
      .unwrap_or(text.len());
    &text[..end]
  }

  /// `Action::<Name>` in `source`, bounded on both sides so `PlaybackAction::`
  /// is skipped and `Action::Play` is not found inside `Action::PlayUris`.
  fn producers_in(source: &str) -> BTreeSet<String> {
    let production = production_half(source);
    let mut found = BTreeSet::new();
    for (start, needle) in production.match_indices("Action::") {
      let before = start.checked_sub(1).map(|i| production.as_bytes()[i]);
      if before.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_') {
        continue;
      }
      let name = leading_ident(&production[start + needle.len()..]);
      if !name.is_empty() {
        found.insert(name.to_string());
      }
    }
    found
  }

  #[test]
  fn the_producer_scan_is_identifier_bounded_and_skips_tests_and_comments() {
    let source = "app.apply(Action::PlayUris { uris: vec![] });\n\
      PlaybackAction::Play;\n\
      /// through the shared `Action::Pause` vocabulary\n\
      #[cfg(test)]\n\
      mod tests {\n  Action::Back\n}\n\
      #[cfg(test)]\n\
      const HIDDEN: Option<Action> = Some(\n  Action::Notify(String::new(), 0),\n);\n\
      #[cfg(test)]\n\
      #[allow(dead_code)]\n\
      fn probe() -> Action { Action::Play }\n\
      let seek = Action::Search(query);\n";
    let expected: BTreeSet<String> = ["PlayUris", "Search"]
      .into_iter()
      .map(str::to_string)
      .collect();
    assert_eq!(producers_in(source), expected);
  }
}
