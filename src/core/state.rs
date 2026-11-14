//! Persisted runtime state and app-managed mutable data.
//!
//! This is the machine-owned counterpart to `config.yml`. It holds values that
//! change as the app runs, or data the app manages directly, so ordinary config
//! edits do not churn every time volume, source, pane sizes, or announcements
//! change.

use crate::core::{
  limits::MAX_PLAYBAR_ROWS,
  pane::{Pane, PaneNode},
  source::Source,
};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "state.yml";

/// The Qobuz web-player constants, cached by bundle version (public values only).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QobuzBundleCache {
  pub bundle_version: String,
  pub app_id: String,
  pub app_secret: String,
  pub oauth_key: String,
}

impl QobuzBundleCache {
  fn is_complete(&self) -> bool {
    !self.bundle_version.is_empty()
      && !self.app_id.is_empty()
      && !self.app_secret.is_empty()
      && !self.oauth_key.is_empty()
  }
}

/// Live runtime/app-managed state.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeState {
  pub volume_percent: u8,
  pub shuffle_enabled: bool,
  pub active_source: Source,
  pub seen_announcement_ids: Vec<String>,
  pub dismissed_announcements: Vec<String>,
  pub sidebar_width_percent: u8,
  pub playbar_height_rows: u16,
  pub library_height_percent: u8,
  pub pane_layout: PaneNode,
}

impl Default for RuntimeState {
  fn default() -> Self {
    Self {
      volume_percent: 100,
      shuffle_enabled: false,
      active_source: Source::default(),
      seen_announcement_ids: Vec::new(),
      dismissed_announcements: Vec::new(),
      sidebar_width_percent: 20,
      playbar_height_rows: 6,
      library_height_percent: 30,
      pane_layout: PaneNode::default(),
    }
  }
}

impl RuntimeState {
  pub fn apply_persisted(&mut self, state: &PersistedRuntimeState) {
    if let Some(volume_percent) = state.volume_percent {
      self.volume_percent = volume_percent.min(100);
    }
    if let Some(shuffle_enabled) = state.shuffle_enabled {
      self.shuffle_enabled = shuffle_enabled;
    }
    if let Some(active_source) = state.active_source {
      self.active_source = active_source;
    }
    if let Some(seen_announcement_ids) = &state.seen_announcement_ids {
      self.seen_announcement_ids = sanitized_ids(seen_announcement_ids);
    }
    if let Some(dismissed_announcements) = &state.dismissed_announcements {
      self.dismissed_announcements = sanitized_ids(dismissed_announcements);
    }
    if let Some(sidebar_width_percent) = state.sidebar_width_percent {
      self.sidebar_width_percent = sidebar_width_percent.min(100);
    }
    if let Some(playbar_height_rows) = state.playbar_height_rows {
      self.playbar_height_rows = playbar_height_rows.min(MAX_PLAYBAR_ROWS);
    }
    if let Some(library_height_percent) = state.library_height_percent {
      self.library_height_percent = library_height_percent.min(100);
    }
    if let Some(mut pane_layout) = state.pane_layout.as_ref().and_then(PaneNode::sanitized) {
      pane_layout.close(Pane::NowPlaying);
      self.pane_layout = pane_layout;
    }
  }

  pub fn to_persisted(&self) -> PersistedRuntimeState {
    PersistedRuntimeState {
      volume_percent: Some(self.volume_percent.min(100)),
      shuffle_enabled: Some(self.shuffle_enabled),
      active_source: Some(self.active_source),
      seen_announcement_ids: Some(sanitized_ids(&self.seen_announcement_ids)),
      dismissed_announcements: Some(sanitized_ids(&self.dismissed_announcements)),
      sidebar_width_percent: Some(self.sidebar_width_percent.min(100)),
      playbar_height_rows: Some(self.playbar_height_rows.min(MAX_PLAYBAR_ROWS)),
      library_height_percent: Some(self.library_height_percent.min(100)),
      pane_layout: self.pane_layout.sanitized(),
      qobuz_bundle_cache: None,
    }
  }

  pub fn mark_announcement_seen(&mut self, announcement_id: impl Into<String>) {
    let id = announcement_id.into();
    if id.is_empty() {
      return;
    }

    if !self.seen_announcement_ids.iter().any(|seen| seen == &id) {
      self.seen_announcement_ids.push(id);
    }
  }
}

