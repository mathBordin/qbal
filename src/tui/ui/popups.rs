use crate::core::app::{AnnouncementLevel, App, HelpMenuModel};
use crate::core::plugin_api::PopupLine;
use ratatui::{
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table, Wrap},
  Frame,
};

use super::help::{get_filtered_help_docs, help_match_ranges};
use crate::tui::theme::{EmphasisExt, ThemeExt};

/// Rebuild [`App::help_menu_model`] if the terminal width, keybindings, or
/// filter changed since the last build. Called from the event loop (and tests)
/// before drawing, so [`draw_help_menu`] renders from immutable `App` state
/// instead of rebuilding ~80 owned Strings (plus per-cell char-count
/// truncation) on every redraw while Help is open.
pub fn ensure_help_menu_model(app: &mut App) {
  // Mirrors draw_help_menu's layout: a margin of 2 on each side of the frame.
  let total_width = (app.view.size.width as usize).saturating_sub(4);
  let stale = app.view.help_menu_model.as_ref().is_none_or(|m| {
    m.width != total_width
      || m.keys != app.user_config.keys
      || m.source != app.active_source
      || m.filter != app.view.help_filter
  });
  if !stale {
    return;
  }
  let (header, rows) = build_help_rows(app, total_width);
  // The pager counts the rows built here, so a source or session change
  // while Help is open cannot leave it paging over a stale count.
  app.view.help_docs_size = rows.len() as u32;
  let match_ranges = rows
    .iter()
    .map(|row| help_match_ranges(row, &app.view.help_filter))
    .collect();
  app.view.help_menu_model = Some(HelpMenuModel {
    width: total_width,
    keys: app.user_config.keys.clone(),
    source: app.active_source,
    filter: app.view.help_filter.clone(),
    header,
    rows,
    match_ranges,
  });
}

/// Split `text` into spans so every filter match renders in the highlight
/// style, making it obvious why a row survived the filter. `ranges` are
/// ascending, non-overlapping byte ranges (e.g. [`help_match_ranges`] from the
/// help model, or a settings row's fuzzy-match ranges).
pub(crate) fn highlighted_spans<'a>(
  text: &'a str,
  ranges: &[(usize, usize)],
  base: Style,
  highlight: Style,
) -> Vec<Span<'a>> {
  if ranges.is_empty() {
    return vec![Span::styled(text, base)];
  }
  let mut spans = Vec::with_capacity(ranges.len() * 2 + 1);
  let mut cursor = 0;
  for &(start, end) in ranges {
    if cursor < start {
      spans.push(Span::styled(&text[cursor..start], base));
    }
    spans.push(Span::styled(&text[start..end], highlight));
    cursor = end;
  }
  if cursor < text.len() {
    spans.push(Span::styled(&text[cursor..], base));
  }
  spans
}

fn build_help_rows(app: &App, total_width: usize) -> (String, Vec<String>) {
  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  // Calculate column widths based on available terminal width.
  let col1_width = (total_width as f32 * 0.40) as usize;
  let col2_width = (total_width as f32 * 0.30) as usize;
  let col3_width = total_width.saturating_sub(col1_width + col2_width + 2);

  let format_row = |r: Vec<String>| -> String {
    format!(
      "{}  {}  {}",
      super::help::padded_help_cell(&r[0], col1_width),
      super::help::padded_help_cell(&r[1], col2_width),
      super::help::padded_help_cell(&r[2], col3_width),
    )
  };

  let strings = &app.strings().help;
  let header = [
    strings.column_description,
    strings.column_event,
    strings.column_context,
  ];
  let header = format_row(header.iter().map(|s| s.to_string()).collect());
  // Filter before formatting/truncating so narrow terminals do not make hidden
  // parts of descriptions or contexts unsearchable.
  let rows = get_filtered_help_docs(app)
    .into_iter()
    .map(format_row)
    .collect();
  (header, rows)
}

