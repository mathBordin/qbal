use crate::core::app::{ActiveBlock, App};
use crate::core::plugin_api::TrackInfo;
use ratatui::{
  layout::{Constraint, Rect},
  style::{Modifier, Style},
  text::Span,
  widgets::{Block, Borders, Row, Table},
  Frame,
};

#[cfg(test)]
use super::columns::resolve_columns;
use super::columns::{resolve_columns_localized, ResolvedColumn, TableColumnSet};
use super::util::{get_color, millis_to_minutes, release_year};
use crate::tui::theme::{EmphasisExt, ThemeExt};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnId {
  #[default]
  None,
  Title,
}

pub struct TableHeader {
  pub items: Vec<TableHeaderItem>,
}

impl TableHeader {
  pub fn get_index(&self, id: ColumnId) -> Option<usize> {
    self.items.iter().position(|item| item.id == id)
  }
}

#[derive(Default)]
pub struct TableHeaderItem {
  pub id: ColumnId,
  pub text: String,
  pub width: u16,
}

#[derive(Clone)]
pub struct TableItem {
  pub uri: Option<String>,
  pub format: Vec<String>,
}

fn table_visible_rows(app: &App, layout_chunk: Rect) -> usize {
  // Clamp the configured padding to half the table height so an oversized
  // value can never zero out the visible-row count (which would pin the
  // scroll offset to 0 and make off-screen rows unreachable).
  let padding = app
    .user_config
    .behavior
    .table_scroll_padding
    .min(layout_chunk.height / 2);
  layout_chunk
    .height
    .checked_sub(padding)
    .map(|height| height as usize)
    .unwrap_or(0)
}

fn window_at<T>(offset: usize, layout_chunk: Rect, items: &[T]) -> (usize, &[T]) {
  // The scroll offset math works with fewer rows than the drawable area (that
  // gap is the scroll padding below the selection), but the table still
  // renders rows into it, so slice a full chunk-height of rows to fill every
  // line.
  let offset = offset.min(items.len());
  let end = items
    .len()
    .min(offset.saturating_add(layout_chunk.height as usize));
  (offset, &items[offset..end])
}

/// Like `visible_window`, but anchored to a persisted scroll offset: the
/// cursor moves within the visible rows without moving the view, and the view
/// scrolls only when the cursor reaches the top visible row (going up) or the
/// padded bottom row (going down). The updated offset is written back to
/// `scroll_offset` so the next frame and the mouse handler see it.
fn visible_window_anchored<'a, T>(
  app: &App,
  layout_chunk: Rect,
  selected_index: usize,
  scroll_offset: &std::cell::Cell<usize>,
  items: &'a [T],
) -> (usize, &'a [T]) {
  let visible_rows = table_visible_rows(app, layout_chunk);
  let max_cursor_row = visible_rows.saturating_sub(1);

  let anchor = scroll_offset.get().min(items.len().saturating_sub(1));
  let offset = if selected_index < anchor {
    selected_index
  } else if selected_index > anchor.saturating_add(max_cursor_row) {
    selected_index - max_cursor_row
  } else {
    anchor
  };
  scroll_offset.set(offset);

  window_at(offset, layout_chunk, items)
}

fn table_columns(app: &App, table: TableColumnSet, layout_width: u16) -> Vec<ResolvedColumn> {
  let configured = match table {
    TableColumnSet::Songs => &app.user_config.tables.songs,
  };
  resolve_columns_localized(table, layout_width, configured, &app.strings().screens)
}

fn table_header(columns: &[ResolvedColumn]) -> TableHeader {
  TableHeader {
    items: columns
      .iter()
      .map(|column| TableHeaderItem {
        id: column.column_id,
        text: column.header.clone(),
        width: column.width,
      })
      .collect(),
  }
}

fn track_table_item(
  track: &TrackInfo,
  columns: &[ResolvedColumn],
  strings: &crate::core::i18n::Strings,
) -> TableItem {
  TableItem {
    uri: track.uri.clone(),
    format: columns
      .iter()
      .map(|column| match column.id.as_str() {
        "index" => track.track_number.to_string(),
        "title" => track.display_name(strings).to_owned(),
        "artist" => track.artists.join(", "),
        "album" => track.album.clone(),
        "length" => millis_to_minutes(track.duration_ms as u128),
        "year" => release_year(track.release_date.as_deref())
          .unwrap_or_default()
          .to_string(),
        _ => String::new(),
      })
      .collect(),
  }
}

pub fn draw_song_table(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  let columns = table_columns(app, TableColumnSet::Songs, layout_chunk.width);
  let header = table_header(&columns);

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::TrackTable,
    current_route.hovered_block == ActiveBlock::TrackTable,
  );

  let (offset, visible) = visible_window_anchored(
    app,
    layout_chunk,
    app.view.track_table_index,
    &app.view.track_table_scroll_offset,
    &app.track_table.tracks,
  );
  let items = visible
    .iter()
    .map(|item| track_table_item(item, &columns, app.strings()))
    .collect::<Vec<TableItem>>();

  let title = if app.is_playlist_track_table_context() {
    if let Some(query) = app.pending_playlist_track_search.as_ref() {
      app.strings().screens.songs_searching(query)
    } else {
      app
        .active_playlist_track_filter
        .as_ref()
        .map(|query| app.strings().screens.songs_filtered(query))
        .unwrap_or_else(|| app.strings().screens.songs.to_string())
    }
  } else {
    app.strings().screens.songs.to_string()
  };

  draw_table(
    f,
    app,
    layout_chunk,
    (&title, &header),
    &items,
    app.view.track_table_index,
    offset,
    highlight_state,
  )
}