/// Optional state fields exactly as stored in `state.yml`.
///
/// Fields are optional so sparse or older `state.yml` files can overlay only the
/// values they contain onto runtime defaults.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedRuntimeState {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub volume_percent: Option<u8>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub shuffle_enabled: Option<bool>,
  #[serde(
    skip_serializing_if = "Option::is_none",
    serialize_with = "source_config::serialize_option",
    deserialize_with = "source_config::deserialize_option"
  )]
  pub active_source: Option<Source>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub seen_announcement_ids: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dismissed_announcements: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sidebar_width_percent: Option<u8>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub playbar_height_rows: Option<u16>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub library_height_percent: Option<u8>,
  /// A layout this build cannot read (for example a pane from a newer release) loads as `None`.
  #[serde(
    skip_serializing_if = "Option::is_none",
    deserialize_with = "deserialize_pane_layout"
  )]
  pub pane_layout: Option<PaneNode>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub qobuz_bundle_cache: Option<QobuzBundleCache>,
}

impl PersistedRuntimeState {
  pub fn volume_percent(volume_percent: u8) -> Self {
    Self {
      volume_percent: Some(volume_percent.min(100)),
      ..Default::default()
    }
  }

  pub fn announcements(
    seen_announcement_ids: &[String],
    dismissed_announcements: &[String],
  ) -> Self {
    Self {
      seen_announcement_ids: Some(sanitized_ids(seen_announcement_ids)),
      dismissed_announcements: Some(sanitized_ids(dismissed_announcements)),
      ..Default::default()
    }
  }

  pub fn sidebar_width_percent(sidebar_width_percent: u8) -> Self {
    Self {
      sidebar_width_percent: Some(sidebar_width_percent.min(100)),
      ..Default::default()
    }
  }

  pub fn playbar_height_rows(playbar_height_rows: u16) -> Self {
    Self {
      playbar_height_rows: Some(playbar_height_rows.min(MAX_PLAYBAR_ROWS)),
      ..Default::default()
    }
  }

  pub fn library_height_percent(library_height_percent: u8) -> Self {
    Self {
      library_height_percent: Some(library_height_percent.min(100)),
      ..Default::default()
    }
  }

  pub fn pane_layout(pane_layout: &PaneNode) -> Self {
    Self {
      pane_layout: pane_layout.sanitized(),
      ..Default::default()
    }
  }

  pub fn qobuz_bundle_cache(qobuz_bundle_cache: QobuzBundleCache) -> Self {
    Self {
      qobuz_bundle_cache: Some(qobuz_bundle_cache),
      ..Default::default()
    }
  }

  pub fn merge_patch(&mut self, patch: &Self) {
    merge_state_patch(self, patch);
  }

  pub fn is_empty(&self) -> bool {
    self.volume_percent.is_none()
      && self.shuffle_enabled.is_none()
      && self.active_source.is_none()
      && self.seen_announcement_ids.is_none()
      && self.dismissed_announcements.is_none()
      && self.sidebar_width_percent.is_none()
      && self.playbar_height_rows.is_none()
      && self.library_height_percent.is_none()
      && self.pane_layout.is_none()
      && self.qobuz_bundle_cache.is_none()
  }
}

/// Location of the app runtime-state file: `<state dir>/state.yml`.
pub fn default_state_path() -> Result<PathBuf> {
  crate::core::paths::app_state_dir()
    .map(|dir| dir.join(FILE_NAME))
    .ok_or_else(|| anyhow!("cannot resolve the qbal state directory"))
}

/// Outcome of reading `state.yml` off disk, before deciding how to react to a
/// bad file. Loading (startup) surfaces a malformed file as an error, while
/// saving heals it; both share this classification so the read logic lives in
/// one place.
enum StateRead {
  /// The file is absent or blank; treat as empty state.
  Empty,
  /// The file parsed cleanly. Boxed: it dwarfs the other variants
  /// (`clippy::large_enum_variant`), and a state read is not a hot loop.
  Parsed(Box<PersistedRuntimeState>),
  /// The file was readable but its contents did not parse.
  Malformed(anyhow::Error),
  /// The file could not be read (permissions, other I/O error).
  Unreadable(anyhow::Error),
}

