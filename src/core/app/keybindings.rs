use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalInputCapabilities {
  pub keyboard_enhancement_supported: bool,
  pub keyboard_enhancement_enabled: bool,
}

impl App {
  pub fn effective_open_settings_key(&self) -> Key {
    self.user_config.keys.open_settings
  }

  pub fn effective_save_settings_key(&self) -> Key {
    self.user_config.keys.save_settings
  }
}
