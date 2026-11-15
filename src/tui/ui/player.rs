#[cfg(feature = "cover-art")]
use crate::tui::layout::center_rect_within;
#[cfg(feature = "cover-art")]
use crate::tui::layout::fullscreen_view_layout;
use crate::tui::theme::EmphasisExt;
#[cfg(feature = "cover-art")]
use ratatui::layout::Alignment;

use crate::core::app::{ActiveBlock, App};
use crate::tui::layout::miniplayer_playbar_area;
use ratatui::{
  layout::{Constraint, Layout, Position, Rect},
  style::{Modifier, Style},
  text::{Line, Span, Text},
  widgets::{Block, BorderType, Borders, LineGauge, Paragraph},
  Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::core::stream_format::{output_candidates, widest_fitting, OutputDevice, OutputFormat};

use super::util::{display_track_progress, get_color, get_track_progress_percentage};

const PLAYBAR_CONTROLS: [PlaybarControl; 7] = [
  PlaybarControl::Prev,
  PlaybarControl::PlayPause,
  PlaybarControl::Next,
  PlaybarControl::Shuffle,
  PlaybarControl::Repeat,
  PlaybarControl::VolumeDown,
  PlaybarControl::VolumeUp,
];
#[cfg(feature = "cover-art")]
const COVER_ART_CELL_RATIO: f32 = 1.9;
#[cfg(feature = "cover-art")]
const PLAYBAR_TRACK_INFO_ROWS: u16 = 2;
#[cfg(feature = "cover-art")]
const PLAYBAR_PROGRESS_ROWS: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlaybarControl {
  Prev,
  PlayPause,
  Next,
  Shuffle,
  Repeat,
  VolumeDown,
  VolumeUp,
}

impl PlaybarControl {
  /// The built-in (default) label for this control.
  fn default_label(self, strings: &crate::core::i18n::screens::ScreenStrings) -> &'static str {
    match self {
      Self::Prev => strings.control_prev,
      Self::PlayPause => strings.control_play,
      Self::Next => strings.control_next,
      Self::Shuffle => strings.control_shuffle,
      Self::Repeat => strings.control_repeat,
      Self::VolumeDown => strings.control_vol_down,
      Self::VolumeUp => strings.control_vol_up,
    }
  }

  /// The `playbar_control_labels` map key for this control.
  const fn config_key(self) -> &'static str {
    match self {
      Self::Prev => "prev",
      Self::PlayPause => "play_pause",
      Self::Next => "next",
      Self::Shuffle => "shuffle",
      Self::Repeat => "repeat",
      Self::VolumeDown => "vol_down",
      Self::VolumeUp => "vol_up",
    }
  }

  /// The label for this control, honoring a `playbar_control_labels` override
  /// from config (falling back to the built-in default). Mouse hit-testing
  /// uses the same resolver, so hitboxes always match what's rendered.
  fn label(
    self,
    behavior: &crate::core::user_config::BehaviorConfig,
    strings: &crate::core::i18n::screens::ScreenStrings,
  ) -> std::borrow::Cow<'static, str> {
    match behavior.playbar_control_labels.get(self.config_key()) {
      Some(override_label) => std::borrow::Cow::Owned(override_label.clone()),
      None => std::borrow::Cow::Borrowed(self.default_label(strings)),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlaybarControlHitbox {
  control: PlaybarControl,
  rect: Rect,
}

#[derive(Clone, Copy, Debug)]
struct PlaybarLayoutAreas {
  artist_area: Rect,
  controls_area: Rect,
  progress_area: Rect,
  #[cfg(feature = "cover-art")]
  cover_art: Option<Rect>,
}

#[cfg(feature = "cover-art")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlaybarCoverLayout {
  text_area: Rect,
  slot: Rect,
  image_area: Rect,
}

fn split_playbar_rows(area: Rect) -> (Rect, Rect, Rect) {
  if area.width == 0 || area.height == 0 {
    let empty = Rect::new(area.x, area.y, area.width, 0);
    return (empty, empty, empty);
  }

  if area.height == 1 {
    let empty = Rect::new(area.x, area.y, area.width, 0);
    return (empty, area, empty);
  }

  if area.height == 2 {
    let [controls_area, progress_area] = area.layout(&Layout::vertical([
      Constraint::Length(1),
      Constraint::Length(1),
    ]));
    let empty = Rect::new(area.x, area.y, area.width, 0);
    return (empty, controls_area, progress_area);
  }

  let [artist_area, controls_area, progress_area] = area.layout(&Layout::vertical([
    Constraint::Min(1),
    Constraint::Length(1),
    Constraint::Length(1),
  ]));

  (artist_area, controls_area, progress_area)
}

/// Whether `layout_chunk` is the main playbar while the now playing panel
/// shows: the panel has the cover, so the playbar leaves its thumbnail out.
/// A Mini player pane's playbar is another rect and keeps its own.
#[cfg(feature = "cover-art")]
fn now_playing_panel_beside(app: &App, layout_chunk: Rect) -> bool {
  crate::tui::layout::compute_main_layout(app)
    .is_some_and(|areas| areas.now_playing.is_some() && areas.playbar == layout_chunk)
}

fn playbar_layout_areas(app: &App, layout_chunk: Rect) -> PlaybarLayoutAreas {
  #[cfg(feature = "cover-art")]
  {
    // first create margins
    let [other] = layout_chunk.layout(&Layout::horizontal([Constraint::Fill(1)]).margin(1));

    let (other, cover_art) = if app
      .user_config
      .do_draw_cover_art(crate::tui::cover_art::full_image_support())
      && !now_playing_panel_beside(app, layout_chunk)
    {
      let cover_layout = playbar_cover_layout(
        other,
        app.user_config.behavior.playbar_cover_art_size_percent,
      );
      if let Some(rendered_size) =
        crate::tui::cover_art::size_for(cover_layout.image_area, &app.cover_art)
      {
        let cover_layout = cover_layout.with_rendered_size(rendered_size);
        let (artist_area, controls_area, progress_area) = split_cover_playbar_rows_localized(
          other,
          cover_layout.text_area,
          cover_layout.image_area,
          &app.user_config.behavior,
          &app.strings().screens,
        );

        return PlaybarLayoutAreas {
          artist_area,
          controls_area,
          progress_area,
          cover_art: Some(cover_layout.image_area),
        };
      } else {
        (other, None)
      }
    } else {
      (other, None)
    };

    let (artist_area, controls_area, progress_area) = split_playbar_rows(other);

    PlaybarLayoutAreas {
      artist_area,
      controls_area,
      progress_area,
      cover_art,
    }
  }

  #[cfg(not(feature = "cover-art"))]
  {
    let _ = app;
    let [inner] = layout_chunk.layout(&Layout::horizontal([Constraint::Fill(1)]).margin(1));
    let (artist_area, controls_area, progress_area) = split_playbar_rows(inner);

    PlaybarLayoutAreas {
      artist_area,
      controls_area,
      progress_area,
    }
  }
}

