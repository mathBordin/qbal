use super::{GuiStrings, EN};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const PT_BR: GuiStrings = GuiStrings {
  date_locale: "pt-BR",
  ..EN
};
