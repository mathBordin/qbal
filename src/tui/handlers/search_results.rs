use super::common_key_events;
use crate::core::action::Action;
use crate::core::app::{ActiveBlock, App, DialogContext, RouteId, SearchResultBlock};
use crate::core::plugin_api::{ShowInfo, TrackInfo};
use crate::tui::event::Key;

fn handle_down_press_on_selected_block(app: &mut App) {
  // Start selecting within the selected block
  match app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results().albums {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.view.search_selected_album_index,
        );
        app.view.search_selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results().tracks {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.view.search_selected_tracks_index,
        );
        app.view.search_selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results().artists {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.view.search_selected_artists_index,
        );
        app.view.search_selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results().playlists {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.view.search_selected_playlists_index,
        );
        app.view.search_selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results().shows {
        let next_index = common_key_events::on_down_press_handler(
          &result.items,
          app.view.search_selected_shows_index,
        );
        app.view.search_selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_down_press_on_hovered_block(app: &mut App) {
  match app.view.search_hovered_block {
    SearchResultBlock::AlbumSearch => {
      app.view.search_hovered_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::SongSearch => {
      app.view.search_hovered_block = SearchResultBlock::AlbumSearch;
    }
    SearchResultBlock::ArtistSearch => {
      app.view.search_hovered_block = SearchResultBlock::PlaylistSearch;
    }
    SearchResultBlock::PlaylistSearch => {
      app.view.search_hovered_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::ShowSearch => {
      app.view.search_hovered_block = SearchResultBlock::SongSearch;
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_up_press_on_selected_block(app: &mut App) {
  // Start selecting within the selected block
  match app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results().albums {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.view.search_selected_album_index,
        );
        app.view.search_selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results().tracks {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.view.search_selected_tracks_index,
        );
        app.view.search_selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results().artists {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.view.search_selected_artists_index,
        );
        app.view.search_selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results().playlists {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.view.search_selected_playlists_index,
        );
        app.view.search_selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results().shows {
        let next_index = common_key_events::on_up_press_handler(
          &result.items,
          app.view.search_selected_shows_index,
        );
        app.view.search_selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_up_press_on_hovered_block(app: &mut App) {
  match app.view.search_hovered_block {
    SearchResultBlock::AlbumSearch => {
      app.view.search_hovered_block = SearchResultBlock::SongSearch;
    }
    SearchResultBlock::SongSearch => {
      app.view.search_hovered_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::ArtistSearch => {
      app.view.search_hovered_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::PlaylistSearch => {
      app.view.search_hovered_block = SearchResultBlock::ArtistSearch;
    }
    SearchResultBlock::ShowSearch => {
      app.view.search_hovered_block = SearchResultBlock::AlbumSearch;
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_high_press_on_selected_block(app: &mut App) {
  match app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(_result) = &app.search_results().albums {
        let next_index = common_key_events::on_high_press_handler();
        app.view.search_selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(_result) = &app.search_results().tracks {
        let next_index = common_key_events::on_high_press_handler();
        app.view.search_selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(_result) = &app.search_results().artists {
        let next_index = common_key_events::on_high_press_handler();
        app.view.search_selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(_result) = &app.search_results().playlists {
        let next_index = common_key_events::on_high_press_handler();
        app.view.search_selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(_result) = &app.search_results().shows {
        let next_index = common_key_events::on_high_press_handler();
        app.view.search_selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_middle_press_on_selected_block(app: &mut App) {
  match app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results().albums {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.view.search_selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results().tracks {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.view.search_selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results().artists {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.view.search_selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results().playlists {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.view.search_selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results().shows {
        let next_index = common_key_events::on_middle_press_handler(&result.items);
        app.view.search_selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_low_press_on_selected_block(app: &mut App) {
  match app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => {
      if let Some(result) = &app.search_results().albums {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.view.search_selected_album_index = Some(next_index);
      }
    }
    SearchResultBlock::SongSearch => {
      if let Some(result) = &app.search_results().tracks {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.view.search_selected_tracks_index = Some(next_index);
      }
    }
    SearchResultBlock::ArtistSearch => {
      if let Some(result) = &app.search_results().artists {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.view.search_selected_artists_index = Some(next_index);
      }
    }
    SearchResultBlock::PlaylistSearch => {
      if let Some(result) = &app.search_results().playlists {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.view.search_selected_playlists_index = Some(next_index);
      }
    }
    SearchResultBlock::ShowSearch => {
      if let Some(result) = &app.search_results().shows {
        let next_index = common_key_events::on_low_press_handler(&result.items);
        app.view.search_selected_shows_index = Some(next_index);
      }
    }
    SearchResultBlock::Empty => {}
  }
}

fn handle_add_item_to_queue(app: &mut App) {
  if let SearchResultBlock::SongSearch = app.view.search_selected_block {
    let track = app.view.search_selected_tracks_index.and_then(|index| {
      app
        .search_results()
        .tracks
        .as_ref()
        .and_then(|tracks| tracks.items.get(index).cloned())
    });
    if let Some(track) = track {
      app.apply(Action::QueueTrack(track));
    }
  }
}

/// The highlighted album's id; `.get()` guards a stale index.
fn selected_search_album_id(app: &App) -> Option<String> {
  let index = app.view.search_selected_album_index?;
  app
    .search_results()
    .albums
    .as_ref()?
    .items
    .get(index)?
    .id
    .clone()
}

/// The highlighted artist's (id, name).
fn selected_search_artist_identity(app: &App) -> Option<(String, String)> {
  let index = app.view.search_selected_artists_index?;
  let artist = app.search_results().artists.as_ref()?.items.get(index)?;
  Some((artist.id.clone()?, artist.name.clone()))
}

/// The highlighted track row; `None` without a selection.
fn selected_search_track_row(app: &App) -> Option<&TrackInfo> {
  let index = app.view.search_selected_tracks_index?;
  app.search_results().tracks.as_ref()?.items.get(index)
}

/// The highlighted show row; `None` without a selection.
fn selected_search_show_row(app: &App) -> Option<&ShowInfo> {
  let index = app.view.search_selected_shows_index?;
  app.search_results().shows.as_ref()?.items.get(index)
}

/// The highlighted show's Spotify id, for the save/unsave keys.
fn selected_search_show_id(app: &App) -> Option<String> {
  selected_search_show_row(app)?.id.clone()
}

fn handle_enter_event_on_selected_block(app: &mut App) {
  match &app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => if let Some(_id) = selected_search_album_id(app) {},
    SearchResultBlock::SongSearch => {
      let Some(paged) = app.search_results().tracks.as_ref() else {
        return;
      };
      // No selection: an out-of-range index yields `offset: None`.
      let selected = app
        .view
        .search_selected_tracks_index
        .unwrap_or(paged.items.len());
      let (uris, offset) = common_key_events::uri_playback_request(
        paged.items.iter().map(|track| track.uri.clone()),
        selected,
      );
      app.apply(Action::PlayUris { uris, offset });
    }
    SearchResultBlock::ArtistSearch => {
      if let Some((_id, _name)) = selected_search_artist_identity(app) {}
    }
    SearchResultBlock::PlaylistSearch => {
      // Go to playlist tracks table: navigates immediately with the cleared
      // table as the loading state (see open_playlist_tracks).
      if let Some(_id) = app.selected_search_result_playlist_id() {}
    }
    SearchResultBlock::ShowSearch => {
      // OpenShowEpisodes populates app.library().show_episodes (opening the show
      // by id sets EpisodeTableContext::Full but does NOT populate it, leaving
      // a blank episode list). `show` is already a domain ShowInfo.
      if let Some(_show) = selected_search_show_row(app).cloned() {}
    }
    SearchResultBlock::Empty => {}
  };
}

fn handle_enter_event_on_hovered_block(app: &mut App) {
  match app.view.search_hovered_block {
    SearchResultBlock::AlbumSearch => {
      let next_index = app.view.search_selected_album_index.unwrap_or(0);

      app.view.search_selected_album_index = Some(next_index);
      app.view.search_selected_block = SearchResultBlock::AlbumSearch;
    }
    SearchResultBlock::SongSearch => {
      let next_index = app.view.search_selected_tracks_index.unwrap_or(0);

      app.view.search_selected_tracks_index = Some(next_index);
      app.view.search_selected_block = SearchResultBlock::SongSearch;
    }
    SearchResultBlock::ArtistSearch => {
      let next_index = app.view.search_selected_artists_index.unwrap_or(0);

      app.view.search_selected_artists_index = Some(next_index);
      app.view.search_selected_block = SearchResultBlock::ArtistSearch;
    }
    SearchResultBlock::PlaylistSearch => {
      let next_index = app.view.search_selected_playlists_index.unwrap_or(0);

      app.view.search_selected_playlists_index = Some(next_index);
      app.view.search_selected_block = SearchResultBlock::PlaylistSearch;
    }
    SearchResultBlock::ShowSearch => {
      let next_index = app.view.search_selected_shows_index.unwrap_or(0);

      app.view.search_selected_shows_index = Some(next_index);
      app.view.search_selected_block = SearchResultBlock::ShowSearch;
    }
    SearchResultBlock::Empty => {}
  };
}

fn handle_recommended_tracks(app: &mut App) {
  match app.view.search_selected_block {
    SearchResultBlock::AlbumSearch => {}
    SearchResultBlock::SongSearch => {
      if let Some(index) = app.view.search_selected_tracks_index {
        let identity = app
          .search_results()
          .tracks
          .as_ref()
          .and_then(|paged| paged.items.get(index))
          .and_then(|track| Some((track.id.clone()?, track.name.clone())));
        if let Some((_id, _name)) = identity {}
      };
    }
    SearchResultBlock::ArtistSearch => {
      if let Some((_id, _name)) = selected_search_artist_identity(app) {}
    }
    SearchResultBlock::PlaylistSearch => {}
    SearchResultBlock::ShowSearch => {}
    SearchResultBlock::Empty => {}
  }
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => {
      app.view.search_selected_block = SearchResultBlock::Empty;
    }
    k if common_key_events::down_event(k, &app.user_config.keys) => {
      if app.view.search_selected_block != SearchResultBlock::Empty {
        handle_down_press_on_selected_block(app);
      } else {
        handle_down_press_on_hovered_block(app);
      }
    }
    k if common_key_events::up_event(k, &app.user_config.keys) => {
      if app.view.search_selected_block != SearchResultBlock::Empty {
        handle_up_press_on_selected_block(app);
      } else {
        handle_up_press_on_hovered_block(app);
      }
    }
    k if common_key_events::left_event(k, &app.user_config.keys) => {
      app.view.search_selected_block = SearchResultBlock::Empty;
      match app.view.search_hovered_block {
        SearchResultBlock::AlbumSearch => {
          if !common_key_events::sidebar_on_right(&app.user_config) {
            common_key_events::focus_sidebar(app);
          } else {
            app.view.search_hovered_block = SearchResultBlock::PlaylistSearch;
          }
        }
        SearchResultBlock::SongSearch => {
          if !common_key_events::sidebar_on_right(&app.user_config) {
            common_key_events::focus_sidebar(app);
          } else {
            app.view.search_hovered_block = SearchResultBlock::ArtistSearch;
          }
        }
        SearchResultBlock::ArtistSearch => {
          app.view.search_hovered_block = SearchResultBlock::SongSearch;
        }
        SearchResultBlock::PlaylistSearch => {
          app.view.search_hovered_block = SearchResultBlock::AlbumSearch;
        }
        SearchResultBlock::ShowSearch => {
          if !common_key_events::sidebar_on_right(&app.user_config) {
            common_key_events::focus_sidebar(app);
          }
        }
        SearchResultBlock::Empty => {}
      }
    }
    k if common_key_events::right_event(k, &app.user_config.keys) => {
      app.view.search_selected_block = SearchResultBlock::Empty;
      match app.view.search_hovered_block {
        SearchResultBlock::AlbumSearch => {
          app.view.search_hovered_block = SearchResultBlock::PlaylistSearch;
        }
        SearchResultBlock::SongSearch => {
          app.view.search_hovered_block = SearchResultBlock::ArtistSearch;
        }
        SearchResultBlock::ArtistSearch => {
          if common_key_events::sidebar_on_right(&app.user_config) {
            common_key_events::focus_sidebar(app);
          } else {
            app.view.search_hovered_block = SearchResultBlock::SongSearch;
          }
        }
        SearchResultBlock::PlaylistSearch => {
          if common_key_events::sidebar_on_right(&app.user_config) {
            common_key_events::focus_sidebar(app);
          } else {
            app.view.search_hovered_block = SearchResultBlock::AlbumSearch;
          }
        }
        SearchResultBlock::ShowSearch => {
          if common_key_events::sidebar_on_right(&app.user_config) {
            common_key_events::focus_sidebar(app);
          }
        }
        SearchResultBlock::Empty => {}
      }
    }
    k if common_key_events::high_event(k)
      && app.view.search_selected_block != SearchResultBlock::Empty =>
    {
      handle_high_press_on_selected_block(app);
    }
    k if common_key_events::middle_event(k)
      && app.view.search_selected_block != SearchResultBlock::Empty =>
    {
      handle_middle_press_on_selected_block(app);
    }
    k if common_key_events::low_event(k)
      && app.view.search_selected_block != SearchResultBlock::Empty =>
    {
      handle_low_press_on_selected_block(app)
    }
    // Handle pressing enter when block is selected to start playing track
    Key::Enter => match app.view.search_selected_block {
      SearchResultBlock::Empty => handle_enter_event_on_hovered_block(app),
      _ => handle_enter_event_on_selected_block(app),
    },
    Key::Char('w') => match app.view.search_selected_block {
      SearchResultBlock::AlbumSearch => if let Some(_id) = selected_search_album_id(app) {},
      SearchResultBlock::SongSearch => {
        let track =
          selected_search_track_row(app).map(|track| (track.id.clone(), track.name.clone()));
        if let Some((_track_id, _track_name)) = track {}
      }
      SearchResultBlock::ArtistSearch => {
        if let Some((_id, _name)) = selected_search_artist_identity(app) {}
      }
      SearchResultBlock::PlaylistSearch => {
        if let Some(_id) = app.selected_search_result_playlist_id() {}
      }
      SearchResultBlock::ShowSearch => if let Some(_id) = selected_search_show_id(app) {},
      SearchResultBlock::Empty => {}
    },
    Key::Char('D') => match app.view.search_selected_block {
      SearchResultBlock::AlbumSearch => if let Some(_id) = selected_search_album_id(app) {},
      SearchResultBlock::SongSearch => {}
      SearchResultBlock::ArtistSearch => {
        if let Some((_id, _name)) = selected_search_artist_identity(app) {}
      }
      SearchResultBlock::PlaylistSearch => {
        if let (Some(playlists), Some(selected_index)) = (
          &app.search_results().playlists,
          app.view.search_selected_playlists_index,
        ) {
          if let Some(selected_playlist) = playlists.items.get(selected_index) {
            let selected_playlist = selected_playlist.name.clone();
            app.view.dialog = Some(selected_playlist);
            app.view.confirm = false;

            app.push_navigation_stack(
              RouteId::Dialog,
              ActiveBlock::Dialog(DialogContext::PlaylistSearch),
            );
          }
        }
      }
      SearchResultBlock::ShowSearch => if let Some(_id) = selected_search_show_id(app) {},
      SearchResultBlock::Empty => {}
    },
    Key::Char('r') => handle_recommended_tracks(app),
    _ if key == app.user_config.keys.add_item_to_queue => handle_add_item_to_queue(app),
    Key::Char('s') => handle_save_track_event(app),
    _ => {}
  }
}

fn handle_save_track_event(app: &mut App) {
  if let SearchResultBlock::SongSearch = app.view.search_selected_block {
    let uri = app.view.search_selected_tracks_index.and_then(|index| {
      app
        .search_results()
        .tracks
        .as_ref()
        .and_then(|tracks| tracks.items.get(index))
        .and_then(|track| track.uri.clone())
    });
    if let Some(_uri) = uri {}
  }
}

#[cfg(test)]
mod tests {}