#[cfg(feature = "cover-art")]
fn playbar_cover_layout(inner: Rect, size_percent: u16) -> PlaybarCoverLayout {
  let size_percent = crate::core::user_config::clamp_playbar_cover_art_size_percent(size_percent);
  let image_height = scaled_cover_art_height(inner.height, size_percent);
  let requested_width = ((image_height as f32) * COVER_ART_CELL_RATIO).ceil() as u16;
  let max_slot_width = if inner.width > 2 {
    inner.width.saturating_sub(2)
  } else {
    inner.width
  };
  let slot_width = requested_width.min(max_slot_width);
  let separator_width = u16::from(slot_width > 0 && inner.width > slot_width);

  let slot = Rect::new(inner.x, inner.y, slot_width, inner.height);
  let text_x = inner
    .x
    .saturating_add(slot_width.saturating_add(separator_width));
  let text_width = inner
    .width
    .saturating_sub(slot_width.saturating_add(separator_width));
  let text_area = Rect::new(text_x, inner.y, text_width, inner.height);
  let image_area = center_rect_within(
    slot,
    Rect::new(0, 0, requested_width.min(slot_width), image_height),
  );

  PlaybarCoverLayout {
    text_area,
    slot,
    image_area,
  }
}

#[cfg(feature = "cover-art")]
fn scaled_cover_art_height(available_height: u16, size_percent: u16) -> u16 {
  if available_height == 0 {
    return 0;
  }

  let size_percent = crate::core::user_config::clamp_playbar_cover_art_size_percent(size_percent);
  let target_percent = if size_percent <= 100 {
    25 + ((size_percent.saturating_sub(25) as u32 * 35).saturating_add(74) / 75) as u16
  } else {
    60 + (((size_percent - 100) as u32 * 40).saturating_add(99) / 100) as u16
  };

  (((available_height as u32 * target_percent as u32).saturating_add(99) / 100) as u16)
    .clamp(1, available_height)
}

#[cfg(feature = "cover-art")]
impl PlaybarCoverLayout {
  fn with_rendered_size(self, rendered_size: Rect) -> Self {
    Self {
      image_area: bottom_aligned_rect_within(self.image_area, rendered_size),
      ..self
    }
  }
}

#[cfg(feature = "cover-art")]
fn bottom_aligned_rect_within(bounds: Rect, size: Rect) -> Rect {
  let width = size.width.min(bounds.width);
  let height = size.height.min(bounds.height);

  Rect {
    x: bounds.x + bounds.width.saturating_sub(width) / 2,
    y: bounds.y + bounds.height.saturating_sub(height),
    width,
    height,
  }
}

#[cfg(feature = "cover-art")]
fn split_cover_playbar_rows_localized(
  inner: Rect,
  text_area: Rect,
  image_area: Rect,
  behavior: &crate::core::user_config::BehaviorConfig,
  strings: &crate::core::i18n::screens::ScreenStrings,
) -> (Rect, Rect, Rect) {
  if inner.width == 0 || inner.height == 0 || text_area.width == 0 || text_area.height == 0 {
    let empty = Rect::new(text_area.x, text_area.y, text_area.width, 0);
    return (empty, empty, empty);
  }

  let progress_y = inner
    .y
    .saturating_add(inner.height.saturating_sub(PLAYBAR_PROGRESS_ROWS));
  let image_bottom = image_area.y.saturating_add(image_area.height);
  let progress_area = if image_bottom <= progress_y {
    Rect::new(inner.x, progress_y, inner.width, PLAYBAR_PROGRESS_ROWS)
  } else {
    Rect::new(
      text_area.x,
      progress_y,
      text_area.width,
      PLAYBAR_PROGRESS_ROWS,
    )
  };

  let controls_area = cover_playbar_controls_area_localized(
    inner,
    text_area,
    image_area,
    progress_area,
    behavior,
    strings,
  );
  let artist_area = cover_playbar_artist_area(text_area, image_area, controls_area, progress_area);

  (artist_area, controls_area, progress_area)
}

#[cfg(feature = "cover-art")]
fn cover_playbar_controls_area_localized(
  inner: Rect,
  text_area: Rect,
  image_area: Rect,
  progress_area: Rect,
  behavior: &crate::core::user_config::BehaviorConfig,
  strings: &crate::core::i18n::screens::ScreenStrings,
) -> Rect {
  // Reserve room for the full control row: this only decides *whether* a controls
  // row fits, and which controls actually apply depends on the owning source
  // (`playbar_supported_controls`), which this layout pass has no `App` to ask.
  // Sizing for the widest case keeps the row appearing in exactly the same
  // situations as before, just with fewer buttons drawn in it.
  let required_width =
    playbar_controls_required_width_localized(behavior, &PLAYBAR_CONTROLS, strings);
  let controls_y = progress_area.y.saturating_sub(1);
  let image_bottom = image_area.y.saturating_add(image_area.height);

  if image_bottom <= controls_y && inner.width >= required_width {
    return Rect::new(inner.x, controls_y, inner.width, 1);
  }

  let artist_y = image_area.y.max(text_area.y);
  let available_text_rows = controls_y.saturating_sub(artist_y);
  if text_area.width >= required_width && available_text_rows >= PLAYBAR_TRACK_INFO_ROWS {
    Rect::new(text_area.x, controls_y, text_area.width, 1)
  } else {
    Rect::new(text_area.x, text_area.y, text_area.width, 0)
  }
}

#[cfg(feature = "cover-art")]
fn cover_playbar_artist_area(
  text_area: Rect,
  image_area: Rect,
  controls_area: Rect,
  progress_area: Rect,
) -> Rect {
  let y = image_area.y.max(text_area.y);
  let bottom = if controls_area.height > 0 {
    controls_area.y
  } else {
    progress_area.y
  };
  let height = bottom.saturating_sub(y).min(PLAYBAR_TRACK_INFO_ROWS);

  Rect::new(text_area.x, y, text_area.width, height)
}

/// The controls that apply to whatever currently owns playback.
///
/// Rendering and hit-testing both go through this, so a button that is drawn is
/// always one that works. They used to disagree — the row drew all eight while
/// hit-testing accepted only Play/Pause — which left inert Shuffle/Repeat
/// buttons sitting on the local playbar, inviting a click that did nothing.
fn playbar_supported_controls(app: &App) -> Vec<PlaybarControl> {
  playbar_supported_controls_for(
    app.queue_owns_playback(),
    qobuz_playback_active(app),
    app.active_queueable_decoded_source(),
  )
}

