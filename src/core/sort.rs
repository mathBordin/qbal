//! Sorting types and utilities for the track table.

use serde::{Deserialize, Serialize};

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
  /// By artist name (for tracks)
  Artist,
  /// By track/album duration
  Duration,
  /// By album name (for tracks)
  Album,
  /// By release date (for albums)
  ReleaseDate,
}

impl SortField {
  /// Get the keyboard shortcut for this field
  pub fn shortcut(&self) -> Option<char> {
    match self {
      SortField::Default => Some('d'),
      SortField::Name => Some('n'),
      SortField::Artist => Some('r'),
      SortField::Duration => Some('t'),
      SortField::Album => Some('l'),
      SortField::ReleaseDate => Some('y'),
    }
  }

  /// Lowercase config-file token, e.g. `"name"`, `"release_date"`.
  pub fn to_config_str(self) -> &'static str {
    match self {
      SortField::Default => "default",
      SortField::Name => "name",
      SortField::Artist => "artist",
      SortField::Duration => "duration",
      SortField::Album => "album",
      SortField::ReleaseDate => "release_date",
    }
  }

  /// Parse a config-file token back to a `SortField`. Returns `None` for
  /// unknown strings (callers surface the context's valid fields in the error).
  pub fn from_config_str(s: &str) -> Option<Self> {
    match s.trim() {
      "default" => Some(SortField::Default),
      "name" => Some(SortField::Name),
      "artist" => Some(SortField::Artist),
      "duration" => Some(SortField::Duration),
      "album" => Some(SortField::Album),
      "release_date" => Some(SortField::ReleaseDate),
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
  /// Tracks in a playlist
  PlaylistTracks,
}

impl SortContext {
  /// Get the available sort fields for this context
  pub fn available_fields(&self) -> &'static [SortField] {
    match self {
      SortContext::PlaylistTracks => &[
        SortField::Default,
        SortField::Name,
        SortField::Artist,
        SortField::Album,
        SortField::Duration,
        SortField::ReleaseDate,
      ],
    }
  }
}

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

/// Sort track rows by `state`. `Default` leaves them in the order they
/// arrived; rows without the field (no release date) sort first.
pub fn sort_tracks(tracks: &mut [crate::core::plugin_api::TrackInfo], state: SortState) {
  let order = state.order;
  match state.field {
    SortField::Default => {}
    SortField::Name => sort_by_key_with_order(tracks, order, |t| t.name.to_lowercase()),
    SortField::Artist => sort_by_key_with_order(tracks, order, |t| {
      t.artists.first().map(|artist| artist.to_lowercase())
    }),
    SortField::Album => sort_by_key_with_order(tracks, order, |t| t.album.to_lowercase()),
    SortField::Duration => sort_by_key_with_order(tracks, order, |t| t.duration_ms),
    SortField::ReleaseDate => sort_by_key_with_order(tracks, order, |t| t.release_date.clone()),
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

  fn track(name: &str, artist: &str, duration_ms: u64) -> crate::core::plugin_api::TrackInfo {
    crate::core::plugin_api::TrackInfo {
      generated_name: None,
      uri: None,
      name: name.to_string(),
      artists: vec![artist.to_string()],
      album: String::new(),
      duration_ms,
      id: None,
      album_id: None,
      artist_refs: Vec::new(),
      is_playable: true,
      is_local: false,
      track_number: 0,
      explicit: false,
      image_url: None,
      release_date: None,
    }
  }

  #[test]
  fn tracks_sort_by_the_chosen_field_and_direction() {
    let mut tracks = vec![
      track("b", "Zed", 300),
      track("A", "yan", 100),
      track("c", "Xi", 200),
    ];
    let names = |tracks: &[crate::core::plugin_api::TrackInfo]| {
      tracks.iter().map(|t| t.name.clone()).collect::<Vec<_>>()
    };
    sort_tracks(
      &mut tracks,
      SortState {
        field: SortField::Name,
        order: SortOrder::Ascending,
      },
    );
    assert_eq!(names(&tracks), ["A", "b", "c"]);
    sort_tracks(
      &mut tracks,
      SortState {
        field: SortField::Duration,
        order: SortOrder::Descending,
      },
    );
    assert_eq!(names(&tracks), ["b", "c", "A"]);
    sort_tracks(
      &mut tracks,
      SortState {
        field: SortField::Artist,
        order: SortOrder::Ascending,
      },
    );
    assert_eq!(names(&tracks), ["c", "A", "b"]);
    sort_tracks(&mut tracks, SortState::default());
    assert_eq!(names(&tracks), ["c", "A", "b"]);
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
  }
}
