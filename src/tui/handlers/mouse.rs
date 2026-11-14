use super::{common_key_events, library, lyrics_view, playbar, playlist, settings};
use crate::core::action::{Action, NavTarget};
use crate::core::app::{ActiveBlock, App, RouteId, SettingValue, SettingsCategory};
use crate::core::pane::Pane;
use crate::tui::event::Key;
use crate::tui::layout::{
  compute_main_layout, compute_pane_layout, fullscreen_view_layout, miniplayer_playbar_area,
  pane_close_mark, MainLayoutAreas,
};
use crate::tui::ui::player::{playbar_control_at, playbar_progress_line};
use crate::tui::ui::settings::{
  filtered_setting_indices, selected_setting_position, settings_layout,
};
use crate::tui::ui::tables::table_scroll_offset;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const SETTINGS_UNSAVED_PROMPT_WIDTH: u16 = 58;
const SETTINGS_UNSAVED_PROMPT_HEIGHT: u16 = 9;

pub fn handler(mouse: MouseEvent, app: &mut App) {
  let current_route = app.get_current_route();
  let current_route_id = current_route.id.clone();
  let current_active_block = current_route.active_block;

  if current_active_block == ActiveBlock::Settings {
    handle_settings_screen_mouse(mouse, app);
    return;
  }

  if current_route_id == RouteId::LyricsView {
    let playbar_area = fullscreen_view_playbar_area(app);
    let over_playbar =
      playbar_area.is_some_and(|area| rect_contains(area, mouse.column, mouse.row));
    if !over_playbar {
      match mouse.kind {
        MouseEventKind::ScrollUp => {
          lyrics_view::scroll_by(app, -1);
          return;
        }
        MouseEventKind::ScrollDown => {
          lyrics_view::scroll_by(app, 1);
          return;
        }
        _ => {}
      }
    }
    handle_fullscreen_playbar_mouse(ActiveBlock::LyricsView, playbar_area, mouse, app);
    return;
  }

  if current_route_id == RouteId::CoverArtView {
    handle_fullscreen_playbar_mouse(
      ActiveBlock::CoverArtView,
      fullscreen_view_playbar_area(app),
      mouse,
      app,
    );
    return;
  }

  if current_route_id == RouteId::MiniPlayer {
    handle_fullscreen_playbar_mouse(
      ActiveBlock::MiniPlayer,
      miniplayer_view_playbar_area(app),
      mouse,
      app,
    );
    return;
  }

  if !is_main_layout_mouse_interactive(current_active_block) {
    return;
  }

  if handle_pane_mouse(mouse, app) {
    return;
  }

  let Some(areas) = main_layout_areas(app) else {
    return;
  };

  if let Some(input_area) = areas.input {
    if rect_contains(input_area, mouse.column, mouse.row) {
      handle_input_mouse(mouse, input_area, app);
      return;
    }
  }

  if let Some(help_area) = areas.help {
    if rect_contains(help_area, mouse.column, mouse.row) {
      handle_help_mouse(mouse, app);
      return;
    }
  }

  if let Some(settings_area) = areas.settings {
    if rect_contains(settings_area, mouse.column, mouse.row) {
      handle_settings_mouse(mouse, app);
      return;
    }
  }

  if rect_contains(areas.playbar, mouse.column, mouse.row) {
    handle_playbar_mouse(mouse, areas.playbar, Some(ActiveBlock::PlayBar), app);
    return;
  }

  if rect_contains(areas.library, mouse.column, mouse.row) {
    handle_library_mouse(mouse, areas.library, app);
    return;
  }

  if rect_contains(areas.playlists, mouse.column, mouse.row) {
    handle_playlist_mouse(mouse, areas.playlists, app);
    return;
  }

  if rect_contains(areas.content, mouse.column, mouse.row) {
    handle_content_table_mouse(mouse, areas.content, current_route_id, app);
  }
}

fn handle_library_mouse(mouse: MouseEvent, list_area: Rect, app: &mut App) {
  match mouse.kind {
    MouseEventKind::ScrollDown => {
      focus_library(app);
      library::select_next(app);
    }
    MouseEventKind::ScrollUp => {
      focus_library(app);
      library::select_previous(app);
    }
    MouseEventKind::Down(MouseButton::Left) => {
      focus_library(app);
      select_clicked_library_item(mouse.row, list_area, app);
    }
    _ => {}
  }
}

fn handle_playlist_mouse(mouse: MouseEvent, list_area: Rect, app: &mut App) {
  match mouse.kind {
    MouseEventKind::ScrollDown => {
      focus_playlists(app);
      playlist::select_next(app);
    }
    MouseEventKind::ScrollUp => {
      focus_playlists(app);
      playlist::select_previous(app);
    }
    MouseEventKind::Down(MouseButton::Left) => {
      focus_playlists(app);
      select_clicked_playlist(mouse.row, list_area, app);
    }
    _ => {}
  }
}

fn handle_content_table_mouse(
  mouse: MouseEvent,
  table_area: Rect,
  route_id: RouteId,
  app: &mut App,
) {
  let Some(active_block) = common_key_events::content_active_block_for_route(&route_id) else {
    return;
  };

  if !matches!(
    mouse.kind,
    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp | MouseEventKind::Down(MouseButton::Left)
  ) {
    return;
  }

  let item_count = content_table_item_count(active_block, app);
  if item_count == 0 {
    return;
  }

  match mouse.kind {
    MouseEventKind::ScrollDown => {
      focus_content_table(active_block, app);
      super::handle_block_events(Key::Down, app);
    }
    MouseEventKind::ScrollUp => {
      focus_content_table(active_block, app);
      super::handle_block_events(Key::Up, app);
    }
    MouseEventKind::Down(MouseButton::Left) => {
      focus_content_table(active_block, app);
      select_clicked_content_table_item(active_block, mouse.row, table_area, item_count, app);
    }
    _ => {}
  }
}

