use super::*;

#[derive(Clone, Copy)]
pub enum DisplayDomain {
  /// The GUI window layout; first, so a page has it before the route draws the frame.
  PaneLayout,
  Route,
  Status,
  Source,
  Theme,
  Playback,
  Search,
  Lyrics,
  Library,
  Queue,
  Stats,
  /// The plays finished since this process started.
  Session,
  /// The rows of the shared track table and the list they belong to.
  TrackTable,
  /// The settings rows a frontend offers.
  Settings,
  /// The desktop shell's updater.
  Update,
  /// The spotatui announcement that is active now.
  Announcement,
}

/// Per-domain revisions that move only when that domain's displayed state changed.
#[derive(Default, Clone, Copy)]
pub struct DisplayRevisions {
  pane_layout: u64,
  route: u64,
  status: u64,
  source: u64,
  theme: u64,
  playback: u64,
  search: u64,
  lyrics: u64,
  library: u64,
  queue: u64,
  stats: u64,
  session: u64,
  track_table: u64,
  settings: u64,
  update: u64,
  announcement: u64,
}

impl DisplayDomain {
  /// Every domain, in declaration order.
  pub const ALL: [DisplayDomain; 16] = [
    DisplayDomain::PaneLayout,
    DisplayDomain::Route,
    DisplayDomain::Status,
    DisplayDomain::Source,
    DisplayDomain::Theme,
    DisplayDomain::Playback,
    DisplayDomain::Search,
    DisplayDomain::Lyrics,
    DisplayDomain::Library,
    DisplayDomain::Queue,
    DisplayDomain::Stats,
    DisplayDomain::Session,
    DisplayDomain::TrackTable,
    DisplayDomain::Settings,
    DisplayDomain::Update,
    DisplayDomain::Announcement,
  ];
}

impl DisplayRevisions {
  pub(super) fn bump(&mut self, domain: DisplayDomain) {
    let slot = match domain {
      DisplayDomain::PaneLayout => &mut self.pane_layout,
      DisplayDomain::Route => &mut self.route,
      DisplayDomain::Status => &mut self.status,
      DisplayDomain::Source => &mut self.source,
      DisplayDomain::Theme => &mut self.theme,
      DisplayDomain::Playback => &mut self.playback,
      DisplayDomain::Search => &mut self.search,
      DisplayDomain::Lyrics => &mut self.lyrics,
      DisplayDomain::Library => &mut self.library,
      DisplayDomain::Queue => &mut self.queue,
      DisplayDomain::Stats => &mut self.stats,
      DisplayDomain::Session => &mut self.session,
      DisplayDomain::TrackTable => &mut self.track_table,
      DisplayDomain::Settings => &mut self.settings,
      DisplayDomain::Update => &mut self.update,
      DisplayDomain::Announcement => &mut self.announcement,
    };
    *slot = slot.wrapping_add(1);
  }

  #[cfg(test)]
  pub fn get(&self, domain: DisplayDomain) -> u64 {
    match domain {
      DisplayDomain::PaneLayout => self.pane_layout,
      DisplayDomain::Route => self.route,
      DisplayDomain::Status => self.status,
      DisplayDomain::Source => self.source,
      DisplayDomain::Theme => self.theme,
      DisplayDomain::Playback => self.playback,
      DisplayDomain::Search => self.search,
      DisplayDomain::Lyrics => self.lyrics,
      DisplayDomain::Library => self.library,
      DisplayDomain::Queue => self.queue,
      DisplayDomain::Stats => self.stats,
      DisplayDomain::Session => self.session,
      DisplayDomain::TrackTable => self.track_table,
      DisplayDomain::Settings => self.settings,
      DisplayDomain::Update => self.update,
      DisplayDomain::Announcement => self.announcement,
    }
  }
}

impl App {
  /// A copy of every display revision, for a frontend to diff against.
  #[cfg(test)]
  pub fn display_revisions(&self) -> DisplayRevisions {
    self.display_revisions
  }
}
