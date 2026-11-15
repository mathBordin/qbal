//! Artists and albums beyond the track lists: the artist page (popular tracks
//! and the discography) and the details of the playing album.
//!
//! Plain data and pure functions; `infra::qobuz` fills them, the TUI draws
//! them.

use crate::core::plugin_api::TrackInfo;

/// What a release is, from Qobuz's `release_type`, in the order the
/// discography lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReleaseKind {
  Album,
  EpSingle,
  Other,
}

impl ReleaseKind {
  /// From Qobuz's `release_type`. Without one, a short release counts as an
  /// EP or single.
  pub fn from_qobuz(release_type: Option<&str>, tracks_count: u32) -> Self {
    match release_type {
      Some("album" | "live") => ReleaseKind::Album,
      Some("epSingle") => ReleaseKind::EpSingle,
      Some(_) => ReleaseKind::Other,
      None if tracks_count > 0 && tracks_count <= 6 => ReleaseKind::EpSingle,
      None => ReleaseKind::Album,
    }
  }

  pub const ALL: [ReleaseKind; 3] = [
    ReleaseKind::Album,
    ReleaseKind::EpSingle,
    ReleaseKind::Other,
  ];
}

/// One release in an artist's discography.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
  /// `qobuz:album:<id>`.
  pub uri: String,
  pub title: String,
  /// The edition's subtitle, e.g. `Deluxe Edition`.
  pub version: Option<String>,
  /// `YYYY-MM-DD`.
  pub release_date: Option<String>,
  pub tracks_count: u32,
  pub kind: ReleaseKind,
  /// The best quality on offer: bits and kHz.
  pub bit_depth: Option<u32>,
  pub sample_rate_khz: Option<f64>,
}

impl Release {
  pub fn year(&self) -> Option<&str> {
    self.release_date.as_deref().and_then(|date| date.get(..4))
  }

  /// `24/96`, `16/44.1`.
  pub fn quality(&self) -> Option<String> {
    let bits = self.bit_depth?;
    let khz = self.sample_rate_khz?;
    let khz = if khz.fract() == 0.0 {
      format!("{khz:.0}")
    } else {
      format!("{khz:.1}")
    };
    Some(format!("{bits}/{khz}"))
  }

  /// The title with its edition: `Discovery (Deluxe Edition)`.
  pub fn full_title(&self) -> String {
    match self.version.as_deref().filter(|v| !v.is_empty()) {
      Some(version) => format!("{} ({version})", self.title),
      None => self.title.clone(),
    }
  }

  /// How much a release is worth keeping over another edition of itself:
  /// the higher quality, then the later release.
  fn rank(&self) -> (u32, u64, &str) {
    let bits = self.bit_depth.unwrap_or(0);
    let rate = (self.sample_rate_khz.unwrap_or(0.0) * 10.0) as u64;
    (bits, rate, self.release_date.as_deref().unwrap_or(""))
  }
}

/// The discography in reading order: albums, then EPs and singles, then the
/// rest, each newest first. Editions of one release (same title and year, the
/// CD and the hi-res one) keep only the best.
pub fn arrange_releases(releases: Vec<Release>) -> Vec<Release> {
  let mut kept: Vec<Release> = Vec::new();
  for release in releases {
    let same = kept.iter().position(|k| {
      k.kind == release.kind
        && k.title.trim().eq_ignore_ascii_case(release.title.trim())
        && k.year() == release.year()
        && k.version == release.version
    });
    match same {
      Some(index) if release.rank() > kept[index].rank() => kept[index] = release,
      Some(_) => {}
      None => kept.push(release),
    }
  }
  kept.sort_by(|a, b| {
    a.kind
      .cmp(&b.kind)
      .then_with(|| b.release_date.cmp(&a.release_date))
  });
  kept
}

/// One row of the discography list: a group heading or a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscographyRow {
  Heading(ReleaseKind),
  Release(usize),
}

/// The rows of an arranged discography: each non-empty group under its
/// heading.
pub fn discography_rows(releases: &[Release]) -> Vec<DiscographyRow> {
  let mut rows = Vec::new();
  for kind in ReleaseKind::ALL {
    let mut first = true;
    for (index, release) in releases.iter().enumerate() {
      if release.kind != kind {
        continue;
      }
      if first {
        rows.push(DiscographyRow::Heading(kind));
        first = false;
      }
      rows.push(DiscographyRow::Release(index));
    }
  }
  rows
}

