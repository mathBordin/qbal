use crate::core::app::{ActiveBlock, App, RouteId};
use crate::core::driver::{Driver, TickEnv};
use crate::core::pane::Pane;
use crate::core::user_config::UserConfig;
use crate::tui::event::{self, Key};
use crate::tui::handlers;
use crate::tui::ui;
use anyhow::Result;
use crossterm::{
  event::{
    DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
  },
  execute,
  terminal::{supports_keyboard_enhancement, SetTitle},
};
use log::info;
use ratatui::backend::Backend;
use ratatui::DefaultTerminal;
use std::{io::stdout, sync::Arc, time::Instant};
use tokio::sync::Mutex;

/// Whether the terminal renders real pixels. False until the renderer is
/// initialized, and always false in a decode-only build (`art-decode` without
/// `cover-art`), where art is fetched solely for the adaptive theme.
#[cfg(feature = "art-decode")]
fn cover_art_full_image_support() -> bool {
  #[cfg(feature = "cover-art")]
  {
    crate::tui::cover_art::full_image_support()
  }
  #[cfg(not(feature = "cover-art"))]
  {
    false
  }
}

/// The runner normally handles the configurable back key before `handle_app`.
/// Help's inline filter must get first chance while it is being edited (so `q`
/// can be query text), when opening the filter, and when Esc should clear a
/// confirmed filter rather than close Help.
fn help_menu_captures_key_before_back(app: &App, key: Key) -> bool {
  app.get_current_route().active_block == ActiveBlock::HelpMenu
    && (app.view.help_filter_editing
      || key == app.user_config.keys.search
      || (key == Key::Esc && !app.view.help_filter.is_empty()))
}

/// An open plugin popup or sort menu gets every key, the back key included.
fn modal_captures_key_before_back(app: &App) -> bool {
  app.plugin_popup.is_some() || app.get_current_route().active_block == ActiveBlock::SortMenu
}

/// Blocks that must see the configurable back key *before* the runner acts on it
/// themselves. Ordering is the whole point, so this is one predicate rather than
/// two conditions spelled out at the call site: a screen that is missing here has
/// its route popped instead of receiving the key, however well its own handler
/// copes with it.
fn key_reaches_handlers_before_back(app: &App, key: Key) -> bool {
  if modal_captures_key_before_back(app) {
    return true;
  }
  help_menu_captures_key_before_back(app, key)
}

/// Returns the cursor position when the focused search box is drawable.
fn frame_cursor_position(app: &App, active_block: ActiveBlock) -> Option<(u16, u16)> {
  if active_block != ActiveBlock::Input {
    return None;
  }

  let input = crate::tui::layout::compute_main_layout(app)?.input?;
  if input.width < 3 || input.height < 3 {
    return None;
  }

  Some((
    input.x + 1 + app.view.input_cursor_position - app.view.input_scroll_offset.get(),
    input.y + 1,
  ))
}

/// Whether this frame moves and needs the animation tick rate.
fn animation_active(app: &App) -> bool {
  let current_route = app.get_current_route();
  // The banner animates whenever the Home screen is displayed, regardless
  // of which block has focus (on Home the focused block is usually Empty
  // or Library, not Home), so gate the fast tick on the route. A disabled
  // banner gradient renders statically and needs no fast tick.
  // A pane animates only while it moves: the visualizer while a track
  // plays or its bars fall, the lyrics while they follow playback, ease,
  // or are browsed by hand.
  let pane_shows = |pane| crate::tui::layout::visible_pane_area(app, pane).is_some();
  let lyrics_pane_moves = app.playing_as_of_last_tick()
    || app.view.lyrics_view.is_easing()
    || app.view.lyrics_view.manual_index.is_some();
  (current_route.id == RouteId::Home && app.user_config.behavior.banner_gradient)
    || current_route.active_block == ActiveBlock::Analysis
    || (app.visualizer_moves() && pane_shows(Pane::Visualizer))
    || ((current_route.id == RouteId::LyricsView
      || (lyrics_pane_moves && pane_shows(Pane::Lyrics)))
      && app.lyrics_status() == crate::core::app::LyricsStatus::Found)
    || app.view.liked_song_animation_frame.is_some()
    || app.theme_fade_active()
}