pub fn draw_help_menu(f: &mut Frame<'_>, app: &App) {
  let strings = &app.strings().help;
  let [area] = f
    .area()
    .layout(&Layout::vertical([Constraint::Percentage(100)]).margin(2));
  let [table_area, filter_area] = area.layout(&Layout::vertical([
    Constraint::Min(0),
    Constraint::Length(1),
  ]));

  // The runner (and tests) call `ensure_help_menu_model` before drawing;
  // rendering itself never rebuilds the model, only reads it.
  let Some(model) = app.view.help_menu_model.as_ref() else {
    return;
  };

  let help_menu_style = app.user_config.theme.base_style();
  let header = &model.header;
  let start = (app.view.help_menu_offset as usize).min(model.rows.len());
  // Two border rows plus the table header leave this many data rows. This is
  // also the value used by the runner for page-size calculations.
  let visible_count = table_area.height.saturating_sub(3) as usize;
  let end = start.saturating_add(visible_count).min(model.rows.len());
  let help_docs = &model.rows[start..end];

  let rows: Vec<Row<'_>> = if model.rows.is_empty() && !app.view.help_filter.is_empty() {
    vec![Row::new([strings.no_matches(&app.view.help_filter)])
      .style(Style::default().fg(app.user_config.theme.inactive.into()))]
  } else {
    let highlight_style = Style::default()
      .fg(app.user_config.theme.active.into())
      .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD));
    help_docs
      .iter()
      .zip(&model.match_ranges[start..end])
      .map(|(item, ranges)| {
        Row::new([Line::from(highlighted_spans(
          item,
          ranges,
          help_menu_style,
          highlight_style,
        ))])
        .style(help_menu_style)
      })
      .collect()
  };

  let help_menu = Table::new(rows, &[Constraint::Percentage(100)])
    .header(Row::new([header.as_str()]))
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(help_menu_style)
        .title(Span::styled(strings.title, help_menu_style))
        .border_style(help_menu_style),
    )
    .style(help_menu_style);
  f.render_widget(help_menu, table_area);

  let theme = app.user_config.theme;
  let filter_line = if app.view.help_filter_editing {
    Line::from(vec![
      Span::styled(
        strings.filter_prompt,
        Style::default().fg(theme.active.into()),
      ),
      Span::styled(app.view.help_filter.as_str(), help_menu_style),
      Span::styled("▏", Style::default().fg(theme.active.into())),
      Span::styled(
        strings.filter_edit_hint,
        Style::default().fg(theme.inactive.into()),
      ),
    ])
  } else if !app.view.help_filter.is_empty() {
    Line::from(vec![
      Span::styled(
        strings.matches(&app.view.help_filter),
        Style::default().fg(theme.active.into()),
      ),
      Span::styled(
        strings.filter_count_hint(model.rows.len()),
        Style::default().fg(theme.inactive.into()),
      ),
    ])
  } else {
    Line::from(vec![
      Span::styled(
        app.user_config.keys.search.to_string(),
        Style::default().fg(theme.active.into()),
      ),
      Span::styled(
        strings.filter_hint,
        Style::default().fg(theme.inactive.into()),
      ),
    ])
  };
  f.render_widget(
    Paragraph::new(filter_line).style(help_menu_style),
    filter_area,
  );
}

#[cfg(test)]
mod help_menu_tests {
  use super::*;
  use ratatui::{backend::TestBackend, Terminal};

  fn rendered_help(app: &mut App) -> String {
    app.view.size = crate::core::geometry::Viewport {
      width: 100,
      height: 30,
    };
    ensure_help_menu_model(app);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| draw_help_menu(f, app)).unwrap();
    let buffer = terminal.backend().buffer();

    (0..30)
      .map(|y| {
        (0..100)
          .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol().to_string()))
          .collect::<String>()
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  #[test]
  fn help_menu_renders_only_filtered_rows_and_live_prompt() {
    let mut app = App::default();
    app.view.help_filter = "volume".to_string();
    app.view.help_filter_editing = true;

    let rendered = rendered_help(&mut app);

    assert!(rendered.contains("Increase volume by 10%"));
    assert!(rendered.contains("Decrease volume by 10%"));
    assert!(!rendered.contains("Jump to start of playlist"));
    assert!(rendered.contains("Filter: volume▏"));
    assert!(rendered.contains("<Enter>: apply  <Esc>: cancel search"));
  }

