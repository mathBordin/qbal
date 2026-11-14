//! The first-launch interaction surface.
//!
//! Core and infra never talk to the console: anything interactive at start-up
//! goes through this trait so the frontend decides how to present it. Today
//! that is only telling the frontend which language was resolved.

use crate::core::i18n::Language;

pub trait Onboarding: Send + Sync {
  /// Select the resolved language after configuration has been loaded.
  /// Frontends with their own language state may keep that selection instead.
  fn set_language(&self, _language: Language) {}
}

#[cfg(test)]
mod tests {}
