//! Presence sync: Discord Rich Presence, MPRIS, and the window title. Each
//! keeps a last-published copy so quiet ticks publish nothing. Moved verbatim
//! out of the terminal runner; nothing here touches a terminal — the frontend
//! only applies the returned window title.

use crate::core::app::App;
#[cfg(feature = "discord-rpc")]
use crate::infra::discord_rpc;
#[cfg(all(feature = "mpris", target_os = "linux"))]
use crate::infra::mpris;

pub(super) const DEFAULT_WINDOW_TITLE: &str = "spotatui";

#[derive(Default)]
pub(super) struct WindowTitleState {
  last_title: Option<String>,
}

#[cfg(feature = "discord-rpc")]
#[derive(Clone, Debug, PartialEq)]
struct DiscordTrackInfo {
  title: String,
  artist: String,
  album: String,
  image_url: Option<String>,
  duration_ms: u32,
}

#[cfg(feature = "discord-rpc")]
#[derive(Default)]
pub(super) struct DiscordPresenceState {
  last_track: Option<DiscordTrackInfo>,
  last_is_playing: Option<bool>,
  last_progress_ms: u128,
}

#[cfg(all(feature = "mpris", target_os = "linux"))]
#[derive(Default, PartialEq)]
struct MprisMetadata {
  title: String,
  artists: Vec<String>,
  album: String,
  duration_ms: u32,
  art_url: Option<String>,
}

#[cfg(all(feature = "mpris", target_os = "linux"))]
#[derive(Default)]
pub(super) struct MprisState {
  last_metadata: Option<MprisMetadata>,
  last_is_playing: Option<bool>,
  last_shuffle: Option<bool>,
  last_loop: Option<mpris::LoopStatusEvent>,
}

#[cfg(feature = "discord-rpc")]
fn build_discord_playback(app: &App) -> Option<discord_rpc::DiscordPlayback> {
  let snapshot = crate::infra::media_metadata::current_playback_snapshot(app)?;
  let artist = snapshot.primary_artist();
  let track_info = DiscordTrackInfo {
    title: snapshot.metadata.title,
    artist,
    album: snapshot.metadata.album,
    image_url: snapshot.metadata.image_url,
    duration_ms: snapshot.metadata.duration_ms,
  };

  let base_state = if track_info.album.is_empty() {
    track_info.artist.clone()
  } else {
    format!("{} - {}", track_info.artist, track_info.album)
  };
  let state = if snapshot.is_playing {
    base_state
  } else if base_state.is_empty() {
    app.strings().status.app.paused.to_string()
  } else {
    app.strings().status.app.paused_details(&base_state)
  };

  Some(discord_rpc::DiscordPlayback {
    title: track_info.title,
    artist: track_info.artist,
    album: track_info.album,
    state,
    image_url: track_info.image_url,
    duration_ms: track_info.duration_ms,
    progress_ms: snapshot.progress_ms,
    is_playing: snapshot.is_playing,
  })
}

