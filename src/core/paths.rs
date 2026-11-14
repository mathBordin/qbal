use anyhow::{Context, Result};
use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

const APP_DIR: &str = "qbal";
const FALLBACK_CONFIG_DIR: [&str; 1] = [".config"];
#[cfg(test)]
const FALLBACK_CACHE_DIR: [&str; 1] = [".cache"];
const FALLBACK_STATE_DIR: [&str; 2] = [".local", "state"];
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";
const XDG_STATE_HOME_ENV: &str = "XDG_STATE_HOME";

/// Resolve qbal's app config directory.
///
/// Uses `$XDG_CONFIG_HOME/qbal` when XDG_CONFIG_HOME is set to an absolute
/// path, otherwise preserves the historical `$HOME/.config/qbal` fallback.
pub(crate) fn app_config_dir() -> Option<PathBuf> {
  app_dir_from(
    std::env::var_os(XDG_CONFIG_HOME_ENV),
    dirs::home_dir(),
    &FALLBACK_CONFIG_DIR,
  )
}

/// Resolve qbal's app state directory.
///
/// Uses `$XDG_STATE_HOME/qbal` when XDG_STATE_HOME is set to an absolute
/// path, otherwise falls back to `$HOME/.local/state/qbal`.
pub(crate) fn app_state_dir() -> Option<PathBuf> {
  app_dir_from(
    std::env::var_os(XDG_STATE_HOME_ENV),
    dirs::home_dir(),
    &FALLBACK_STATE_DIR,
  )
}

/// The Qobuz credentials file: `<config dir>/qobuz_credentials.yml`.
pub(crate) fn qobuz_credentials_path() -> Option<PathBuf> {
  app_config_dir().map(|dir| dir.join("qobuz_credentials.yml"))
}

/// Directory holding this run's log file.
///
/// The OS temp directory rather than the state dir: a log file is written per
/// process id, so they accumulate, and temp is the one location the platform
/// clears on its own. Resolved through `std::env::temp_dir`, which honours
/// `$TMPDIR`.
pub(crate) fn app_log_dir() -> PathBuf {
  static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
  DIR
    .get_or_init(|| log_dir_in(&std::env::temp_dir()))
    .clone()
}

/// `qbal_logs` when this user owns it, else a fresh one: another account can take that name.
fn log_dir_in(temp: &Path) -> PathBuf {
  let shared = temp.join("qbal_logs");
  // A symlink first: `ensure_private_dir` follows one and would chmod its target.
  let is_symlink = shared
    .symlink_metadata()
    .is_ok_and(|meta| meta.file_type().is_symlink());
  let owned = !is_symlink
    && ensure_private_dir(&shared).is_ok()
    && shared
      .symlink_metadata()
      .is_ok_and(|meta| meta.file_type().is_dir());
  if owned {
    return shared;
  }
  fresh_private_dir(temp, "qbal_logs").unwrap_or(shared)
}

/// A new, uniquely named, owner-only directory under `temp`, or `None` when
/// none could be made. A non-recursive create refuses an existing entry, a
/// symlink included, so the result is always one this process made. It is not
/// removed on exit: temp is the one location the platform clears on its own.
pub(crate) fn fresh_private_dir(temp: &Path, prefix: &str) -> Option<PathBuf> {
  // Owner-only from the start, whatever the umask.
  #[cfg(unix)]
  let builder = {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    builder
  };
  #[cfg(not(unix))]
  let builder = std::fs::DirBuilder::new();
  let pid = std::process::id();
  (0..8).find_map(|attempt| {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|since| since.subsec_nanos())
      .unwrap_or(0);
    let dir = temp.join(format!("{prefix}-{pid}-{nanos}-{attempt}"));
    builder.create(&dir).ok().map(|()| dir)
  })
}

/// Path of this process's log file. Callers on both the writing side
/// (`setup_logging`) and the reporting side (the help screen) use this so the
/// path shown to a user is always the path actually written.
pub(crate) fn app_log_path() -> PathBuf {
  app_log_dir().join(format!("qballog{}", std::process::id()))
}

/// Ensure a directory that stores credentials or other private app data exists
/// and is owner-only where supported.
pub(crate) fn ensure_private_dir(dir: &Path) -> Result<()> {
  std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
      .with_context(|| format!("setting private permissions on {}", dir.display()))?;
  }

  Ok(())
}

fn app_dir_from(
  xdg_home: Option<OsString>,
  home: Option<PathBuf>,
  fallback_dir: &[&str],
) -> Option<PathBuf> {
  base_dir_from(xdg_home, home, fallback_dir).map(|dir| dir.join(APP_DIR))
}

