//! Differential tests for `App::apply`: one test per arm (or small arm
//! family), pinning the exact observable behavior — dispatched `IoEvent`s
//! and `App` state — that the scripting effect drain had before the port.
//!
//! House pattern: `app_with_channel` keeps the `IoEvent` receiver alive;
//! `IoEvent` derives nothing, so assertions pattern-match. Catch-all match
//! arms are spelled `_other` because this directory's gate counts bare
//! wildcard arms.

use super::NavTarget;
use crate::core::theme::ThemeField;

// --- transport ---

// --- transport without a Spotify session ---

// --- jump-to navigation ---

// --- recap ---

// --- playback starts ---

// --- library / playlists ---

// --- status messages ---

// --- navigation ---

// --- plugin surfaces ---

#[test]
fn theme_field_names_round_trip() {
  for field in ThemeField::ALL {
    assert_eq!(ThemeField::from_name(field.name()), Some(field));
  }
  assert_eq!(ThemeField::from_name("not_a_field"), None);
}

#[test]
fn nav_target_names_round_trip() {
  for target in NavTarget::ALL {
    assert_eq!(NavTarget::from_name(target.name()), Some(target));
  }
  assert_eq!(NavTarget::from_name("nowhere"), None);
}

// --- DJ ---

// --- pagination, open family, source-routed search, library rows, dialogs ---

// --- LoadMore ---

// --- Open family ---

// --- SearchActiveSource (Action::Search stays Spotify-only) ---

// --- playback starts ---

// --- dialogs ---

// --- playlist sync ---

// --- library sections ---

// --- SelectSource ---

// --- the now-playing item family ---

// --- the listening party family ---

// --- decoded-source playlists, sidebar folders and saved stations ---

// --- podcasts, episodes, the native queue and the Discover page ---

// --- the create-playlist form and the friends social graph ---

// --- stats, settings, sort and the native queue ---

// --- the DJ screen's own actions (no-ops without ai-dj) ---
