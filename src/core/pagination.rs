//! Pagination containers for the paginated state on
//! [`crate::core::app::App`]: offset-based ([`Paged`]) and cursor-based
//! ([`CursorPaged`]) pages of domain items ([`crate::core::plugin_api`]).

// Wired up by the per-screen migration slices, not yet by the binary.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// An offset-based page of domain items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paged<T> {
  pub items: Vec<T>,
  pub offset: u32,
  pub limit: u32,
  pub total: u32,
  pub next: Option<String>,
  pub previous: Option<String>,
}

impl<T> Default for Paged<T> {
  fn default() -> Self {
    Paged {
      items: Vec::new(),
      offset: 0,
      limit: 0,
      total: 0,
      next: None,
      previous: None,
    }
  }
}

impl<T> Paged<T> {
  /// Whether a following page exists.
  pub fn has_next(&self) -> bool {
    self.next.is_some()
  }
}

/// A cursor-based page of domain items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPaged<T> {
  pub items: Vec<T>,
  pub limit: u32,
  pub next: Option<String>,
  /// `cursors.after` from the API — the cursor to request the following page.
  pub cursor_after: Option<String>,
  /// Absent when the source does not report a total.
  pub total: Option<u32>,
}

impl<T> Default for CursorPaged<T> {
  fn default() -> Self {
    CursorPaged {
      items: Vec::new(),
      limit: 0,
      next: None,
      cursor_after: None,
      total: None,
    }
  }
}

impl<T> CursorPaged<T> {
  pub fn has_next(&self) -> bool {
    self.next.is_some()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn paged_default_is_empty_with_no_next() {
    let p: Paged<String> = Paged::default();
    assert!(p.items.is_empty());
    assert!(!p.has_next());
    assert_eq!(p.total, 0);
  }

  #[test]
  fn cursor_paged_tracks_after_cursor() {
    let p = CursorPaged {
      items: vec![1u32, 2, 3],
      limit: 3,
      next: Some("https://api/next".to_string()),
      cursor_after: Some("abc".to_string()),
      total: Some(50),
    };
    assert!(p.has_next());
    assert_eq!(p.cursor_after.as_deref(), Some("abc"));
    assert_eq!(p.items.len(), 3);
  }
}