  #[test]
  fn help_menu_shows_back_hint_when_filter_is_inactive() {
    let rendered = rendered_help(&mut App::default());

    assert!(rendered.contains("<Esc>: go back"));
    assert!(!rendered.contains("press <Esc> to go back"));
  }

  #[test]
  fn help_menu_highlights_matched_text_in_filtered_rows() {
    let mut app = App::default();
    app.view.help_filter = "volume".to_string();
    app.view.size = crate::core::geometry::Viewport {
      width: 100,
      height: 30,
    };
    ensure_help_menu_model(&mut app);

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| draw_help_menu(f, &app)).unwrap();
    let buffer = terminal.backend().buffer();

    // Locate the row and column of the first "volume" match in the buffer.
    // `find` returns a byte offset, but border symbols are multi-byte, so
    // convert to a cell column by counting the chars before the match.
    let (match_x, match_y) = (0..30)
      .find_map(|y| {
        let line: String = (0..100)
          .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol().to_string()))
          .collect();
        line.find("Increase volume").map(|byte_idx| {
          let cell_x = line[..byte_idx].chars().count() as u16;
          (cell_x + "Increase ".len() as u16, y)
        })
      })
      .expect("filtered help row should be rendered");

    let matched = buffer.cell((match_x, match_y)).unwrap();
    let unmatched = buffer
      .cell((match_x - "Increase ".len() as u16, match_y))
      .unwrap();
    assert_eq!(
      matched.style().fg,
      Some(app.user_config.theme.active.into())
    );
    assert_ne!(
      unmatched.style().fg,
      Some(app.user_config.theme.active.into())
    );
  }

  #[test]
  fn help_menu_renders_no_matches_and_clamps_a_stale_offset() {
    let mut app = App::default();
    app.view.help_filter = "not-a-real-help-row".to_string();
    app.view.help_menu_offset = u32::MAX;

    let rendered = rendered_help(&mut app);

    assert!(rendered.contains("No help rows match 'not-a-real-help-row'"));
    assert!(rendered.contains("matches for 'not-a-real-help-row' (0)"));
    assert!(rendered.contains("<Esc>: clear filter"));
  }
}

/// Build the dimmed "up next from context" preview rows shown under the native
/// queue: what resumes once the queue drains. Returns an empty vector when
/// nothing is suspended and no queued items are pending, so `draw_queue` omits
/// the section entirely.
///
/// The source of truth is [`App::queue_suspended`](crate::core::app::App) when
/// the queue is draining over a suspended context; otherwise (queued items
/// pending over a still-playing context) it is that context's own upcoming
/// tracks. Rows read from the still-alive per-source `*_playback` state.
fn context_preview_lines(app: &App, max: usize) -> Vec<String> {
  // Format the upcoming rows of the playlist.
  fn track_rows(
    tracks: &[crate::core::plugin_api::TrackInfo],
    start: usize,
    max: usize,
  ) -> Vec<String> {
    tracks
      .iter()
      .skip(start)
      .take(max)
      .map(|t| format!("{} - {}", t.name, t.artists.join(", ")))
      .collect()
  }

  // 1. A suspended context is authoritative: the queue is draining over it.
  if let Some(ctx) = app.queue_suspended.as_ref() {
    use crate::core::queue::SuspendedContext;
    return match ctx {
      SuspendedContext::Qobuz { resume_index, .. } => {
        match (resume_index, app.qobuz_playback.as_ref()) {
          (Some(i), Some(s)) => track_rows(&s.tracks, *i, max),
          _ => Vec::new(),
        }
      }
    };
  }

  // 2. Queued items pending over a still-playing context: preview what resumes
  //    after them (the context's upcoming tracks, from the next index on).
  if !app.native_queue.is_empty() {
    if let Some(s) = app.qobuz_playback.as_ref() {
      return track_rows(&s.tracks, s.index + 1, max);
    }
  }

  Vec::new()
}