fn handle_input_mouse(mouse: MouseEvent, input_area: Rect, app: &mut App) {
  if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
    return;
  }

  if super::open_global_search(app) {
    set_input_cursor_from_column(input_area, mouse.column, app);
  }
}

fn handle_help_mouse(mouse: MouseEvent, app: &mut App) {
  if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
    super::help_menu::open(app);
  }
}

fn handle_settings_mouse(mouse: MouseEvent, app: &mut App) {
  if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
    app.apply(Action::Navigate(NavTarget::Settings));
  }
}

fn handle_fullscreen_playbar_mouse(
  focus_block: ActiveBlock,
  playbar_area: Option<Rect>,
  mouse: MouseEvent,
  app: &mut App,
) {
  let Some(playbar_area) = playbar_area else {
    return;
  };

  if rect_contains(playbar_area, mouse.column, mouse.row) {
    handle_playbar_mouse(mouse, playbar_area, Some(focus_block), app);
  }
}

/// Close marks, border drags and clicks in a detached pane; false leaves the event to `Main`.
fn handle_pane_mouse(mouse: MouseEvent, app: &mut App) -> bool {
  let Some(layout) = compute_pane_layout(app) else {
    app.view.dragged_pane_split = None;
    return false;
  };
  let (column, row) = (mouse.column, mouse.row);
  match mouse.kind {
    MouseEventKind::Drag(MouseButton::Left) => {
      if let Some(index) = app.view.dragged_pane_split {
        match layout.borders.iter().find(|border| border.index == index) {
          Some(border) => {
            let percent = border.percent_at(column, row);
            app.change_pane_layout(|tree| tree.set_split_percent(index, percent));
          }
          None => app.view.dragged_pane_split = None,
        }
        return true;
      }
    }
    MouseEventKind::Up(MouseButton::Left) => {
      if app.view.dragged_pane_split.take().is_some() {
        return true;
      }
    }
    MouseEventKind::Down(MouseButton::Left) => {
      app.view.dragged_pane_split = None;
      let closed = layout.panes.iter().find(|(pane, area)| {
        *pane != Pane::Main
          && pane_close_mark(*area).is_some_and(|mark| rect_contains(mark, column, row))
      });
      if let Some((pane, _)) = closed {
        let pane = *pane;
        app.change_pane_layout(|tree| tree.close(pane));
        return true;
      }
      if let Some(border) = layout
        .borders
        .iter()
        .find(|border| border.is_hit(column, row))
      {
        app.view.dragged_pane_split = Some(border.index);
        return true;
      }
    }
    _ => {}
  }

  let Some((pane, area)) = layout
    .panes
    .into_iter()
    .find(|(_, area)| rect_contains(*area, column, row))
  else {
    return false;
  };
  if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
    app.view.focused_pane = pane;
  }
  match pane {
    Pane::Main | Pane::NowPlaying => return false,
    Pane::Lyrics => match mouse.kind {
      MouseEventKind::ScrollUp => lyrics_view::scroll_by(app, -1),
      MouseEventKind::ScrollDown => lyrics_view::scroll_by(app, 1),
      _ => {}
    },
    Pane::MiniPlayer => handle_playbar_mouse(mouse, area, None, app),
    Pane::Visualizer | Pane::CoverArt | Pane::Queue => {}
  }
  true
}

fn handle_playbar_mouse(
  mouse: MouseEvent,
  playbar_area: Rect,
  focus_block: Option<ActiveBlock>,
  app: &mut App,
) {
  match mouse.kind {
    MouseEventKind::Down(MouseButton::Left) => {
      // Control buttons take precedence; they sit on a different row than the gauge.
      if let Some(control) = playbar_control_at(app, playbar_area, mouse.column, mouse.row) {
        focus_playbar(focus_block, app);
        playbar::handle_control(control, app);
        return;
      }

      // Otherwise, a click on the progress line seeks to that position (#157).
      if let Some(line) = playbar_progress_line(app, playbar_area) {
        if line.contains(mouse.column, mouse.row) {
          focus_playbar(focus_block, app);
          app.apply(Action::SeekTo(line.position_at(mouse.column)));
        }
      }
    }
    // Dragging along the progress row scrubs; the column is clamped into the line.
    MouseEventKind::Drag(MouseButton::Left) => {
      if let Some(line) = playbar_progress_line(app, playbar_area) {
        if line.on_row(mouse.row) {
          focus_playbar(focus_block, app);
          app.apply(Action::SeekTo(line.position_at(mouse.column)));
        }
      }
    }
    _ => {}
  }
}

fn handle_settings_screen_mouse(mouse: MouseEvent, app: &mut App) {
  if app.view.settings_unsaved_prompt_visible {
    handle_unsaved_settings_prompt_mouse(mouse, app);
    return;
  }

  let Some(areas) = settings_layout_areas(app) else {
    return;
  };

  if rect_contains(areas.tabs, mouse.column, mouse.row) {
    handle_settings_tabs_mouse(mouse, areas.tabs, app);
    return;
  }

  if rect_contains(areas.list, mouse.column, mouse.row) {
    handle_settings_list_mouse(mouse, areas.list, app);
  }
}

