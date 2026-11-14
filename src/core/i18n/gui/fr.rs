use super::{GuiStrings, EN};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const FR: GuiStrings = GuiStrings {
  date_locale: "fr-FR",
  ..EN
};
