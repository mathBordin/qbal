//! The first-launch interaction surface.
//!
//! Core and infra never talk to the console: every interactive first-run flow
//! (the source picker, the Spotify pairing code, source credential prompts)
//! goes through this trait so the
//! frontend decides how to present it. The terminal frontend's
//! `ConsoleOnboarding` (`tui/onboarding.rs`) reproduces the historical stdout
//! byte for byte; a windowed frontend can implement the same surface with
//! dialogs instead.
//!
//! The retry loops, validation, and persistence stay with the callers - this
//! trait is only the presentation boundary.

use crate::core::i18n::Language;

pub trait Onboarding: Send + Sync {
  /// Select the resolved language after configuration has been loaded.
  /// Frontends with their own language state may keep that selection instead.
  fn set_language(&self, _language: Language) {}
}

#[cfg(test)]
mod tests {}