fn handle_unsaved_settings_prompt_mouse(mouse: MouseEvent, app: &mut App) {
  if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
    return;
  }

  let Some(areas) = settings_unsaved_prompt_areas(app) else {
    return;
  };

  if rect_contains(areas.yes, mouse.column, mouse.row) {
    settings::save_and_close(app);
    return;
  }

  if rect_contains(areas.no, mouse.column, mouse.row) {
    settings::close_settings(app);
  }
}

fn handle_settings_tabs_mouse(mouse: MouseEvent, tabs_area: Rect, app: &mut App) {
  match mouse.kind {
    MouseEventKind::ScrollDown => switch_settings_category_right(app),
    MouseEventKind::ScrollUp => switch_settings_category_left(app),
    MouseEventKind::Down(MouseButton::Left) => {
      if let Some(clicked_tab_index) =
        settings_tab_index_from_click(tabs_area, mouse.column, mouse.row, app.strings())
      {
        switch_settings_category_to(clicked_tab_index, app);
      }
    }
    _ => {}
  }
}

fn handle_settings_list_mouse(mouse: MouseEvent, list_area: Rect, app: &mut App) {
  // Reaching for the list ends the query. Without this the row activation
  // below would be spent confirming the filter instead of activating the row
  // the click landed on, and a scroll would be swallowed as an unhandled
  // filter-input key.
  if app.view.settings_filter_editing
    && matches!(
      mouse.kind,
      MouseEventKind::ScrollUp
        | MouseEventKind::ScrollDown
        | MouseEventKind::Down(MouseButton::Left)
    )
  {
    app.apply_settings_filter();
  }

  match mouse.kind {
    MouseEventKind::ScrollDown if !selected_setting_expects_key_capture(app) => {
      settings::step_list(app, true);
    }
    MouseEventKind::ScrollUp if !selected_setting_expects_key_capture(app) => {
      settings::step_list(app, false);
    }
    MouseEventKind::Down(MouseButton::Left) => {
      select_clicked_setting(mouse.row, list_area, app);
    }
    _ => {}
  }
}

fn selected_setting_expects_key_capture(app: &App) -> bool {
  app.view.settings_edit_mode
    && app
      .settings_items
      .get(app.view.settings_selected_index)
      .map(|setting| matches!(setting.value, SettingValue::Key(_)))
      .unwrap_or(false)
}

fn select_clicked_setting(mouse_row: u16, list_area: Rect, app: &mut App) {
  // Rows are drawn from the filtered view, so a click resolves to a position in
  // it before it means anything as an index into `settings_items`. Use the last
  // frame's offset because the selection can sit anywhere in the viewport.
  let visible = filtered_setting_indices(app);
  let selected_position = selected_setting_position(app).unwrap_or(0);
  let Some(clicked_row) = list_item_index_from_click(
    list_area,
    mouse_row,
    selected_position,
    Some(app.view.settings_scroll_offset.get()),
    visible.len(),
  ) else {
    return;
  };
  let clicked_index = visible[clicked_row];

  if app.view.settings_edit_mode {
    let clicked_selected_item = clicked_index == app.view.settings_selected_index;
    if clicked_selected_item {
      if selected_setting_expects_key_capture(app) {
        settings::cancel_edit(app);
      } else {
        settings::confirm_edit(app);
      }
    } else {
      app.view.settings_selected_index = clicked_index;
      app.view.settings_edit_mode = false;
      app.view.settings_edit_buffer.clear();
    }
    return;
  }

  let was_selected = app.view.settings_selected_index == clicked_index;
  app.view.settings_selected_index = clicked_index;
  if was_selected {
    settings::enter_edit_mode(app);
  }
}

fn switch_settings_category_left(app: &mut App) {
  let current_index = app.view.settings_category.index();
  let new_index = if current_index == 0 {
    SettingsCategory::all().len() - 1
  } else {
    current_index - 1
  };
  switch_settings_category_to(new_index, app);
}

fn switch_settings_category_right(app: &mut App) {
  let current_index = app.view.settings_category.index();
  let new_index = (current_index + 1) % SettingsCategory::all().len();
  switch_settings_category_to(new_index, app);
}

fn switch_settings_category_to(index: usize, app: &mut App) {
  let category = SettingsCategory::from_index(index);
  if app.view.settings_category != category || app.settings_items.is_empty() {
    app.view.settings_category = category;
    settings::reload_category(app);
  }

  if app.view.settings_edit_mode {
    app.view.settings_edit_mode = false;
    app.view.settings_edit_buffer.clear();
  }
}

fn is_main_layout_mouse_interactive(active_block: ActiveBlock) -> bool {
  !matches!(
    active_block,
    ActiveBlock::HelpMenu
      | ActiveBlock::Queue
      | ActiveBlock::Error
      | ActiveBlock::Analysis
      | ActiveBlock::LyricsView
      | ActiveBlock::CoverArtView
      | ActiveBlock::MiniPlayer
      | ActiveBlock::AnnouncementPrompt
      | ActiveBlock::ExitPrompt
      | ActiveBlock::Settings
      | ActiveBlock::Dialog(_)
      | ActiveBlock::SortMenu
      | ActiveBlock::RecapPrompt
  )
}

