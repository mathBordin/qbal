//! Sorting types and utilities for spotatui contexts
//!
//! Provides sorting functionality for playlists, albums, artists, etc.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Fields that can be used for sorting
///
/// The serde derives on this enum, `SortOrder` and `SortContext` are the
/// action-vocabulary wire shape: `Action::Sort` carries them directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortField {
  /// Original API order (no sorting applied)
  #[default]
  Default,
  /// Alphabetical by name/title
  Name,
  /// By date added (for playlists, saved albums)
  DateAdded,
  /// By artist name (for tracks)
  Artist,
  /// By track/album duration
  Duration,
  /// By album name (for tracks)
  Album,
  /// By release date (for albums)
  ReleaseDate,
  /// Latest local listen in a playlist context
  RecentlyPlayed,
}

impl SortField {
  /// Get the keyboard shortcut for this field
  pub fn shortcut(&self) -> Option<char> {
    match self {
      SortField::Default => Some('d'),
      SortField::Name => Some('n'),
      SortField::DateAdded => Some('a'),
      SortField::Artist => Some('r'),
      SortField::Duration => Some('t'),
      SortField::Album => Some('l'),
      SortField::ReleaseDate => Some('y'),
      SortField::RecentlyPlayed => Some('r'),
    }
  }

  /// Lowercase config-file token, e.g. `"name"`, `"date_added"`.
  pub fn to_config_str(self) -> &'static str {
    match self {
      SortField::Default => "default",
      SortField::Name => "name",
      SortField::DateAdded => "date_added",
      SortField::Artist => "artist",
      SortField::Duration => "duration",
      SortField::Album => "album",
      SortField::ReleaseDate => "release_date",
      SortField::RecentlyPlayed => "recently_played",
    }
  }

  /// Parse a config-file token back to a `SortField`. Returns `None` for
  /// unknown strings (callers surface the context's valid fields in the error).
  pub fn from_config_str(s: &str) -> Option<Self> {
    match s.trim() {
      "default" => Some(SortField::Default),
      "name" => Some(SortField::Name),
      "date_added" => Some(SortField::DateAdded),
      "artist" => Some(SortField::Artist),
      "duration" => Some(SortField::Duration),
      "album" => Some(SortField::Album),
      "release_date" => Some(SortField::ReleaseDate),
      "recently_played" => Some(SortField::RecentlyPlayed),
      _ => None,
    }
  }
}

/// Sort order direction
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortOrder {
  #[default]
  Ascending,
  Descending,
}

impl SortOrder {
  /// Toggle between ascending and descending
  pub fn toggle(&self) -> Self {
    match self {
      SortOrder::Ascending => SortOrder::Descending,
      SortOrder::Descending => SortOrder::Ascending,
    }
  }

  /// Get the sort indicator using caller-supplied icons (from config).
  pub fn indicator_icon<'a>(&self, ascending: &'a str, descending: &'a str) -> &'a str {
    match self {
      SortOrder::Ascending => ascending,
      SortOrder::Descending => descending,
    }
  }

  /// Parse a `:desc` direction suffix. Only `desc` flips to descending;
  /// everything else (including `asc` or empty) is ascending.
  pub fn parse_suffix(s: &str) -> Self {
    match s.trim() {
      "desc" => SortOrder::Descending,
      _ => SortOrder::Ascending,
    }
  }
}

/// Context that supports sorting
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SortContext {
  /// Spotify playlists in the library sidebar
  SidebarPlaylists,
  /// Tracks in a playlist
  PlaylistTracks,
  /// User's saved albums
  SavedAlbums,
  /// User's followed artists
  SavedArtists,
  /// Recently played tracks
  RecentlyPlayed,
}

impl SortContext {
  /// Get the available sort fields for this context
  pub fn available_fields(&self) -> &'static [SortField] {
    match self {
      SortContext::SidebarPlaylists => &[
        SortField::Default,
        SortField::RecentlyPlayed,
        SortField::Name,
      ],
      SortContext::PlaylistTracks => &[
        SortField::Default,
        SortField::Name,
        SortField::DateAdded,
        SortField::Artist,
        SortField::Album,
        SortField::Duration,
        SortField::ReleaseDate,
      ],
      SortContext::SavedAlbums => &[
        SortField::Default,
        SortField::Name,
        SortField::DateAdded,
        SortField::Artist,
        SortField::ReleaseDate,
      ],
      SortContext::SavedArtists => &[SortField::Default, SortField::Name],
      SortContext::RecentlyPlayed => &[
        SortField::Default,
        SortField::Name,
        SortField::Artist,
        SortField::Album,
      ],
    }
  }
}