/// One keypress, from the runner's event loop. Returns `true` when the user asked
/// to quit and the loop must break.
///
/// Extracted from the loop so the ordering of these branches is testable: the bug
/// this guards against is not "does a screen's handler cope with `q`" but "does
/// `q` ever get as far as that handler". `Key::Ctrl('c')` stays at the call site,
/// because it is an unconditional quit that predates any of this.
fn dispatch_key(key: Key, app: &mut App) -> bool {
  let key = match key.plain_arrow() {
    Some(plain) if !handlers::pane_arrows_apply(app) => plain,
    _ => key,
  };
  let current_active_block = app.get_current_route().active_block;

  if current_active_block == ActiveBlock::ExitPrompt {
    match key {
      Key::Enter | Key::Char('y') | Key::Char('Y') => {
        app.close_io_channel();
        return true;
      }
      Key::Esc | Key::Char('n') | Key::Char('N') => {
        app.pop_navigation_stack();
      }
      _ if key == app.user_config.keys.back => {
        app.pop_navigation_stack();
      }
      _ => {}
    }
  } else if current_active_block == ActiveBlock::Input {
    handlers::input_handler(key, app);
  } else if key_reaches_handlers_before_back(app, key) {
    handlers::handle_app(key, app);
  } else if key == app.user_config.keys.back {
    if !handlers::leave_focused_pane(app) {
      if current_active_block == ActiveBlock::Settings {
        handlers::handle_app(key, app);
      } else if app.get_current_route().active_block == ActiveBlock::AnnouncementPrompt {
        handlers::dismiss_announcement(app);
      } else if app.get_current_route().active_block != ActiveBlock::Input {
        let pop_result = match app.pop_navigation_stack() {
          Some(ref x) if x.id == RouteId::Search => app.pop_navigation_stack(),
          Some(x) => Some(x),
          None => None,
        };
        if pop_result.is_none() {
          app.push_navigation_stack(RouteId::ExitPrompt, ActiveBlock::ExitPrompt);
        }
      }
    }
  } else {
    handlers::handle_app(key, app);
  }
  false
}

/// The terminal state `start_ui` leaves for the exit path; fields drop in the order the old locals did.
pub struct TerminalSession {
  _events: event::Events,
  _terminal: DefaultTerminal,
  keyboard_enhancement_enabled: bool,
}

impl TerminalSession {
  /// Undoes the terminal setup of `start_ui`.
  pub fn restore(&self, driver: &mut Driver) {
    if let Some(title) = driver.window_title_reset() {
      let _ = execute!(stdout(), SetTitle(title));
    }
    let _ = execute!(stdout(), DisableMouseCapture);
    if self.keyboard_enhancement_enabled {
      let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
  }
}

/// Runs the terminal loop; the returned session keeps the terminal alive until the exit path restores it.
pub async fn start_ui(
  user_config: UserConfig,
  app: &Arc<Mutex<App>>,
  driver: &mut Driver,
) -> Result<TerminalSession> {
  let mut terminal = ratatui::init();
  // Probe the terminal's image protocol only now that the terminal is set up;
  // `App` construction must not touch stdout.
  #[cfg(feature = "cover-art")]
  crate::tui::cover_art::init_renderer();
  execute!(stdout(), EnableMouseCapture)?;
  let keyboard_enhancement_supported = supports_keyboard_enhancement().unwrap_or(false);
  let keyboard_enhancement_enabled = keyboard_enhancement_supported
    && execute!(
      stdout(),
      PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
          | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
          | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
      )
    )
    .is_ok();
  if keyboard_enhancement_enabled {
    info!("enabled keyboard enhancement flags");
  }
  {
    let mut app = app.lock().await;
    app.view.terminal_input_caps.keyboard_enhancement_supported = keyboard_enhancement_supported;
    app.view.terminal_input_caps.keyboard_enhancement_enabled = keyboard_enhancement_enabled;
    app.keep_pane_focus_in_layout();
  }
  let mut route_depth = app.lock().await.navigation_depth();