#[allow(clippy::too_many_arguments)]
fn draw_table(
  f: &mut Frame<'_>,
  app: &App,
  layout_chunk: Rect,
  table_layout: (&str, &TableHeader), // (title, header colums)
  items: &[TableItem], // Visible window only (see `visible_window`); same length as `header_columns`
  selected_index: usize,
  offset: usize, // Index of `items[0]` within the full backing collection
  highlight_state: (bool, bool),
) {
  let selected_style = get_color(highlight_state, app.user_config.theme)
    .add_modifier(Modifier::BOLD | Modifier::REVERSED);

  let (title, header) = table_layout;
  let playing_uri =
    crate::infra::media_metadata::current_playback_snapshot(app).and_then(|s| s.item_uri);
  // The marker goes on the title cell, or the first cell when the columns omit the title.
  let title_index = header.get_index(ColumnId::Title).unwrap_or(0);

  let rows = items.iter().enumerate().map(|(i, item)| {
    let mut formatted_row = item.format.clone();
    let mut style = app.user_config.theme.base_style(); // default styling

    if playing_uri.is_some() && item.uri == playing_uri {
      mark_playing(
        &mut formatted_row,
        title_index,
        &app.user_config.behavior.playing_icon,
      );
      style = Style::default()
        .fg(app.user_config.theme.active.into())
        .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD));
    }

    // Next check if the item is under selection.
    if Some(i) == selected_index.checked_sub(offset) {
      style = selected_style;
    }

    // Return row styled data
    Row::new(formatted_row).style(style)
  });

  let widths = header
    .items
    .iter()
    .map(|h| Constraint::Length(h.width))
    .collect::<Vec<Constraint>>();

  let table = Table::new(rows, &widths)
    .header(
      Row::new(header.items.iter().map(|h| h.text.as_str()))
        .style(Style::default().fg(app.user_config.theme.header.into())),
    )
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(app.user_config.theme.base_style())
        .title(Span::styled(
          title,
          get_color(highlight_state, app.user_config.theme),
        ))
        .border_style(get_color(highlight_state, app.user_config.theme)),
    )
    .style(app.user_config.theme.base_style());
  f.render_widget(table, layout_chunk);
}

/// Prefix the playing row's title cell with the playing icon.
fn mark_playing(cells: &mut [String], title_index: usize, icon: &str) {
  if let Some(cell) = cells.get_mut(title_index) {
    cell.insert_str(0, &format!("{icon} "));
  }
}

pub fn table_scroll_offset(selected_index: usize, visible_rows: usize) -> usize {
  if visible_rows == 0 {
    return 0;
  }

  selected_index.saturating_sub(visible_rows.saturating_sub(1))
}

#[cfg(test)]
mod tests {
  use super::*;

  use ratatui::{backend::TestBackend, Terminal};