/// The three orders for Spotify's playlists sidebar. Folder slots stay fixed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarPlaylistOrder {
  #[default]
  Spotify,
  RecentlyPlayed,
  Name,
}

impl SidebarPlaylistOrder {
  pub fn field(self) -> SortField {
    match self {
      Self::Spotify => SortField::Default,
      Self::RecentlyPlayed => SortField::RecentlyPlayed,
      Self::Name => SortField::Name,
    }
  }

  pub fn from_field(field: SortField) -> Option<Self> {
    match field {
      SortField::Default => Some(Self::Spotify),
      SortField::RecentlyPlayed => Some(Self::RecentlyPlayed),
      SortField::Name => Some(Self::Name),
      _ => None,
    }
  }
}

/// Latest local listen by playlist context URI; read once off the UI thread and
/// maintained as the history collector appends records.
pub type PlaylistLastListens = BTreeMap<String, DateTime<Utc>>;

/// Current sort state
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SortState {
  pub field: SortField,
  pub order: SortOrder,
}

impl SortState {
  pub fn new() -> Self {
    Self::default()
  }

  /// Apply a new sort field, toggling order if same field selected
  pub fn apply_field(&mut self, field: SortField) {
    if self.field == field {
      self.order = self.order.toggle();
    } else {
      self.field = field;
      self.order = SortOrder::Ascending;
    }
  }

  /// Parse a `"<field>"` / `"<field>:desc"` spec, validating that the field is
  /// available in `ctx`. Hard error (with the context's valid fields) if the
  /// field is unknown or unavailable in this context.
  pub fn parse(spec: &str, ctx: SortContext) -> Result<Self, String> {
    let (field_str, order_str) = match spec.split_once(':') {
      Some((f, o)) => (f, o),
      None => (spec, ""),
    };
    let field = SortField::from_config_str(field_str).ok_or_else(|| {
      format!(
        "unknown sort field '{}' (valid for this context: {})",
        field_str.trim(),
        ctx
          .available_fields()
          .iter()
          .map(|f| f.to_config_str())
          .collect::<Vec<_>>()
          .join(", ")
      )
    })?;
    if !ctx.available_fields().contains(&field) {
      return Err(format!(
        "sort field '{}' is not available in this context (valid: {})",
        field.to_config_str(),
        ctx
          .available_fields()
          .iter()
          .map(|f| f.to_config_str())
          .collect::<Vec<_>>()
          .join(", ")
      ));
    }
    Ok(Self {
      field,
      order: SortOrder::parse_suffix(order_str),
    })
  }
}

/// Sort by a precomputed key — one key per item (O(n) allocations) instead of
/// one per comparison (O(n log n)) — honoring the sort direction. Stable in
/// both directions: equal keys keep their prior relative order.
pub fn sort_by_key_with_order<T, K: Ord, F: FnMut(&T) -> K>(
  items: &mut [T],
  order: SortOrder,
  mut key: F,
) {
  match order {
    SortOrder::Ascending => items.sort_by_cached_key(|item| key(item)),
    SortOrder::Descending => items.sort_by_cached_key(|item| std::cmp::Reverse(key(item))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_sort_state_apply_field() {
    let mut state = SortState::new();
    assert_eq!(state.field, SortField::Default);
    assert_eq!(state.order, SortOrder::Ascending);

    // Apply new field
    state.apply_field(SortField::Name);
    assert_eq!(state.field, SortField::Name);
    assert_eq!(state.order, SortOrder::Ascending);

    // Apply same field toggles order
    state.apply_field(SortField::Name);
    assert_eq!(state.field, SortField::Name);
    assert_eq!(state.order, SortOrder::Descending);

    // Apply different field resets order
    state.apply_field(SortField::Artist);
    assert_eq!(state.field, SortField::Artist);
    assert_eq!(state.order, SortOrder::Ascending);
  }

  #[test]
  fn test_sort_order_toggle() {
    assert_eq!(SortOrder::Ascending.toggle(), SortOrder::Descending);
    assert_eq!(SortOrder::Descending.toggle(), SortOrder::Ascending);
  }

  #[test]
  fn test_context_available_fields() {
    let fields = SortContext::PlaylistTracks.available_fields();
    assert!(fields.contains(&SortField::Name));
    assert!(fields.contains(&SortField::Artist));

    let fields = SortContext::SavedArtists.available_fields();
    assert!(fields.contains(&SortField::Name));
    assert!(!fields.contains(&SortField::Artist));
  }
}
