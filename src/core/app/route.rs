use super::*;

pub(super) const DEFAULT_ROUTE: Route = Route {
  id: RouteId::Home,
  active_block: ActiveBlock::Empty,
  hovered_block: ActiveBlock::Library,
};

#[derive(PartialEq, Debug, Default)]
pub enum SearchResultBlock {
  AlbumSearch,
  SongSearch,
  ArtistSearch,
  PlaylistSearch,
  ShowSearch,
  #[default]
  Empty,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DialogContext {
  PlaylistSearch,
  AddTrackToPlaylistPicker,
  PersistKeybindingFallback,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Analysis,
  PlayBar,
  Empty,
  Error,
  HelpMenu,
  Home,
  Input,
  Library,
  MyPlaylists,
  SearchResultBlock,
  TrackTable,
  LyricsView,
  CoverArtView,
  MiniPlayer,
  Dialog(DialogContext),

  AnnouncementPrompt,
  RecapPrompt,
  ExitPrompt,
  Settings,
  SortMenu,
  Queue,
  Stats,
  /// A plugin-registered custom screen (the screen name lives in
  /// [`RouteId::PluginScreen`]; `ActiveBlock` is `Copy` and can't carry it).
  /// Only script effects construct it.
  #[cfg_attr(not(feature = "scripting"), allow(dead_code))]
  PluginScreen,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SidebarBlock {
  #[default]
  Library,
  Playlists,
}

impl From<SidebarBlock> for ActiveBlock {
  fn from(block: SidebarBlock) -> Self {
    match block {
      SidebarBlock::Library => ActiveBlock::Library,
      SidebarBlock::Playlists => ActiveBlock::MyPlaylists,
    }
  }
}

impl From<ActiveBlock> for SidebarBlock {
  fn from(block: ActiveBlock) -> Self {
    match block {
      ActiveBlock::Library => SidebarBlock::Library,
      ActiveBlock::MyPlaylists => SidebarBlock::Playlists,
      _ => SidebarBlock::Library, // fallback for non-sidebar blocks
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[allow(clippy::enum_variant_names)]
pub enum InputContext {
  #[default]
  GlobalSearch,
  SavedTrackSearch,
  PlaylistTrackSearch,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Analysis,
  LyricsView,
  CoverArtView,
  MiniPlayer,
  Error,
  Home,
  Search,
  TrackTable,
  Dialog,

  AnnouncementPrompt,
  RecapPrompt,
  ExitPrompt,
  Settings,
  HelpMenu,
  Queue,
  Stats,
  /// A plugin-registered custom screen, keyed by its registered name.
  /// Only script effects construct it.
  #[cfg_attr(not(feature = "scripting"), allow(dead_code))]
  PluginScreen(String),
}

impl RouteId {
  /// Routes that can be shown at startup with no extra context (no album/artist
  /// id, search query, etc.). These are the only routes `startup_route` may
  /// select.
  pub const STARTUP_OPTIONS: &'static [RouteId] = &[RouteId::Home, RouteId::Stats];

  /// What a startup route needs before its screen opens populated.
  pub(crate) fn startup_requirement(&self) -> Requirement {
    Requirement::None
  }

  /// Parse a `startup_route` config token. Unknown / non-context-free strings
  /// return `None` (the caller logs a warning and falls back to Home).
  pub fn from_config_str(s: &str) -> Option<RouteId> {
    match s.trim().to_ascii_lowercase().as_str() {
      "home" => Some(RouteId::Home),
      "stats" => Some(RouteId::Stats),
      _ => None,
    }
  }

  /// The config-file token for this route (inverse of `from_config_str`).
  pub fn to_config_str(&self) -> &'static str {
    match self {
      RouteId::Home => "home",
      RouteId::Stats => "stats",
      _ => "home",
    }
  }
}

#[derive(Debug)]
pub struct Route {
  pub id: RouteId,
  pub active_block: ActiveBlock,
  pub hovered_block: ActiveBlock,
}

impl App {
  /// Switch the active browse source and mirror + persist the choice so the
  /// selection survives restarts. Browse scope only: never interrupts
  /// playback (routing goes by URI scheme). Sidebar data loading and any
  /// view resets stay with the caller.
  pub fn set_active_source(&mut self, source: Source) {
    if self.active_source != source {
      self.display_revisions.bump(DisplayDomain::Source);
      self.view.playlist_scroll_offset.set(0);
    }
    self.active_source = source;
    self.runtime_state.active_source = source;
    // The Spotify scope reaches disk when the login succeeds (see
    // `persist_active_source`), so a cancelled login never forces a browser at
    // the next boot.

    self.persist_active_source();
  }

  /// Whether the active browse scope and the session can serve a row or key.
  pub(crate) fn availability(&self, requirement: Requirement) -> Availability {
    availability(requirement, self.active_source)
  }

  /// Write the current browse scope to `state.yml`.
  pub(crate) fn persist_active_source(&mut self) {
    if let Err(e) = self.save_runtime_state(
      &crate::core::state::PersistedRuntimeState::active_source(self.runtime_state.active_source),
    ) {
      log::warn!("[source] failed to persist active_source: {e}");
    }
  }

  /// Fetch the sidebar data a browse source needs; Spotify's playlists arrive
  /// with the session.
  pub(crate) fn load_source_sidebar(&mut self, source: Source) {
    let event = match source {
      Source::Qobuz => IoEvent::GetQobuzPlaylists,
    };
    self.dispatch(event);
  }

  // The navigation_stack actually only controls the large block to the right of `library` and
  // `playlists`
  pub fn push_navigation_stack(&mut self, next_route_id: RouteId, next_active_block: ActiveBlock) {
    info!("navigating to {:?}", next_route_id);
    if !self
      .navigation_stack
      .last()
      .map(|last_route| last_route.id == next_route_id)
      .unwrap_or(false)
    {
      self.navigation_stack.push(Route {
        id: next_route_id,
        active_block: next_active_block,
        hovered_block: next_active_block,
      });
      self.display_revisions.bump(DisplayDomain::Route);
    }
  }

  pub fn pop_navigation_stack(&mut self) -> Option<Route> {
    info!("navigating back");
    if self.navigation_stack.len() == 1 {
      None
    } else {
      let popped = self.navigation_stack.pop();
      self.display_revisions.bump(DisplayDomain::Route);
      // Leaving the error screen dismisses the error. Done here rather than in
      // a key handler so every back path clears it (the escape key, the
      // configurable back key, a script's `Back`), and so a frontend with no
      // back key at all is not the odd one out. Keyed on the frame that was
      // POPPED, not on the new top: pressing `d` from the error page stacks the
      // device picker over it, and coming back must still show the message.
      if popped
        .as_ref()
        .is_some_and(|route| route.id == RouteId::Error)
      {
        self.clear_api_error();
      }
      popped
    }
  }

  /// Remove every frame still serving as the error screen.
  ///
  /// Matched on the id AND the block: a producer that rewrites the current
  /// frame in place could leave a `{ Error, Input }` frame holding text-input
  /// focus, and an id-only match would drop its keystrokes into the global
  /// bindings. No such producer exists today; such a frame no longer draws
  /// the error screen, so leaving it is harmless.
  ///
  /// Both `push_navigation_stack` and the frames below the top are covered:
  /// pushes dedupe only against the top frame, so navigating away and failing
  /// again buries a second error frame that would otherwise resurface later
  /// rendering an unrelated message, or an empty one.
  ///
  /// Cannot empty the stack: the bottom frame is `DEFAULT_ROUTE` (Home) or a
  /// `RouteId::STARTUP_OPTIONS` route, and `Error` is in neither.
  pub(super) fn drop_error_routes(&mut self) {
    let before = self.navigation_stack.len();
    self
      .navigation_stack
      .retain(|route| !(route.id == RouteId::Error && route.active_block == ActiveBlock::Error));
    if self.navigation_stack.len() != before {
      self.display_revisions.bump(DisplayDomain::Route);
    }
  }

  pub fn navigation_depth(&self) -> usize {
    self.navigation_stack.len()
  }

  pub fn get_current_route(&self) -> &Route {
    // if for some reason there is no route return the default
    self.navigation_stack.last().unwrap_or(&DEFAULT_ROUTE)
  }

  fn get_current_route_mut(&mut self) -> &mut Route {
    self.navigation_stack.last_mut().unwrap()
  }

  pub fn set_current_route_state(
    &mut self,
    active_block: Option<ActiveBlock>,
    hovered_block: Option<ActiveBlock>,
  ) {
    let current_route = self.get_current_route_mut();
    let before = (current_route.active_block, current_route.hovered_block);
    if let Some(active_block) = active_block {
      current_route.active_block = active_block;
    }
    if let Some(hovered_block) = hovered_block {
      current_route.hovered_block = hovered_block;
    }
    if (current_route.active_block, current_route.hovered_block) != before {
      self.display_revisions.bump(DisplayDomain::Route);
    }
  }

  /// Opens the audio analysis screen with the visualizer `V` last picked.
  pub fn get_audio_analysis(&mut self) {
    info!("entering audio analysis view");
    if self.get_current_route().id != RouteId::Analysis {
      self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
    }
  }

  /// Generate a listening recap: the selected period on the Stats screen
  /// when that screen is current, 30 days anywhere else.
  pub(crate) fn generate_recap(&mut self) {
    let period = if self.get_current_route().active_block == ActiveBlock::Stats {
      self.stats_period
    } else {
      RecapPeriod::ThirtyDays
    };
    self.dispatch(IoEvent::GenerateRecap(period));
  }

  /// A recap of the selected Stats period; a frontend with no Stats route asks for it by name.
  pub(crate) fn generate_stats_recap(&mut self) {
    self.dispatch(IoEvent::GenerateRecap(self.stats_period));
  }
}

#[cfg(test)]
mod tests {}