  let mut events = event::Events::new(user_config.behavior.tick_rate_milliseconds);
  let inspector = crate::tui::dev_inspect::Inspector::from_env(events.sender());
  let mut pending_snapshot = None;

  let mut is_first_render = true;

  loop {
    let terminal_size =
      terminal
        .backend()
        .size()
        .ok()
        .map(|size| crate::core::geometry::Viewport {
          width: size.width,
          height: size.height,
        });
    let title_update = {
      let mut app = app.lock().await;

      if let Some(size) = terminal_size {
        if is_first_render || app.view.size != size {
          app.view.help_menu_offset = 0;
          app.view.help_menu_page = 0;
          app.view.size = size;

          app.view.help_menu_max_lines = (size.height as u32).saturating_sub(8);
        }
      };

      // Rebuild the formatted Help rows before drawing so the render path
      // below reads immutable App state only.
      if app.get_current_route().active_block == ActiveBlock::HelpMenu {
        ui::ensure_help_menu_model(&mut app);
      }
      handlers::show_main_for_new_screen(&mut app, route_depth);
      route_depth = app.navigation_depth();

      let current_route = app.get_current_route();
      let current_tick_rate = if animation_active(&app) {
        app.user_config.behavior.animation_tick_rate_milliseconds
      } else {
        app.user_config.behavior.tick_rate_milliseconds
      };
      events.set_tick_rate(current_tick_rate);

      // Drop protocol caches for art the store no longer holds; rebuilds are
      // lazy, so this is a no-op whenever the art is unchanged.
      #[cfg(feature = "cover-art")]
      crate::tui::cover_art::sync(&app.cover_art);

      // The cursor must be invisible while the frame diff is written. Ratatui
      // applies the diff first and shows or hides the cursor afterwards, so a
      // cursor left visible by the previous frame travels over every freshly
      // written cell run and shows up as a strobing block inside the animated
      // banner. Hide it here; the draw closure below re-shows it at the input
      // position after the write.
      terminal.hide_cursor()?;

      let frame = terminal.draw(|f| {
        use ratatui::{prelude::Style, widgets::Block};
        f.render_widget(
          Block::default().style(Style::default().bg(app.user_config.theme.background.into())),
          f.area(),
        );

        match current_route.active_block {
          ActiveBlock::HelpMenu => ui::draw_help_menu(f, &app),
          ActiveBlock::Queue => ui::draw_queue(f, &app),
          ActiveBlock::Error => ui::draw_error_screen(f, &app),
          ActiveBlock::Analysis => ui::audio_analysis::draw(f, &app),
          ActiveBlock::LyricsView => ui::draw_lyrics_view(f, &app),
          ActiveBlock::MiniPlayer => ui::draw_miniplayer(f, &app),
          #[cfg(feature = "cover-art")]
          ActiveBlock::CoverArtView => ui::draw_cover_art_view(f, &app),
          ActiveBlock::AnnouncementPrompt => ui::draw_announcement_prompt(f, &app),
          ActiveBlock::RecapPrompt => {
            ui::draw_main_layout(f, &app);
            ui::draw_recap_prompt(f, &app);
          }
          ActiveBlock::ExitPrompt => ui::draw_exit_prompt(f, &app),
          ActiveBlock::Settings => ui::settings::draw_settings(f, &app),
          ActiveBlock::PluginScreen => ui::draw_plugin_screen(f, &app),
          _ => ui::draw_main_layout(f, &app),
        }

        // Plugin popup overlays every screen.
        ui::draw_plugin_popup(f, &app);

        // Cursor management lives inside the frame: ratatui shows and moves the
        // terminal cursor when a widget requests a position and hides it when
        // none does. The old post-draw hide/show pair ran on every animation
        // frame (16 ms on Home), and the alternating toggle reset the
        // terminal's blink phase each frame, so the cursor strobed.
        if let Some(position) = frame_cursor_position(&app, current_route.active_block) {
          f.set_cursor_position(position);
        }
      })?;
      if let (Some(inspector), Some(snapshot)) = (&inspector, pending_snapshot.take()) {
        let route = format!("{:?}/{:?}", current_route.id, current_route.active_block);
        inspector.write_frame(&snapshot, &route, frame.buffer);
      }

      driver.next_window_title(&app)
    };
    if let Some(title) = title_update {
      execute!(stdout(), SetTitle(title.as_str()))?;
    }

    match events.next()? {
      event::Event::Input(key) => {
        let mut app = app.lock().await;
        if key == Key::Ctrl('c') {
          app.close_io_channel();
          break;
        }

        // `break` before the scripting hook below, not after: a quit must not run
        // one more round of pending plugin commands on its way out.
        if dispatch_key(key, &mut app) {
          break;
        }
        driver.run_pending_script_commands(&mut app);
      }
      event::Event::Snapshot(snapshot) => pending_snapshot = Some(snapshot),
      event::Event::Mouse(mouse) => {
        let mut app = app.lock().await;
        if !app.user_config.behavior.disable_mouse_inputs {
          handlers::mouse_handler(mouse, &mut app);
        }
      }
      event::Event::Tick(elapsed) => {
        let mut app = app.lock().await;
        // The visualizer bar count is frontend geometry (terminal width and
        // layout margin), so it is resolved here and injected.
        #[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
        let viz_bars = ui::audio_analysis::visualizer_inner_width(&app).map(|width| {
          ui::audio_analysis::visualizer_bar_count(app.user_config.behavior.visualizer_style, width)
        });
        driver.tick(
          &mut app,
          elapsed,
          TickEnv {
            now: Instant::now(),
            #[cfg(any(feature = "audio-viz", feature = "audio-viz-cpal"))]
            viz_bars,
            #[cfg(feature = "art-decode")]
            cover_art_full_image_support: cover_art_full_image_support(),
          },
        );
      }
    }

    if is_first_render {
      let mut app = app.lock().await;
      driver.dispatch_startup(&mut app);
      is_first_render = false;
    }
  }

