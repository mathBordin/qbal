use super::*;

/// The audio analysis of the playing Spotify track, by base62 id.
#[derive(Debug, Default, PartialEq)]
pub enum AnalysisState {
  #[default]
  Idle,
  Loading(String),
}

impl AnalysisState {
  fn track_id(&self) -> Option<&str> {
    match self {
      Self::Idle => None,
      Self::Loading(id) => Some(id),
    }
  }
}

/// Pitch class names, C to B.
pub const PITCH_NAMES: [&str; 12] = [
  "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// The longest time the pitch view moves on past the last position update.
const MAX_CLOCK_AHEAD: Duration = Duration::from_millis(1500);

/// The time over which a position correction is blended in.
const BLEND: Duration = Duration::from_millis(400);

/// A larger correction is a seek or a new track and applies at once.
const MAX_BLEND_MS: f64 = 1000.0;

/// How fast a level rises toward a higher target, per second.
const RISE_RATE: f32 = 35.0;

/// The fall acceleration of a level, in full heights per second squared.
const GRAVITY: f32 = 4.0;

/// The pitch view's clock: the last position update, and the correction it blends in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AnalysisClock {
  base: u128,
  start_ms: f64,
  error_ms: f64,
  at: Instant,
  playing: bool,
}

impl AnalysisClock {
  fn position_ms(&self, now: Instant) -> f64 {
    if !self.playing {
      return self.start_ms + self.error_ms;
    }
    let elapsed = now.saturating_duration_since(self.at);
    let ahead = elapsed.min(MAX_CLOCK_AHEAD).as_secs_f64() * 1000.0;
    let blend = (elapsed.as_secs_f64() / BLEND.as_secs_f64()).min(1.0);
    self.start_ms + ahead + self.error_ms * blend
  }
}

/// "F# minor", "C major", or "?" when no key was detected.
#[cfg(test)]
pub fn key_label(key: Option<u8>, minor: Option<bool>) -> String {
  let Some(name) = key.and_then(|key| PITCH_NAMES.get(usize::from(key))) else {
    return "?".to_string();
  };
  match minor {
    Some(true) => format!("{name} minor"),
    Some(false) => format!("{name} major"),
    None => name.to_string(),
  }
}

impl App {
  /// Whether the analysis screen shows the Spotify pitch view instead of a capture style.
  pub fn pitch_view_shows(&self) -> bool {
    self.pitch_view_available() && self.runtime_state.pitch_view_chosen
  }

  /// Remember whether `V` last picked the pitch view, so the next launch opens the same visualizer.
  pub(crate) fn choose_pitch_view(&mut self, chosen: bool) {
    if self.runtime_state.pitch_view_chosen != chosen {
      self.runtime_state.pitch_view_chosen = chosen;
      self.schedule_state_save(PersistedRuntimeState::pitch_view_chosen(chosen));
    }
  }

  /// Whether `V` can pick the pitch view: the analysis screen or its pane shows and a Spotify track plays.
  pub(crate) fn pitch_view_available(&self) -> bool {
    self.spotify_track_playing
      && (self.get_current_route().active_block == ActiveBlock::Analysis
        || self.pane_open(Pane::Visualizer))
  }

  /// The tick's decision for the playing Spotify track; an analysis stays while its track plays.
  pub(crate) fn sync_audio_analysis(&mut self, track: Option<&str>) {
    self.spotify_track_playing = track.is_some();
    if self.pitch_view_shows() {
      self.want_audio_analysis(track);
    } else if self.audio_analysis.track_id() != track {
      self.want_audio_analysis(None);
    }
  }

  /// Whether the last tick saw any source play.
  pub fn playing_as_of_last_tick(&self) -> bool {
    self.analysis_clock.is_some_and(|clock| clock.playing)
  }

  /// Whether the visualizer still moves: a source plays, or its bars still fall after a pause.
  pub fn visualizer_moves(&self) -> bool {
    const SETTLED: f32 = 0.01;
    self.playing_as_of_last_tick()
      || self.pitch_levels.iter().any(|level| *level > SETTLED)
      || self
        .spectrum_data
        .as_ref()
        .is_some_and(|spectrum| spectrum.bands.iter().any(|band| *band > SETTLED))
  }

  /// Starts the clock from a new position: a small correction is blended in, a seek is not.
  pub(crate) fn note_analysis_clock(&mut self, playing: bool, now: Instant) {
    let base = self.song_progress_ms;
    if self
      .analysis_clock
      .is_some_and(|clock| clock.base == base && clock.playing == playing)
    {
      return;
    }
    let predicted = self
      .analysis_clock
      .filter(|clock| clock.playing && playing)
      .map(|clock| clock.position_ms(now));
    let (start_ms, error_ms) = match predicted {
      Some(predicted) if (base as f64 - predicted).abs() < MAX_BLEND_MS => {
        (predicted, base as f64 - predicted)
      }
      _ => (base as f64, 0.0),
    };
    self.analysis_clock = Some(AnalysisClock {
      base,
      start_ms,
      error_ms,
      at: now,
      playing,
    });
  }

  /// The position for the pitch view, moved on between the native player's 1 s updates.
  pub fn analysis_position_ms(&self, now: Instant) -> Option<u32> {
    let base = self.playback_position_ms()?;
    let position = match self.analysis_clock {
      Some(clock) if clock.base == base => clock.position_ms(now),
      _ => base as f64,
    };
    Some(position.max(0.0).round() as u32)
  }

  /// Moves the shown levels toward the analysis as cava moves its bars: up fast, down with gravity.
  pub(crate) fn step_pitch_levels(&mut self, _now: Instant, elapsed: Duration) {
    let target = [0.0_f32; 12];
    let dt = elapsed.as_secs_f32().min(0.1);
    let rise = 1.0 - (-RISE_RATE * dt).exp();
    for ((level, fall), goal) in self
      .pitch_levels
      .iter_mut()
      .zip(self.pitch_fall.iter_mut())
      .zip(target)
    {
      if goal >= *level {
        *level += (goal - *level) * rise;
        *fall = 0.0;
      } else {
        *fall += GRAVITY * dt;
        *level = (*level - *fall * dt).max(goal);
      }
    }
  }

  pub fn pitch_levels(&self) -> [f32; 12] {
    self.pitch_levels
  }

  /// Fetches the analysis of `track_id` once, or again after a failure; `None` drops it.
  pub(crate) fn want_audio_analysis(&mut self, track_id: Option<&str>) {
    let Some(id) = track_id else {
      self.audio_analysis = AnalysisState::Idle;
      return;
    };
    self.audio_analysis = AnalysisState::Loading(id.to_string());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_undetected_key_reads_as_unknown_not_c() {
    assert_eq!(key_label(None, Some(true)), "?");
    assert_eq!(key_label(Some(6), Some(true)), "F# minor");
    assert_eq!(key_label(Some(0), Some(false)), "C major");
  }
}
