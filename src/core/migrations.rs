//! Short-lived upgrade shims for moving old on-disk shapes into current state.
//!
//! Keep each migration isolated here so it can be deleted cleanly once the
//! minimum supported upgrade path no longer includes the legacy release.

use crate::core::source::Source;
use crate::core::state::{PersistedRuntimeState, RuntimeState};
use anyhow::{anyhow, Context, Result};
use serde_yaml::{Mapping, Value};
use std::{
  fs,
  path::{Path, PathBuf},
};

const LEGACY_STATE_FILE_RELATIVE_PATHS: [&str; 4] = [
  "last_session.yml",
  "history/listens.jsonl",
  "history/last_recap_at.txt",
  "history/last_synced_at.txt",
];

/// Copy the folders the app used before its rename (`spotatui`) to Qbal's own
/// on the first launch, so settings, history and the Qobuz login carry over.
/// A folder is copied only when Qbal's does not exist yet; the old folder is
/// left in place. Returns the pairs it copied.
pub(crate) fn migrate_pre_rename_dirs() -> Vec<(PathBuf, PathBuf)> {
  migrate_pre_rename_dirs_in(crate::core::paths::pre_rename_app_dirs())
}

fn migrate_pre_rename_dirs_in(pairs: Vec<(PathBuf, PathBuf)>) -> Vec<(PathBuf, PathBuf)> {
  let mut copied = Vec::new();
  for (old, new) in pairs {
    if !old.is_dir() || new.exists() {
      continue;
    }
    // Copy into a sibling first, then rename: an interrupted copy never
    // leaves a half-filled folder that would block the next attempt.
    let staging = new.with_extension("migrating");
    let _ = std::fs::remove_dir_all(&staging);
    match copy_dir(&old, &staging).and_then(|()| Ok(std::fs::rename(&staging, &new)?)) {
      Ok(()) => copied.push((old, new)),
      Err(e) => {
        log::warn!(
          "[migrate] could not copy {} to {}: {e}",
          old.display(),
          new.display()
        );
        let _ = std::fs::remove_dir_all(&staging);
      }
    }
  }
  copied
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
  crate::core::paths::ensure_private_dir(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let kind = entry.file_type()?;
    let target = to.join(entry.file_name());
    if kind.is_dir() {
      copy_dir(&entry.path(), &target)?;
    } else if kind.is_file() {
      // `fs::copy` keeps the permission bits, so owner-only files stay private.
      std::fs::copy(entry.path(), &target)?;
    }
    // Symlinks are skipped: their targets may sit outside the folder.
  }
  Ok(())
}

/// Move app-managed files that used to live under the XDG config directory into
/// the XDG state directory.
///
/// This intentionally never overwrites an existing state file: once the new
/// path has data, merging the old file is file-format-specific and unsafe for a
/// generic shim. Run once at startup and remove after the config-to-state
/// upgrade window closes.
pub(crate) fn apply_legacy_state_file_migrations() -> Result<()> {
  let Some(config_dir) = crate::core::paths::app_config_dir() else {
    return Ok(());
  };
  let state_dir = crate::core::paths::app_state_dir()
    .ok_or_else(|| anyhow!("cannot resolve the qbal state directory"))?;
  crate::core::paths::ensure_private_dir(&state_dir)?;

  for relative_path in LEGACY_STATE_FILE_RELATIVE_PATHS {
    migrate_legacy_path_if_unclaimed(
      &config_dir.join(relative_path),
      &state_dir.join(relative_path),
    )?;
  }

  Ok(())
}

pub(crate) fn migrate_legacy_path_if_unclaimed(
  legacy_path: &Path,
  state_path: &Path,
) -> Result<bool> {
  if path_exists(state_path)? || !path_exists(legacy_path)? {
    return Ok(false);
  }

  if let Some(parent) = state_path.parent() {
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
  }

  match fs::rename(legacy_path, state_path) {
    Ok(()) => {
      log::info!(
        "migrated legacy app data from {} to {}",
        legacy_path.display(),
        state_path.display()
      );
      Ok(true)
    }
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(rename_error) => {
      if !copy_legacy_path_if_unclaimed(legacy_path, state_path, &rename_error)? {
        return Ok(false);
      }
      remove_legacy_path_after_migration(legacy_path, state_path);
      log::info!(
        "migrated legacy app data from {} to {}",
        legacy_path.display(),
        state_path.display()
      );
      Ok(true)
    }
  }
}