fn focus_playlists(app: &mut App) {
  app.set_current_route_state(
    Some(ActiveBlock::MyPlaylists),
    Some(ActiveBlock::MyPlaylists),
  );
}

fn focus_library(app: &mut App) {
  app.set_current_route_state(Some(ActiveBlock::Library), Some(ActiveBlock::Library));
}

fn focus_content_table(active_block: ActiveBlock, app: &mut App) {
  app.set_current_route_state(Some(active_block), Some(active_block));
}

fn focus_playbar(block: Option<ActiveBlock>, app: &mut App) {
  let Some(block) = block else {
    return;
  };
  if block == ActiveBlock::PlayBar {
    common_key_events::remember_playbar_origin(app);
  }
  app.set_current_route_state(Some(block), Some(block));
}

fn set_input_cursor_from_column(input_area: Rect, mouse_column: u16, app: &mut App) {
  let inner_start = input_area.x.saturating_add(1);
  let target_col = mouse_column
    .saturating_sub(inner_start)
    .saturating_add(app.view.input_scroll_offset.get()) as usize;

  let mut width = 0usize;
  let mut idx = 0usize;
  for (i, ch) in app.view.input.iter().enumerate() {
    let ch_width = UnicodeWidthChar::width(*ch).unwrap_or(1);
    if width + ch_width > target_col {
      idx = i;
      break;
    }
    width += ch_width;
    idx = i + 1;
  }

  app.view.input_idx = idx;
  app.view.input_cursor_position = width as u16;
}

fn select_clicked_library_item(mouse_row: u16, list_area: Rect, app: &mut App) {
  let item_count = app.library_rows().len();
  let selected_index = app.library_cursor();

  let Some(clicked_index) = list_item_index_from_click(
    list_area,
    mouse_row,
    selected_index,
    Some(app.view.library_scroll_offset.get()),
    item_count,
  ) else {
    return;
  };

  let was_selected = selected_index == clicked_index;
  library::select_row(app, clicked_index);

  if was_selected {
    library::activate_selected(app);
  }
}

fn select_clicked_playlist(mouse_row: u16, list_area: Rect, app: &mut App) {
  // The rendered row count, including the leading "+ Add Playlist" row under
  // Spotify (and the trailing "+ New Playlist" row under YouTube).
  let item_count = playlist::total_display_count(app);
  if item_count == 0 {
    return;
  }

  let selected_index = app
    .view
    .selected_playlist_index
    .unwrap_or(0)
    .min(item_count.saturating_sub(1));

  let Some(clicked_index) = list_item_index_from_click(
    list_area,
    mouse_row,
    selected_index,
    Some(app.view.playlist_scroll_offset.get()),
    item_count,
  ) else {
    return;
  };

  let was_selected = app.view.selected_playlist_index == Some(clicked_index);
  app.view.selected_playlist_index = Some(clicked_index);

  // Open the selected item when clicking it again.
  if was_selected {
    playlist::activate_selected(app);
  }
}

fn select_clicked_content_table_item(
  active_block: ActiveBlock,
  mouse_row: u16,
  table_area: Rect,
  item_count: usize,
  app: &mut App,
) {
  let selected_index =
    content_table_selected_index(active_block, app).min(item_count.saturating_sub(1));

  // The track table anchors its view to a persisted scroll offset (the cursor
  // can sit anywhere in the window), so clicks must use that offset instead of
  // re-deriving one from the selection.
  let anchored_offset = match active_block {
    ActiveBlock::TrackTable => Some(app.view.track_table_scroll_offset.get()),
    _ => None,
  };

  let Some(clicked_index) = table_item_index_from_click(
    table_area,
    mouse_row,
    selected_index,
    anchored_offset,
    item_count,
  ) else {
    return;
  };

  let was_selected = content_table_selected_index(active_block, app) == clicked_index;
  set_content_table_selected_index(active_block, clicked_index, app);
  if was_selected {
    super::handle_block_events(Key::Enter, app);
  }
}

fn content_table_item_count(active_block: ActiveBlock, app: &App) -> usize {
  match active_block {
    ActiveBlock::TrackTable => app.track_table.tracks.len(),
    _ => 0,
  }
}

fn content_table_selected_index(active_block: ActiveBlock, app: &App) -> usize {
  match active_block {
    ActiveBlock::TrackTable => app.view.track_table_index,
    _ => 0,
  }
}

fn set_content_table_selected_index(active_block: ActiveBlock, index: usize, app: &mut App) {
  if active_block == ActiveBlock::TrackTable {
    app.view.track_table_index = index;
  }
}

fn list_item_index_from_click(
  list_area: Rect,
  mouse_row: u16,
  selected_index: usize,
  anchored_offset: Option<usize>,
  item_count: usize,
) -> Option<usize> {
  if item_count == 0 || list_area.height <= 2 {
    return None;
  }

  let viewport_height = list_area.height.saturating_sub(2) as usize;
  if viewport_height == 0 {
    return None;
  }

  let inner_top = list_area.y.saturating_add(1);
  let inner_bottom_exclusive = list_area
    .y
    .saturating_add(list_area.height)
    .saturating_sub(1);

  if mouse_row < inner_top || mouse_row >= inner_bottom_exclusive {
    return None;
  }

  let offset = anchored_offset.unwrap_or_else(|| {
    selected_index
      .saturating_add(1)
      .saturating_sub(viewport_height)
  });
  let row_index = (mouse_row - inner_top) as usize;
  let clicked_index = offset + row_index;

  (clicked_index < item_count).then_some(clicked_index)
}

