//! Console implementation of the first-launch surface.
//!
//! Reproduces the pre-trait stdout byte for byte: `info` is a `println!`,
//! `progress` a flushed `print!`, `prompt_line` shows the prompt verbatim and
//! reads one raw line, and `pick_sources` is the terminal source picker in
//! `tui/first_run.rs`.

use crate::core::i18n::{Language, Strings, EN};
use crate::core::onboarding::Onboarding;
use std::sync::Mutex;

pub struct ConsoleOnboarding {
  strings: Mutex<&'static Strings>,
}

impl Default for ConsoleOnboarding {
  fn default() -> Self {
    Self {
      strings: Mutex::new(&EN),
    }
  }
}

impl Onboarding for ConsoleOnboarding {
  fn set_language(&self, language: Language) {
    *self
      .strings
      .lock()
      .unwrap_or_else(|error| error.into_inner()) = language.strings();
  }
}

#[cfg(test)]
mod tests {}
