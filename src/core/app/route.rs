use super::*;

pub(super) const DEFAULT_ROUTE: Route = Route {
  id: RouteId::Home,
  active_block: ActiveBlock::Empty,
  hovered_block: ActiveBlock::Library,
};

#[derive(PartialEq, Debug, Default)]
pub enum SearchResultBlock {
  SongSearch,
  AlbumSearch,
  ArtistSearch,
  #[default]
  Empty,
}

/// The search screen's tab: songs (Enter plays the song), albums (Enter plays
/// the whole album) or artists (Enter opens the artist's page). One catalogue
/// search fills all three, so switching needs no new request.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SearchMode {
  #[default]
  Songs,
  Albums,
  Artists,
}

impl SearchMode {
  pub const ALL: [SearchMode; 3] = [SearchMode::Songs, SearchMode::Albums, SearchMode::Artists];

  /// The next tab, or the previous one when `forward` is false, wrapping.
  pub fn cycled(self, forward: bool) -> Self {
    let index = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
    let len = Self::ALL.len();
    Self::ALL[if forward {
      (index + 1) % len
    } else {
      (index + len - 1) % len
    }]
  }

  /// The results block this mode lists.
  pub fn block(self) -> SearchResultBlock {
    match self {
      SearchMode::Songs => SearchResultBlock::SongSearch,
      SearchMode::Albums => SearchResultBlock::AlbumSearch,
      SearchMode::Artists => SearchResultBlock::ArtistSearch,
    }
  }
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

  AnnouncementPrompt,
  RecapPrompt,
  ExitPrompt,
  Settings,
  SortMenu,
  Queue,
  Stats,
  Artist,
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

  AnnouncementPrompt,
  RecapPrompt,
  ExitPrompt,
  Settings,
  HelpMenu,
  Queue,
  Stats,
  /// An artist's page: popular tracks and discography.
  Artist,
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
  /// Whether the active browse scope and the session can serve a row or key.
  pub(crate) fn availability(&self, requirement: Requirement) -> Availability {
    availability(requirement, self.active_source)
  }

  /// Fetch the sidebar data a browse source needs.
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
    }
  }

  pub fn pop_navigation_stack(&mut self) -> Option<Route> {
    info!("navigating back");
    if self.navigation_stack.len() == 1 {
      None
    } else {
      let popped = self.navigation_stack.pop();
      // Leaving the error screen dismisses the error. Done here rather than in
      // a key handler so every back path clears it (the escape key, the
      // configurable back key, a script's `Back`), and so a frontend with no
      // back key at all is not the odd one out. Keyed on the frame that was
      // POPPED, not on the new top: a screen opened from the error page stacks
      // over it, and coming back must still show the message.
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
    self
      .navigation_stack
      .retain(|route| !(route.id == RouteId::Error && route.active_block == ActiveBlock::Error));
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
    if let Some(active_block) = active_block {
      current_route.active_block = active_block;
    }
    if let Some(hovered_block) = hovered_block {
      current_route.hovered_block = hovered_block;
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
}

#[cfg(test)]
mod tests {}
