//! The format the sound card runs at, read from the kernel.
//!
//! ALSA publishes each open playback stream's parameters in
//! `/proc/asound/cardN/pcmMp/subK/hw_params` ("closed" when nothing plays).
//! With PipeWire or PulseAudio in between, this is the sound server's stream to
//! the card, so it shows what reaches the DAC after any resampling.
//!
//! A background thread re-reads it every [`POLL_EVERY`], so neither the tick
//! nor a draw ever touches the file system.

use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use crate::core::stream_format::{parse_hw_params, HardwareFormat};

/// How often the card's format is re-read.
const POLL_EVERY: Duration = Duration::from_secs(2);

/// Polls the running sound card's format on its own thread; the thread ends
/// once the probe is dropped.
pub struct HardwareProbe {
  latest: Arc<Mutex<Option<HardwareFormat>>>,
}

impl HardwareProbe {
  pub fn start() -> Self {
    let latest = Arc::new(Mutex::new(None));
    let weak: Weak<Mutex<Option<HardwareFormat>>> = Arc::downgrade(&latest);
    let spawned = std::thread::Builder::new()
      .name("qbal-hw-format".into())
      .spawn(move || loop {
        let found = read_running(Path::new("/proc/asound"));
        let Some(latest) = weak.upgrade() else {
          return;
        };
        if let Ok(mut slot) = latest.lock() {
          *slot = found;
        }
        drop(latest);
        std::thread::sleep(POLL_EVERY);
      });
    if let Err(e) = spawned {
      log::warn!("could not start the sound card format probe: {e}");
    }
    HardwareProbe { latest }
  }

  /// The format read last; `None` when no card is playing.
  pub fn latest(&self) -> Option<HardwareFormat> {
    self.latest.lock().ok().and_then(|slot| slot.clone())
  }
}

/// The first running playback stream under `root` (normally `/proc/asound`).
fn read_running(root: &Path) -> Option<HardwareFormat> {
  let mut cards: Vec<_> = std::fs::read_dir(root)
    .ok()?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| {
      path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("card"))
    })
    .collect();
  cards.sort();
  cards.into_iter().find_map(|card| {
    let id = std::fs::read_to_string(card.join("id")).ok();
    let id = id.as_deref().map(str::trim);
    let mut streams: Vec<_> = std::fs::read_dir(&card)
      .ok()?
      .flatten()
      .map(|entry| entry.path())
      .filter(|path| {
        path
          .file_name()
          .and_then(|name| name.to_str())
          .is_some_and(|name| name.starts_with("pcm") && name.ends_with('p'))
      })
      .collect();
    streams.sort();
    streams.into_iter().find_map(|pcm| {
      let mut subs: Vec<_> = std::fs::read_dir(&pcm)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("hw_params"))
        .filter(|path| path.is_file())
        .collect();
      subs.sort();
      subs.into_iter().find_map(|file| {
        let text = std::fs::read_to_string(file).ok()?;
        parse_hw_params(&text, id)
      })
    })
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
  }

  #[test]
  fn the_first_running_playback_stream_wins_and_capture_is_ignored() {
    let root = tempfile::tempdir().unwrap();
    let running = "format: S32_LE\nchannels: 2\nrate: 44100 (44100/1)\n";
    write(root.path(), "card0/id", "PCH\n");
    write(root.path(), "card0/pcm0p/sub0/hw_params", "closed\n");
    write(root.path(), "card0/pcm0c/sub0/hw_params", running);
    write(root.path(), "card1/id", "DAC\n");
    write(root.path(), "card1/pcm0p/sub0/hw_params", running);
    let found = read_running(root.path()).unwrap();
    assert_eq!(found.card.as_deref(), Some("DAC"));
    assert_eq!(found.sample_rate, 44_100);
    assert_eq!(found.bits, 32);
  }

  #[test]
  fn nothing_running_or_no_asound_directory_reports_nothing() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "card0/pcm0p/sub0/hw_params", "closed\n");
    assert_eq!(read_running(root.path()), None);
    assert_eq!(read_running(&root.path().join("missing")), None);
  }
}
