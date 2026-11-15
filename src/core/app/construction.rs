use super::*;

impl Default for App {
  fn default() -> Self {
    App {
      spectrum_data: None,
      audio_capture_active: false,
      playing_last_tick: false,
      hardware_output: None,
      artist_page: None,
      album_details: None,
      album_details_failed_at: None,
      view: ViewState {
        search_hovered_block: SearchResultBlock::SongSearch,
        ..ViewState::default()
      },
      source_table_uri: None,
      stats_period: RecapPeriod::ThirtyDays,
      stats_loading: false,
      stats_data: None,
      listening_streaks: None,
      session_plays: Vec::new(),
      recap_prompt: None,
      qobuz_playlists: Vec::new(),
      active_source: Source::default(),
      user_config: UserConfig::new(),
      strings: &crate::core::i18n::EN,
      system_language: crate::core::i18n::Language::En,
      localize_messages: true,
      runtime_state: RuntimeState::default(),
      state_path: None,
      recently_played: None,
      library: Library {
        saved_tracks: ScrollableResultPages::new(),
        saved_albums: ScrollableResultPages::new(),
        saved_shows: ScrollableResultPages::new(),
      },
      navigation_stack: vec![DEFAULT_ROUTE],
      api_error: String::new(),
      api_error_expires_at: None,
      native_queue: Vec::new(),
      queue_suspended: None,
      queue_now: None,
      queue_slot_desired_playing: true,
      active_playlist_track_filter: None,
      pending_playlist_track_search: None,
      track_table_epoch: 0,
      search_results: SearchResult::default(),
      song_progress_ms: 0,
      last_tick_at: Instant::now(),
      seek_ms: None,
      last_source_seek: None,
      pending_source_seek: None,
      track_table: Default::default(),
      is_loading: false,
      io_tx: None,

      active_announcement: None,
      pending_announcements: Vec::new(),
      announcement_feed: Vec::new(),
      announcement_batch_dismissed: 0,
      lyrics: None,
      lyrics_status: LyricsStatus::default(),
      desired_lyrics_identity: None,
      lyrics_synced: false,
      // Settings defaults
      settings_items: Vec::new(),
      settings_saved_items: Vec::new(),
      keepawake: Default::default(),
      // Sort menu defaults
      playlist_sort: SortState::new(),
      track_table_unsorted: Vec::new(),
      status_message: None,
      status_message_expires_at: None,
      status_message_is_error: false,
      all_playlists: Vec::new(),
      state_save_due: None,
      pending_state_save_patch: PersistedRuntimeState::default(),
      state_save_error_reported: false,
      decoded_repeat: RepeatMode::Off,
      decoded_shuffle: false,
      qobuz_playback: None,
      decoded_sink_claim: None,
      #[cfg(all(feature = "mpris", target_os = "linux"))]
      mpris_manager: None,
      #[cfg(feature = "art-decode")]
      cover_art: crate::core::art::CoverArtStore::default(),
      #[cfg(feature = "art-decode")]
      desired_cover_art_key: None,
      #[cfg(feature = "art-decode")]
      cover_art_palette: None,
      #[cfg(feature = "art-decode")]
      cover_theme_state: crate::core::cover_theme::CoverThemeState::default(),
      #[cfg(feature = "art-decode")]
      theme_transition: None,
      pending_plugin_commands: Vec::new(),
      plugin_data_generations: PluginDataGenerations::default(),
      plugin_screens: std::collections::BTreeMap::new(),
      pending_plugin_screen_keys: Vec::new(),
      plugin_playbar_segments: std::collections::BTreeMap::new(),
      plugin_popup: None,
      log_path: crate::core::paths::app_log_path().display().to_string(),
    }
  }
}

impl App {
  pub fn new_with_state(
    io_tx: Sender<IoEvent>,
    user_config: UserConfig,
    mut runtime_state: RuntimeState,
    state_path: Option<PathBuf>,
    system_language: crate::core::i18n::Language,
  ) -> App {
    let strings = user_config
      .behavior
      .language
      .resolve(system_language)
      .strings();
    // A saved source can outlive the build that offered it (a state file from
    // the app Qbal forked from). Restore only a source this build can select.
    let sources = Source::picker_sources();
    if !sources.contains(&runtime_state.active_source) {
      runtime_state.active_source = sources[0];
    }
    let active_source = runtime_state.active_source;
    // Resolve configurable per-context default sort states. Config validation
    // already rejected invalid specs at load time, so parse failure here is a
    // defensive fallback to the built-in default sort.
    let parse_sort = |spec: &str, ctx: SortContext| -> SortState {
      SortState::parse(spec, ctx).unwrap_or_default()
    };
    let playlist_sort = parse_sort(
      &user_config.behavior.default_sort_playlist_tracks,
      SortContext::PlaylistTracks,
    );
    // Resolve the configurable startup route. Unknown / non-context-free values
    // degrade to Home + warn (precedent: StartupBehavior::from_name).
    let startup_route_id = match RouteId::from_config_str(&user_config.behavior.startup_route) {
      Some(id) => id,
      None => {
        log::warn!(
          "[config] startup_route '{}' is not a valid context-free route (valid: {}); using Home",
          user_config.behavior.startup_route,
          RouteId::STARTUP_OPTIONS
            .iter()
            .map(|r| r.to_config_str())
            .collect::<Vec<_>>()
            .join(", ")
        );
        RouteId::Home
      }
    };
    let startup_route = Route {
      id: startup_route_id,
      active_block: ActiveBlock::Empty,
      hovered_block: ActiveBlock::Library,
    };
    App {
      io_tx: Some(io_tx),
      user_config,
      strings,
      system_language,
      runtime_state,
      state_path,
      active_source,
      navigation_stack: vec![startup_route],
      playlist_sort,
      ..App::default()
    }
  }
}

#[cfg(test)]
mod tests {}