/// The pure core of [`playbar_supported_controls`], taking the three facts it
/// depends on rather than an [`App`]: a real playback session needs an
/// `Arc<LocalPlayer>` holding an open audio device, so the support matrix would
/// otherwise be untestable.
fn playbar_supported_controls_for(
  queue_owns_playback: bool,
  qobuz_active: bool,
  queueable_decoded: bool,
) -> Vec<PlaybarControl> {
  let mut controls = PLAYBAR_CONTROLS.to_vec();

  // The queue slot owns the sink, so `draw_playbar` renders the *queued* track
  // with the mode indicators hidden. Transport still works: `App::next_track`
  // advances the queue and `App::previous_track` restarts the queued track.
  // Shuffle/Repeat have no queue-slot state to show, so they are dropped.
  if queue_owns_playback {
    controls.retain(|control| !matches!(control, PlaybarControl::Shuffle | PlaybarControl::Repeat));
    return controls;
  }

  // Nothing plays: every control stays drawn, and each reports that nothing
  // is playing when pressed.
  if !qobuz_active {
    return controls;
  }

  // Prev/Next/Shuffle/Repeat need a finite track list to act on.
  controls.retain(|control| match control {
    PlaybarControl::PlayPause | PlaybarControl::VolumeDown | PlaybarControl::VolumeUp => true,
    PlaybarControl::Prev
    | PlaybarControl::Next
    | PlaybarControl::Shuffle
    | PlaybarControl::Repeat => queueable_decoded,
  });
  controls
}

fn playbar_control_hitboxes_in_area_localized(
  controls_area: Rect,
  behavior: &crate::core::user_config::BehaviorConfig,
  controls: &[PlaybarControl],
  strings: &crate::core::i18n::screens::ScreenStrings,
) -> Vec<PlaybarControlHitbox> {
  if controls_area.width == 0 || controls_area.height == 0 {
    return Vec::new();
  }

  let required_width = playbar_controls_required_width_localized(behavior, controls, strings);
  let start_x = if controls_area.width > required_width {
    controls_area
      .x
      .saturating_add((controls_area.width - required_width) / 2)
  } else {
    controls_area.x
  };

  let mut x = start_x;
  let y = controls_area.y.saturating_add(controls_area.height / 2);
  let right = controls_area.x.saturating_add(controls_area.width);
  let mut hitboxes = Vec::with_capacity(controls.len());

  for control in controls.iter().copied() {
    let label = control.label(behavior, strings);
    let width = unicode_width::UnicodeWidthStr::width(label.as_ref()) as u16;
    if x.saturating_add(width) > right {
      break;
    }
    hitboxes.push(PlaybarControlHitbox {
      control,
      rect: Rect {
        x,
        y,
        width,
        height: 1,
      },
    });
    x = x.saturating_add(width.saturating_add(1));
  }

  hitboxes
}

fn playbar_controls_required_width_localized(
  behavior: &crate::core::user_config::BehaviorConfig,
  controls: &[PlaybarControl],
  strings: &crate::core::i18n::screens::ScreenStrings,
) -> u16 {
  controls
    .iter()
    .enumerate()
    .fold(0u16, |width, (idx, control)| {
      width.saturating_add(u16::from(idx > 0)).saturating_add(
        unicode_width::UnicodeWidthStr::width(control.label(behavior, strings).as_ref()) as u16,
      )
    })
}

pub(crate) fn playbar_control_hitboxes(
  app: &App,
  playbar_area: Rect,
) -> Vec<(PlaybarControl, Rect)> {
  if !playbar_controls_available(app) {
    return Vec::new();
  }

  let controls_area = playbar_layout_areas(app, playbar_area).controls_area;
  playbar_control_hitboxes_in_area_localized(
    controls_area,
    &app.user_config.behavior,
    &playbar_supported_controls(app),
    &app.strings().screens,
  )
  .into_iter()
  .map(|hitbox| (hitbox.control, hitbox.rect))
  .collect()
}

fn playbar_controls_available(app: &App) -> bool {
  // The queue slot owns playback, so `draw_playbar` is rendering the queued track
  // and `playbar_supported_controls` offers the controls that drive it. Checked
  // first because queueing from an idle app leaves `qobuz_playback` `None`.
  if app.queue_owns_playback() {
    return true;
  }

  qobuz_playback_active(app)
}

/// True when a Qobuz session owns playback.
fn qobuz_playback_active(app: &App) -> bool {
  app.qobuz_playback.is_some()
}

pub(crate) fn playbar_control_at(
  app: &App,
  playbar_area: Rect,
  x: u16,
  y: u16,
) -> Option<PlaybarControl> {
  playbar_control_hitboxes(app, playbar_area)
    .into_iter()
    .find_map(|(control, rect)| rect.contains(Position { x, y }).then_some(control))
}

/// Geometry of the seekable region of the playbar progress line, used to translate
/// a mouse column into an absolute playback position. Mirrors ratatui's `LineGauge`
/// layout: the left-aligned label is drawn first, then the gauge line begins one
/// column after it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlaybarProgressLine {
  /// Row the progress line is rendered on.
  pub(crate) row: u16,
  /// First column of the gauge line (the cell after the label + 1-column gap).
  pub(crate) start: u16,
  /// Number of cells in the gauge line.
  pub(crate) width: u16,
  /// Track duration the line represents, in milliseconds.
  pub(crate) duration_ms: u32,
}

impl PlaybarProgressLine {
  /// True when `(x, y)` lands on the seekable gauge line (excludes the time label).
  pub(crate) fn contains(&self, x: u16, y: u16) -> bool {
    y == self.row && x >= self.start && x < self.start + self.width
  }

  /// True when `y` is on the progress row, regardless of column (used for drags,
  /// where the column is clamped into range rather than rejected).
  pub(crate) fn on_row(&self, y: u16) -> bool {
    y == self.row
  }

  /// Map a column to an absolute position in milliseconds, clamped to the line.
  /// The far-right cell maps to just under the full duration so a click never
  /// overshoots into the next track.
  pub(crate) fn position_at(&self, x: u16) -> u32 {
    let last = self.start + self.width.saturating_sub(1);
    let offset = x.clamp(self.start, last) - self.start;
    let fraction = f64::from(offset) / f64::from(self.width.max(1));
    (f64::from(self.duration_ms) * fraction).round() as u32
  }
}

/// Compute the seekable geometry of the playbar progress line for the current
/// playback, or `None` when nothing is playing or the line is not rendered (e.g.
/// the single-row playbar, or a terminal too narrow to fit the gauge).
pub(crate) fn playbar_progress_line(app: &App, playbar_area: Rect) -> Option<PlaybarProgressLine> {
  let progress_area = playbar_layout_areas(app, playbar_area).progress_area;
  if progress_area.width == 0 || progress_area.height == 0 {
    return None;
  }

  // Duration and position as shown on the playbar.
  let snapshot = crate::infra::media_metadata::current_playback_snapshot(app)?;
  let duration_ms = snapshot.metadata.duration_ms;
  // A playbar whose position has gone stale (the frontend stopped ticking)
  // swaps the gauge label for a warning, so the geometry below would not
  // match the rendered bar; no seeking then.
  app.playback_position_ms()?;
  let progress_ms = snapshot.progress_ms;
  let duration_std = std::time::Duration::from_millis(u64::from(duration_ms));
  let label = display_track_progress(progress_ms, duration_std);

  // LineGauge writes the label (capped at the area width), then starts the line one
  // column later: `start = label_end + 1` (see ratatui-widgets LineGauge::render).
  let label_width = (UnicodeWidthStr::width(label.as_str()) as u16).min(progress_area.width);
  let start = progress_area.x + label_width + 1;
  let right = progress_area.x + progress_area.width;
  if start >= right {
    return None;
  }

  Some(PlaybarProgressLine {
    row: progress_area.y,
    start,
    width: right - start,
    duration_ms,
  })
}