fn copy_legacy_path_if_unclaimed(
  legacy_path: &Path,
  state_path: &Path,
  rename_error: &std::io::Error,
) -> Result<bool> {
  let metadata = match fs::symlink_metadata(legacy_path) {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
    Err(error) => {
      return Err(error).with_context(|| format!("checking {}", legacy_path.display()));
    }
  };

  if metadata.is_dir() {
    return copy_legacy_dir_if_unclaimed(legacy_path, state_path, rename_error);
  }

  let mut source = match fs::File::open(legacy_path) {
    Ok(file) => file,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
    Err(error) => {
      return Err(error).with_context(|| format!("opening {}", legacy_path.display()));
    }
  };
  let mut target_options = fs::OpenOptions::new();
  target_options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    target_options.mode(0o600);
  }
  let mut target = match target_options.open(state_path) {
    Ok(file) => file,
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
    Err(error) => {
      return Err(error).with_context(|| {
        format!(
          "renaming {} to {} failed ({rename_error}); creating copy target also failed",
          legacy_path.display(),
          state_path.display()
        )
      });
    }
  };

  if let Err(error) = std::io::copy(&mut source, &mut target) {
    let _ = fs::remove_file(state_path);
    return Err(error).with_context(|| {
      format!(
        "renaming {} to {} failed ({rename_error}); copying fallback also failed",
        legacy_path.display(),
        state_path.display()
      )
    });
  }

  Ok(true)
}

fn copy_legacy_dir_if_unclaimed(
  legacy_path: &Path,
  state_path: &Path,
  rename_error: &std::io::Error,
) -> Result<bool> {
  match fs::create_dir(state_path) {
    Ok(()) => set_private_dir_permissions(state_path)?,
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
    Err(error) => {
      return Err(error).with_context(|| {
        format!(
          "renaming {} to {} failed ({rename_error}); creating copy target also failed",
          legacy_path.display(),
          state_path.display()
        )
      });
    }
  }

  if let Err(error) = copy_dir_contents(legacy_path, state_path) {
    let _ = fs::remove_dir_all(state_path);
    return Err(error);
  }

  Ok(true)
}

fn create_private_dir(path: &Path) -> Result<()> {
  fs::create_dir(path).with_context(|| format!("creating {}", path.display()))?;
  set_private_dir_permissions(path)
}

fn set_private_dir_permissions(path: &Path) -> Result<()> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
      .with_context(|| format!("setting private permissions on {}", path.display()))?;
  }
  // Not a Unix system: there are no mode bits to tighten.
  #[cfg(not(unix))]
  let _ = path;

  Ok(())
}

fn copy_dir_contents(legacy_path: &Path, state_path: &Path) -> Result<()> {
  for entry in
    fs::read_dir(legacy_path).with_context(|| format!("reading {}", legacy_path.display()))?
  {
    let entry = entry?;
    let source = entry.path();
    let target = state_path.join(entry.file_name());
    let file_type = entry
      .file_type()
      .with_context(|| format!("checking {}", source.display()))?;

    if file_type.is_symlink() {
      continue;
    }

    if file_type.is_dir() {
      create_private_dir(&target)?;
      copy_dir_contents(&source, &target)?;
    } else {
      fs::copy(&source, &target).with_context(|| {
        format!(
          "copying legacy app data from {} to {}",
          source.display(),
          target.display()
        )
      })?;
    }
  }

  Ok(())
}

fn remove_legacy_path_after_migration(legacy_path: &Path, state_path: &Path) {
  let remove_result = match fs::symlink_metadata(legacy_path) {
    Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(legacy_path),
    Ok(_) => fs::remove_file(legacy_path),
    Err(error) => Err(error),
  };

  match remove_result {
    Ok(()) => {}
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
    Err(error) => log::warn!(
      "migrated {} to {}, but failed to remove the legacy file: {}",
      legacy_path.display(),
      state_path.display(),
      error
    ),
  }
}