pub fn draw_queue(f: &mut Frame<'_>, app: &App) {
  let [area] = f
    .area()
    .layout(&Layout::vertical([Constraint::Percentage(100)]).margin(2));
  draw_queue_in(f, app, area);
}

pub fn draw_queue_in(f: &mut Frame<'_>, app: &App, area: Rect) {
  let style = app.user_config.theme.base_style();
  let mut items: Vec<ListItem> = Vec::new();

  // Row 0: "Now playing" header: the queued track when the queue owns
  // playback, otherwise whatever the playlist is playing.
  let now_text = app
    .queue_now_display()
    .or_else(|| {
      crate::infra::media_metadata::current_playback_snapshot(app)
        .map(|s| format!("{} - {}", s.metadata.title, s.primary_artist()))
    })
    .unwrap_or_else(|| "—".to_string());
  items.push(
    ListItem::new(Line::from(vec![
      Span::styled(
        app.strings().popups.now_playing,
        style.add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
      ),
      Span::raw(now_text),
    ]))
    .style(style),
  );

  // The native queue is the selectable list.
  if app.native_queue.is_empty() {
    if !app.queue_owns_playback() {
      // While the queue owns playback the last queued track is the "Now playing"
      // row above, so an "empty" hint would contradict it — omit it there.
      items.push(
        ListItem::new(Span::raw(
          app
            .strings()
            .popups
            .queue_empty(&app.user_config.keys.add_item_to_queue.to_string()),
        ))
        .style(style),
      );
    }
  } else {
    for track in &app.native_queue {
      let line = format!(
        "{} - {}",
        track.display_name(app.strings()),
        track.artists.join(", "),
      );
      items.push(ListItem::new(line).style(style));
    }
  }

  // Dimmed, non-selectable preview of what resumes once the queue drains. It is
  // appended after the selectable native-queue rows; selection stays confined to
  // those rows (queue_menu.rs counts only `1 + native_queue.len()` rows), so
  // these extra rows never receive the highlight.
  let preview = context_preview_lines(app, 5);
  if !preview.is_empty() {
    let header_style = Style::default().fg(app.user_config.theme.hint.into());
    let row_style = Style::default()
      .fg(app.user_config.theme.inactive.into())
      .add_modifier(Modifier::DIM);
    items.push(
      ListItem::new(Span::styled(
        app.strings().popups.queue_up_next,
        header_style,
      ))
      .style(style),
    );
    for line in preview {
      items.push(ListItem::new(Span::styled(line, row_style)).style(style));
    }
  }

  let mut state = ListState::default();
  let len = items.len();
  let selected = if len == 0 {
    None
  } else {
    Some(app.view.queue_selected_index.min(len.saturating_sub(1)))
  };
  state.select(selected);
  let list = List::new(items)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(style)
        .title(Span::styled(
          app
            .strings()
            .popups
            .queue_title(&app.user_config.keys.remove_from_queue.to_string()),
          style,
        ))
        .border_style(style),
    )
    .style(style)
    .highlight_style(
      Style::default()
        .fg(app.user_config.theme.active.into())
        .bg(app.user_config.theme.inactive.into())
        .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    )
    .highlight_symbol(
      Line::from("▶ ").style(Style::default().fg(app.user_config.theme.active.into())),
    );
  f.render_stateful_widget(list, area, &mut state);
}