fn render_playbar_template(app: &App, values: &[&str]) -> String {
  use crate::core::user_config::{FormatConfig, FORMAT_KEYS};
  static DEFAULTS: std::sync::OnceLock<FormatConfig> = std::sync::OnceLock::new();
  let defaults = DEFAULTS.get_or_init(FormatConfig::default_templates);
  let configured = &app.user_config.format.playbar_status;
  let default = &defaults.playbar_status;
  let localized = app.strings().screens.playbar_status;
  if configured == default {
    let named: Vec<_> = FORMAT_KEYS
      .iter()
      .copied()
      .zip(values.iter().copied())
      .collect();
    let mut rendered = crate::core::i18n::format_template(localized, &named);
    crate::core::format::strip_line_controls(&mut rendered);
    rendered
  } else {
    configured.render(values)
  }
}

fn draw_playbar_controls(f: &mut Frame<'_>, app: &App, controls_area: Rect) {
  let controls_style = Style::default().fg(app.user_config.theme.playbar_text.into());
  for hitbox in playbar_control_hitboxes_in_area_localized(
    controls_area,
    &app.user_config.behavior,
    &playbar_supported_controls(app),
    &app.strings().screens,
  ) {
    let control = Paragraph::new(Span::styled(
      hitbox
        .control
        .label(&app.user_config.behavior, &app.strings().screens)
        .into_owned(),
      controls_style,
    ));
    f.render_widget(control, hitbox.rect);
  }
}

#[cfg(feature = "cover-art")]
pub fn draw_cover_art_view(f: &mut Frame<'_>, app: &App) {
  let (content_area, playbar_area) = fullscreen_view_layout(&app.runtime_state, f.area());

  draw_cover_art_content(f, app, content_area);
  if let Some(playbar_area) = playbar_area {
    draw_playbar(f, app, playbar_area);
  }
}

pub fn draw_miniplayer(f: &mut Frame<'_>, app: &App) {
  let area = miniplayer_playbar_area(f.area());
  draw_playbar(f, app, area);
}

#[cfg(feature = "cover-art")]
pub(crate) fn draw_cover_art_content(f: &mut Frame<'_>, app: &App, area: Rect) {
  use ratatui::widgets::Clear;

  // Clear the area to remove any lingering terminal image protocol artifacts
  f.render_widget(Clear, area);

  // Extract track info for display below the cover art
  let (track_name, artist_str) = extract_track_info(app);

  if !app.cover_art.available() {
    // No image is loaded: show an explicit message for the current state rather
    // than a blank pane, so "no art" always reads as a deliberate outcome.
    let message = super::util::cover_art_status(app);
    let p = Paragraph::new(message)
      .style(Style::default().fg(app.user_config.theme.inactive.into()))
      .alignment(Alignment::Center);

    let vertical_center = area.y + area.height / 2;
    let center_area = Rect {
      x: area.x,
      y: vertical_center,
      width: area.width,
      height: 1,
    };
    f.render_widget(p, center_area);
    return;
  }

  let show_title = track_name.is_some();
  let show_artist = show_title && artist_str.is_some();
  let info_height = if show_title {
    1 + 1 + u16::from(show_artist)
  } else {
    0
  };
  let image_bounds = Rect {
    x: area.x,
    y: area.y,
    width: area.width,
    height: area.height.saturating_sub(info_height),
  };
  let available_image_size = Rect::new(
    0,
    0,
    image_bounds.width.saturating_sub(2),
    image_bounds.height.saturating_sub(2),
  );
  let fitted_image_size =
    crate::tui::cover_art::fullscreen_size_for(available_image_size, &app.cover_art)
      .unwrap_or(available_image_size);
  let centered_area = center_rect_within(image_bounds, fitted_image_size);

  crate::tui::cover_art::render_fullscreen(f, centered_area, &app.cover_art, &app.user_config);

  // Draw song info below the cover art
  if let Some(name) = track_name {
    let title_y = centered_area.y + centered_area.height + 1;
    if title_y < area.y + area.height {
      let title = Paragraph::new(name)
        .style(
          Style::default()
            .fg(app.user_config.theme.selected.into())
            .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
        )
        .alignment(Alignment::Center);
      f.render_widget(
        title,
        Rect {
          x: area.x,
          y: title_y,
          width: area.width,
          height: 1,
        },
      );
    }

    if let Some(artists) = artist_str {
      let artist_y = title_y + 1;
      if artist_y < area.y + area.height {
        let artist = Paragraph::new(artists)
          .style(Style::default().fg(app.user_config.theme.playbar_text.into()))
          .alignment(Alignment::Center);
        f.render_widget(
          artist,
          Rect {
            x: area.x,
            y: artist_y,
            width: area.width,
            height: 1,
          },
        );
      }
    }
  }
}

#[cfg(feature = "cover-art")]
fn extract_track_info(app: &App) -> (Option<String>, Option<String>) {
  // Read from the playback snapshot so the fullscreen cover view labels the
  // current track whether a playlist or the queue plays it.
  match crate::infra::media_metadata::current_playback_snapshot(app) {
    Some(snapshot) => (
      Some(snapshot.metadata.title.clone()),
      Some(snapshot.primary_artist()),
    ),
    None => (None, None),
  }
}

/// Display snapshot for the playbar (a Qobuz playlist or the native queue
/// slot).
///
/// Extracted from the live player so [`render_local_playbar`] is a pure function
/// of plain values and can be unit-tested with `TestBackend` (no audio device).
struct LocalPlaybarView {
  /// Source name shown in the playbar title: `"Qobuz"`, or the queue label.
  source_label: &'static str,
  name: String,
  artists: String,
  is_playing: bool,
  position_ms: u128,
  duration_ms: u64,
  volume_percent: u8,
  /// 1-based position in the queue and total length, e.g. `(3, 12)` => "3/12".
  /// `None` hides the indicator (e.g. a one-track session).
  queue_position: Option<(usize, usize)>,
  /// Whether the shuffle/repeat controls apply here. `true` for a playlist
  /// session; `false` for the native queue slot, whose playbar drops the mode
  /// segments entirely rather than showing blank `Shuffle:`/`Repeat:` labels.
  show_modes: bool,
  /// The delivered audio format (Qobuz), from the most to the least detailed
  /// wording; the widest that fits shows on the right of the name row.
  quality: Vec<String>,
  /// The format the output device runs at, on the right of the artists row.
  output: Option<OutputFormat>,
  /// The device the sound reaches and its format, after the output format.
  hardware: Option<OutputDevice>,
}

