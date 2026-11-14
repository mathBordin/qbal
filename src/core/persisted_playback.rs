//! Persist the last playback session so it survives restarts: which Qobuz
//! track list was playing, where in it, whether it was paused, and the native
//! queue.
//!
//! A playback session is a machine-written, frequently-updated blob (a list of
//! [`TrackInfo`], an index, a live position). Serializing it into the
//! hand-editable `config.yml` would bury the user's real settings under
//! churning metadata, so it lives in its own `last_session.yml` under the app
//! state dir. The full [`TrackInfo`] list is stored because Qobuz metadata
//! cannot be re-derived offline.

use crate::core::plugin_api::TrackInfo;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "last_session.yml";

/// The full persisted session: the active playback (if any) plus the native
/// queue. Wraps [`PersistedPlayback`] so the queue can be persisted
/// independently of the playback.
///
/// `deny_unknown_fields` is load-bearing: [`load`] parses this wrapper *first*,
/// and a legacy file is a bare top-level [`PersistedPlayback`] whose `source`
/// tag key would otherwise be silently ignored here (serde ignores unknown
/// fields by default), yielding an empty session and dropping the saved
/// playback. Rejecting unknown fields forces the legacy file to fail this parse
/// so [`load`] falls through to the legacy path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PersistedSession {
  #[serde(default)]
  pub playback: Option<PersistedPlayback>,
  #[serde(default)]
  pub queue: Vec<TrackInfo>,
}

/// Environment override for the session file location (used by tests, and
/// available to users who keep their config elsewhere).
pub const PATH_ENV: &str = "QBAL_LAST_SESSION_PATH";

/// What to resume on the next launch. The `source` tag is kept so sessions
/// from other sources, written by the app this one forked from, fail to load
/// instead of resuming as something else.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source")]
pub enum PersistedPlayback {
  /// Qobuz: full track metadata (from the API) plus the queue position.
  Qobuz {
    tracks: Vec<TrackInfo>,
    index: usize,
    position_ms: u64,
    paused: bool,
    #[serde(default)]
    repeat: crate::infra::queue::RepeatMode,
    /// The user's shuffle *intent* (`decoded_shuffle`). Stored separately from
    /// `shuffle` because a toggle deferred while a track change is in flight can
    /// leave the two temporarily disagreeing; the restore trusts this flag and
    /// reconciles the queue order to it.
    #[serde(default)]
    shuffle_on: bool,
    /// The applied shuffle permutation, for restoring the original order on
    /// un-shuffle. May be absent while `shuffle_on` is set (deferred toggle).
    #[serde(default)]
    shuffle: Option<crate::infra::queue::ShuffleBackup>,
  },
}

/// Location of the session file: `$QBAL_LAST_SESSION_PATH` when set, else
/// `<state dir>/last_session.yml`.
pub fn default_session_path() -> Result<PathBuf> {
  if let Ok(path) = std::env::var(PATH_ENV) {
    return Ok(PathBuf::from(path));
  }
  crate::core::paths::app_state_dir()
    .map(|dir| dir.join(FILE_NAME))
    .ok_or_else(|| anyhow!("cannot resolve the qbal state directory"))
}

/// Load the persisted session. A missing file means "no session to resume"
/// (`Ok(None)`); a malformed file is an error the caller logs and ignores
/// (never crash startup over an auto-written file).
///
/// Parses the current [`PersistedSession`] wrapper first, then falls back to a
/// legacy top-level [`PersistedPlayback`] (files written before the queue
/// existed), wrapping it with an empty queue. This is an explicit two-step
/// parse — see the note on [`PersistedSession`] for why the ordering is safe.
pub fn load(path: &Path) -> Result<Option<PersistedSession>> {
  match std::fs::read_to_string(path) {
    Ok(contents) => {
      if let Ok(session) = serde_yaml::from_str::<PersistedSession>(&contents) {
        return Ok(Some(session));
      }
      serde_yaml::from_str::<PersistedPlayback>(&contents)
        .map(|playback| {
          Some(PersistedSession {
            playback: Some(playback),
            queue: Vec::new(),
          })
        })
        .with_context(|| format!("malformed session file: {}", path.display()))
    }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
  }
}

/// Save the session atomically (write a sibling tempfile, then rename) so a
/// crash mid-write can't leave a half-written file that fails to parse.
pub fn save(path: &Path, session: &PersistedSession) -> Result<()> {
  let yaml = serde_yaml::to_string(session).context("serializing playback session")?;
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
  }
  let tmp = path.with_extension("yml.tmp");
  std::fs::write(&tmp, yaml).with_context(|| format!("writing {}", tmp.display()))?;
  std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
  Ok(())
}