pub fn draw_error_screen(f: &mut Frame<'_>, app: &App) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Percentage(100)])
    .margin(5)
    .split(f.area());

  let playing_text = vec![
    Line::from(vec![
      Span::raw(app.strings().popups.api_response),
      Span::styled(
        app.api_error(),
        Style::default().fg(app.user_config.theme.error_text.into()),
      ),
    ]),
    Line::from(Span::styled(
      app.strings().popups.playback_check,
      Style::default().fg(app.user_config.theme.text.into()),
    )),
    Line::from(Span::styled(
      app.strings().popups.check_subscription,
      Style::default().fg(app.user_config.theme.text.into()),
    )),
    Line::from(Span::styled(
      app.strings().popups.check_audio,
      Style::default().fg(app.user_config.theme.text.into()),
    )),
    Line::from(Span::styled(
      app.strings().popups.playback_hint,
      Style::default().fg(app.user_config.theme.hint.into()),
    )),
    Line::from(Span::styled(
      app.strings().popups.error_back,
      Style::default().fg(app.user_config.theme.inactive.into()),
    )),
  ];

  // The log path gets its own reserved rows instead of a place in the list
  // above: `api_error` has no length limit, and at 80x24 the margin and the
  // border leave twelve text lines, so a wrapping API error can fill the
  // frame on its own. Ordering alone would hold only for errors short enough
  // to leave room — pinning holds for all of them, which matters because the
  // user who is reading this screen is the one with something to report.
  let block = Block::default()
    .borders(Borders::ALL)
    .style(app.user_config.theme.base_style())
    .title(Span::styled(
      app.strings().popups.error_title,
      Style::default().fg(app.user_config.theme.error_border.into()),
    ))
    .border_style(Style::default().fg(app.user_config.theme.error_border.into()));
  let inner = block.inner(chunks[0]);
  f.render_widget(block, chunks[0]);

  let log_hint = app.strings().popups.log_hint;
  let log_label = app.strings().popups.log_file;
  let log_text = vec![
    Line::from(vec![
      Span::styled(
        log_label,
        Style::default().fg(app.user_config.theme.text.into()),
      ),
      Span::styled(
        app.log_path.clone(),
        Style::default().fg(app.user_config.theme.hint.into()),
      ),
    ]),
    Line::from(Span::styled(
      log_hint,
      Style::default().fg(app.user_config.theme.hint.into()),
    )),
  ];

  // Measured rather than assumed to be two rows: at 80 columns the margin and
  // the border leave 68, and a temp-directory log path is routinely longer
  // than what is left of that after the label. A reserved row count that is
  // too small would cut the filename off the one line a bug reporter needs.
  let log_rows = wrapped_rows(&format!("{log_label}{}", app.log_path), inner.width)
    + wrapped_rows(log_hint, inner.width);

  let sections = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Min(0),
      // Capped so a pathological path cannot leave no room for the error
      // itself, which is the other half of the report.
      Constraint::Length(log_rows.min(inner.height.saturating_sub(2).max(1))),
    ])
    .split(inner);

  f.render_widget(
    Paragraph::new(playing_text)
      .wrap(Wrap { trim: true })
      .style(app.user_config.theme.base_style()),
    sections[0],
  );
  f.render_widget(
    Paragraph::new(log_text)
      .wrap(Wrap { trim: true })
      .style(app.user_config.theme.base_style()),
    sections[1],
  );
}

