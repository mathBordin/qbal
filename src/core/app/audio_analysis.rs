use super::*;

impl App {
  /// Records whether a source played on this tick.
  pub(crate) fn note_playing(&mut self, playing: bool) {
    self.playing_last_tick = playing;
  }

  /// Records the format the sound card runs at.
  pub(crate) fn set_hardware_output(
    &mut self,
    format: Option<crate::core::stream_format::HardwareFormat>,
  ) {
    self.hardware_output = format;
  }

  /// The format the sound card runs at, when one is playing.
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub fn hardware_output(&self) -> Option<&crate::core::stream_format::HardwareFormat> {
    self.hardware_output.as_ref()
  }

  /// Whether the last tick saw any source play.
  pub fn playing_as_of_last_tick(&self) -> bool {
    self.playing_last_tick
  }

  /// Whether the visualizer still moves: a source plays, or its bars still fall after a pause.
  pub fn visualizer_moves(&self) -> bool {
    const SETTLED: f32 = 0.01;
    self.playing_as_of_last_tick()
      || self
        .spectrum_data
        .as_ref()
        .is_some_and(|spectrum| spectrum.bands.iter().any(|band| *band > SETTLED))
  }
}