/// Render the playbar for an active Qobuz playback session; the download
/// window (`advancing`) shows as a loading suffix, like the queue slot.
fn draw_qobuz_playbar(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  let Some(qobuz) = app.qobuz_playback.as_ref() else {
    return;
  };
  let track = qobuz.current();
  let name = track.map(|t| t.name.clone()).unwrap_or_default();
  let view = LocalPlaybarView {
    source_label: "Qobuz",
    name: if qobuz.advancing {
      app.strings().screens.loading_track(&name)
    } else {
      name
    },
    artists: track.map(|t| t.artists.join(", ")).unwrap_or_default(),
    is_playing: !qobuz.player.is_paused(),
    position_ms: qobuz.player.position().as_millis(),
    duration_ms: track.map(|t| t.duration_ms).unwrap_or(0),
    volume_percent: app.runtime_state.volume_percent,
    queue_position: (qobuz.tracks.len() > 1).then(|| (qobuz.index + 1, qobuz.tracks.len())),
    show_modes: true,
    quality: qobuz.quality.map(|q| q.labels()).unwrap_or_default(),
    output: Some(qobuz.player.output_format()),
    hardware: app.hardware_output().cloned(),
  };
  render_local_playbar(f, app, layout_chunk, &view);
}

fn render_local_playbar(f: &mut Frame<'_>, app: &App, layout_chunk: Rect, view: &LocalPlaybarView) {
  let playbar_areas = playbar_layout_areas(app, layout_chunk);

  let play_title = if view.is_playing {
    app.strings().screens.playing
  } else {
    app.strings().screens.paused
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    matches!(
      current_route.active_block,
      ActiveBlock::PlayBar | ActiveBlock::MiniPlayer
    ),
    matches!(
      current_route.hovered_block,
      ActiveBlock::PlayBar | ActiveBlock::MiniPlayer
    ),
  );

  let queue_label = match view.queue_position {
    Some((current, total)) => format!(" | {current}/{total}"),
    None => String::new(),
  };
  // Shuffle + repeat (Off / Track / All). Each carries its own ` | Label: value`
  // prefix (values pre-padded to the `{:-3}` / `{:-5}` widths); the queue slot,
  // which has no such controls, renders them empty so the whole segment —
  // label included — disappears instead of leaking a blank control.
  use crate::infra::queue::RepeatMode;
  let (shuffle_text, repeat_text) = if view.show_modes {
    let shuffle = if app.decoded_shuffle {
      app.strings().screens.on
    } else {
      app.strings().screens.off
    };
    let repeat = match app.decoded_repeat {
      RepeatMode::Off => app.strings().screens.off,
      RepeatMode::Track => app.strings().screens.repeat_track,
      RepeatMode::Context => app.strings().screens.repeat_all,
    };
    (
      app
        .strings()
        .screens
        .shuffle_segment(&super::util::pad_to_width(shuffle, 3)),
      app
        .strings()
        .screens
        .repeat_segment(&super::util::pad_to_width(repeat, 5)),
    )
  } else {
    (String::new(), String::new())
  };
  // Build the title from the configurable playbar_status template.
  let title = render_playbar_template(
    app,
    &[
      // state — left-aligned to 7 cols (matches `{:-7}`)
      &super::util::pad_to_width(play_title, 7),
      // source / queue
      view.source_label,
      &queue_label,
      // shuffle / repeat — self-contained ` | Label: value` segments (or empty)
      &shuffle_text,
      &repeat_text,
      // volume — left-aligned to 2 (matches `{:-2}`)
      &format!("{:<2}", view.volume_percent),
    ],
  );
  let mut title_spans = vec![Span::styled(
    title,
    get_color(highlight_state, app.user_config.theme),
  )];
  if let Some(message) = app.status_message() {
    let msg_style = if app.status_message_is_error() {
      Style::default().fg(app.user_config.theme.error_text.into())
    } else {
      get_color(highlight_state, app.user_config.theme)
    };
    title_spans.push(Span::styled(format!(" | {}", message), msg_style));
  }
  for seg in app.plugin_playbar_segments.values() {
    title_spans.push(Span::styled(
      format!(" | {}", seg),
      Style::default().fg(app.user_config.theme.playbar_text.into()),
    ));
  }

  let title_block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .style(Style::default().bg(app.user_config.theme.playbar_background.into()))
    .title(Line::from(title_spans))
    .border_style(get_color(highlight_state, app.user_config.theme));
  f.render_widget(title_block, layout_chunk);

  let artist_area = playbar_areas.artist_area;
  let dim = Style::default()
    .fg(app.user_config.theme.playbar_text.into())
    .add_modifier(app.user_config.behavior.emphasis(Modifier::DIM));
  let mut name_block = Block::default().title(Span::styled(
    view.name.clone(),
    Style::default()
      .fg(app.user_config.theme.selected.into())
      .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
  ));
  // The delivered stream format sits on the right of the name row, in the
  // widest wording the name leaves room for.
  let name_width = UnicodeWidthStr::width(view.name.as_str());
  let stream_room = usize::from(artist_area.width).saturating_sub(name_width + 2);
  if let Some(label) = widest_fitting(&view.quality, stream_room) {
    name_block = name_block.title(Line::from(Span::styled(label.to_string(), dim)).right_aligned());
  }
  let artist = Paragraph::new(Text::from(Line::from(Span::styled(
    view.artists.clone(),
    Style::default().fg(app.user_config.theme.playbar_text.into()),
  ))))
  .style(Style::default().fg(app.user_config.theme.playbar_text.into()))
  .block(name_block);
  f.render_widget(artist, artist_area);

  // The output device's format sits on the right of the artists row.
  if artist_area.height >= 2 {
    let screens = &app.strings().screens;
    let candidates = output_candidates(
      view.output.as_ref(),
      view.hardware.as_ref(),
      &screens.device_words(),
    );
    if !candidates.is_empty() {
      let prefix = format!("{}: ", screens.output);
      let artists_width = UnicodeWidthStr::width(view.artists.as_str());
      let room = usize::from(artist_area.width)
        .saturating_sub(artists_width + 2)
        .saturating_sub(UnicodeWidthStr::width(prefix.as_str()));
      if let Some(label) = widest_fitting(&candidates, room) {
        let row = Rect::new(artist_area.x, artist_area.y + 1, artist_area.width, 1);
        let line = Line::from(vec![
          Span::styled(prefix, dim),
          Span::styled(label.to_string(), dim),
        ])
        .right_aligned();
        f.render_widget(Paragraph::new(line), row);
      }
    }
  }

  draw_playbar_controls(f, app, playbar_areas.controls_area);

  let duration_std = std::time::Duration::from_millis(view.duration_ms);
  let perc = get_track_progress_percentage(view.position_ms, duration_std);
  let song_progress_label = display_track_progress(view.position_ms, duration_std);
  let modifier = if app.user_config.behavior.enable_text_emphasis {
    Modifier::ITALIC | Modifier::BOLD
  } else {
    Modifier::empty()
  };
  let song_progress = LineGauge::default()
    .filled_style(
      Style::default()
        .fg(app.user_config.theme.playbar_progress.into())
        .add_modifier(modifier),
    )
    .unfilled_style(
      Style::default()
        .fg(app.user_config.theme.playbar_background.into())
        .add_modifier(modifier),
    )
    .ratio(perc as f64 / 100.0)
    .filled_symbol(app.user_config.behavior.gauge_filled_icon.as_str())
    .unfilled_symbol(app.user_config.behavior.gauge_unfilled_icon.as_str())
    .label(Span::styled(
      &song_progress_label,
      Style::default().fg(app.user_config.theme.playbar_progress_text.into()),
    ));
  f.render_widget(song_progress, playbar_areas.progress_area);

  // Paint the cover art into the slot `playbar_layout_areas` reserved — the
  // layout only carves out the space; without this the playbar shows a blank
  // indent where the image belongs.
  #[cfg(feature = "cover-art")]
  if let Some(cover_art) = playbar_areas.cover_art {
    crate::tui::cover_art::render(f, cover_art, &app.cover_art, &app.user_config);
  }
}