/// Rows `text` occupies once ratatui has word-wrapped it to `width`: greedy,
/// and a word longer than a row is broken across rows rather than clipped.
/// Pure so the reserved height can be tested without a terminal.
///
/// Measured in terminal cells rather than characters, because that is what
/// ratatui wraps on: a path under a CJK user directory is twice as wide as it
/// is long, and counting characters would reserve too few rows and clip it.
fn wrapped_rows(text: &str, width: u16) -> u16 {
  use unicode_width::UnicodeWidthStr;

  let width = usize::from(width);
  if width == 0 {
    return 1;
  }

  let mut rows = 1usize;
  let mut used = 0usize;
  for word in text.split_whitespace() {
    let len = UnicodeWidthStr::width(word);
    if used > 0 {
      if used + 1 + len <= width {
        used += 1 + len;
        continue;
      }
      rows += 1;
    }
    // The word now starts a row of its own, and may outgrow it.
    let overflow = len.saturating_sub(1) / width;
    rows += overflow;
    used = len - overflow * width;
  }

  u16::try_from(rows).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod error_screen_tests {
  use super::*;
  use ratatui::{backend::TestBackend, Terminal};

  fn rendered_error_screen_at(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| draw_error_screen(f, app)).unwrap();
    let buffer = terminal.backend().buffer();

    (0..height)
      .map(|y| {
        (0..width)
          .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol().to_string()))
          .collect::<String>()
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn rendered_error_screen(app: &App) -> String {
    rendered_error_screen_at(app, 100, 30)
  }

  #[test]
  fn error_screen_names_the_log_file_and_the_debug_flag() {
    let mut app = App::default();
    app.log_path = "/tmp/qbal_logs/qballog42".to_string();

    let rendered = rendered_error_screen(&app);

    assert!(
      rendered.contains("/tmp/qbal_logs/qballog42"),
      "the error screen must name the log file a reporter has to attach:\n{rendered}"
    );
    assert!(
      rendered.contains("--debug"),
      "the error screen must point at the debug mode:\n{rendered}"
    );
  }

  /// 80x24 is the floor a terminal is allowed to be, and a wrapping API error
  /// is the normal case on this screen rather than an exotic one.
  #[test]
  fn wrapped_rows_counts_word_wrapping_and_oversized_words() {
    assert_eq!(wrapped_rows("", 10), 1);
    assert_eq!(wrapped_rows("short", 10), 1);
    assert_eq!(wrapped_rows("one two three", 7), 2);
    // A path is one unbreakable word: 25 characters over rows of 10.
    assert_eq!(wrapped_rows(&"x".repeat(25), 10), 3);
    assert_eq!(wrapped_rows(&"x".repeat(20), 10), 2);
    // Exactly full must not spill into an extra row.
    assert_eq!(wrapped_rows(&"x".repeat(10), 10), 1);
    assert_eq!(wrapped_rows("anything", 0), 1);
    // Double-width characters take two cells each, so half as many fit.
    assert_eq!(wrapped_rows(&"中".repeat(5), 10), 1);
    assert_eq!(wrapped_rows(&"中".repeat(6), 10), 2);
  }

  /// A log path under a CJK user directory is twice as wide as it is long.
  #[test]
  fn error_screen_shows_a_double_width_log_path_in_full() {
    let path = format!("{}/qbal.log", "中".repeat(65));
    let mut app = App::default();
    app.log_path = path.clone();

    let rendered = rendered_error_screen_at(&app, 80, 24);
    let unbroken = rendered.replace([' ', '\n', '\u{2502}'], "");

    assert!(
      unbroken.contains(&path),
      "a double-width path must not be clipped:\n{rendered}"
    );
    assert!(
      rendered.contains("--debug"),
      "reserving too few rows would push the hint out:\n{rendered}"
    );
  }
}

#[cfg(test)]
mod queue_tests {}

/// A modal box centred horizontally and a third of the way down `bounds`, clamped
/// to fit. Shared so every overlay in the app sits in the same place.
pub(crate) fn centered_modal_rect(
  bounds: Rect,
  requested_width: u16,
  requested_height: u16,
) -> Rect {
  let width = requested_width.min(bounds.width.saturating_sub(2).max(1));
  let height = requested_height.min(bounds.height.saturating_sub(2).max(1));
  let left = bounds.x + bounds.width.saturating_sub(width) / 2;
  let top = bounds.y + bounds.height.saturating_sub(height) / 3;
  Rect::new(left, top, width, height)
}

pub fn draw_announcement_prompt(f: &mut Frame<'_>, app: &App) {
  let Some(announcement) = &app.active_announcement else {
    return;
  };

  let width = std::cmp::min(f.area().width.saturating_sub(4), 74);
  let height = std::cmp::min(f.area().height.saturating_sub(4), 16);
  let rect = f
    .area()
    .centered(Constraint::Length(width), Constraint::Length(height));

  f.render_widget(Clear, rect);

  let (level_label, accent_color) = match announcement.level {
    AnnouncementLevel::Info => (
      app.strings().popups.announcement_info,
      app.user_config.theme.active,
    ),
    AnnouncementLevel::Warning => (
      app.strings().popups.announcement_warning,
      app.user_config.theme.hint,
    ),
    AnnouncementLevel::Critical => (
      app.strings().popups.announcement_critical,
      app.user_config.theme.error_text,
    ),
  };

  let mut text = vec![
    Line::from(Span::styled(
      format!("{}  {}", level_label, announcement.title),
      Style::default().add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    )),
    Line::from(""),
  ];

  for line in announcement.body.lines() {
    text.push(Line::from(line.to_string()));
  }

  if let Some(url) = &announcement.url {
    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
      app.strings().popups.announcement_more(url),
      Style::default().add_modifier(app.user_config.behavior.emphasis(Modifier::ITALIC)),
    )));
  }

  text.push(Line::from(""));
  text.push(Line::from(Span::styled(
    app.strings().popups.announcement_dismiss,
    Style::default().fg(app.user_config.theme.inactive.into()),
  )));

  let paragraph = Paragraph::new(text)
    .style(app.user_config.theme.base_style())
    .alignment(Alignment::Left)
    .wrap(Wrap { trim: false })
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(app.user_config.theme.base_style())
        .border_style(Style::default().fg(accent_color.into()))
        .title(app.strings().popups.announcement_title),
    );

  f.render_widget(paragraph, rect);
}

