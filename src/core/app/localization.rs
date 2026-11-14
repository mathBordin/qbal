use super::*;

impl App {
  /// One table per app; neither another instance nor a test can change it.
  pub fn strings(&self) -> &'static crate::core::i18n::Strings {
    self.strings
  }

  pub(crate) fn language(&self) -> crate::core::i18n::Language {
    if self.localize_messages {
      self
        .user_config
        .behavior
        .language
        .resolve(self.system_language)
    } else {
      crate::core::i18n::Language::En
    }
  }

  /// Re-select after a settings change. Persisted values remain locale-neutral.
  pub(crate) fn refresh_language(&mut self) {
    let strings = self.language().strings();
    if !std::ptr::eq(self.strings, strings) {
      self.strings = strings;
      self.view.help_menu_model = None;
      // Stats contain generated count/duration prose. Rebuild it from local
      // history; raw track titles and artist names remain untouched.
      if self.stats_data.is_some() || self.stats_loading {
        self.stats_data = None;
        self.reload_stats();
      }
    }
  }

  /// Shared core/network diagnostics keep the CLI's English output contract.
  pub(crate) fn use_english_messages(&mut self) {
    self.localize_messages = false;
    self.refresh_language();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::i18n::{Language, EN, FR, PT_BR};

  #[test]
  fn language_changes_are_per_app() {
    let mut app = App::default();
    let other = App::default();
    app.user_config.behavior.language = Language::PtBr;
    app.refresh_language();
    assert!(std::ptr::eq(app.strings(), &PT_BR));
    assert!(std::ptr::eq(other.strings(), &EN));
    app.user_config.behavior.language = Language::Fr;
    app.refresh_language();
    assert!(std::ptr::eq(app.strings(), &FR));
  }

  #[test]
  fn auto_resolves_the_startup_locale_and_cli_keeps_english() {
    let mut app = App {
      system_language: Language::Fr,
      ..Default::default()
    };
    app.refresh_language();
    assert_eq!(app.language(), Language::Fr);
    app.use_english_messages();
    assert_eq!(app.language(), Language::En);
    assert_eq!(app.user_config.behavior.language, Language::Auto);
    app.user_config.behavior.language = Language::PtBr;
    app.refresh_language();
    assert_eq!(app.language(), Language::En);
  }
}
