use super::common_key_events;
use crate::core::action::Action;
use crate::core::app::{App, TrackTable, TrackTableContext};
use crate::tui::event::Key;

pub fn handler(key: Key, app: &mut App) {
  // A row parked by an earlier key follows only that key; the paging arms below re-park.
  match key {
    k if common_key_events::toward_sidebar_event(k, &app.user_config) => {
      common_key_events::focus_sidebar(app)
    }
    k if common_key_events::down_event(k, &app.user_config.keys) => {
      if app.track_table.tracks.is_empty() {
        return;
      }

      let next_index = common_key_events::on_down_press_handler(
        &app.track_table.tracks,
        Some(app.view.track_table_index),
      );
      app.view.track_table_index = next_index;
    }
    k if common_key_events::up_event(k, &app.user_config.keys) => {
      if app.track_table.tracks.is_empty() {
        return;
      }

      let next_index = common_key_events::on_up_press_handler(
        &app.track_table.tracks,
        Some(app.view.track_table_index),
      );
      app.view.track_table_index = next_index;
    }
    k if common_key_events::high_event(k) => {
      let next_index = common_key_events::on_high_press_handler();
      app.view.track_table_index = next_index;
    }
    k if common_key_events::middle_event(k) => {
      let next_index = common_key_events::on_middle_press_handler(&app.track_table.tracks);
      app.view.track_table_index = next_index;
    }
    k if common_key_events::low_event(k) => {
      let next_index = common_key_events::on_low_press_handler(&app.track_table.tracks);
      app.view.track_table_index = next_index;
    }
    Key::Enter => {
      on_enter(app);
    }
    k if k == app.user_config.keys.next_page => {
      let len = app.track_table.tracks.len();
      if len == 0 {
        return;
      }
      let target = app.view.track_table_index.saturating_add(page_step(app));
      app.view.track_table_index = target.min(len - 1);
    }
    k if k == app.user_config.keys.previous_page => {
      app.view.track_table_index = app.view.track_table_index.saturating_sub(page_step(app));
    }
    Key::Char('S') => play_random_song(app),
    k if k == app.user_config.keys.jump_to_end => jump_to_end(app),
    k if k == app.user_config.keys.jump_to_start => jump_to_start(app),
    _ if key == app.user_config.keys.add_item_to_queue => on_queue(app),
    // Open sort menu
    Key::Char(',') => {
      super::sort_menu::open_sort_menu(app, crate::core::sort::SortContext::PlaylistTracks);
    }
    _ => {}
  }
}

/// Play a random track from the table's context. Offsets are computed here so
/// `apply` stays deterministic.
fn play_random_song(app: &mut App) {
  if let Some(context) = &app.track_table.context {
    match context {
      TrackTableContext::QobuzPlaylist => {
        // Queue the whole playlist and start at a random offset, so
        // Next/Previous and auto-advance keep working within the playlist.
        let playable_ids: Vec<String> = app
          .track_table
          .tracks
          .iter()
          .filter_map(|track| track.uri.clone())
          .collect();
        if !playable_ids.is_empty() {
          let rand_idx = rand::random_range(0..playable_ids.len());
          app.apply(Action::PlayUris {
            uris: playable_ids,
            offset: Some(rand_idx),
          });
        }
      }
    }
  };
}

fn jump_to_end(app: &mut App) {
  if !app.track_table.tracks.is_empty() {
    app.view.track_table_index = app.track_table.tracks.len() - 1;
  }
}

fn on_enter(app: &mut App) {
  let TrackTable { context, tracks } = &app.track_table;
  let selected_index = &app.view.track_table_index;
  if let Some(context) = &context {
    match context {
      TrackTableContext::QobuzPlaylist => {
        // Queue the whole playlist (in displayed order) and start at the
        // selected track, so Next/Previous/auto-advance have a queue to move
        // through.
        let (uris, offset) = common_key_events::uri_playback_request(
          tracks.iter().map(|track| track.uri.clone()),
          *selected_index,
        );
        if !uris.is_empty() {
          app.apply(Action::PlayUris {
            uris,
            offset: Some(offset.unwrap_or(0)),
          });
        }
      }
    }
  };
}

fn on_queue(app: &mut App) {
  // Hand the selected row to the native queue.
  if app.track_table.context.is_none() {
    return;
  }
  if let Some(track) = app
    .track_table
    .tracks
    .get(app.view.track_table_index)
    .cloned()
  {
    app.apply(Action::QueueTrack(track));
  }
}

fn jump_to_start(app: &mut App) {
  app.view.track_table_index = 0;
}

/// Rows one `next_page`/`previous_page` press moves: half the terminal, like vim's Ctrl-D.
fn page_step(app: &App) -> usize {
  usize::from(app.view.size.height / 2).max(1)
}