fn read_state(path: &Path) -> StateRead {
  // Read raw bytes so non-UTF-8 contents count as a corrupt file we can heal on
  // save, not an I/O error to propagate. `read_to_string` would fold invalid
  // UTF-8 (e.g. a write truncated mid-multibyte-character) into `ErrorKind`,
  // leaving it wedged forever.
  let bytes = match std::fs::read(path) {
    Ok(bytes) => bytes,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return StateRead::Empty,
    Err(error) => {
      return StateRead::Unreadable(
        anyhow::Error::new(error).context(format!("reading {}", path.display())),
      )
    }
  };

  let contents = match std::str::from_utf8(&bytes) {
    Ok(contents) => contents,
    Err(error) => {
      return StateRead::Malformed(
        anyhow::Error::new(error)
          .context(format!("malformed runtime state file: {}", path.display())),
      )
    }
  };

  if contents.trim().is_empty() {
    return StateRead::Empty;
  }

  match serde_yaml::from_str::<PersistedRuntimeState>(contents) {
    Ok(state) => StateRead::Parsed(Box::new(sanitized_persisted_state(&state))),
    Err(error) => StateRead::Malformed(
      anyhow::Error::new(error)
        .context(format!("malformed runtime state file: {}", path.display())),
    ),
  }
}

/// Load state. A missing file means empty state; malformed state is returned as
/// an error so callers can log and continue with config/default values.
pub fn load(path: &Path) -> Result<PersistedRuntimeState> {
  match read_state(path) {
    StateRead::Empty => Ok(PersistedRuntimeState::default()),
    StateRead::Parsed(state) => Ok(*state),
    StateRead::Malformed(error) | StateRead::Unreadable(error) => Err(error),
  }
}

/// Load existing state for a read-modify-write save, healing a corrupt file.
///
/// `save` re-reads `state.yml` before merging, so a malformed file would
/// otherwise wedge every future save forever and surface a UI error on each
/// runtime change (volume nudge, layout tweak). Mirror the `config.yml`
/// fallback: move the unreadable file aside to `state.yml.bak` (best effort),
/// warn, and continue from empty state. A genuine read failure is still
/// propagated so a transient error never discards good data by overwriting it
/// with defaults.
fn load_for_save(path: &Path) -> Result<PersistedRuntimeState> {
  match read_state(path) {
    StateRead::Empty => Ok(PersistedRuntimeState::default()),
    StateRead::Parsed(state) => Ok(*state),
    StateRead::Malformed(error) => {
      let backup = path.with_extension("yml.bak");
      match std::fs::rename(path, &backup) {
        Ok(()) => log::warn!(
          "[state] {error:#}; backed up to {} and starting from defaults",
          backup.display()
        ),
        Err(rename_error) => log::warn!(
          "[state] {error:#}; could not back up to {} ({rename_error}); overwriting with defaults",
          backup.display()
        ),
      }
      Ok(PersistedRuntimeState::default())
    }
    StateRead::Unreadable(error) => Err(error),
  }
}

/// Save a state patch atomically and privately.
///
/// Runtime saves are intentionally read-modify-write: most save call sites only
/// own one field, so blindly writing a whole in-memory snapshot would clobber
/// updates made by another running instance.
pub fn save(path: &Path, state: &PersistedRuntimeState) -> Result<()> {
  let mut merged = load_for_save(path)?;
  merge_state_patch(&mut merged, state);
  write_state(path, &merged)
}

fn write_state(path: &Path, state: &PersistedRuntimeState) -> Result<()> {
  let state = sanitized_persisted_state(state);
  let yaml = serde_yaml::to_string(&state).context("serializing runtime state")?;
  if let Some(dir) = path.parent() {
    crate::core::paths::ensure_private_dir(dir)?;
  }
  crate::core::auth::write_private_file_atomic(path, yaml.as_bytes())
    .with_context(|| format!("writing {}", path.display()))?;
  Ok(())
}

fn sanitized_persisted_state(state: &PersistedRuntimeState) -> PersistedRuntimeState {
  PersistedRuntimeState {
    volume_percent: state.volume_percent.map(|volume| volume.min(100)),
    shuffle_enabled: state.shuffle_enabled,
    active_source: state.active_source,
    seen_announcement_ids: state.seen_announcement_ids.as_deref().map(sanitized_ids),
    dismissed_announcements: state.dismissed_announcements.as_deref().map(sanitized_ids),
    sidebar_width_percent: state.sidebar_width_percent.map(|width| width.min(100)),
    playbar_height_rows: state
      .playbar_height_rows
      .map(|rows| rows.min(MAX_PLAYBAR_ROWS)),
    library_height_percent: state.library_height_percent.map(|height| height.min(100)),
    pane_layout: state.pane_layout.as_ref().and_then(PaneNode::sanitized),
    qobuz_bundle_cache: state
      .qobuz_bundle_cache
      .clone()
      .filter(QobuzBundleCache::is_complete),
  }
}

