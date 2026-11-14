use super::*;

impl App {
  /// Returns the volume the UI should show and volume-up/down should use as a base.
  pub fn desired_volume(&self) -> u32 {
    self.runtime_state.volume_percent as u32
  }

  /// Set volume to an absolute percentage (0-100). Routes through the decoded
  /// source's own dispatcher, so Lua actions behave identically to keypresses.
  /// A no-op without a decoded source owning the sink: there is no other
  /// player left to carry the volume.
  #[cfg_attr(not(feature = "scripting"), allow(dead_code))]
  pub fn set_volume_percent(&mut self, volume: u8) {
    let next_volume = volume.min(100);
    let current_volume = self.desired_volume() as u8;

    if next_volume != current_volume && self.active_decoded_source() {
      info!("setting volume to {}", next_volume);
      self.dispatch(IoEvent::ChangeVolume(next_volume));
      self.runtime_state.volume_percent = next_volume;
      self.schedule_state_save(PersistedRuntimeState::volume_percent(next_volume));
    }
  }

  /// Bump volume up. Uses `desired_volume()` as the base so rapid presses
  /// don't accidentally calculate from a stale value.
  pub fn increase_volume(&mut self) {
    let current_volume = self.desired_volume() as u8;
    let next_volume = min(
      current_volume + self.user_config.behavior.volume_increment,
      100,
    );

    if next_volume != current_volume && self.active_decoded_source() {
      info!("increasing volume: {} -> {}", current_volume, next_volume);
      self.dispatch(IoEvent::ChangeVolume(next_volume));
      self.runtime_state.volume_percent = next_volume;
      self.schedule_state_save(PersistedRuntimeState::volume_percent(next_volume));
    }
  }

  /// Bump volume down. Uses `desired_volume()` as the base so rapid presses
  /// don't accidentally calculate from a stale value.
  pub fn decrease_volume(&mut self) {
    let current_volume = self.desired_volume() as i8;
    let next_volume = max(
      current_volume - self.user_config.behavior.volume_increment as i8,
      0,
    );

    if next_volume != current_volume && self.active_decoded_source() {
      let next_volume_u8 = next_volume as u8;
      info!(
        "decreasing volume: {} -> {}",
        current_volume, next_volume_u8
      );
      self.dispatch(IoEvent::ChangeVolume(next_volume_u8));
      self.runtime_state.volume_percent = next_volume_u8;
      self.schedule_state_save(PersistedRuntimeState::volume_percent(next_volume_u8));
    }
  }
}
