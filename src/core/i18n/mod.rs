//! Frontend-neutral, serializable UI messages. Add English fields first; each
//! translated instance inherits untranslated fields from its English instance.
pub mod help;
mod locale;
mod message;
pub mod popups;
pub mod recap;
pub mod screens;
pub mod settings;
pub mod sidebar;
pub mod status_app;
pub mod status_infra;
pub use locale::{system_language, Language};

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Strings {
  pub recap: recap::RecapStrings,
  pub popups: popups::PopupStrings,
  pub status: StatusStrings,
  pub screens: screens::ScreenStrings,
  pub settings: settings::SettingsStrings,
  pub sidebar: sidebar::SidebarStrings,
  pub help: help::HelpStrings,
}

pub static EN: Strings = Strings {
  recap: recap::EN,
  popups: popups::EN,
  status: StatusStrings {
    app: status_app::EN,
    infra: status_infra::EN,
  },
  screens: screens::EN,
  settings: settings::EN,
  sidebar: sidebar::EN,
  help: help::EN,
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub static PT_BR: Strings = Strings {
  recap: recap::PT_BR,
  popups: popups::PT_BR,
  status: StatusStrings {
    app: status_app::PT_BR,
    infra: status_infra::PT_BR,
  },
  screens: screens::PT_BR,
  settings: settings::PT_BR,
  sidebar: sidebar::PT_BR,
  help: help::PT_BR,
  ..EN
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub static FR: Strings = Strings {
  recap: recap::FR,
  popups: popups::FR,
  status: StatusStrings {
    app: status_app::FR,
    infra: status_infra::FR,
  },
  screens: screens::FR,
  settings: settings::FR,
  sidebar: sidebar::FR,
  help: help::FR,
  ..EN
};

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct StatusStrings {
  pub app: status_app::AppStatusStrings,
  pub infra: status_infra::InfraStatusStrings,
}

impl Language {
  pub const fn strings(self) -> &'static Strings {
    match self {
      Self::PtBr => &PT_BR,
      Self::Fr => &FR,
      Self::Auto | Self::En => &EN,
    }
  }
}

/// Replace named placeholders once. Values are never reparsed, so a track
/// called "{count}" stays literal. Unknown placeholders remain visible, making
/// a contributor's typo inspectable; table tests validate placeholder parity.
pub fn format_template(template: &str, values: &[(&str, &str)]) -> String {
  let mut result = String::with_capacity(template.len());
  let mut tail = template;
  while let Some(start) = tail.find('{') {
    result.push_str(&tail[..start]);
    let Some(end) = tail[start + 1..].find('}').map(|end| start + 1 + end) else {
      result.push_str(&tail[start..]);
      return result;
    };
    let key = &tail[start + 1..end];
    match values.iter().find(|(name, _)| *name == key) {
      Some((_, value)) => result.push_str(value),
      None => result.push_str(&tail[start..=end]),
    }
    tail = &tail[end + 1..];
  }
  result.push_str(tail);
  result
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod template_tests;