fn merge_state_patch(merged: &mut PersistedRuntimeState, patch: &PersistedRuntimeState) {
  if let Some(volume_percent) = patch.volume_percent {
    merged.volume_percent = Some(volume_percent.min(100));
  }
  if let Some(shuffle_enabled) = patch.shuffle_enabled {
    merged.shuffle_enabled = Some(shuffle_enabled);
  }
  if let Some(active_source) = patch.active_source {
    merged.active_source = Some(active_source);
  }
  if let Some(seen_announcement_ids) = &patch.seen_announcement_ids {
    merged.seen_announcement_ids = Some(merged_ids(
      merged.seen_announcement_ids.as_deref().unwrap_or(&[]),
      seen_announcement_ids,
    ));
  }
  if let Some(dismissed_announcements) = &patch.dismissed_announcements {
    merged.dismissed_announcements = Some(merged_ids(
      merged.dismissed_announcements.as_deref().unwrap_or(&[]),
      dismissed_announcements,
    ));
  }
  if let Some(sidebar_width_percent) = patch.sidebar_width_percent {
    merged.sidebar_width_percent = Some(sidebar_width_percent.min(100));
  }
  if let Some(playbar_height_rows) = patch.playbar_height_rows {
    merged.playbar_height_rows = Some(playbar_height_rows.min(MAX_PLAYBAR_ROWS));
  }
  if let Some(library_height_percent) = patch.library_height_percent {
    merged.library_height_percent = Some(library_height_percent.min(100));
  }
  if let Some(pane_layout) = patch.pane_layout.as_ref().and_then(PaneNode::sanitized) {
    merged.pane_layout = Some(pane_layout);
  }
  if let Some(qobuz_bundle_cache) = &patch.qobuz_bundle_cache {
    merged.qobuz_bundle_cache = Some(qobuz_bundle_cache.clone());
  }
}

fn deserialize_pane_layout<'de, D>(deserializer: D) -> Result<Option<PaneNode>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  deserialize_lenient(deserializer, "pane_layout", |layout: PaneNode| {
    layout.sanitized()
  })
}

/// Reads an optional field that falls back to None when it does not parse.
fn deserialize_lenient<'de, D, T>(
  deserializer: D,
  field: &str,
  sanitize: impl FnOnce(T) -> Option<T>,
) -> Result<Option<T>, D::Error>
where
  D: serde::Deserializer<'de>,
  T: serde::de::DeserializeOwned,
{
  let value = Option::<serde_yaml::Value>::deserialize(deserializer)?;
  Ok(
    value.and_then(|value| match serde_yaml::from_value::<T>(value) {
      Ok(parsed) => sanitize(parsed),
      Err(e) => {
        log::warn!("[state] ignoring an unreadable {field}: {e}");
        None
      }
    }),
  )
}

fn sanitized_ids(ids: &[String]) -> Vec<String> {
  ids
    .iter()
    .map(|id| id.trim().to_string())
    .filter(|id| !id.is_empty())
    .collect()
}

fn merged_ids(existing: &[String], incoming: &[String]) -> Vec<String> {
  let mut seen = std::collections::HashSet::new();
  existing
    .iter()
    .chain(incoming.iter())
    .filter_map(|id| {
      let id = id.trim();
      if id.is_empty() || !seen.insert(id.to_string()) {
        return None;
      }
      Some(id.to_string())
    })
    .collect()
}

mod source_config {
  use super::Source;
  use serde::{Deserialize, Deserializer, Serializer};

  pub(super) fn serialize_option<S>(
    source: &Option<Source>,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match source {
      Some(source) => serializer.serialize_some(source.to_config_str()),
      None => serializer.serialize_none(),
    }
  }