#[cfg(feature = "discord-rpc")]
pub(super) fn update_discord_presence(
  manager: &discord_rpc::DiscordRpcManager,
  state: &mut DiscordPresenceState,
  app: &App,
) {
  let playback = build_discord_playback(app);

  match playback {
    Some(playback) => {
      let track_info = DiscordTrackInfo {
        title: playback.title.clone(),
        artist: playback.artist.clone(),
        album: playback.album.clone(),
        image_url: playback.image_url.clone(),
        duration_ms: playback.duration_ms,
      };

      let track_changed = state.last_track.as_ref() != Some(&track_info);
      let playing_changed = state.last_is_playing != Some(playback.is_playing);
      let progress_delta = playback.progress_ms.abs_diff(state.last_progress_ms);
      let progress_changed = progress_delta > 5000;

      if track_changed || playing_changed || progress_changed {
        manager.set_activity(&playback);
        state.last_track = Some(track_info);
        state.last_is_playing = Some(playback.is_playing);
        state.last_progress_ms = playback.progress_ms;
      }
    }
    None => {
      if state.last_track.is_some() {
        manager.clear();
        state.last_track = None;
        state.last_is_playing = None;
        state.last_progress_ms = 0;
      }
    }
  }
}

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub(super) fn update_mpris_state(manager: &mpris::MprisManager, state: &mut MprisState, app: &App) {
  if let Some(snapshot) = crate::infra::media_metadata::current_playback_snapshot(app) {
    let new_metadata = MprisMetadata {
      title: snapshot.metadata.title.clone(),
      artists: snapshot.metadata.artists.clone(),
      album: snapshot.metadata.album.clone(),
      duration_ms: snapshot.metadata.duration_ms,
      art_url: snapshot.metadata.image_url.clone(),
    };
    if state.last_metadata.as_ref() != Some(&new_metadata) {
      manager.set_metadata(
        &snapshot.metadata.title,
        &snapshot.metadata.artists,
        &snapshot.metadata.album,
        snapshot.metadata.duration_ms,
        snapshot.metadata.image_url.clone(),
      );
      state.last_metadata = Some(new_metadata);
    }

    if state.last_is_playing != Some(snapshot.is_playing) {
      manager.set_playback_status(snapshot.is_playing);
      state.last_is_playing = Some(snapshot.is_playing);
    }

    manager.set_position(snapshot.progress_ms as u64);

    if state.last_shuffle != Some(snapshot.shuffle) {
      manager.set_shuffle(snapshot.shuffle);
      state.last_shuffle = Some(snapshot.shuffle);
    }

    // A `None` repeat means the source has no repeat control (native queue,
    // radio); reset to `None` rather than leaving a stale Track/Playlist that a
    // prior decoded context pushed.
    let loop_status = match snapshot.repeat {
      Some(crate::core::plugin_api::RepeatState::Track) => mpris::LoopStatusEvent::Track,
      Some(crate::core::plugin_api::RepeatState::Context) => mpris::LoopStatusEvent::Playlist,
      Some(crate::core::plugin_api::RepeatState::Off) | None => mpris::LoopStatusEvent::None,
    };
    if state.last_loop != Some(loop_status) {
      manager.set_loop_status(loop_status);
      state.last_loop = Some(loop_status);
    }
  } else if state.last_metadata.is_some() {
    manager.set_stopped();
    state.last_metadata = None;
    state.last_is_playing = None;
  }
}

fn playback_window_title(app: &App) -> String {
  let Some(snapshot) = crate::infra::media_metadata::current_playback_snapshot(app) else {
    return DEFAULT_WINDOW_TITLE.to_string();
  };

  let title = sanitize_window_title_component(&snapshot.metadata.title);
  let artist_raw = sanitize_window_title_component(&snapshot.primary_artist());
  // Compose the artist segment with the em-dash separator, matching today's
  // `"{title} — {artist}"` output; omitted when there's no artist.
  let artist = if artist_raw.trim().is_empty() {
    String::new()
  } else {
    format!(" — {}", artist_raw)
  };
  app
    .user_config
    .format
    .window_title
    .render(&[&title, &artist])
}

fn sanitize_window_title_component(value: &str) -> String {
  value.chars().filter(|c| !c.is_control()).collect()
}

pub(super) fn next_window_title(state: &mut WindowTitleState, app: &App) -> Option<String> {
  if !app.user_config.behavior.set_window_title {
    return state
      .last_title
      .take()
      .map(|_| DEFAULT_WINDOW_TITLE.to_string());
  }

  let title = playback_window_title(app);
  if state.last_title.as_ref() == Some(&title) {
    None
  } else {
    state.last_title = Some(title.clone());
    Some(title)
  }
}

/// The teardown half: the default title to restore on exit, or `None` when
/// the title was never changed away from it. Clears the latch either way.
pub(super) fn window_title_reset(state: &mut WindowTitleState) -> Option<&'static str> {
  state
    .last_title
    .take()
    .filter(|title| title != DEFAULT_WINDOW_TITLE)
    .map(|_| DEFAULT_WINDOW_TITLE)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn window_title_reset_fires_only_after_a_custom_title() {
    let mut untouched = WindowTitleState::default();
    assert_eq!(window_title_reset(&mut untouched), None);

    let mut changed = WindowTitleState {
      last_title: Some("The Track — The Artist".to_string()),
    };
    assert_eq!(window_title_reset(&mut changed), Some(DEFAULT_WINDOW_TITLE));
    assert_eq!(window_title_reset(&mut changed), None);
  }
}
