use crate::core::app::{ActiveBlock, App};
use crate::core::source::Source;
use crate::tui::layout::{is_wide_layout, library_constraints, split_input_help_and_settings};
use ratatui::{
  layout::{Constraint, Layout, Rect},
  Frame,
};

use super::{search::draw_input_and_help_box, util::draw_selectable_list};

pub fn draw_library_block(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Library,
    current_route.hovered_block == ActiveBlock::Library,
  );
  let rows: Vec<&str> = app
    .library_rows()
    .iter()
    .map(|target| target.localized_name(app.strings()))
    .collect();
  draw_selectable_list(
    f,
    app,
    layout_chunk,
    app.strings().sidebar.library,
    &rows,
    highlight_state,
    Some(app.library_cursor()),
    &app.view.library_scroll_offset,
  );
}

pub fn draw_playlist_block(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  let strings = &app.strings().sidebar;
  let highlight_state = {
    let current_route = app.get_current_route();
    (
      current_route.active_block == ActiveBlock::MyPlaylists,
      current_route.hovered_block == ActiveBlock::MyPlaylists,
    )
  };

  // Qobuz: the sidebar Playlists panel lists the favorites row, the user's
  // playlists, and the favorite albums, each opening the shared track table.
  {
    let items: Vec<String> = if app.qobuz_playlists().is_empty() {
      vec![]
    } else {
      app
        .qobuz_playlists()
        .iter()
        .map(|p| {
          strings
            .playlist_label(Source::Qobuz, &p.uri, &p.name)
            .to_string()
        })
        .collect()
    };
    draw_selectable_list(
      f,
      app,
      layout_chunk,
      strings.qobuz,
      &items,
      highlight_state,
      app.view.selected_playlist_index,
      &app.view.playlist_scroll_offset,
    );
  }
}

pub fn draw_user_block(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  let lib_constraints = library_constraints(&app.runtime_state);
  if is_wide_layout(app) {
    let [input_area, library_area, playlist_area] = layout_chunk.layout(&Layout::vertical([
      Constraint::Length(3),
      lib_constraints[0],
      lib_constraints[1],
    ]));

    // Search input and help
    let [input_text_area, help_area, settings_area] =
      split_input_help_and_settings(app, input_area);
    draw_input_and_help_box(f, app, input_text_area, help_area, settings_area);
    draw_library_block(f, app, library_area);
    draw_playlist_block(f, app, playlist_area);
  } else {
    let [library_area, playlist_area] = layout_chunk.layout(&Layout::vertical(lib_constraints));
    draw_library_block(f, app, library_area);
    draw_playlist_block(f, app, playlist_area);
  }
}

#[cfg(test)]
mod tests {}