pub fn draw_playbar(f: &mut Frame<'_>, app: &App, layout_chunk: Rect) {
  // The native queue slot owns playback: render the queued track, not the
  // suspended playlist (whose `qobuz_playback` is still `Some`). Checked first
  // so the playbar always shows what is actually audible.
  if let Some(crate::infra::queue::QueueNowPlaying::Decoded(d)) = app.queue_now.as_ref() {
    let source_label = app.strings().screens.queue;
    // `advancing` spans the download/decode window (the slot is published
    // before the fetch), so surface it instead of a silent frozen bar.
    let name = if d.advancing {
      app
        .strings()
        .screens
        .loading_track(d.track.display_name(app.strings()))
    } else {
      d.track.display_name(app.strings()).to_owned()
    };
    let view = LocalPlaybarView {
      source_label,
      name,
      artists: d.track.artists.join(", "),
      is_playing: !d.player.is_paused(),
      position_ms: d.player.position().as_millis(),
      duration_ms: d.track.duration_ms,
      volume_percent: app.runtime_state.volume_percent,
      queue_position: None,
      // The native queue ignores the decoded shuffle/repeat modes (they belong
      // to the suspended source resumed once the queue drains), so hide them.
      show_modes: false,
      quality: d.quality.map(|q| q.labels()).unwrap_or_default(),
      output: Some(d.player.output_format()),
      hardware: app.hardware_output().cloned(),
    };
    render_local_playbar(f, app, layout_chunk, &view);
    return;
  }

  // Qobuz playback likewise renders from its own session state.
  if app.qobuz_playback.is_some() {
    draw_qobuz_playbar(f, app, layout_chunk);
    return;
  }

  {
    if let Some(message) = app.status_message() {
      let msg_style = if app.status_message_is_error() {
        Style::default().fg(app.user_config.theme.error_text.into())
      } else {
        Style::default().fg(app.user_config.theme.playbar_text.into())
      };
      let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(app.user_config.theme.playbar_background.into()))
        .title(Span::styled(
          app.strings().screens.status_title(message),
          msg_style,
        ))
        .border_style(Style::default().fg(app.user_config.theme.inactive.into()));
      f.render_widget(title_block, layout_chunk);
    }
  }
}