/// Seed runtime-state fields that used to live under `behavior` in `config.yml`.
///
/// This reads the raw YAML rather than `BehaviorConfigString` because those
/// fields were removed from the typed config. Remove it once the legacy
/// upgrade path no longer needs to be supported.
pub(crate) fn apply_legacy_config_runtime_state_migration(
  path: &Path,
  runtime_state: &mut RuntimeState,
  persisted_state: &PersistedRuntimeState,
) -> Result<PersistedRuntimeState> {
  let Some(config) = read_config_value(path)? else {
    return Ok(PersistedRuntimeState::default());
  };
  let Some(behavior) = behavior_mapping(&config) else {
    return Ok(PersistedRuntimeState::default());
  };

  let mut patch = PersistedRuntimeState::default();
  if persisted_state.active_source.is_none() {
    if let Some(source) = behavior
      .get(yaml_key("active_source"))
      .and_then(source_from_value)
    {
      runtime_state.active_source = source;
      patch.active_source = Some(source);
    }
  }
  if persisted_state.shuffle_enabled.is_none() {
    if let Some(shuffle_enabled) = behavior
      .get(yaml_key("shuffle_enabled"))
      .and_then(Value::as_bool)
    {
      runtime_state.shuffle_enabled = shuffle_enabled;
      patch.shuffle_enabled = Some(shuffle_enabled);
    }
  }
  if persisted_state.seen_announcement_ids.is_none() {
    if let Some(ids) = behavior
      .get(yaml_key("seen_announcement_ids"))
      .and_then(string_sequence)
    {
      runtime_state.seen_announcement_ids = ids.clone();
      patch.seen_announcement_ids = Some(ids);
    }
  }
  if persisted_state.dismissed_announcements.is_none() {
    if let Some(ids) = behavior
      .get(yaml_key("dismissed_announcements"))
      .and_then(string_sequence)
    {
      runtime_state.dismissed_announcements = ids.clone();
      patch.dismissed_announcements = Some(ids);
    }
  }

  Ok(patch)
}

fn path_exists(path: &Path) -> Result<bool> {
  match fs::symlink_metadata(path) {
    Ok(_) => Ok(true),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(error) => Err(error).with_context(|| format!("checking {}", path.display())),
  }
}

fn read_config_value(path: &Path) -> Result<Option<Value>> {
  let config_yml = match fs::read_to_string(path) {
    Ok(config_yml) => config_yml,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(e) => return Err(e.into()),
  };
  if config_yml.trim().is_empty() {
    return Ok(None);
  }

  Ok(Some(serde_yaml::from_str(&config_yml)?))
}

fn behavior_mapping(config: &Value) -> Option<&Mapping> {
  config
    .as_mapping()
    .and_then(|map| map.get(yaml_key("behavior")))
    .and_then(Value::as_mapping)
}

fn yaml_key(key: &str) -> Value {
  Value::String(key.to_string())
}

fn source_from_value(value: &Value) -> Option<Source> {
  let source = value.as_str()?.trim();
  if source.is_empty() {
    return None;
  }

  Some(Source::from_config_str(source))
}