  pub(super) fn deserialize_option<'de, D>(deserializer: D) -> Result<Option<Source>, D::Error>
  where
    D: Deserializer<'de>,
  {
    Option::<String>::deserialize(deserializer)
      .map(|source| source.map(|source| Source::from_config_str(&source)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::pane::{Pane, SplitAxis};

  fn lyrics_beside_main() -> PaneNode {
    let mut layout = PaneNode::default();
    layout.detach(Pane::Main, Pane::Lyrics, SplitAxis::Horizontal);
    layout
  }

  #[test]
  fn the_pane_layout_saves_as_a_sparse_patch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");
    save(&path, &PersistedRuntimeState::volume_percent(42)).unwrap();

    save(
      &path,
      &PersistedRuntimeState::pane_layout(&lyrics_beside_main()),
    )
    .unwrap();

    let loaded = load(&path).unwrap();
    assert_eq!(loaded.volume_percent, Some(42));
    let mut runtime = RuntimeState::default();
    runtime.apply_persisted(&loaded);
    assert_eq!(runtime.pane_layout, lyrics_beside_main());
  }

  #[test]
  fn an_unreadable_pane_layout_loads_as_none_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");
    std::fs::write(
      &path,
      "volume_percent: 42\npane_layout:\n  axis: vertical\n  percent: 50\n  first: main\n  second: hologram\n",
    )
    .unwrap();

    let loaded = load(&path).unwrap();

    assert_eq!(loaded.volume_percent, Some(42));
    assert_eq!(loaded.pane_layout, None);
  }

  #[test]
  fn a_now_playing_leaf_in_the_tui_layout_is_dropped_on_load() {
    let mut layout = lyrics_beside_main();
    layout.detach(Pane::Lyrics, Pane::NowPlaying, SplitAxis::Vertical);
    let persisted = PersistedRuntimeState::pane_layout(&layout);

    let mut runtime = RuntimeState::default();
    runtime.apply_persisted(&persisted);

    assert_eq!(runtime.pane_layout, lyrics_beside_main());
  }

  #[test]
  fn missing_file_loads_as_empty_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");

    assert_eq!(load(&path).unwrap(), PersistedRuntimeState::default());
  }

  #[test]
  fn an_old_state_file_with_key_tiers_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");
    std::fs::write(
      &path,
      "volume_percent: 30\nclient_key_tiers:\n  abc: restricted2026\n",
    )
    .unwrap();

    assert_eq!(load(&path).unwrap().volume_percent, Some(30));
  }

  #[test]
  fn incomplete_qobuz_bundle_cache_is_dropped_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");

    save(
      &path,
      &PersistedRuntimeState::qobuz_bundle_cache(QobuzBundleCache {
        bundle_version: "8.2.0-b034".to_string(),
        app_id: String::new(),
        app_secret: "s".to_string(),
        oauth_key: "k".to_string(),
      }),
    )
    .unwrap();

    assert_eq!(load(&path).unwrap().qobuz_bundle_cache, None);
  }

  #[test]
  fn state_load_and_save_caps_playbar_height_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");
    std::fs::write(
      &path,
      format!("playbar_height_rows: {}\n", MAX_PLAYBAR_ROWS + 10),
    )
    .unwrap();

    assert_eq!(
      load(&path).unwrap().playbar_height_rows,
      Some(MAX_PLAYBAR_ROWS)
    );

    save(
      &path,
      &PersistedRuntimeState {
        volume_percent: Some(40),
        ..Default::default()
      },
    )
    .unwrap();

    assert_eq!(
      load(&path).unwrap().playbar_height_rows,
      Some(MAX_PLAYBAR_ROWS)
    );
  }

  #[test]
  fn save_heals_a_malformed_state_file_and_keeps_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");
    let garbage = "this: is: not: valid: yaml: [\n";
    std::fs::write(&path, garbage).unwrap();

    // A malformed file makes `load` fail, but `save` must still succeed.
    assert!(load(&path).is_err());
    save(&path, &PersistedRuntimeState::volume_percent(42)).unwrap();

    // The corrupt bytes are preserved next to the healed file, untouched.
    let backup = path.with_extension("yml.bak");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), garbage);

    // The rewritten file parses and reflects the patch, starting from defaults.
    assert_eq!(
      load(&path).unwrap(),
      PersistedRuntimeState {
        volume_percent: Some(42),
        ..Default::default()
      }
    );
  }

  #[test]
  fn save_heals_a_non_utf8_state_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");
    // A write truncated mid-multibyte-character leaves invalid UTF-8 on disk.
    let garbage = b"volume_percent: 5\xff\xfe";
    std::fs::write(&path, garbage).unwrap();

    assert!(load(&path).is_err());
    save(&path, &PersistedRuntimeState::volume_percent(42)).unwrap();

    let backup = path.with_extension("yml.bak");
    assert_eq!(std::fs::read(&backup).unwrap(), garbage);
    assert_eq!(
      load(&path).unwrap(),
      PersistedRuntimeState {
        volume_percent: Some(42),
        ..Default::default()
      }
    );
  }

  #[test]
  fn save_leaves_no_temporary_files_behind() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.yml");

    save(&path, &PersistedRuntimeState::volume_percent(30)).unwrap();

    let has_temp = std::fs::read_dir(dir.path())
      .unwrap()
      .filter_map(Result::ok)
      .any(|entry| entry.file_name().to_string_lossy().contains(".tmp"));
    assert!(!has_temp, "atomic save left a temporary file behind");
  }
}
