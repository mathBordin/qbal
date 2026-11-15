pub mod artist;
pub mod audio_analysis;
pub mod columns;
pub mod head_unit;
pub mod help;
pub mod home;
pub mod library;
pub mod lyrics;
pub mod now_playing;
pub mod panes;
pub mod player;
pub mod plugin_screen;
pub mod popups;
pub mod retro;
pub mod search;
pub mod settings;
pub mod stats;
pub mod tables;
pub mod util;

use crate::core::app::{App, RouteId};
use crate::tui::layout::{compute_main_layout, is_wide_layout, main_hidden};
use ratatui::{layout::Rect, Frame};

pub use self::home::draw_home;
pub use self::library::draw_user_block;
pub use self::lyrics::draw_lyrics_view;
#[cfg(feature = "cover-art")]
pub use self::player::draw_cover_art_view;
pub use self::player::draw_miniplayer;
pub use self::player::draw_playbar;
pub use self::plugin_screen::draw_plugin_screen;
pub use self::popups::{
  draw_announcement_prompt, draw_error_screen, draw_exit_prompt, draw_help_menu, draw_plugin_popup,
  draw_queue, draw_recap_prompt, draw_sort_menu, ensure_help_menu_model,
};
pub use self::search::{draw_input_and_help_box, draw_search_results};
pub use self::stats::draw_stats;
pub use self::tables::draw_song_table;
pub fn draw_main_layout(f: &mut Frame<'_>, app: &App) {
  if let Some(areas) = compute_main_layout(app) {
    // In wide layout the input row lives inside the sidebar and is drawn by
    // draw_user_block (it varies per source); the top row is only drawn here.
    if !is_wide_layout(app) {
      if let (Some(input_area), Some(help_area), Some(settings_area)) =
        (areas.input, areas.help, areas.settings)
      {
        draw_input_and_help_box(f, app, input_area, help_area, settings_area);
      }
    }
    if !main_hidden(app) {
      draw_user_block(f, app, areas.sidebar);
      draw_route_content(f, app, areas.content);
      if let Some(now_playing_area) = areas.now_playing {
        now_playing::draw_now_playing(f, app, now_playing_area);
      }
    }
    draw_playbar(f, app, areas.playbar);
  }
  panes::draw_detached_panes(f, app);

  // Possibly draw confirm dialog

  // Possibly draw sort menu
  draw_sort_menu(f, app);
}

fn draw_route_content(f: &mut Frame<'_>, app: &App, content_area: Rect) {
  let current_route = app.get_current_route();

  match current_route.id {
    RouteId::Search => {
      draw_search_results(f, app, content_area);
    }
    RouteId::TrackTable => {
      draw_song_table(f, app, content_area);
    }
    RouteId::Home => {
      draw_home(f, app, content_area);
    }
    RouteId::Stats => {
      draw_stats(f, app, content_area);
    }
    RouteId::Artist => {
      artist::draw_artist(f, app, content_area);
    }
    RouteId::Error
    | RouteId::Analysis
    | RouteId::LyricsView
    | RouteId::CoverArtView
    | RouteId::MiniPlayer
    | RouteId::AnnouncementPrompt
    | RouteId::RecapPrompt
    | RouteId::ExitPrompt
    | RouteId::Settings
    | RouteId::HelpMenu
    | RouteId::Queue
    | RouteId::PluginScreen(_) => {} // These are drawn outside the main routed content area.
                                     // This is drawn as an overlay via draw_create_playlist_form.
  };
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::app::ActiveBlock;
  use crate::core::geometry::Viewport;
  use crate::core::pane::Pane;
  use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

  fn render(width: u16, height: u16) -> (App, Buffer) {
    let mut app = App::default();
    app.view.size = Viewport { width, height };
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Home);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| draw_main_layout(f, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    (app, buffer)
  }

  #[test]
  fn the_now_playing_panel_draws_beside_the_content_with_nothing_playing() {
    let (width, height) = (160, 50);
    let mut app = App::default();
    app.view.size = Viewport { width, height };
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Home);
    app.view.now_playing_panel = true;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| draw_main_layout(f, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();

    let panel = compute_main_layout(&app).unwrap().now_playing.unwrap();
    let title_row = row_text(&buffer, panel.y);
    let strings = &app.strings().screens;
    let from_panel: String = title_row.chars().skip(usize::from(panel.x)).collect();
    assert!(
      from_panel.starts_with(&format!("╭{}", strings.now_playing_title)),
      "title row: {title_row}"
    );
    let middle_row = row_text(&buffer, panel.y + 1 + (panel.height - 2) / 2);
    assert!(
      middle_row.contains(strings.now_playing_nothing),
      "middle row: {middle_row}"
    );
  }

  fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area().width)
      .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
      .collect()
  }

  #[test]
  fn detached_panes_draw_beside_main_with_their_close_marks() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Home);
    let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
    terminal.draw(|f| draw_main_layout(f, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let rect_of = |pane| crate::tui::layout::pane_area(&app, pane).unwrap();
    let (lyrics, visualizer) = (rect_of(Pane::Lyrics), rect_of(Pane::Visualizer));

    let lyrics_top = row_text(&buffer, lyrics.y);
    let visualizer_top = row_text(&buffer, visualizer.y);
    let pane_part = |row: &str, area: Rect| -> String {
      row
        .chars()
        .skip(area.x as usize)
        .take(area.width as usize)
        .collect()
    };

    let lyrics_title = app.strings().screens.lyrics_title;
    assert!(
      pane_part(&lyrics_top, lyrics).contains(lyrics_title),
      "{lyrics_top}"
    );
    assert!(pane_part(&lyrics_top, lyrics).contains(panes::CLOSE_MARK));
    assert!(pane_part(&visualizer_top, visualizer).contains(panes::CLOSE_MARK));
    let main_part = pane_part(&row_text(&buffer, 2), rect_of(Pane::Main));
    assert!(!main_part.contains(panes::CLOSE_MARK), "{main_part}");
  }

  #[test]
  fn narrow_layout_draws_input_row_and_library_at_hitbox_positions() {
    let (app, buffer) = render(100, 40);
    let areas = compute_main_layout(&app).expect("layout areas");
    let input = areas.input.expect("input area");

    let input_row = row_text(&buffer, input.y);
    assert!(input_row.contains("Search"), "input row: {input_row}");
    assert!(input_row.contains("Help"), "input row: {input_row}");
    assert!(input_row.contains("Settings"), "input row: {input_row}");

    let library_row = row_text(&buffer, areas.library.y);
    assert!(
      library_row.contains("Library"),
      "library panel not at hitbox row: {library_row}"
    );
  }

  #[test]
  fn stats_route_renders_period_tabs() {
    let mut app = App::default();
    app.view.size = Viewport {
      width: 160,
      height: 50,
    };
    app.push_navigation_stack(RouteId::Stats, ActiveBlock::Stats);
    let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
    terminal.draw(|f| draw_main_layout(f, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();

    let all_text: String = (0..50).map(|y| row_text(&buffer, y)).collect();
    assert!(all_text.contains("Stats"), "missing Stats title");
    assert!(
      all_text.contains("Last 30 Days"),
      "missing selected period label"
    );
    assert!(all_text.contains("Top Tracks"), "missing Top Tracks panel");
  }
}

#[cfg(test)]
mod localization_tests;