fn string_sequence(value: &Value) -> Option<Vec<String>> {
  value.as_sequence().map(|items| {
    items
      .iter()
      .filter_map(Value::as_str)
      .map(|id| id.trim().to_string())
      .filter(|id| !id.is_empty())
      .collect()
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pre_rename_folders_are_copied_once_and_the_old_ones_kept() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("spotatui");
    let new = dir.path().join("qbal");
    fs::create_dir_all(old.join("history")).unwrap();
    fs::write(old.join("qobuz_credentials.yml"), "token: x\n").unwrap();
    fs::write(old.join("history/listens.jsonl"), "{}\n").unwrap();

    let copied = migrate_pre_rename_dirs_in(vec![(old.clone(), new.clone())]);

    assert_eq!(copied, vec![(old.clone(), new.clone())]);
    assert_eq!(
      fs::read_to_string(new.join("qobuz_credentials.yml")).unwrap(),
      "token: x\n"
    );
    assert!(new.join("history/listens.jsonl").is_file());
    assert!(old.join("qobuz_credentials.yml").is_file());
    assert!(!new.with_extension("migrating").exists());
  }

  #[test]
  fn an_existing_qbal_folder_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("spotatui");
    let new = dir.path().join("qbal");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("config.yml"), "old").unwrap();
    fs::create_dir_all(&new).unwrap();
    fs::write(new.join("config.yml"), "new").unwrap();

    assert!(migrate_pre_rename_dirs_in(vec![(old, new.clone())]).is_empty());
    assert_eq!(fs::read_to_string(new.join("config.yml")).unwrap(), "new");
  }

  #[test]
  fn nothing_happens_without_a_pre_rename_folder() {
    let dir = tempfile::tempdir().unwrap();
    let new = dir.path().join("qbal");
    let copied = migrate_pre_rename_dirs_in(vec![(dir.path().join("spotatui"), new.clone())]);
    assert!(copied.is_empty());
    assert!(!new.exists());
  }

  #[test]
  fn legacy_file_rename_moves_file_when_state_path_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir
      .path()
      .join("config")
      .join("history")
      .join("listens.jsonl");
    let state_path = dir
      .path()
      .join("state")
      .join("history")
      .join("listens.jsonl");
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::write(&legacy_path, "legacy listens").unwrap();

    assert!(migrate_legacy_path_if_unclaimed(&legacy_path, &state_path).unwrap());
    assert!(!legacy_path.exists());
    assert_eq!(
      std::fs::read_to_string(&state_path).unwrap(),
      "legacy listens"
    );
  }

  #[test]
  fn legacy_file_rename_ignores_missing_legacy_file() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("config").join("last_session.yml");
    let state_path = dir.path().join("state").join("last_session.yml");

    assert!(!migrate_legacy_path_if_unclaimed(&legacy_path, &state_path).unwrap());
    assert!(!state_path.exists());
  }

  #[test]
  fn legacy_file_rename_does_not_overwrite_existing_state_file() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir
      .path()
      .join("config")
      .join("history")
      .join("last_synced_at.txt");
    let state_path = dir
      .path()
      .join("state")
      .join("history")
      .join("last_synced_at.txt");
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    std::fs::write(&legacy_path, "legacy cursor").unwrap();
    std::fs::write(&state_path, "offset:42").unwrap();

    assert!(!migrate_legacy_path_if_unclaimed(&legacy_path, &state_path).unwrap());
    assert_eq!(
      std::fs::read_to_string(&legacy_path).unwrap(),
      "legacy cursor"
    );
    assert_eq!(std::fs::read_to_string(&state_path).unwrap(), "offset:42");
  }

  #[test]
  fn legacy_path_migration_moves_directory_when_target_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("config").join("streaming_cache");
    let state_path = dir.path().join("cache").join("streaming_cache");
    std::fs::create_dir_all(&legacy_path).unwrap();
    std::fs::write(legacy_path.join("credentials.json"), "legacy credentials").unwrap();
    std::fs::create_dir_all(legacy_path.join("audio")).unwrap();
    std::fs::write(legacy_path.join("audio").join("chunk"), "audio cache").unwrap();

    assert!(migrate_legacy_path_if_unclaimed(&legacy_path, &state_path).unwrap());

    assert!(!legacy_path.exists());
    assert_eq!(
      std::fs::read_to_string(state_path.join("credentials.json")).unwrap(),
      "legacy credentials"
    );
    assert_eq!(
      std::fs::read_to_string(state_path.join("audio").join("chunk")).unwrap(),
      "audio cache"
    );
  }

  #[cfg(unix)]
  #[test]
  fn legacy_directory_copy_skips_symlink_entries_before_recursing() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("config").join("streaming_cache");
    let state_path = dir.path().join("state").join("streaming_cache");
    std::fs::create_dir_all(legacy_path.join("audio")).unwrap();
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    std::fs::write(legacy_path.join("audio").join("chunk"), "audio cache").unwrap();
    std::os::unix::fs::symlink(&legacy_path, legacy_path.join("loop")).unwrap();
    let rename_error = std::io::Error::other("forcing copy fallback");

    assert!(copy_legacy_dir_if_unclaimed(&legacy_path, &state_path, &rename_error).unwrap());

    assert_eq!(
      std::fs::read_to_string(state_path.join("audio").join("chunk")).unwrap(),
      "audio cache"
    );
    assert!(!state_path.join("loop").exists());
  }

  #[cfg(unix)]
  #[test]
  fn legacy_directory_copy_sets_private_permissions_on_created_directories() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("config").join("streaming_cache");
    let state_path = dir.path().join("state").join("streaming_cache");
    std::fs::create_dir_all(legacy_path.join("audio")).unwrap();
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    std::fs::write(legacy_path.join("audio").join("chunk"), "audio cache").unwrap();
    let rename_error = std::io::Error::other("forcing copy fallback");

    assert!(copy_legacy_dir_if_unclaimed(&legacy_path, &state_path, &rename_error).unwrap());

    let top_level_mode = std::fs::metadata(&state_path).unwrap().permissions().mode() & 0o777;
    let subdirectory_mode = std::fs::metadata(state_path.join("audio"))
      .unwrap()
      .permissions()
      .mode()
      & 0o777;
    assert_eq!(top_level_mode, 0o700);
    assert_eq!(subdirectory_mode, 0o700);
  }
}
