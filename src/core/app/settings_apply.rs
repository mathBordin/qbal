use super::*;

impl App {
  /// Commit `settings_items` and write `config.yml`; `false` when the write
  /// failed (the error frame is on top, do not close the screen over it).
  pub(crate) fn save_settings_from_items(&mut self) -> bool {
    // Apply settings to user_config and save to file
    self.apply_settings_changes();

    #[cfg(target_os = "macos")]
    if self.user_config.keys.open_settings != Key::Ctrl(',') {
      self.keybinding_runtime.effective_open_settings = None;
      self.keybinding_runtime.fallback_reason = None;
    }
    if let Err(e) = self.user_config.save_config() {
      let message = self.strings().settings.save_failed(&e.to_string());
      self.handle_error_with_message(anyhow::anyhow!("Failed to save settings: {}", e), message);
      return false;
    }

    self.settings_saved_items = self.settings_items.clone();
    true
  }

  /// Set one Behavior row and write `config.yml`, past the Settings screen's staged rows.
  pub(crate) fn set_setting(&mut self, id: &str, input: crate::core::action::SettingInput) -> bool {
    use crate::core::action::SettingInput;
    let Some(mut item) = self
      .behavior_setting_items()
      .into_iter()
      .find(|item| item.id == id && self.setting_offered(item))
    else {
      return false;
    };
    item.value = match (item.value, input) {
      (SettingValue::Bool(_), SettingInput::Bool(v)) => SettingValue::Bool(v),
      (SettingValue::Number(_), SettingInput::Number(v)) => SettingValue::Number(v),
      (SettingValue::String(_), SettingInput::Text(v)) => SettingValue::String(v),
      (SettingValue::Cycle(_, options), SettingInput::Text(v)) if options.contains(&v.as_str()) => {
        SettingValue::Cycle(v, options)
      }
      _ => return false,
    };
    let staged = std::mem::replace(&mut self.settings_items, vec![item]);
    let staged_saved = std::mem::take(&mut self.settings_saved_items);
    let saved = self.save_settings_from_items();
    self.settings_items = staged;
    self.settings_saved_items = staged_saved;
    self.refresh_settings_labels();
    // Also after a failed write: the live value changed all the same.
    self.display_revisions.bump(DisplayDomain::Settings);
    saved
  }

  /// Override live theme colors field by field, the runtime `set_theme` path.
  pub(crate) fn set_theme_colors(
    &mut self,
    pairs: Vec<(crate::core::theme::ThemeField, crate::core::theme::Color)>,
  ) {
    let before = self.user_config.theme;
    for (field, color) in pairs {
      self.user_config.theme.set(field, color);
    }
    if self.user_config.theme != before {
      self.display_revisions.bump(DisplayDomain::Theme);
    }
  }

  /// Cycles through every style in `VisualizerStyle::all()` order (Cava, Bar
  /// Graph, Head Unit, Analog VU, Winamp, Phosphor Scope, Outrun, Rack EQ,
  /// Cassette Deck), then the Spotify pitch view while a Spotify track plays.
  /// Both the style and the pitch view choice are saved.
  pub(crate) fn cycle_visualizer_style(&mut self) {
    let style = self.user_config.behavior.visualizer_style;
    let first = VisualizerStyle::all()[0];
    let next = if self.pitch_view_shows() {
      first
    } else if self.pitch_view_available() && style.next() == first {
      self.choose_pitch_view(true);
      return;
    } else {
      style.next()
    };
    self.choose_pitch_view(false);
    if next != style {
      self.user_config.behavior.visualizer_style = next;
      // Save the config so the preference persists
      let _ = self.user_config.save_config();
    }
  }