/// The next release row from `from` in direction `step` (`1` or `-1`),
/// skipping headings; stays put at either end.
pub fn step_release_row(rows: &[DiscographyRow], from: usize, step: isize) -> usize {
  let mut index = from as isize;
  loop {
    index += step;
    match rows.get(usize::try_from(index).ok().unwrap_or(usize::MAX)) {
      Some(DiscographyRow::Release(_)) => return index as usize,
      Some(DiscographyRow::Heading(_)) => continue,
      None => return from,
    }
  }
}

/// The first release row, for a fresh cursor.
pub fn first_release_row(rows: &[DiscographyRow]) -> usize {
  rows
    .iter()
    .position(|row| matches!(row, DiscographyRow::Release(_)))
    .unwrap_or(0)
}

/// The tabs of the artist page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArtistTab {
  #[default]
  TopTracks,
  Albums,
}

impl ArtistTab {
  pub fn toggled(self) -> Self {
    match self {
      ArtistTab::TopTracks => ArtistTab::Albums,
      ArtistTab::Albums => ArtistTab::TopTracks,
    }
  }
}

/// An artist page: loading until both halves arrive.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArtistPage {
  /// `qobuz:artist:<id>`.
  pub uri: String,
  pub name: String,
  pub loading: bool,
  /// The artist's most popular tracks, from a catalogue search.
  pub top_tracks: Vec<TrackInfo>,
  /// Arranged by [`arrange_releases`].
  pub releases: Vec<Release>,
}

/// The playing album's details, from `album/get`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AlbumDetails {
  pub album_id: String,
  pub title: String,
  pub release_date: Option<String>,
  pub label: Option<String>,
  pub genre: Option<String>,
  /// The album's tracks in order: id and length in ms.
  pub tracks: Vec<(String, u64)>,
}

impl AlbumDetails {
  pub fn year(&self) -> Option<&str> {
    self.release_date.as_deref().and_then(|date| date.get(..4))
  }

  /// The 1-based position of `track_id` and the album's track count.
  pub fn position_of(&self, track_id: &str) -> Option<(usize, usize)> {
    let index = self.tracks.iter().position(|(id, _)| id == track_id)?;
    Some((index + 1, self.tracks.len()))
  }

  /// What is left of the album from `position_ms` into `track_id`: the rest of
  /// that track plus every track after it.
  pub fn remaining_ms(&self, track_id: &str, position_ms: u64) -> Option<u64> {
    let index = self.tracks.iter().position(|(id, _)| id == track_id)?;
    let current = self.tracks[index].1.saturating_sub(position_ms);
    let after: u64 = self.tracks[index + 1..].iter().map(|(_, ms)| ms).sum();
    Some(current + after)
  }
}