fn table_item_index_from_click(
  table_area: Rect,
  mouse_row: u16,
  selected_index: usize,
  anchored_offset: Option<usize>,
  item_count: usize,
) -> Option<usize> {
  if item_count == 0 || table_area.height <= 5 {
    return None;
  }

  let visible_rows = table_area.height.saturating_sub(5) as usize;
  if visible_rows == 0 {
    return None;
  }

  let first_data_row = table_area.y.saturating_add(2);
  let last_data_row_exclusive = first_data_row.saturating_add(visible_rows as u16);
  if mouse_row < first_data_row || mouse_row >= last_data_row_exclusive {
    return None;
  }

  let offset = anchored_offset.unwrap_or_else(|| table_scroll_offset(selected_index, visible_rows));

  let row_index = (mouse_row - first_data_row) as usize;
  let row_index = row_index.min(visible_rows.saturating_sub(1));
  let clicked_index = (offset + row_index).min(item_count.saturating_sub(1));

  Some(clicked_index)
}

fn settings_tab_index_from_click(
  tabs_area: Rect,
  mouse_column: u16,
  mouse_row: u16,
  strings: &crate::core::i18n::Strings,
) -> Option<usize> {
  if tabs_area.width <= 2 || tabs_area.height <= 2 {
    return None;
  }

  // `Tabs` renders a single row in the block's inner area.
  let tab_row = tabs_area.y.saturating_add(1);
  if mouse_row != tab_row {
    return None;
  }

  let inner_left = tabs_area.x.saturating_add(1);
  let inner_width = tabs_area.width.saturating_sub(2);
  if inner_width == 0 {
    return None;
  }

  if mouse_column < inner_left {
    return None;
  }

  let relative_column = (mouse_column - inner_left) as usize;
  if relative_column >= inner_width as usize {
    return None;
  }

  // Match ratatui's default Tabs layout in `render_tabs`: left padding + title + right padding,
  // then divider between tabs.
  let left_padding_width = 1usize;
  let right_padding_width = 1usize;
  let divider_width = 1usize;

  let mut cursor = 0usize;
  let categories = SettingsCategory::all();
  for (index, category) in categories.iter().enumerate() {
    let tab_start = cursor;
    let tab_width =
      left_padding_width + category.localized_name(strings).width() + right_padding_width;
    let tab_end = tab_start.saturating_add(tab_width);

    if relative_column >= tab_start && relative_column < tab_end {
      return Some(index);
    }

    cursor = tab_end;
    let is_last_tab = index + 1 == categories.len();
    if !is_last_tab {
      let divider_end = cursor.saturating_add(divider_width);

      // Divider clicks still map to a nearby tab for a forgiving UX.
      if relative_column >= cursor && relative_column < divider_end {
        return Some(index);
      }

      cursor = divider_end;
    }

    if cursor >= inner_width as usize {
      break;
    }
  }

  None
}

struct SettingsLayoutAreas {
  tabs: Rect,
  list: Rect,
}

struct SettingsUnsavedPromptAreas {
  yes: Rect,
  no: Rect,
}

fn settings_layout_areas(app: &App) -> Option<SettingsLayoutAreas> {
  if app.view.size.width == 0 || app.view.size.height == 0 {
    return None;
  }

  let root = Rect::new(0, 0, app.view.size.width, app.view.size.height);
  let [tabs_area, list_area, _help_area] = settings_layout(root);

  Some(SettingsLayoutAreas {
    tabs: tabs_area,
    list: list_area,
  })
}

fn settings_unsaved_prompt_areas(app: &App) -> Option<SettingsUnsavedPromptAreas> {
  if app.view.size.width == 0 || app.view.size.height == 0 {
    return None;
  }

  let bounds = Rect::new(0, 0, app.view.size.width, app.view.size.height);
  let width = std::cmp::min(
    bounds.width.saturating_sub(4),
    SETTINGS_UNSAVED_PROMPT_WIDTH,
  );
  if width == 0 {
    return None;
  }

  let height = SETTINGS_UNSAVED_PROMPT_HEIGHT.min(bounds.height.saturating_sub(2).max(1));
  let left = bounds.x + bounds.width.saturating_sub(width) / 2;
  let top = bounds.y + bounds.height.saturating_sub(height) / 2;
  let rect = Rect::new(left, top, width, height);

  let [_message_area, buttons_area, _hint_area] = rect.layout(
    &Layout::vertical([
      Constraint::Min(2),
      Constraint::Length(3),
      Constraint::Length(1),
    ])
    .margin(1),
  );

  let [yes_area, no_area] = buttons_area.layout(
    &Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).horizontal_margin(3),
  );

  Some(SettingsUnsavedPromptAreas {
    yes: yes_area,
    no: no_area,
  })
}

fn fullscreen_view_playbar_area(app: &App) -> Option<Rect> {
  if app.view.size.width == 0 || app.view.size.height == 0 {
    return None;
  }

  let root = Rect::new(0, 0, app.view.size.width, app.view.size.height);
  let (_, playbar_area) = fullscreen_view_layout(&app.runtime_state, root);
  playbar_area
}

fn miniplayer_view_playbar_area(app: &App) -> Option<Rect> {
  if app.view.size.width == 0 || app.view.size.height == 0 {
    return None;
  }

  Some(miniplayer_playbar_area(Rect::new(
    0,
    0,
    app.view.size.width,
    app.view.size.height,
  )))
}

