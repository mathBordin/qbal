use super::common_key_events;
use crate::core::action::Action;
use crate::core::app::App;
use crate::tui::event::Key;

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::toward_sidebar_event(k, &app.user_config) => {
      common_key_events::focus_sidebar(app)
    }
    k if common_key_events::down_event(k, &app.user_config.keys) => {
      if let Some(stats) = &app.stats_data {
        app.view.stats_selected_track = common_key_events::on_down_press_handler(
          &stats.top_tracks,
          Some(app.view.stats_selected_track),
        );
      }
    }
    k if common_key_events::up_event(k, &app.user_config.keys) => {
      if let Some(stats) = &app.stats_data {
        app.view.stats_selected_track = common_key_events::on_up_press_handler(
          &stats.top_tracks,
          Some(app.view.stats_selected_track),
        );
      }
    }
    k if common_key_events::high_event(k) => {
      if app.stats_data.is_some() {
        app.view.stats_selected_track = common_key_events::on_high_press_handler();
      }
    }
    k if common_key_events::middle_event(k) => {
      if let Some(stats) = &app.stats_data {
        app.view.stats_selected_track =
          common_key_events::on_middle_press_handler(&stats.top_tracks);
      }
    }
    k if common_key_events::low_event(k) => {
      if let Some(stats) = &app.stats_data {
        app.view.stats_selected_track = common_key_events::on_low_press_handler(&stats.top_tracks);
      }
    }
    Key::Char('[') => cycle_period(app, false),
    Key::Char(']') => cycle_period(app, true),
    Key::Enter => {
      let uri = app.stats_data.as_ref().and_then(|stats| {
        stats
          .top_tracks
          .get(app.view.stats_selected_track)
          .and_then(|entry| entry.uri.clone())
      });
      match uri {
        Some(uri) if uri.starts_with("qobuz:") => {
          app.apply(Action::PlayUris {
            uris: vec![uri],
            offset: Some(0),
          });
        }
        Some(_) | None => {
          let has_tracks = app
            .stats_data
            .as_ref()
            .is_some_and(|stats| !stats.top_tracks.is_empty());
          if has_tracks {
            app.set_status_message(app.strings().status.app.entry_not_playable.to_string(), 4);
          }
        }
      }
    }
    _ => {}
  }
}

fn cycle_period(app: &mut App, forward: bool) {
  app.apply(Action::CycleStatsPeriod { forward });
  app.view.stats_selected_track = 0;
}

#[cfg(test)]
mod tests {}