/// Keep the tracks of a search that belong to the artist: by id when the
/// tracks carry ids, else by name. At most `limit`, in search (popularity)
/// order.
pub fn tracks_by_artist(
  tracks: Vec<TrackInfo>,
  artist_id: &str,
  artist_name: &str,
  limit: usize,
) -> Vec<TrackInfo> {
  let by_id = |track: &TrackInfo| {
    track
      .artist_refs
      .iter()
      .any(|a| a.id.as_deref() == Some(artist_id))
  };
  let by_name = |track: &TrackInfo| {
    track
      .artists
      .iter()
      .any(|name| name.trim().eq_ignore_ascii_case(artist_name.trim()))
  };
  let any_ids = tracks
    .iter()
    .any(|t| t.artist_refs.iter().any(|a| a.id.is_some()));
  tracks
    .into_iter()
    .filter(|t| if any_ids { by_id(t) } else { by_name(t) })
    .take(limit)
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::plugin_api::ArtistRef;

  fn release(title: &str, date: &str, kind: ReleaseKind, bits: u32, khz: f64) -> Release {
    Release {
      uri: format!("qobuz:album:{title}{date}{bits}"),
      title: title.into(),
      version: None,
      release_date: Some(date.into()),
      tracks_count: 10,
      kind,
      bit_depth: Some(bits),
      sample_rate_khz: Some(khz),
    }
  }

  #[test]
  fn release_types_map_to_the_three_groups() {
    assert_eq!(
      ReleaseKind::from_qobuz(Some("album"), 12),
      ReleaseKind::Album
    );
    assert_eq!(
      ReleaseKind::from_qobuz(Some("live"), 12),
      ReleaseKind::Album
    );
    assert_eq!(
      ReleaseKind::from_qobuz(Some("epSingle"), 2),
      ReleaseKind::EpSingle
    );
    assert_eq!(
      ReleaseKind::from_qobuz(Some("compilation"), 20),
      ReleaseKind::Other
    );
    assert_eq!(ReleaseKind::from_qobuz(None, 3), ReleaseKind::EpSingle);
    assert_eq!(ReleaseKind::from_qobuz(None, 11), ReleaseKind::Album);
  }

  #[test]
  fn the_discography_groups_by_kind_newest_first_and_keeps_the_best_edition() {
    let arranged = arrange_releases(vec![
      release("Single", "2020-01-01", ReleaseKind::EpSingle, 16, 44.1),
      release("Discovery", "2001-03-12", ReleaseKind::Album, 16, 44.1),
      release(
        "Random Access Memories",
        "2013-05-17",
        ReleaseKind::Album,
        24,
        88.2,
      ),
      release("discovery", "2001-03-12", ReleaseKind::Album, 24, 96.0),
      release("Alive", "2007-11-19", ReleaseKind::Other, 16, 44.1),
    ]);
    let titles: Vec<&str> = arranged.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(
      titles,
      ["Random Access Memories", "discovery", "Single", "Alive"]
    );
    assert_eq!(arranged[1].quality().as_deref(), Some("24/96"));
    assert_eq!(arranged[0].quality().as_deref(), Some("24/88.2"));
  }

  #[test]
  fn discography_rows_put_a_heading_over_each_group_and_the_cursor_skips_them() {
    let arranged = arrange_releases(vec![
      release("A", "2010-01-01", ReleaseKind::Album, 16, 44.1),
      release("B", "2005-01-01", ReleaseKind::Album, 16, 44.1),
      release("S", "2012-01-01", ReleaseKind::EpSingle, 16, 44.1),
    ]);
    let rows = discography_rows(&arranged);
    assert_eq!(
      rows,
      [
        DiscographyRow::Heading(ReleaseKind::Album),
        DiscographyRow::Release(0),
        DiscographyRow::Release(1),
        DiscographyRow::Heading(ReleaseKind::EpSingle),
        DiscographyRow::Release(2),
      ]
    );
    assert_eq!(first_release_row(&rows), 1);
    assert_eq!(step_release_row(&rows, 2, 1), 4, "down skips the heading");
    assert_eq!(step_release_row(&rows, 4, -1), 2, "up skips it too");
    assert_eq!(
      step_release_row(&rows, 1, -1),
      1,
      "the top heading is not reachable"
    );
    assert_eq!(step_release_row(&rows, 4, 1), 4);
  }

  #[test]
  fn album_details_give_the_track_position_and_the_time_left() {
    let details = AlbumDetails {
      tracks: vec![
        ("a".into(), 60_000),
        ("b".into(), 120_000),
        ("c".into(), 30_000),
      ],
      release_date: Some("2001-03-12".into()),
      ..AlbumDetails::default()
    };
    assert_eq!(details.year(), Some("2001"));
    assert_eq!(details.position_of("b"), Some((2, 3)));
    assert_eq!(details.remaining_ms("b", 20_000), Some(130_000));
    assert_eq!(details.remaining_ms("c", 60_000), Some(0));
    assert_eq!(details.position_of("zz"), None);
  }

  fn track(name: &str, artist: &str, id: Option<&str>) -> TrackInfo {
    TrackInfo {
      generated_name: None,
      uri: Some(format!("qobuz:track:{name}")),
      name: name.into(),
      artists: vec![artist.into()],
      album: String::new(),
      duration_ms: 0,
      id: None,
      album_id: None,
      artist_refs: vec![ArtistRef {
        id: id.map(str::to_string),
        name: artist.into(),
      }],
      is_playable: true,
      is_local: false,
      track_number: 0,
      explicit: false,
      image_url: None,
      release_date: None,
    }
  }

  #[test]
  fn popular_tracks_keep_the_artist_s_own_by_id_or_else_by_name() {
    let found = vec![
      track("One More Time", "Daft Punk", Some("36819")),
      track("Cover", "Someone Else", Some("1")),
      track("Get Lucky", "Daft Punk", Some("36819")),
    ];
    let kept = tracks_by_artist(found, "36819", "Daft Punk", 10);
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].name, "One More Time");

    let no_ids = vec![track("A", "daft punk", None), track("B", "Other", None)];
    assert_eq!(tracks_by_artist(no_ids, "36819", "Daft Punk", 10).len(), 1);
  }
}