pub fn draw_recap_prompt(f: &mut Frame<'_>, app: &App) {
  let Some(prompt) = app.recap_prompt() else {
    return;
  };

  let width = std::cmp::min(f.area().width.saturating_sub(4), 64);
  let height = 10;
  let rect = f
    .area()
    .centered(Constraint::Length(width), Constraint::Length(height));

  f.render_widget(Clear, rect);

  let text = vec![
    Line::from(Span::styled(
      app.strings().popups.recap_ready,
      Style::default()
        .fg(app.user_config.theme.active.into())
        .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    )),
    Line::from(""),
    Line::from(app.strings().popups.recap_listens(prompt.listens)),
    Line::from(app.strings().popups.recap_browser),
    Line::from(""),
    Line::from(Span::styled(
      app.strings().popups.recap_controls,
      Style::default().fg(app.user_config.theme.inactive.into()),
    )),
  ];

  let paragraph = Paragraph::new(text)
    .style(app.user_config.theme.base_style())
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: false })
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(app.user_config.theme.base_style())
        .border_style(Style::default().fg(app.user_config.theme.active.into()))
        .title(app.strings().popups.recap_title),
    );

  f.render_widget(paragraph, rect);
}

pub fn draw_exit_prompt(f: &mut Frame<'_>, app: &App) {
  let width = std::cmp::min(f.area().width.saturating_sub(4), 56);
  let height = 8;
  let rect = f
    .area()
    .centered(Constraint::Length(width), Constraint::Length(height));

  f.render_widget(Clear, rect);

  let text = vec![
    Line::from(Span::styled(
      app.strings().popups.exit_question,
      Style::default().add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    )),
    Line::from(""),
    Line::from(app.strings().popups.exit_yes_no),
    Line::from(Span::styled(
      app.strings().popups.exit_controls,
      Style::default().fg(app.user_config.theme.inactive.into()),
    )),
  ];

  let paragraph = Paragraph::new(text)
    .style(app.user_config.theme.base_style())
    .alignment(Alignment::Center)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(app.user_config.theme.base_style())
        .border_style(Style::default().fg(app.user_config.theme.active.into()))
        .title(app.strings().popups.exit_title),
    );

  f.render_widget(paragraph, rect);
}

