//! Locale selection is separate from reading the process/OS environment.
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Language {
  #[default]
  #[serde(rename = "auto")]
  Auto,
  #[serde(rename = "en")]
  En,
  #[serde(rename = "pt-BR")]
  PtBr,
  #[serde(rename = "fr")]
  Fr,
}

impl Language {
  pub const OPTIONS: &'static [&'static str] = &["auto", "en", "pt-BR", "fr"];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::En => "en",
      Self::PtBr => "pt-BR",
      Self::Fr => "fr",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    let normalized = normalized_locale(value);
    match normalized.split('-').next().unwrap_or("") {
      "auto" if normalized == "auto" => Some(Self::Auto),
      "en" => Some(Self::En),
      "pt" => Some(Self::PtBr),
      "fr" => Some(Self::Fr),
      _ => None,
    }
  }

  pub const fn resolve(self, system: Self) -> Self {
    match self {
      Self::Auto => match system {
        Self::PtBr => Self::PtBr,
        Self::Fr => Self::Fr,
        Self::Auto | Self::En => Self::En,
      },
      explicit => explicit,
    }
  }
}

impl<'de> Deserialize<'de> for Language {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
      Text(String),
      Other(serde::de::IgnoredAny),
    }
    if let Value::Text(value) = Value::deserialize(deserializer)? {
      if let Some(language) = Self::parse(&value) {
        return Ok(language);
      }
    }
    log::warn!("Invalid behavior.language; using auto (expected auto, en, pt-BR or fr)");
    Ok(Self::Auto)
  }
}

fn normalized_locale(value: &str) -> String {
  value
    .trim()
    .split(['.', '@', ':'])
    .next()
    .unwrap_or("")
    .replace('_', "-")
    .to_ascii_lowercase()
}

/// Interpret one locale, including POSIX encoding/modifier suffixes and BCP 47
/// region tags. Any Portuguese locale (pt, pt-BR, pt-PT) selects Brazilian
/// Portuguese, any French locale selects French; every other language uses English.
pub fn language_with(value: &str) -> Language {
  let normalized = normalized_locale(value);
  match normalized.split('-').next() {
    Some("pt") => Language::PtBr,
    Some("fr") => Language::Fr,
    _ => Language::En,
  }
}

/// POSIX message-locale overrides precede the native UI-language fallback.
/// Empty variables are ignored. C/POSIX are deliberate English selections.
fn system_language_with(values: [Option<&str>; 3], native: Option<&str>) -> Language {
  values
    .into_iter()
    .flatten()
    .find(|value| !value.trim().is_empty())
    .or(native)
    .map(language_with)
    .unwrap_or(Language::En)
}

pub fn system_language() -> Language {
  static LANGUAGE: std::sync::OnceLock<Language> = std::sync::OnceLock::new();
  *LANGUAGE.get_or_init(read_system_language)
}

fn read_system_language() -> Language {
  let values = ["LC_ALL", "LC_MESSAGES", "LANG"].map(|key| std::env::var(key).ok());
  system_language_with(values.each_ref().map(|value| value.as_deref()), None)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn portuguese_and_french_locales_select_their_tables() {
    for locale in ["pt_BR.UTF-8", "pt-BR", "pt", "pt_PT", "PT-br"] {
      assert_eq!(language_with(locale), Language::PtBr, "{locale}");
    }
    for locale in ["fr_FR.UTF-8", "fr", "fr-CA", "fr_BE@euro"] {
      assert_eq!(language_with(locale), Language::Fr, "{locale}");
    }
    for locale in ["en_US.UTF-8", "C", "POSIX", "de_DE", "zh-CN", ""] {
      assert_eq!(language_with(locale), Language::En, "{locale}");
    }
  }

  #[test]
  fn explicit_settings_parse_and_round_trip() {
    for option in Language::OPTIONS {
      let language = Language::parse(option).unwrap();
      assert_eq!(language.as_str(), *option);
    }
    assert_eq!(Language::parse("pt_BR"), Some(Language::PtBr));
    assert_eq!(Language::parse("FR"), Some(Language::Fr));
    assert_eq!(Language::parse("zh-Hans"), None);
  }

  #[test]
  fn auto_follows_the_system_language() {
    assert_eq!(Language::Auto.resolve(Language::PtBr), Language::PtBr);
    assert_eq!(Language::Auto.resolve(Language::Fr), Language::Fr);
    assert_eq!(Language::Auto.resolve(Language::En), Language::En);
    assert_eq!(Language::Fr.resolve(Language::PtBr), Language::Fr);
  }

  #[test]
  fn env_overrides_win_and_blank_values_are_skipped() {
    assert_eq!(
      system_language_with([None, Some(" "), Some("fr_FR.UTF-8")], Some("pt-BR")),
      Language::Fr
    );
    assert_eq!(
      system_language_with([None, None, None], Some("pt-BR")),
      Language::PtBr
    );
    assert_eq!(system_language_with([None, None, None], None), Language::En);
  }
}