  fn rendered_with(app: &App, area: Rect, draw: fn(&mut Frame<'_>, &App, Rect)) -> String {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| draw(f, app, area)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..area.height)
      .flat_map(|y| (0..area.width).map(move |x| (x, y)))
      .filter_map(|(x, y)| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
      .collect()
  }

  fn track(i: u32) -> TrackInfo {
    TrackInfo {
      generated_name: None,
      uri: Some(format!("qobuz:track:{i}")),
      name: format!("Track {i}"),
      artists: vec![format!("Artist {i}")],
      album: format!("Album {i}"),
      duration_ms: 180_000,
      id: Some(format!("{i:022}")),
      album_id: None,
      artist_refs: Vec::new(),
      is_playable: true,
      is_local: false,
      track_number: i + 1,
      explicit: false,
      image_url: None,
      release_date: None,
    }
  }

  fn column(id: &str) -> crate::core::user_config::ColumnSpec {
    crate::core::user_config::ColumnSpec {
      id: id.to_string(),
      ..Default::default()
    }
  }

  fn dated(release_date: Option<&str>) -> TrackInfo {
    TrackInfo {
      release_date: release_date.map(str::to_string),
      ..track(1)
    }
  }

  #[test]
  fn the_playing_marker_prefixes_the_title_cell_or_the_first_one() {
    let mut cells = vec!["3".to_string(), "Blue".to_string()];
    mark_playing(&mut cells, 1, "▶");
    assert_eq!(cells, ["3", "▶ Blue"]);
    let mut cells = vec!["Joni".to_string()];
    mark_playing(&mut cells, 5, "▶");
    assert_eq!(cells, ["Joni"]);
  }

  #[test]
  fn the_year_column_renders_the_album_release_year() {
    let columns = resolve_columns(
      TableColumnSet::Songs,
      80,
      &[column("title"), column("year")],
    );
    for date in ["1997", "1997-01", "1997-01-20"] {
      assert_eq!(
        track_table_item(&dated(Some(date)), &columns, &crate::core::i18n::EN).format,
        ["Track 1", "1997"],
        "{date}"
      );
    }
  }

  #[test]
  fn the_year_column_is_empty_for_an_unknown_release_date() {
    let columns = resolve_columns(TableColumnSet::Songs, 80, &[column("year")]);
    for date in [None, Some(""), Some("0000")] {
      assert_eq!(
        track_table_item(&dated(date), &columns, &crate::core::i18n::EN).format,
        [""],
        "{date:?}"
      );
    }
  }

  #[test]
  fn a_configured_year_column_shows_in_the_song_table() {
    let mut app = App::default();
    app.user_config.tables.songs = vec![column("title"), column("year")];
    app.track_table.tracks = vec![dated(Some("1997-01-20"))];

    let content = rendered_with(&app, Rect::new(0, 0, 60, 6), draw_song_table);
    assert!(content.contains("Year"), "header: {content}");
    assert!(content.contains("1997"), "row: {content}");
  }

  #[test]
  fn song_table_fills_rows_below_scroll_padding() {
    let mut app = App::default();
    app.track_table.tracks = (0..30).map(track).collect();
    app.view.track_table_index = 15;

    // Height 12 with the default scroll padding of 5: the scroll offset math
    // treats 7 rows as visible, but the drawable area holds 9 data rows
    // (12 minus 2 borders and 1 header). The rows past the padding window
    // must still be filled with the following tracks, not left blank.
    let area = Rect::new(0, 0, 80, 12);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| draw_song_table(f, &app, area)).unwrap();
    let buffer = terminal.backend().buffer();
    let content: String = (0..area.height)
      .flat_map(|y| (0..area.width).map(move |x| (x, y)))
      .filter_map(|(x, y)| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
      .collect();

    for i in [15, 16, 17] {
      assert!(
        content.contains(&format!("Track {i}")),
        "row for track {i} should be rendered: {content}"
      );
    }
    assert!(
      !content.contains("Track 18"),
      "track 18 is below the drawable area and should not be rendered: {content}"
    );
  }

  #[test]
  fn song_table_view_stays_anchored_until_cursor_hits_top() {
    let mut app = App::default();
    app.track_table.tracks = (0..30).map(track).collect();

    let area = Rect::new(0, 0, 80, 12);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let first_data_row = |terminal: &Terminal<TestBackend>| -> String {
      let buffer = terminal.backend().buffer();
      (0..area.width)
        .filter_map(|x| buffer.cell((x, 2)).map(|c| c.symbol().to_string()))
        .collect()
    };

    // Scroll down to row 15: with height 12 and default padding 5 the offset
    // math treats 7 rows as visible, so the view lands at offset 9.
    app.view.track_table_index = 15;
    terminal.draw(|f| draw_song_table(f, &app, area)).unwrap();
    assert!(
      first_data_row(&terminal).contains("Track 9"),
      "view should scroll to offset 9"
    );

    // Moving the cursor back up within the window must NOT move the view.
    app.view.track_table_index = 12;
    terminal.draw(|f| draw_song_table(f, &app, area)).unwrap();
    assert!(
      first_data_row(&terminal).contains("Track 9"),
      "view should stay anchored while the cursor moves inside the window"
    );

    // At the top visible row the view still holds...
    app.view.track_table_index = 9;
    terminal.draw(|f| draw_song_table(f, &app, area)).unwrap();
    assert!(
      first_data_row(&terminal).contains("Track 9"),
      "cursor on the top visible row should not scroll yet"
    );

    // ...and only going past it scrolls the view up.
    app.view.track_table_index = 8;
    terminal.draw(|f| draw_song_table(f, &app, area)).unwrap();
    assert!(
      first_data_row(&terminal).contains("Track 8"),
      "going above the top visible row should scroll the view up"
    );
  }

  #[test]
  fn cached_generated_track_titles_follow_language_in_table_cells_without_renaming_user_tracks() {
    use crate::core::i18n::Language;
    use crate::core::plugin_api::GeneratedTrackName;
    let mut app = App::default();
    let mut generated = track(0);
    generated.name = "Unknown Track".into();
    generated.generated_name = Some(GeneratedTrackName::LocalUnknown);
    let mut user = generated.clone();
    user.generated_name = None;
    let columns = resolve_columns(TableColumnSet::Songs, 80, &[column("title")]);
    for language in [Language::En, Language::PtBr, Language::Fr] {
      app.user_config.behavior.language = language;
      app.refresh_language();
      assert_eq!(
        track_table_item(&generated, &columns, app.strings()).format,
        [app.strings().status.infra.local_unknown_track]
      );
      assert_eq!(
        track_table_item(&user, &columns, app.strings()).format,
        ["Unknown Track"]
      );
      assert_eq!(generated.name, "Unknown Track");
    }
  }
}