  Ok(TerminalSession {
    _events: events,
    _terminal: terminal,
    keyboard_enhancement_enabled,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_visualizer_pane_ticks_fast_only_while_a_track_plays_or_its_bars_fall() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    app.push_navigation_stack(RouteId::Search, ActiveBlock::SearchResultBlock);

    assert!(!animation_active(&app));

    app.spectrum_data = Some(crate::infra::audio::SpectrumData {
      bands: vec![0.0, 0.4],
      ..Default::default()
    });
    assert!(animation_active(&app));

    app.spectrum_data = None;
    app.note_playing(true);
    assert!(animation_active(&app));
  }

  #[test]
  fn a_paused_lyrics_pane_ticks_fast_only_while_it_is_browsed() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    app.change_pane_layout(|tree| tree.close(Pane::Visualizer));
    app.push_navigation_stack(RouteId::Search, ActiveBlock::SearchResultBlock);
    app.set_lyrics(
      crate::core::app::LyricsStatus::Found,
      Some(vec![(0, "one".to_string()), (5_000, "two".to_string())]),
      false,
    );
    app.view.lyrics_view.snap_to(0.0);

    assert!(!animation_active(&app));

    app.view.lyrics_view.manual_index = Some(1);

    assert!(animation_active(&app));
  }

  #[test]
  fn a_screen_that_covers_the_panes_drops_their_fast_tick_and_capture() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    app.note_playing(true);
    app.push_navigation_stack(RouteId::Settings, ActiveBlock::Settings);

    assert!(!animation_active(&app));
    assert_eq!(ui::audio_analysis::visualizer_inner_width(&app), None);
  }

  #[test]
  fn back_key_in_a_focused_pane_returns_the_focus_without_a_pop() {
    let mut app = crate::tui::layout::app_with_panes(160, 50);
    app.view.focused_pane = Pane::Visualizer;
    let route = app.get_current_route().id.clone();

    dispatch_key(app.user_config.keys.back, &mut app);

    assert_eq!(app.view.focused_pane, Pane::Main);
    assert_eq!(app.get_current_route().id, route);
  }
}