#[cfg(all(test, feature = "cover-art"))]
fn split_cover_playbar_rows(
  inner: Rect,
  text_area: Rect,
  image_area: Rect,
  behavior: &crate::core::user_config::BehaviorConfig,
) -> (Rect, Rect, Rect) {
  split_cover_playbar_rows_localized(
    inner,
    text_area,
    image_area,
    behavior,
    &crate::core::i18n::screens::EN,
  )
}
#[cfg(test)]
fn playbar_control_hitboxes_in_area(
  area: Rect,
  behavior: &crate::core::user_config::BehaviorConfig,
  controls: &[PlaybarControl],
) -> Vec<PlaybarControlHitbox> {
  playbar_control_hitboxes_in_area_localized(
    area,
    behavior,
    controls,
    &crate::core::i18n::screens::EN,
  )
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn idle_playbar_keeps_every_control() {
    // Nothing owns playback, so every control stays drawn.
    assert_eq!(
      playbar_supported_controls_for(false, false, false),
      PLAYBAR_CONTROLS.to_vec()
    );
  }

  #[test]
  fn a_session_without_a_track_list_keeps_only_play_and_volume() {
    // Without a finite track list there is nothing to skip through or shuffle,
    // so those buttons would invite a click that does nothing.
    let controls = playbar_supported_controls_for(false, true, false);
    assert_eq!(
      controls,
      vec![
        PlaybarControl::PlayPause,
        PlaybarControl::VolumeDown,
        PlaybarControl::VolumeUp,
      ]
    );
  }

  #[test]
  fn queue_slot_drops_the_modes_but_keeps_transport() {
    // A queued track played from an idle app and one played over a suspended
    // playlist must offer the same controls. Prev/Next stay: the queue honours
    // them (`next_track` advances the queue, `previous_track` restarts the
    // queued track).
    let over_idle = playbar_supported_controls_for(true, false, false);
    let over_decoded = playbar_supported_controls_for(true, true, false);
    let expected = vec![
      PlaybarControl::Prev,
      PlaybarControl::PlayPause,
      PlaybarControl::Next,
      PlaybarControl::VolumeDown,
      PlaybarControl::VolumeUp,
    ];
    assert_eq!(over_idle, expected);
    assert_eq!(over_decoded, expected);
  }

  #[test]
  fn control_hitboxes_handle_zero_sized_area() {
    let behavior = crate::core::user_config::UserConfig::new().behavior;
    assert!(
      playbar_control_hitboxes_in_area(Rect::new(0, 0, 0, 0), &behavior, &PLAYBAR_CONTROLS)
        .is_empty()
    );
    assert!(
      playbar_control_hitboxes_in_area(Rect::new(0, 0, 10, 0), &behavior, &PLAYBAR_CONTROLS)
        .is_empty()
    );
  }

  #[test]
  fn control_hitboxes_truncate_for_tiny_widths() {
    let behavior = crate::core::user_config::UserConfig::new().behavior;
    let hitboxes =
      playbar_control_hitboxes_in_area(Rect::new(5, 10, 8, 1), &behavior, &PLAYBAR_CONTROLS);
    assert_eq!(hitboxes.len(), 1);
    assert_eq!(hitboxes[0].control, PlaybarControl::Prev);
    assert_eq!(hitboxes[0].rect, Rect::new(5, 10, 6, 1));
  }

  #[test]
  fn control_hitboxes_include_all_controls_when_wide_enough() {
    let behavior = crate::core::user_config::UserConfig::new().behavior;
    let hitboxes =
      playbar_control_hitboxes_in_area(Rect::new(0, 0, 200, 1), &behavior, &PLAYBAR_CONTROLS);
    assert_eq!(hitboxes.len(), PLAYBAR_CONTROLS.len());
    assert_eq!(hitboxes[0].control, PlaybarControl::Prev);
    assert_eq!(
      hitboxes[PLAYBAR_CONTROLS.len() - 1].control,
      PlaybarControl::VolumeUp
    );
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn center_rect_within_centers_smaller_rect() {
    let bounds = Rect::new(10, 20, 100, 50);
    let size = Rect::new(0, 0, 80, 40);

    assert_eq!(center_rect_within(bounds, size), Rect::new(20, 25, 80, 40));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn playbar_cover_layout_uses_default_slot_width() {
    let layout = playbar_cover_layout(Rect::new(2, 3, 100, 4), 100);

    assert_eq!(layout.slot, Rect::new(2, 3, 6, 4));
    assert_eq!(layout.image_area, Rect::new(2, 3, 6, 3));
    assert_eq!(layout.text_area, Rect::new(9, 3, 93, 4));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn playbar_cover_layout_centers_rendered_image_in_slot() {
    let layout =
      playbar_cover_layout(Rect::new(2, 3, 100, 6), 200).with_rendered_size(Rect::new(0, 0, 8, 4));

    assert_eq!(layout.slot, Rect::new(2, 3, 12, 6));
    assert_eq!(layout.image_area, Rect::new(4, 5, 8, 4));
    assert_eq!(layout.text_area, Rect::new(15, 3, 87, 6));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn playbar_cover_layout_bottom_aligns_smaller_rendered_image_at_max_size() {
    let layout =
      playbar_cover_layout(Rect::new(2, 3, 100, 4), 200).with_rendered_size(Rect::new(0, 0, 6, 3));

    assert_eq!(layout.image_area, Rect::new(3, 4, 6, 3));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn cover_playbar_progress_reclaims_the_row_below_the_cover() {
    let behavior = crate::core::user_config::UserConfig::new().behavior;
    let inner = Rect::new(1, 3, 49, 4);
    let text_area = Rect::new(10, 3, 40, 4);
    let image_area = Rect::new(1, 3, 6, 3);
    let (artist_area, controls_area, progress_area) =
      split_cover_playbar_rows(inner, text_area, image_area, &behavior);

    assert_eq!(artist_area, Rect::new(10, 3, 40, 2));
    assert_eq!(controls_area, Rect::new(10, 3, 40, 0));
    assert_eq!(progress_area, Rect::new(1, 6, 49, 1));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn cover_playbar_progress_stays_beside_a_full_height_cover() {
    let behavior = crate::core::user_config::UserConfig::new().behavior;
    let inner = Rect::new(1, 3, 49, 4);
    let text_area = Rect::new(10, 3, 40, 4);
    let image_area = Rect::new(1, 3, 8, 4);
    let (artist_area, controls_area, progress_area) =
      split_cover_playbar_rows(inner, text_area, image_area, &behavior);

    assert_eq!(artist_area, Rect::new(10, 3, 40, 2));
    assert_eq!(controls_area, Rect::new(10, 3, 40, 0));
    assert_eq!(progress_area, Rect::new(10, 6, 40, 1));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn cover_playbar_rows_reserve_full_width_controls_below_the_cover() {
    let behavior = crate::core::user_config::UserConfig::new().behavior;
    let inner = Rect::new(1, 3, 109, 6);
    let text_area = Rect::new(14, 3, 96, 6);
    let image_area = Rect::new(1, 3, 10, 4);
    let (artist_area, controls_area, progress_area) =
      split_cover_playbar_rows(inner, text_area, image_area, &behavior);

    assert_eq!(artist_area, Rect::new(14, 3, 96, 2));
    assert_eq!(controls_area, Rect::new(1, 7, 109, 1));
    assert_eq!(progress_area, Rect::new(1, 8, 109, 1));
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn playbar_cover_layout_scales_smaller_and_larger_sizes() {
    let smaller = playbar_cover_layout(Rect::new(0, 0, 100, 4), 50);
    let larger = playbar_cover_layout(Rect::new(0, 0, 100, 4), 200);

    assert_eq!(smaller.slot.width, 4);
    assert_eq!(smaller.text_area, Rect::new(5, 0, 95, 4));
    assert_eq!(larger.slot.width, 8);
    assert_eq!(larger.text_area, Rect::new(9, 0, 91, 4));
    assert_eq!(smaller.image_area.height, 2);
    assert_eq!(larger.image_area.height, 4);
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn scaled_cover_art_height_maps_200_to_full_available_height() {
    assert_eq!(scaled_cover_art_height(5, 25), 2);
    assert_eq!(scaled_cover_art_height(5, 100), 3);
    assert_eq!(scaled_cover_art_height(5, 200), 5);
  }

  #[cfg(feature = "cover-art")]
  #[test]
  fn playbar_cover_layout_clamps_to_tiny_playbar_area() {
    let layout = playbar_cover_layout(Rect::new(0, 0, 10, 6), 200);

    assert_eq!(layout.slot, Rect::new(0, 0, 8, 6));
    assert_eq!(layout.image_area, Rect::new(0, 0, 8, 6));
    assert_eq!(layout.text_area, Rect::new(9, 0, 1, 6));

    let zero_height = playbar_cover_layout(Rect::new(4, 5, 10, 0), 100);
    assert_eq!(zero_height.slot, Rect::new(4, 5, 0, 0));
    assert_eq!(zero_height.text_area, Rect::new(4, 5, 10, 0));
  }

  /// Collect every rendered cell symbol in `area` into one string for substring
  /// assertions.
  fn rendered_text(app: &App, area: Rect, view: &LocalPlaybarView) -> String {
    use ratatui::{backend::TestBackend, Terminal};

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
      .draw(|f| render_local_playbar(f, app, area, view))
      .unwrap();
    let buffer = terminal.backend().buffer();
    (0..area.height)
      .flat_map(|y| (0..area.width).map(move |x| (x, y)))
      .filter_map(|(x, y)| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
      .collect()
  }

  fn flac_labels() -> Vec<String> {
    crate::infra::qobuz::StreamQuality {
      sample_rate: 44_100,
      bits_per_sample: 16,
      flac: true,
      bitrate_kbps: Some(987),
    }
    .labels()
  }

  fn view_with(quality: Vec<String>, output: Option<OutputFormat>) -> LocalPlaybarView {
    LocalPlaybarView {
      source_label: "Qobuz",
      name: "Track".to_string(),
      artists: "Artist".to_string(),
      is_playing: true,
      position_ms: 0,
      duration_ms: 200_000,
      volume_percent: 50,
      queue_position: None,
      show_modes: true,
      quality,
      output,
      hardware: None,
    }
  }

  fn built_in_output() -> OutputFormat {
    OutputFormat {
      sample_rate: 48_000,
      channels: 2,
      bits: 32,
      float: true,
      device: Some("Built-in Audio".to_string()),
    }
  }

  #[test]
  fn playbar_shows_the_stream_format_on_the_name_row() {
    let view = view_with(flac_labels(), None);
    let content = rendered_text(&App::default(), Rect::new(0, 0, 160, 6), &view);
    assert!(
      content.contains("FLAC \u{b7} 16-bit \u{b7} 44.1 kHz \u{b7} 987 kbps"),
      "the delivered format shows in full: {content}"
    );
    assert!(
      !content.contains("Output"),
      "no output row without a device"
    );
  }

  #[test]
  fn the_stream_format_shortens_to_what_the_width_allows() {
    let view = view_with(flac_labels(), None);
    let content = rendered_text(&App::default(), Rect::new(0, 0, 40, 6), &view);
    assert!(
      content.contains("FLAC 16/44.1 \u{b7} 987 kbps"),
      "{content}"
    );
    assert!(!content.contains("16-bit"), "{content}");

    let long_name = LocalPlaybarView {
      name: "A track whose name takes the whole row".to_string(),
      ..view
    };
    let content = rendered_text(&App::default(), Rect::new(0, 0, 44, 6), &long_name);
    assert!(
      !content.contains("FLAC"),
      "the name wins over the format: {content}"
    );
  }

  #[test]
  fn playbar_shows_the_output_format_on_the_artists_row() {
    let view = view_with(flac_labels(), Some(built_in_output()));
    let content = rendered_text(&App::default(), Rect::new(0, 0, 160, 6), &view);
    assert!(
      content.contains("Output: 32-bit float \u{b7} 48 kHz \u{b7} 2 ch \u{b7} Built-in Audio"),
      "{content}"
    );
    let narrow = rendered_text(&App::default(), Rect::new(0, 0, 50, 6), &view);
    assert!(
      !narrow.contains("Built-in Audio") && narrow.contains("48 kHz"),
      "the device name is the first thing dropped: {narrow}"
    );
  }

  #[test]
  fn a_playbar_with_one_text_row_keeps_only_the_name_row() {
    let view = view_with(flac_labels(), Some(built_in_output()));
    let content = rendered_text(&App::default(), Rect::new(0, 0, 160, 5), &view);
    assert!(content.contains("987 kbps"), "{content}");
    assert!(
      !content.contains("Output"),
      "no second row to put the output on: {content}"
    );
  }

  #[test]
  fn translated_control_hitboxes_use_cell_width_and_keep_user_overrides() {
    use crate::core::i18n::screens::{FR, PT_BR};
    let mut behavior = App::default().user_config.behavior;
    let controls = [
      PlaybarControl::Prev,
      PlaybarControl::PlayPause,
      PlaybarControl::Next,
    ];
    for strings in [&PT_BR, &FR] {
      let area = Rect::new(4, 3, 100, 1);
      let hitboxes =
        playbar_control_hitboxes_in_area_localized(area, &behavior, &controls, strings);
      assert_eq!(hitboxes.len(), controls.len());
      for hitbox in &hitboxes {
        let label = hitbox.control.label(&behavior, strings);
        assert_eq!(
          usize::from(hitbox.rect.width),
          UnicodeWidthStr::width(label.as_ref())
        );
        assert!(hitbox.rect.right() <= area.right());
      }
      for pair in hitboxes.windows(2) {
        assert_eq!(pair[0].rect.right() + 1, pair[1].rect.x);
      }
    }
    behavior
      .playbar_control_labels
      .insert("prev".into(), "[我自己的 Prev]".into());
    assert_eq!(
      PlaybarControl::Prev.label(&behavior, &PT_BR),
      "[我自己的 Prev]"
    );
    assert_eq!(
      PlaybarControl::Prev.label(&behavior, &FR),
      "[我自己的 Prev]"
    );
  }

  #[test]
  fn playbar_templates_translate_only_defaults_and_keep_placeholder_values_literal() {
    use crate::core::i18n::Language;
    use crate::core::user_config::FORMAT_KEYS;
    let mut app = App::default();
    let values = ["Playing", "Desk {volume}", "", "", "", "50"];
    assert_eq!(
      render_playbar_template(&app, &values),
      "Playing (Desk {volume} | Volume: 50%)"
    );
    app.user_config.behavior.language = Language::Fr;
    app.refresh_language();
    let title = render_playbar_template(&app, &values);
    assert!(title.contains("Volume : 50 %"), "{title}");
    assert!(title.contains("Desk {volume}"), "{title}");
    app.user_config.format.playbar_status =
      crate::core::format::Template::parse("Volume: {volume}% / {source}", FORMAT_KEYS).unwrap();
    assert_eq!(
      render_playbar_template(&app, &values),
      "Volume: 50% / Desk {volume}"
    );
  }

  #[test]
  fn default_playbar_strips_controls_like_the_original_template_in_text_width_and_cells() {
    use crate::core::user_config::{FormatConfig, FORMAT_KEYS};
    use ratatui::{buffer::Buffer, widgets::Widget};
    use unicode_width::UnicodeWidthStr;
    let app = App::default();
    let values = [
      "Playing",
      "Desk\tSpeaker\nX\rY",
      "",
      " | Shuffle: Off",
      " | Repeat: Off  ",
      "50",
    ];
    let baseline =
      crate::core::format::Template::parse(FormatConfig::DEFAULT_PLAYBAR_STATUS, FORMAT_KEYS)
        .unwrap()
        .render(&values);
    assert_eq!(
      baseline,
      "Playing (DeskSpeakerXY | Shuffle: Off | Repeat: Off   | Volume: 50%)"
    );
    let current = render_playbar_template(&app, &values);
    assert_eq!(current, baseline);
    assert_eq!(current.width(), baseline.width());
    let render = |title: &str| {
      let area = Rect::new(0, 0, 100, 3);
      let mut buffer = Buffer::empty(area);
      Block::bordered()
        .title(Line::from(vec![Span::raw(title), Span::raw(" | MESSAGE")]))
        .render(area, &mut buffer);
      buffer
    };
    assert_eq!(render(&current), render(&baseline));
  }

  #[test]
  fn the_output_device_follows_the_output_format() {
    use crate::core::stream_format::{parse_hw_params, DeviceKind, OutputDevice};
    let mut view = view_with(flac_labels(), Some(built_in_output()));
    view.hardware = Some(OutputDevice {
      kind: DeviceKind::UsbDac,
      name: Some("FiiO KA3".into()),
      format: parse_hw_params(
        "format: S24_3LE\nchannels: 2\nrate: 96000 (96000/1)\n",
        None,
      ),
    });
    let content = rendered_text(&App::default(), Rect::new(0, 0, 160, 6), &view);
    assert!(
      content.contains(
        "Output: 32-bit float \u{b7} 48 kHz \u{b7} 2 ch \u{2192} USB DAC: 24-bit \u{b7} 96 kHz \u{b7} FiiO KA3"
      ),
      "{content}"
    );
    let narrow = rendered_text(&App::default(), Rect::new(0, 0, 50, 6), &view);
    assert!(
      narrow.contains("USB DAC: 24-bit \u{b7} 96 kHz"),
      "the device stays: {narrow}"
    );

    view.hardware = Some(OutputDevice {
      kind: DeviceKind::Bluetooth {
        codec: Some("ldac".into()),
      },
      name: Some("WH-1000XM4".into()),
      format: parse_hw_params("format: S32_LE\nchannels: 2\nrate: 96000 (96000/1)\n", None),
    });
    let content = rendered_text(&App::default(), Rect::new(0, 0, 160, 6), &view);
    assert!(
      content.contains("Bluetooth: LDAC \u{b7} 96 kHz \u{b7} WH-1000XM4"),
      "{content}"
    );
  }
}