fn main_layout_areas(app: &App) -> Option<MainLayoutAreas> {
  compute_main_layout(app)
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
  let right = rect.x.saturating_add(rect.width);
  let bottom = rect.y.saturating_add(rect.height);
  x >= rect.x && x < right && y >= rect.y && y < bottom
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::geometry::Viewport;

  use crossterm::event::{KeyModifiers, MouseEvent};

  fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
      kind,
      column,
      row,
      modifiers: KeyModifiers::NONE,
    }
  }

  fn open_settings(app: &mut App) {
    app.view.settings_category = SettingsCategory::Behavior;
    app.load_settings_for_category();
    app.push_navigation_stack(RouteId::Settings, ActiveBlock::Settings);
  }

  /// Settings, sized for the mouse tests, with `query` typed into the row
  /// filter through the real key path and the input still open.
  fn filtered_settings_app(query: &str) -> App {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);

    crate::tui::handlers::handle_app(app.user_config.keys.search, &mut app);
    for character in query.chars() {
      crate::tui::handlers::handle_app(Key::Char(character), &mut app);
    }
    app
  }

  #[test]
  fn click_settings_opens_settings_screen() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };

    let areas = main_layout_areas(&app).expect("layout areas");
    let settings = areas.settings.expect("settings area");

    handler(
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        settings.x + 1,
        settings.y + 1,
      ),
      &mut app,
    );

    let route = app.get_current_route();
    assert_eq!(route.id, RouteId::Settings);
    assert_eq!(route.active_block, ActiveBlock::Settings);
    assert!(!app.settings_items.is_empty());
  }

  #[test]
  fn fullscreen_view_playbar_area_uses_runtime_state_height() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    app.runtime_state.playbar_height_rows = 8;

    let playbar_area = fullscreen_view_playbar_area(&app).expect("fullscreen playbar area");

    assert_eq!(playbar_area, Rect::new(0, 42, 160, 8));
  }

  #[test]
  fn fullscreen_view_playbar_area_is_hidden_when_height_is_zero() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    app.runtime_state.playbar_height_rows = 0;

    assert!(fullscreen_view_playbar_area(&app).is_none());
  }

  #[test]
  fn click_settings_tab_switches_category() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);

    let areas = settings_layout_areas(&app).expect("settings layout areas");
    let inner_left = areas.tabs.x + 1;
    let left_padding = 1u16;
    let right_padding = 1u16;
    let divider = 1u16;
    let behavior_tab_width = left_padding
      + SettingsCategory::Behavior
        .localized_name(&crate::core::i18n::EN)
        .len() as u16
      + right_padding;
    let icons_tab_width = left_padding
      + SettingsCategory::Icons
        .localized_name(&crate::core::i18n::EN)
        .len() as u16
      + right_padding;
    let keybindings_tab_width = left_padding
      + SettingsCategory::Keybindings
        .localized_name(&crate::core::i18n::EN)
        .len() as u16
      + right_padding;
    let theme_title_start = inner_left
      + behavior_tab_width
      + divider
      + icons_tab_width
      + divider
      + keybindings_tab_width
      + divider
      + left_padding;
    let theme_tab_x = theme_title_start + 1;
    let tab_y = areas.tabs.y + 1;

    handler(
      mouse_event(MouseEventKind::Down(MouseButton::Left), theme_tab_x, tab_y),
      &mut app,
    );

    assert_eq!(app.view.settings_category, SettingsCategory::Theme);
    assert_eq!(app.view.settings_selected_index, 0);
    assert!(!app.settings_items.is_empty());
  }

  #[test]
  fn chinese_settings_tabs_use_the_same_cell_width_for_drawing_and_clicks() {
    use crate::core::i18n::{FR, PT_BR};
    for strings in [&PT_BR, &FR] {
      let area = Rect::new(3, 2, 80, 3);
      let mut column = area.x + 1;
      for (index, category) in SettingsCategory::all().iter().enumerate() {
        let width = category.localized_name(strings).width() as u16 + 2;
        for x in column..column + width {
          assert_eq!(
            settings_tab_index_from_click(area, x, area.y + 1, strings),
            Some(index)
          );
        }
        column += width + 1;
      }
      assert_eq!(
        settings_tab_index_from_click(area, column, area.y + 1, strings),
        None
      );
    }
  }

  #[test]
  fn scroll_in_settings_list_changes_selected_item() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);

    let areas = settings_layout_areas(&app).expect("settings layout areas");
    let x = areas.list.x + 2;
    let y = areas.list.y + 2;

    assert_eq!(app.view.settings_selected_index, 0);
    handler(mouse_event(MouseEventKind::ScrollDown, x, y), &mut app);
    assert_eq!(app.view.settings_selected_index, 1);

    handler(mouse_event(MouseEventKind::ScrollUp, x, y), &mut app);
    assert_eq!(app.view.settings_selected_index, 0);
  }

  #[test]
  fn clicking_selected_bool_setting_toggles_value() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);

    let bool_index = app
      .settings_items
      .iter()
      .position(|setting| matches!(setting.value, SettingValue::Bool(_)))
      .expect("expected a boolean setting");
    app.view.settings_selected_index = bool_index;

    let initial_value = app
      .settings_items
      .get(bool_index)
      .and_then(|setting| match setting.value {
        SettingValue::Bool(value) => Some(value),
        _ => None,
      })
      .expect("selected setting should be boolean");

    let areas = settings_layout_areas(&app).expect("settings layout areas");
    let drawn_row = filtered_setting_indices(&app)
      .iter()
      .position(|index| *index == bool_index)
      .expect("the boolean row is drawn");
    let y = areas.list.y + 1 + drawn_row as u16;
    handler(
      mouse_event(MouseEventKind::Down(MouseButton::Left), areas.list.x + 2, y),
      &mut app,
    );

    let updated_value = app
      .settings_items
      .get(bool_index)
      .and_then(|setting| match setting.value {
        SettingValue::Bool(value) => Some(value),
        _ => None,
      })
      .expect("selected setting should stay boolean");
    assert_ne!(updated_value, initial_value);
    assert!(!app.view.settings_edit_mode);
  }

  #[test]
  fn clicking_a_filtered_row_closes_the_filter_and_selects_the_row_that_was_drawn() {
    // No Enter: the filter input is still open when the click lands.
    let mut app = filtered_settings_app("sort");
    let visible = filtered_setting_indices(&app);
    assert!(visible.len() > 1, "expected several sort rows");

    // Second drawn row, which is not the second row of `settings_items`.
    let areas = settings_layout_areas(&app).expect("settings layout areas");
    handler(
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        areas.list.x + 2,
        areas.list.y + 2,
      ),
      &mut app,
    );

    assert!(!app.view.settings_filter_editing);
    // Applying is not clearing: the rows stay filtered after the click.
    assert_eq!(app.view.settings_filter, "sort");
    assert_eq!(app.view.settings_selected_index, visible[1]);
    assert_ne!(visible[1], 1);
  }

  #[test]
  fn clicking_the_top_row_of_a_scrolled_settings_list_selects_the_row_that_was_drawn() {
    use crate::tui::ui::settings::draw_settings;
    use ratatui::{backend::TestBackend, Terminal};

    let mut app = App::default();
    app.view.size = Viewport {
      width: 100,
      height: 20,
    };
    open_settings(&mut app);
    let visible = filtered_setting_indices(&app);
    let areas = settings_layout_areas(&app).expect("settings layout areas");
    let inner_height = (areas.list.height - 2) as usize;
    assert!(
      visible.len() > inner_height + 2,
      "the list must overflow its box to scroll"
    );

    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    app.view.settings_selected_index = *visible.last().unwrap();
    terminal.draw(|f| draw_settings(f, &app)).unwrap();
    crate::tui::handlers::handle_app(Key::Up, &mut app);
    crate::tui::handlers::handle_app(Key::Up, &mut app);
    terminal.draw(|f| draw_settings(f, &app)).unwrap();

    handler(
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        areas.list.x + 2,
        areas.list.y + 1,
      ),
      &mut app,
    );

    assert_eq!(
      app.view.settings_selected_index,
      visible[app.view.settings_scroll_offset.get()]
    );
    assert!(!app.view.settings_edit_mode);
  }

  #[test]
  fn clicking_a_settings_row_before_the_list_scrolls_selects_the_row_under_the_pointer() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 100,
      height: 20,
    };
    open_settings(&mut app);
    let visible = filtered_setting_indices(&app);
    assert!(visible.len() > 2, "expected several Behavior rows");
    app.view.settings_selected_index = visible[0];

    let areas = settings_layout_areas(&app).expect("settings layout areas");
    handler(
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        areas.list.x + 2,
        areas.list.y + 3,
      ),
      &mut app,
    );

    assert_eq!(app.view.settings_selected_index, visible[2]);
  }

  #[test]
  fn keybinding_capture_can_be_cancelled_with_mouse_click() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);
    app.view.settings_category = SettingsCategory::Keybindings;
    app.load_settings_for_category();
    app.view.settings_selected_index = 0;

    let original_value = app
      .settings_items
      .first()
      .and_then(|setting| match &setting.value {
        SettingValue::Key(value) => Some(value.clone()),
        _ => None,
      })
      .expect("first keybinding should be a key setting");

    let areas = settings_layout_areas(&app).expect("settings layout areas");
    let x = areas.list.x + 2;
    let y = areas.list.y + 1;

    handler(
      mouse_event(MouseEventKind::Down(MouseButton::Left), x, y),
      &mut app,
    );
    assert!(app.view.settings_edit_mode);

    handler(
      mouse_event(MouseEventKind::Down(MouseButton::Left), x, y),
      &mut app,
    );

    let current_value = app
      .settings_items
      .first()
      .and_then(|setting| match &setting.value {
        SettingValue::Key(value) => Some(value.clone()),
        _ => None,
      })
      .expect("first keybinding should still be a key setting");

    assert!(!app.view.settings_edit_mode);
    assert_eq!(current_value, original_value);
  }

  #[test]
  fn click_no_on_unsaved_prompt_discards_changes_and_exits() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);

    let bool_index = app
      .settings_items
      .iter()
      .position(|setting| matches!(setting.value, SettingValue::Bool(_)))
      .expect("expected a boolean setting");
    app.view.settings_selected_index = bool_index;
    crate::tui::handlers::handle_app(Key::Enter, &mut app);

    crate::tui::handlers::handle_app(Key::Esc, &mut app);
    assert!(app.view.settings_unsaved_prompt_visible);

    let prompt_areas = settings_unsaved_prompt_areas(&app).expect("unsaved prompt areas");
    handler(
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        prompt_areas.no.x + 1,
        prompt_areas.no.y,
      ),
      &mut app,
    );

    assert!(!app.view.settings_unsaved_prompt_visible);
    assert_ne!(app.get_current_route().active_block, ActiveBlock::Settings);
  }

  #[test]
  fn clicked_index_respects_list_scroll_offset() {
    let area = Rect::new(0, 0, 20, 8); // Inner height = 6 rows
    let selected_index = 8;
    let total_items = 20;

    // Derived fallback
    let first_visible = list_item_index_from_click(area, 1, selected_index, None, total_items);
    let second_visible = list_item_index_from_click(area, 2, selected_index, None, total_items);
    assert_eq!(first_visible, Some(3));
    assert_eq!(second_visible, Some(4));

    // Anchored offset
    let anchored_first = list_item_index_from_click(area, 1, selected_index, Some(5), total_items);
    assert_eq!(anchored_first, Some(5));
  }

  fn number_value(app: &App, index: usize) -> i64 {
    match app.settings_items.get(index).map(|setting| &setting.value) {
      Some(SettingValue::Number(value)) => *value,
      _ => panic!("expected a number setting"),
    }
  }

  #[test]
  fn scrolling_while_editing_a_number_setting_steps_its_value() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    open_settings(&mut app);
    let number_index = app
      .settings_items
      .iter()
      .position(|setting| matches!(setting.value, SettingValue::Number(_)))
      .expect("expected a number setting");
    app.view.settings_selected_index = number_index;
    let initial = number_value(&app, number_index);

    let areas = settings_layout_areas(&app).expect("settings layout areas");
    let x = areas.list.x + 2;
    let y = areas.list.y + 1 + number_index as u16;
    handler(
      mouse_event(MouseEventKind::Down(MouseButton::Left), x, y),
      &mut app,
    );
    assert!(app.view.settings_edit_mode);

    handler(mouse_event(MouseEventKind::ScrollDown, x, y), &mut app);

    assert_eq!(number_value(&app, number_index), initial - 1);
    assert_eq!(app.view.settings_edit_buffer, (initial - 1).to_string());
    assert!(
      app.view.settings_edit_mode,
      "the wheel steps the value; it does not leave the edit"
    );
  }

  #[test]
  fn table_click_mapping_respects_table_offset() {
    let area = Rect::new(0, 0, 80, 12);
    let selected_index = 15;
    let item_count = 40;

    let first = table_item_index_from_click(area, 2, selected_index, None, item_count);
    let second = table_item_index_from_click(area, 3, selected_index, None, item_count);

    assert_eq!(first, Some(9));
    assert_eq!(second, Some(10));
  }

  #[test]
  fn table_click_mapping_uses_anchored_offset_when_present() {
    let area = Rect::new(0, 0, 80, 12);
    let item_count = 40;

    // Cursor sits mid-window (selection 12) while the view is anchored at
    // row 9; the first data row must map to the anchor, not the selection.
    let first = table_item_index_from_click(area, 2, 12, Some(9), item_count);
    let second = table_item_index_from_click(area, 3, 12, Some(9), item_count);

    assert_eq!(first, Some(9));
    assert_eq!(second, Some(10));
  }

  fn pane_rect(app: &App, pane: Pane) -> Rect {
    crate::tui::layout::pane_area(app, pane).unwrap()
  }

  #[test]
  fn a_click_in_a_detached_pane_focuses_it() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    let lyrics = pane_rect(&app, Pane::Lyrics);

    handler(
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        lyrics.x + 5,
        lyrics.y + 5,
      ),
      &mut app,
    );

    assert_eq!(app.view.focused_pane, Pane::Lyrics);
  }

  #[test]
  fn a_click_on_the_close_mark_closes_the_pane() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    let mark = pane_close_mark(pane_rect(&app, Pane::Visualizer)).unwrap();

    handler(
      mouse_event(MouseEventKind::Down(MouseButton::Left), mark.x + 1, mark.y),
      &mut app,
    );

    assert!(!app.pane_open(Pane::Visualizer));
    assert!(app.pane_open(Pane::Lyrics));
  }

  #[test]
  fn a_drag_on_a_split_border_resizes_the_split() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    let main = pane_rect(&app, Pane::Main);
    let border_column = main.x + main.width - 1;

    for event in [
      mouse_event(MouseEventKind::Down(MouseButton::Left), border_column, 20),
      mouse_event(MouseEventKind::Drag(MouseButton::Left), 79, 20),
      mouse_event(MouseEventKind::Up(MouseButton::Left), 79, 20),
    ] {
      handler(event, &mut app);
    }

    assert_eq!(pane_rect(&app, Pane::Main).width, 79);
    assert_eq!(app.view.dragged_pane_split, None);
  }

  #[test]
  fn a_click_away_from_a_border_ends_a_drag_that_lost_its_release() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    let main = pane_rect(&app, Pane::Main);
    let lyrics = pane_rect(&app, Pane::Lyrics);

    for event in [
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        main.x + main.width - 1,
        20,
      ),
      mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        lyrics.x + 5,
        lyrics.y + 5,
      ),
      mouse_event(MouseEventKind::Drag(MouseButton::Left), 79, 20),
    ] {
      handler(event, &mut app);
    }

    assert_eq!(pane_rect(&app, Pane::Main), main);
    assert_eq!(app.view.dragged_pane_split, None);
  }
}