  // Apply changes from settings_items back to user_config
  pub fn apply_settings_changes(&mut self) {
    use crate::core::user_config::{parse_theme_item, ThemePreset};

    let mut settings_error: Option<String> = None;
    // What is actually on screen right now, put back by the reconciliation at
    // the end so an adaptive-theme fade continues from it.
    let theme_before = self.user_config.theme;
    #[cfg(feature = "art-decode")]
    let cover_art_theme_before = self.user_config.behavior.cover_art_theme;
    // Run the arms against the user's own colors rather than the live blend:
    // settings_items holds only the current category's rows, so a save from
    // another category rewrites no theme field at all, and analysis_bar has no
    // row in any category. Whatever survives the arms IS the new base theme.
    #[cfg(feature = "art-decode")]
    {
      self.user_config.theme = self.user_theme();
    }
    for setting in &self.settings_items {
      match setting.id.as_str() {
        // Behavior settings
        "behavior.language" => {
          if let SettingValue::Cycle(value, _) = &setting.value {
            if let Some(language) = crate::core::i18n::Language::parse(value) {
              self.user_config.behavior.language = language;
            }
          }
        }
        "behavior.seek_milliseconds" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.seek_milliseconds = (*v).clamp(0, i64::from(u32::MAX)) as u32;
          }
        }
        "behavior.volume_increment" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.volume_increment = (*v).clamp(0, 100) as u8;
          }
        }
        "behavior.tick_rate_milliseconds" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.tick_rate_milliseconds = normalize_tick_rate_milliseconds(*v);
          }
        }
        "behavior.animation_tick_rate_milliseconds" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.animation_tick_rate_milliseconds =
              normalize_tick_rate_milliseconds(*v);
          }
        }
        "behavior.status_message_ttl_percent" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.status_message_ttl_percent = (*v).clamp(10, 1000) as u16;
          }
        }
        "behavior.qobuz_quality" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.qobuz_quality =
              crate::core::user_config::qobuz_quality_from_label(v);
          }
        }
        "behavior.playback_poll_seconds" => if let SettingValue::Number(_v) = &setting.value {},
        "behavior.table_scroll_padding" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.table_scroll_padding =
              (*v).clamp(0, i64::from(u16::MAX)) as u16;
          }
        }
        "behavior.like_animation_frames" => if let SettingValue::Number(_v) = &setting.value {},
        "behavior.enable_text_emphasis" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enable_text_emphasis = *v;
          }
        }
        "behavior.banner_gradient" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.banner_gradient = *v;
          }
        }
        "behavior.show_loading_indicator" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.show_loading_indicator = *v;
          }
        }
        "behavior.enforce_wide_search_bar" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enforce_wide_search_bar = *v;
          }
        }
        "behavior.group_folders_first" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.group_folders_first = *v;
          }
        }
        "behavior.disable_mouse_inputs" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.disable_mouse_inputs = *v;
          }
        }
        "behavior.set_window_title" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.set_window_title = *v;
          }
        }
        "behavior.enable_discord_rpc" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enable_discord_rpc = *v;
          }
        }
        "behavior.stop_after_current_track" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.stop_after_current_track = *v;
          }
        }
        "behavior.startup_behavior" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.startup_behavior =
              crate::core::user_config::StartupBehavior::from_name(v);
          }
        }
        "behavior.player_bar_visualizer" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.player_bar_visualizer =
              crate::core::user_config::PlayerBarVisualizer::from_name(v);
          }
        }
        "behavior.startup_route" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.startup_route = v.clone();
          }
        }
        "behavior.default_sort_playlist_tracks" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.default_sort_playlist_tracks = v.clone();
          }
        }
        "behavior.default_sort_saved_albums" => {
          if let SettingValue::Cycle(_v, _) = &setting.value {}
        }
        "behavior.default_sort_saved_artists" => {
          if let SettingValue::Cycle(_v, _) = &setting.value {}
        }
        "behavior.default_sort_recently_played" => {
          if let SettingValue::Cycle(_v, _) = &setting.value {}
        }
        "behavior.sidebar_position" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.sidebar_position = v.clone();
          }
        }
        "behavior.playbar_position" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.playbar_position = v.clone();
          }
        }
        "behavior.small_terminal_width" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.small_terminal_width =
              (*v).clamp(1, i64::from(u16::MAX)) as u16;
          }
        }
        "behavior.small_terminal_height" => {
          if let SettingValue::Number(v) = &setting.value {
            self.user_config.behavior.small_terminal_height =
              (*v).clamp(1, i64::from(u16::MAX)) as u16;
          }
        }
        "behavior.keepawake_enabled" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.keepawake_enabled = *v;
          }
        }
        "behavior.enable_media_keys" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enable_media_keys = *v;
          }
        }
        "behavior.enable_streaming" => if let SettingValue::Bool(_v) = &setting.value {},
        "behavior.streaming_device_name" => {
          if let SettingValue::String(v) = &setting.value {
            let name = v.trim();
            if !name.is_empty() {}
          }
        }
        "behavior.streaming_bitrate" => if let SettingValue::Cycle(_v, _) = &setting.value {},
        "behavior.streaming_audio_cache" => if let SettingValue::Bool(_v) = &setting.value {},
        "behavior.streaming_autoplay" => if let SettingValue::Bool(_v) = &setting.value {},
        "behavior.enable_announcements" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enable_announcements = *v;
          }
        }
        "behavior.enable_lyrics" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enable_lyrics = *v;
          }
        }
        "behavior.enable_monthly_recap_prompt" => {
          if let SettingValue::Bool(v) = &setting.value {
            self.user_config.behavior.enable_monthly_recap_prompt = *v;
          }
        }
        "behavior.pin_community_playlist" => if let SettingValue::Bool(_v) = &setting.value {},
        "behavior.announcement_feed_url" => {
          if let SettingValue::String(v) = &setting.value {
            let trimmed = v.trim();
            self.user_config.behavior.announcement_feed_url = if trimmed.is_empty() {
              None
            } else {
              Some(trimmed.to_string())
            };
          }
        }
        "behavior.sync_token" => {
          if let SettingValue::String(v) = &setting.value {
            let _trimmed = v.trim();
          }
        }
        "behavior.liked_icon" => if let SettingValue::String(_v) = &setting.value {},
        "behavior.shuffle_icon" => {
          if let SettingValue::String(v) = &setting.value {
            self.user_config.behavior.shuffle_icon = v.clone();
          }
        }
        "behavior.repeat_track_icon" => {
          if let SettingValue::String(v) = &setting.value {
            self.user_config.behavior.repeat_track_icon = v.clone();
          }
        }
        "behavior.repeat_context_icon" => {
          if let SettingValue::String(v) = &setting.value {
            self.user_config.behavior.repeat_context_icon = v.clone();
          }
        }
        "behavior.paused_icon" => {
          if let SettingValue::String(v) = &setting.value {
            self.user_config.behavior.paused_icon = v.clone();
          }
        }
        "behavior.playing_icon"
        | "behavior.gauge_filled_icon"
        | "behavior.gauge_unfilled_icon"
        | "behavior.episode_played_icon"
        | "behavior.sort_ascending_icon"
        | "behavior.sort_descending_icon" => {
          if let SettingValue::String(v) = &setting.value {
            if UnicodeWidthStr::width(v.as_str()) == 1 {
              match setting.id.as_str() {
                "behavior.gauge_filled_icon" => {
                  self.user_config.behavior.gauge_filled_icon = v.clone()
                }
                "behavior.gauge_unfilled_icon" => {
                  self.user_config.behavior.gauge_unfilled_icon = v.clone()
                }
                "behavior.episode_played_icon" => {}
                "behavior.sort_ascending_icon" => {
                  self.user_config.behavior.sort_ascending_icon = v.clone()
                }
                "behavior.sort_descending_icon" => {
                  self.user_config.behavior.sort_descending_icon = v.clone()
                }
                "behavior.playing_icon" => self.user_config.behavior.playing_icon = v.clone(),
                _ => {}
              }
            } else {
              settings_error = Some(self.strings().settings.icon_width(&setting.name));
            }
          }
        }
        "behavior.active_source_icon" => {
          if let SettingValue::String(v) = &setting.value {
            self.user_config.behavior.active_source_icon = v.clone();
          }
        }
        "behavior.list_highlight_icon" => {
          if let SettingValue::String(v) = &setting.value {
            self.user_config.behavior.list_highlight_icon = v.clone();
          }
        }
        #[cfg(feature = "cover-art")]
        "behavior.draw_cover_art" => {
          if let SettingValue::Bool(v) = setting.value {
            self.user_config.behavior.draw_cover_art = v;
          }
        }
        #[cfg(feature = "art-decode")]
        "behavior.cover_art_theme" => {
          if let SettingValue::Bool(v) = setting.value {
            self.user_config.behavior.cover_art_theme = v;
          }
        }
        #[cfg(feature = "cover-art")]
        "behavior.draw_cover_art_forced" => {
          if let SettingValue::Bool(v) = setting.value {
            self.user_config.behavior.draw_cover_art_forced = v;
          }
        }
        #[cfg(feature = "cover-art")]
        "behavior.playbar_cover_art_size_percent" => {
          if let SettingValue::Number(v) = setting.value {
            self.user_config.behavior.playbar_cover_art_size_percent =
              crate::core::user_config::normalize_playbar_cover_art_size_percent(v);
          }
        }
        #[cfg(feature = "cover-art")]
        "behavior.cover_art_dither" => {
          if let SettingValue::Bool(v) = setting.value {
            self.user_config.behavior.cover_art_dither = v;
          }
        }
        #[cfg(feature = "cover-art")]
        "behavior.cover_art_dither_algorithm" => {
          if let SettingValue::Cycle(v, _) = &setting.value {
            self.user_config.behavior.cover_art_dither_algorithm =
              crate::core::user_config::normalize_cover_art_dither_algorithm(v).to_string();
          }
        }
        #[cfg(feature = "cover-art")]
        "behavior.cover_art_dither_pixel_scale" => {
          if let SettingValue::Number(v) = setting.value {
            self.user_config.behavior.cover_art_dither_pixel_scale =
              crate::core::user_config::normalize_cover_art_dither_pixel_scale(v);
          }
        }
        // Keybindings
        "keys.back" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.back = key;
            }
          }
        }
        "keys.move_up" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.move_up = key;
            }
          }
        }
        "keys.move_down" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.move_down = key;
            }
          }
        }
        "keys.move_left" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.move_left = key;
            }
          }
        }
        "keys.move_right" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.move_right = key;
            }
          }
        }
        "keys.next_page" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.next_page = key;
            }
          }
        }
        "keys.previous_page" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.previous_page = key;
            }
          }
        }
        "keys.jump_to_start" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.jump_to_start = key;
            }
          }
        }
        "keys.jump_to_end" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.jump_to_end = key;
            }
          }
        }
        "keys.toggle_playback" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.toggle_playback = key;
            }
          }
        }
        "keys.seek_backwards" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.seek_backwards = key;
            }
          }
        }
        "keys.seek_forwards" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.seek_forwards = key;
            }
          }
        }
        "keys.next_track" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.next_track = key;
            }
          }
        }
        "keys.previous_track" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.previous_track = key;
            }
          }
        }
        "keys.force_previous_track" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.force_previous_track = key;
            }
          }
        }
        "keys.shuffle" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.shuffle = key;
            }
          }
        }
        "keys.repeat" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.repeat = key;
            }
          }
        }
        "keys.search" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.search = key;
            }
          }
        }
        "keys.help" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.help = key;
            }
          }
        }
        "keys.open_settings" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.open_settings = key;
            }
          }
        }
        "keys.save_settings" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.save_settings = key;
            }
          }
        }
        "keys.jump_to_album" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.jump_to_artist_album" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.jump_to_context" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.manage_devices" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.listening_party" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.decrease_volume" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.decrease_volume = key;
            }
          }
        }
        "keys.increase_volume" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.increase_volume = key;
            }
          }
        }
        "keys.add_item_to_queue" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.add_item_to_queue = key;
            }
          }
        }
        "keys.show_queue" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.show_queue = key;
            }
          }
        }
        "keys.remove_from_queue" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.remove_from_queue = key;
            }
          }
        }
        "keys.like_track" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.generate_recap" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.generate_recap = key;
            }
          }
        }
        "keys.copy_song_url" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.copy_album_url" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(_key) = crate::core::user_config::parse_key_public(v.clone()) {}
          }
        }
        "keys.audio_analysis" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.audio_analysis = key;
            }
          }
        }
        "keys.lyrics_view" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.lyrics_view = key;
            }
          }
        }
        "keys.miniplayer_view" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.miniplayer_view = key;
            }
          }
        }
        #[cfg(feature = "cover-art")]
        "keys.cover_art_view" => {
          if let SettingValue::Key(v) = &setting.value {
            if let Ok(key) = crate::core::user_config::parse_key_public(v.clone()) {
              self.user_config.keys.cover_art_view = key;
            }
          }
        }
        // Decides whether the per-color changes following will apply.
        // A named preset takes priority; the user's custom_theme is preserved
        // so they can return to it later by selecting Custom.
        "theme.preset" => {
          if let SettingValue::Preset(name) = &setting.value {
            let preset = ThemePreset::from_name(name);
            self.user_config.current_preset = preset;
            if preset != ThemePreset::Custom {
              self.user_config.theme = preset.to_theme();
            }
          }
        }
        // Individual theme color overrides only apply when on Custom; they
        // update both the active theme and the persisted custom_theme.
        "theme.active" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.active = c;
              self.user_config.custom_theme.active = c;
            }
          }
        }
        "theme.banner" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.banner = c;
              self.user_config.custom_theme.banner = c;
            }
          }
        }
        "theme.hint" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.hint = c;
              self.user_config.custom_theme.hint = c;
            }
          }
        }
        "theme.hovered" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.hovered = c;
              self.user_config.custom_theme.hovered = c;
            }
          }
        }
        "theme.selected" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.selected = c;
              self.user_config.custom_theme.selected = c;
            }
          }
        }
        "theme.inactive" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.inactive = c;
              self.user_config.custom_theme.inactive = c;
            }
          }
        }
        "theme.text" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.text = c;
              self.user_config.custom_theme.text = c;
            }
          }
        }
        "theme.error_text" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.error_text = c;
              self.user_config.custom_theme.error_text = c;
            }
          }
        }
        "theme.error_border" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.error_border = c;
              self.user_config.custom_theme.error_border = c;
            }
          }
        }
        "theme.playbar_background" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.playbar_background = c;
              self.user_config.custom_theme.playbar_background = c;
            }
          }
        }
        "theme.playbar_progress" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.playbar_progress = c;
              self.user_config.custom_theme.playbar_progress = c;
            }
          }
        }
        #[cfg(feature = "cover-art")]
        "theme.cover_art_dither_color" => {
          if let SettingValue::Color(v) = &setting.value {
            if v.trim().eq_ignore_ascii_case("auto") || v.trim().is_empty() {
              self.user_config.cover_art_dither_color = None;
            } else if let Ok(c) = parse_theme_item(v) {
              self.user_config.cover_art_dither_color = Some(c);
            }
          }
        }
        "theme.playbar_progress_text" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.playbar_progress_text = c;
              self.user_config.custom_theme.playbar_progress_text = c;
            }
          }
        }
        "theme.playbar_text" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.playbar_text = c;
              self.user_config.custom_theme.playbar_text = c;
            }
          }
        }
        "theme.highlighted_lyrics" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.highlighted_lyrics = c;
              self.user_config.custom_theme.highlighted_lyrics = c;
            }
          }
        }
        "theme.background" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.background = c;
              self.user_config.custom_theme.background = c;
            }
          }
        }
        "theme.header" if self.user_config.current_preset == ThemePreset::Custom => {
          if let SettingValue::Color(v) = &setting.value {
            if let Ok(c) = parse_theme_item(v) {
              self.user_config.theme.header = c;
              self.user_config.custom_theme.header = c;
            }
          }
        }
        _ => {}
      }
    }
    self.refresh_language();
    self.refresh_settings_labels();
    #[cfg(feature = "art-decode")]
    self.reconcile_cover_theme_after_settings(theme_before, cover_art_theme_before);
    if self.user_config.theme != theme_before {
      self.display_revisions.bump(DisplayDomain::Theme);
    }
    if let Some(message) = settings_error {
      self.set_status_message(message, 4);
    }
    let reload = false;
    if reload {
      self.load_source_sidebar(self.active_source);
    }
  }

  /// Updates the colour RGB entries when switching through the presets in themes
  pub fn sync_theme_color_settings(&mut self, theme: &crate::core::user_config::Theme) {
    let mappings: [(&str, crate::core::theme::Color); 16] = [
      ("theme.active", theme.active),
      ("theme.banner", theme.banner),
      ("theme.hint", theme.hint),
      ("theme.hovered", theme.hovered),
      ("theme.selected", theme.selected),
      ("theme.inactive", theme.inactive),
      ("theme.text", theme.text),
      ("theme.error_text", theme.error_text),
      ("theme.error_border", theme.error_border),
      ("theme.playbar_background", theme.playbar_background),
      ("theme.playbar_progress", theme.playbar_progress),
      ("theme.playbar_progress_text", theme.playbar_progress_text),
      ("theme.playbar_text", theme.playbar_text),
      ("theme.highlighted_lyrics", theme.highlighted_lyrics),
      ("theme.background", theme.background),
      ("theme.header", theme.header),
    ];
    for setting in &mut self.settings_items {
      if let Some((_, color)) = mappings.iter().find(|(id, _)| *id == setting.id) {
        setting.value = SettingValue::Color(color_to_string(*color));
      }
    }
  }

  /// Sync the Banner Gradient toggle to a newly selected preset's default
  /// (Terminal defaults to off so the banner follows the terminal palette).
  /// Custom keeps whatever the user chose.
  pub fn sync_banner_gradient_setting(&mut self, preset: crate::core::user_config::ThemePreset) {
    if preset == crate::core::user_config::ThemePreset::Custom {
      return;
    }
    if let Some(setting) = self
      .settings_items
      .iter_mut()
      .find(|s| s.id == "behavior.banner_gradient")
    {
      setting.value = SettingValue::Bool(preset.default_banner_gradient());
    }
  }
}

#[cfg(test)]
mod tests {}