/// Remove the session file. A missing file is not an error — clearing an
/// already-absent session is a no-op.
pub fn clear(path: &Path) -> Result<()> {
  match std::fs::remove_file(path) {
    Ok(()) => Ok(()),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::infra::queue::RepeatMode;

  fn track(uri: &str, name: &str) -> TrackInfo {
    TrackInfo {
      generated_name: None,
      uri: Some(uri.to_string()),
      name: name.to_string(),
      artists: vec!["Artist".to_string()],
      album: "Album".to_string(),
      duration_ms: 1000,
      id: None,
      album_id: None,
      artist_refs: vec![],
      is_playable: true,
      is_local: false,
      track_number: 0,
      explicit: false,
      image_url: None,
      release_date: None,
    }
  }

  fn session(playback: Option<PersistedPlayback>, queue: Vec<TrackInfo>) -> PersistedSession {
    PersistedSession { playback, queue }
  }

  #[test]
  fn missing_file_is_no_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.yml");
    assert_eq!(load(&path).unwrap(), None);
  }

  #[test]
  fn shuffle_intent_round_trips_independently_of_the_backup() {
    // A shuffle toggled while a track change is in flight is deferred, so the
    // intent (`shuffle_on: true`) can be persisted while the permutation
    // (`shuffle`) has not been applied yet. Both must survive independently so
    // the restore trusts the flag and reconciles the order to it.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    let s = session(
      Some(PersistedPlayback::Qobuz {
        tracks: vec![track("qobuz:track:1", "A"), track("qobuz:track:2", "B")],
        index: 0,
        position_ms: 0,
        paused: false,
        repeat: RepeatMode::Context,
        shuffle_on: true,
        shuffle: None,
      }),
      vec![],
    );
    save(&path, &s).unwrap();
    assert_eq!(load(&path).unwrap(), Some(s));
  }

  #[test]
  fn save_then_load_round_trips_qobuz_playback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    let s = session(
      Some(PersistedPlayback::Qobuz {
        tracks: vec![track("qobuz:track:1", "Hi-res")],
        index: 0,
        position_ms: 12_000,
        paused: true,
        repeat: RepeatMode::Context,
        shuffle_on: false,
        shuffle: None,
      }),
      vec![],
    );
    save(&path, &s).unwrap();
    assert_eq!(load(&path).unwrap(), Some(s));
  }

  #[test]
  fn save_then_load_round_trips_playback_with_queue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    let s = session(
      Some(PersistedPlayback::Qobuz {
        tracks: vec![track("qobuz:track:1", "Playing")],
        index: 0,
        position_ms: 5_000,
        paused: false,
        repeat: RepeatMode::Off,
        shuffle_on: false,
        shuffle: None,
      }),
      vec![
        track("qobuz:track:2", "Queued A"),
        track("qobuz:track:3", "Queued B"),
      ],
    );
    save(&path, &s).unwrap();
    assert_eq!(load(&path).unwrap(), Some(s));
  }

  #[test]
  fn save_then_load_round_trips_queue_only_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    // No active playback source, but the native queue has items to persist.
    let s = session(None, vec![track("qobuz:track:9", "Just Queued")]);
    save(&path, &s).unwrap();
    let loaded = load(&path).unwrap().expect("should load");
    assert!(loaded.playback.is_none());
    assert_eq!(loaded.queue.len(), 1);
    assert_eq!(loaded.queue[0].name, "Just Queued");
  }

  #[test]
  fn legacy_top_level_playback_file_loads_with_empty_queue() {
    // A file written before the queue existed is a bare top-level enum with a
    // `source` tag; it must still resume playback (and load an empty queue).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    let legacy = PersistedPlayback::Qobuz {
      tracks: vec![
        track("qobuz:track:1", "Legacy A"),
        track("qobuz:track:2", "Legacy B"),
      ],
      index: 1,
      position_ms: 12_345,
      paused: true,
      repeat: RepeatMode::Off,
      shuffle_on: false,
      shuffle: None,
    };
    let legacy_yaml = serde_yaml::to_string(&legacy).unwrap();
    std::fs::write(&path, legacy_yaml).unwrap();

    let loaded = load(&path).unwrap().expect("legacy file should load");
    assert!(loaded.queue.is_empty());
    // The discriminating assertion: the legacy playback survives the fallback.
    assert_eq!(loaded.playback, Some(legacy));
  }

  #[test]
  fn clear_removes_the_file_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    save(
      &path,
      &session(
        Some(PersistedPlayback::Qobuz {
          tracks: vec![track("qobuz:track:1", "A")],
          index: 0,
          position_ms: 0,
          paused: false,
          repeat: RepeatMode::Off,
          shuffle_on: false,
          shuffle: None,
        }),
        vec![],
      ),
    )
    .unwrap();
    assert!(path.exists());
    clear(&path).unwrap();
    assert!(!path.exists());
    // Clearing an absent file is a no-op, not an error.
    clear(&path).unwrap();
  }

  #[test]
  fn a_session_from_another_source_does_not_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    std::fs::write(
      &path,
      "playback:\n  source: Local\n  queue: [\"file:///a.mp3\"]\n  index: 0\n  position_ms: 0\n  paused: false\nqueue: []\n",
    )
    .unwrap();
    assert!(load(&path).is_err());
  }

  #[test]
  fn malformed_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("last_session.yml");
    std::fs::write(&path, "this: is: not: valid: yaml: for: our: enum").unwrap();
    assert!(load(&path).is_err());
  }
}
