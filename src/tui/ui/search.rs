use crate::core::app::{ActiveBlock, App, SearchResultBlock};
use crate::tui::layout::COMPACT_TOP_ROW_THRESHOLD;
use crate::tui::theme::ThemeExt;
use ratatui::{
  layout::Rect,
  style::Style,
  text::{Span, Text},
  widgets::{Block, BorderType, Borders, Paragraph},
  Frame,
};

use super::util::{draw_selectable_list, get_color, get_search_results_highlight_state};

/// Draws the search input, Help button and Settings button into pre-split
/// areas (see `tui::layout::split_input_help_and_settings`). Splitting is
/// owned by `compute_main_layout` so mouse hit-testing always matches.
pub fn draw_input_and_help_box(
  f: &mut Frame<'_>,
  app: &App,
  input_area: Rect,
  help_area: Rect,
  settings_area: Rect,
) {
  let row_width = input_area
    .width
    .saturating_add(help_area.width)
    .saturating_add(settings_area.width);
  let compact_top_row = row_width < COMPACT_TOP_ROW_THRESHOLD;

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::Input,
    current_route.hovered_block == ActiveBlock::Input,
  );

  let show_loading = app.is_loading && app.user_config.behavior.show_loading_indicator;
  let border_type = if show_loading {
    BorderType::Double
  } else {
    BorderType::Rounded
  };

  let input_string: String = app.view.input.iter().collect();
  let lines = Text::from(input_string.clone());
  // Compute horizontal scroll so the cursor stays visible within the input box.
  // inner width = total width - 2 (for left and right borders)
  let inner_width = input_area.width.saturating_sub(2);
  let scroll_offset = if inner_width > 0 && app.view.input_cursor_position >= inner_width {
    app.view.input_cursor_position - inner_width + 1
  } else {
    0
  };
  app.view.input_scroll_offset.set(scroll_offset);

  let input_title = app.strings().screens.search;

  let input = Paragraph::new(lines).scroll((0, scroll_offset)).block(
    Block::default()
      .borders(Borders::ALL)
      .border_type(border_type)
      .title(Span::styled(
        input_title,
        get_color(highlight_state, app.user_config.theme),
      ))
      .style(app.user_config.theme.base_style())
      .border_style(get_color(highlight_state, app.user_config.theme)),
  );
  f.render_widget(input, input_area);

  let help_content = if show_loading {
    (app.user_config.theme.hint, "...")
  } else if compact_top_row {
    (app.user_config.theme.inactive, "?")
  } else {
    (
      app.user_config.theme.inactive,
      app.strings().screens.type_help,
    )
  };

  let block = Block::default()
    .title(Span::styled(
      app.strings().screens.help,
      Style::default().fg(help_content.0.into()),
    ))
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(help_content.0.into()));

  let lines = Text::from(help_content.1);
  let help = Paragraph::new(lines).block(block).style(
    Style::default()
      .fg(help_content.0.into())
      .bg(app.user_config.theme.background.into()),
  );
  f.render_widget(help, help_area);

  let settings_keybind_string = app
    .effective_open_settings_key()
    .to_string()
    .trim_matches(|c| c == '<' || c == '>')
    .to_string();
  let settings_hint = if compact_top_row {
    settings_keybind_string
  } else {
    app.strings().screens.type_key(&settings_keybind_string)
  };
  let settings_color = app.user_config.theme.inactive;
  let settings_block = Block::default()
    .title(Span::styled(
      app.strings().screens.settings,
      Style::default().fg(settings_color.into()),
    ))
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(settings_color.into()));

  let settings = Paragraph::new(settings_hint).block(settings_block).style(
    Style::default()
      .fg(settings_color.into())
      .bg(app.user_config.theme.background.into()),
  );
  f.render_widget(settings, settings_area);
}

pub fn draw_search_results(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  let songs: Vec<String> = app
    .search_results()
    .tracks
    .as_ref()
    .map(|tracks| {
      tracks
        .items
        .iter()
        .map(|item| {
          format!(
            "{} - {}",
            item.display_name(app.strings()),
            item.artists.join(", ")
          )
        })
        .collect()
    })
    .unwrap_or_default();

  draw_selectable_list(
    f,
    app,
    layout_chunk,
    app.strings().screens.songs,
    &songs,
    get_search_results_highlight_state(app, SearchResultBlock::SongSearch),
    app.view.search_selected_tracks_index,
    &app.view.search_tracks_scroll_offset,
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  use ratatui::{backend::TestBackend, Terminal};

  fn rendered(app: &App, area: Rect) -> String {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
      .draw(|f| draw_search_results(f, app, area))
      .unwrap();
    let buffer = terminal.backend().buffer();
    (0..area.height)
      .flat_map(|y| (0..area.width).map(move |x| (x, y)))
      .filter_map(|(x, y)| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
      .collect()
  }

  fn track(name: &str, artist: &str) -> crate::core::plugin_api::TrackInfo {
    crate::core::plugin_api::TrackInfo {
      generated_name: None,
      uri: Some(format!("qobuz:track:{name}")),
      name: name.to_string(),
      artists: vec![artist.to_string()],
      album: "Album".to_string(),
      duration_ms: 180_000,
      id: Some(name.to_string()),
      album_id: None,
      artist_refs: Vec::new(),
      is_playable: true,
      is_local: false,
      track_number: 1,
      explicit: false,
      image_url: None,
      release_date: None,
    }
  }

  #[test]
  fn search_results_are_one_full_size_song_list() {
    let mut app = App::default();
    app.show_source_search_tracks("q", vec![track("Blue", "Joni"), track("Kind", "Miles")]);
    let content = rendered(&app, Rect::new(0, 0, 90, 30));
    assert!(content.contains("Songs"), "{content}");
    assert!(content.contains("Blue - Joni"), "{content}");
    assert!(content.contains("Kind - Miles"), "{content}");
    for removed in ["Artists", "Albums", "Playlists", "Podcasts"] {
      assert!(
        !content.contains(removed),
        "{removed} pane is gone: {content}"
      );
    }
  }

  fn rendered_input(app: &App, area: Rect) -> String {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
      .draw(|f| {
        draw_input_and_help_box(
          f,
          app,
          area,
          // The help and settings boxes only matter for the compact-row
          // decision; the title under test is on the input box itself.
          Rect::new(0, 0, 0, 0),
          Rect::new(0, 0, 0, 0),
        )
      })
      .unwrap();
    let buffer = terminal.backend().buffer();
    (0..area.height)
      .flat_map(|y| (0..area.width).map(move |x| (x, y)))
      .filter_map(|(x, y)| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
      .collect()
  }

  #[test]
  fn the_input_box_is_titled_search_in_the_chosen_language() {
    use crate::core::i18n::Language;
    let mut app = App::default();
    app.view.input = "cherry".chars().collect();
    assert!(rendered_input(&app, Rect::new(0, 0, 40, 3)).contains("Search"));
    app.user_config.behavior.language = Language::Fr;
    app.refresh_language();
    let content = rendered_input(&app, Rect::new(0, 0, 40, 3));
    assert!(content.contains(app.strings().screens.search), "{content}");
  }
}