fn base_dir_from(
  xdg_home: Option<OsString>,
  home: Option<PathBuf>,
  fallback_dir: &[&str],
) -> Option<PathBuf> {
  xdg_home.and_then(valid_xdg_home).or_else(|| {
    home.map(|home| {
      fallback_dir
        .iter()
        .fold(home, |home, component| home.join(component))
    })
  })
}

/// The folder name this app used before it was renamed to Qbal.
const PRE_RENAME_APP_DIR: &str = "spotatui";

/// `(old, new)` pairs for the config and state folders: where the pre-rename
/// build kept its files, and where Qbal keeps them.
pub(crate) fn pre_rename_app_dirs() -> Vec<(PathBuf, PathBuf)> {
  [
    (XDG_CONFIG_HOME_ENV, &FALLBACK_CONFIG_DIR[..]),
    (XDG_STATE_HOME_ENV, &FALLBACK_STATE_DIR[..]),
  ]
  .into_iter()
  .filter_map(|(env, fallback)| base_dir_from(std::env::var_os(env), dirs::home_dir(), fallback))
  .map(|base| (base.join(PRE_RENAME_APP_DIR), base.join(APP_DIR)))
  .collect()
}

fn valid_xdg_home(value: OsString) -> Option<PathBuf> {
  if value.is_empty() {
    return None;
  }

  let path = PathBuf::from(value);
  if path.is_absolute() {
    Some(path)
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_owned_log_dir_keeps_the_shared_name() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(log_dir_in(temp.path()), temp.path().join("qbal_logs"));
  }

  #[test]
  fn a_log_dir_name_that_is_taken_falls_back_to_a_fresh_dir() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("qbal_logs"), b"").unwrap();
    let dir = log_dir_in(temp.path());
    assert_ne!(dir, temp.path().join("qbal_logs"));
    assert!(dir.is_dir());
  }

  #[cfg(unix)]
  #[test]
  fn a_planted_log_dir_symlink_leaves_its_target_alone() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(&target, temp.path().join("qbal_logs")).unwrap();
    let dir = log_dir_in(temp.path());
    assert_ne!(dir, temp.path().join("qbal_logs"));
    let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
  }

  fn test_home() -> PathBuf {
    std::env::temp_dir().join("qbal-home-alice")
  }

  fn test_xdg_home(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
  }

  #[test]
  fn app_config_dir_uses_absolute_xdg_config_home() {
    let xdg_home = test_xdg_home("xdg-config");
    let path = app_dir_from(
      Some(xdg_home.clone().into_os_string()),
      Some(test_home()),
      &FALLBACK_CONFIG_DIR,
    );

    assert_eq!(path, Some(xdg_home.join(APP_DIR)));
  }

  #[test]
  fn app_config_dir_falls_back_to_home_when_xdg_is_unset() {
    let home = test_home();
    let path = app_dir_from(None, Some(home.clone()), &FALLBACK_CONFIG_DIR);

    assert_eq!(path, Some(home.join(".config").join(APP_DIR)));
  }

  #[test]
  fn app_config_dir_ignores_empty_or_relative_xdg_config_home() {
    let home = test_home();

    assert_eq!(
      app_dir_from(
        Some(OsString::new()),
        Some(home.clone()),
        &FALLBACK_CONFIG_DIR
      ),
      Some(home.clone().join(".config").join(APP_DIR))
    );
    assert_eq!(
      app_dir_from(
        Some(OsString::from("relative")),
        Some(home.clone()),
        &FALLBACK_CONFIG_DIR
      ),
      Some(home.join(".config").join(APP_DIR))
    );
  }

  #[test]
  fn app_cache_dir_uses_xdg_cache_home_or_home_fallback() {
    let xdg_home = test_xdg_home("xdg-cache");
    assert_eq!(
      app_dir_from(
        Some(xdg_home.clone().into_os_string()),
        Some(test_home()),
        &FALLBACK_CACHE_DIR,
      ),
      Some(xdg_home.join(APP_DIR))
    );

    let home = test_home();
    assert_eq!(
      app_dir_from(None, Some(home.clone()), &FALLBACK_CACHE_DIR,),
      Some(home.join(".cache").join(APP_DIR))
    );
  }

  #[test]
  fn app_state_dir_uses_xdg_state_home_or_home_fallback() {
    let xdg_home = test_xdg_home("xdg-state");
    assert_eq!(
      app_dir_from(
        Some(xdg_home.clone().into_os_string()),
        Some(test_home()),
        &FALLBACK_STATE_DIR,
      ),
      Some(xdg_home.join(APP_DIR))
    );

    let home = test_home();
    assert_eq!(
      app_dir_from(None, Some(home.clone()), &FALLBACK_STATE_DIR,),
      Some(home.join(".local").join("state").join(APP_DIR))
    );
  }
}