/// Draw the sort menu popup overlay
pub fn draw_sort_menu(f: &mut Frame<'_>, app: &App) {
  if !app.view.sort_menu_visible {
    return;
  }

  let context = match app.view.sort_context {
    Some(ctx) => ctx,
    None => return,
  };

  let available_fields = context.available_fields();
  let current_sort = app.sort_state(context);

  let width = std::cmp::min(f.area().width.saturating_sub(4), 35);
  let height = (available_fields.len() + 4) as u16; // +4 for borders/padding
  let rect = f
    .area()
    .centered(Constraint::Length(width), Constraint::Length(height));

  f.render_widget(Clear, rect);

  // Build list items
  let items: Vec<ListItem> = available_fields
    .iter()
    .enumerate()
    .map(|(i, field)| {
      let shortcut = field
        .shortcut()
        .map(|c| format!(" ({})", c))
        .unwrap_or_default();
      let indicator = if *field == current_sort.field {
        format!(
          " {}",
          current_sort.order.indicator_icon(
            &app.user_config.behavior.sort_ascending_icon,
            &app.user_config.behavior.sort_descending_icon,
          )
        )
      } else {
        String::new()
      };
      let text = format!(
        "{}{}{}",
        app.strings().popups.sort_field(*field),
        shortcut,
        indicator
      );

      let style = if i == app.view.sort_menu_selected {
        Style::default()
          .fg(app.user_config.theme.active.into())
          .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD))
      } else if *field == current_sort.field {
        Style::default().fg(app.user_config.theme.hovered.into())
      } else {
        Style::default().fg(app.user_config.theme.text.into())
      };

      ListItem::new(text).style(style)
    })
    .collect();

  let title = match context {
    crate::core::sort::SortContext::PlaylistTracks => app.strings().popups.sort_tracks,
  };

  let list = List::new(items)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(app.user_config.theme.base_style())
        .border_style(Style::default().fg(app.user_config.theme.active.into()))
        .title(Span::styled(
          title,
          Style::default()
            .fg(app.user_config.theme.active.into())
            .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
        )),
    )
    .highlight_style(
      Style::default()
        .fg(app.user_config.theme.active.into())
        .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    )
    .highlight_symbol(
      Line::from("▶ ").style(Style::default().fg(app.user_config.theme.active.into())),
    );

  let mut state = ListState::default();
  state.select(Some(app.view.sort_menu_selected));

  f.render_stateful_widget(list, rect, &mut state);
}

/// Draw the plugin popup overlay, if one is active.
///
/// Called last in the terminal draw closure so it overlays every screen.
pub fn draw_plugin_popup(f: &mut Frame<'_>, app: &App) {
  let popup = match &app.plugin_popup {
    Some(p) => p,
    None => return,
  };

  // Compute width: fit to longest line/title, clamped to 70% of area.
  let area = f.area();
  let max_width = (area.width as u32 * 70 / 100).min(u16::MAX as u32) as u16;
  let content_width = popup
    .lines
    .iter()
    .map(|l| l.text.len() as u16)
    .chain(std::iter::once(popup.title.len() as u16))
    .max()
    .unwrap_or(0)
    .saturating_add(4); // 2 border + 2 padding
  let width = content_width.clamp(20, max_width);

  // Height: line count + 2 borders + 1 footer, clamped.
  let footer_lines = 1u16;
  let content_height = popup.lines.len() as u16 + 2 + footer_lines;
  let max_height = area.height.saturating_sub(2).max(3);
  let height = content_height.clamp(4, max_height);

  let rect = centered_modal_rect(area, width, height);
  f.render_widget(Clear, rect);

  // Build styled lines.
  let mut ratatui_lines: Vec<Line> = popup.lines.iter().map(|pl| build_popup_line(pl)).collect();

  // Footer hint.
  ratatui_lines.push(Line::from(Span::styled(
    app.strings().popups.plugin_close_hint,
    Style::default().fg(app.user_config.theme.hint.into()),
  )));

  let block = Block::default()
    .borders(Borders::ALL)
    .style(app.user_config.theme.base_style())
    .border_style(Style::default().fg(app.user_config.theme.active.into()))
    .title(Span::styled(
      popup.title.clone(),
      Style::default()
        .fg(app.user_config.theme.header.into())
        .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    ));

  let paragraph = Paragraph::new(ratatui_lines)
    .block(block)
    .scroll((app.view.plugin_popup_scroll, 0));

  f.render_widget(paragraph, rect);
}

fn build_popup_line<'a>(pl: &'a PopupLine) -> Line<'a> {
  let mut style = Style::default();
  if let Some(fg) = pl.fg {
    style = style.fg(fg.into());
  }
  if pl.bold {
    style = style.add_modifier(Modifier::BOLD);
  }
  if pl.italic {
    style = style.add_modifier(Modifier::ITALIC);
  }
  Line::from(Span::styled(pl.text.clone(), style))
}
