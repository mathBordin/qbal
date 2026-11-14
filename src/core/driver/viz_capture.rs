//! The visualizer's audio capture, opened off the tick. Opening one waits on
//! the audio system (PipeWire's setup handshake, cpal's device scan), and the
//! tick runs under the `App` lock every frame, so the open runs on its own
//! thread and the tick only polls for the result. Backend-neutral: the
//! PipeWire and cpal capture managers both come through here.

use crate::core::app::App;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;

/// Shown once per visit to the visualizer when the capture cannot open.
#[cfg(test)]
pub(super) const AUDIO_CAPTURE_UNAVAILABLE: &str =
  "Visualizer can't capture system audio - see the log for details.";
const AUDIO_CAPTURE_UNAVAILABLE_TTL_SECS: u64 = 8;

enum Slot<C> {
  /// The visualizer is closed, or open with nothing tried yet.
  Closed,
  /// The open is running on its thread; it sends exactly one result.
  Opening(Receiver<Option<C>>),
  Open(C),
  /// Opening failed during this visit to the visualizer. Not retried until
  /// the visualizer closes; `reported` once the message is on screen.
  Failed {
    reported: bool,
  },
}

/// Clears the open flag when its open ends, by return or by panic.
struct OpenEnded(Arc<AtomicBool>);

impl Drop for OpenEnded {
  fn drop(&mut self) {
    self.0.store(false, Ordering::Release);
  }
}

/// One capture per visit to the visualizer: opened once, in the background,
/// and dropped when the visualizer closes.
pub(super) struct VizCapture<C> {
  slot: Slot<C>,
  /// An open thread still runs, maybe one a close abandoned. A device that
  /// hangs its open would otherwise pile up one stuck thread per try.
  opening: Arc<AtomicBool>,
}

impl<C: Send + 'static> VizCapture<C> {
  pub(super) fn new() -> Self {
    Self {
      slot: Slot::Closed,
      opening: Arc::new(AtomicBool::new(false)),
    }
  }

  /// One tick while the visualizer shows; never blocks. Starts `open` on a
  /// background thread if nothing was tried yet this visit, installs its
  /// result once it arrives, and keeps offering the failure message until
  /// the status bar takes it (a live error status drops plain messages).
  pub(super) fn poll(
    &mut self,
    app: &mut App,
    open: impl FnOnce() -> Option<C> + Send + 'static,
  ) -> Option<&C> {
    if matches!(self.slot, Slot::Closed) && !self.opening.load(Ordering::Acquire) {
      let (tx, rx) = mpsc::sync_channel(1);
      let opening = Arc::clone(&self.opening);
      opening.store(true, Ordering::Release);
      let spawned = thread::Builder::new()
        .name("audio-viz-open".into())
        .spawn(move || {
          // Cleared before the result goes out, so a poll that has it can
          // open again; also cleared when `open` panics.
          let capture = {
            let _ended = OpenEnded(opening);
            open()
          };
          // The receiver is gone if the visualizer closed meanwhile; the
          // capture is then dropped here.
          let _ = tx.send(capture);
        });
      self.slot = match spawned {
        Ok(_) => Slot::Opening(rx),
        Err(e) => {
          self.opening.store(false, Ordering::Release);
          log::error!("[audio-viz] could not start the capture thread: {e}");
          Slot::Failed { reported: false }
        }
      };
    }

    if let Slot::Opening(rx) = &self.slot {
      match rx.try_recv() {
        Ok(Some(capture)) => self.slot = Slot::Open(capture),
        // Logged once, on the transition: the status message points at the
        // log, and the cpal backend returns None without logging anything.
        Ok(None) => {
          log::warn!(
            "[audio-viz] the audio capture could not be opened; the visualizer has no audio"
          );
          self.slot = Slot::Failed { reported: false };
        }
        Err(TryRecvError::Disconnected) => {
          log::warn!(
            "[audio-viz] the capture thread ended without a result; the visualizer has no audio"
          );
          self.slot = Slot::Failed { reported: false };
        }
        Err(TryRecvError::Empty) => {}
      }
    }

    if let Slot::Failed { reported: false } = self.slot {
      app.set_status_message(
        app.strings().status.app.capture_unavailable,
        AUDIO_CAPTURE_UNAVAILABLE_TTL_SECS,
      );
      let shown = app.status_message() == Some(app.strings().status.app.capture_unavailable)
        && !app.status_message_is_error();
      self.slot = Slot::Failed { reported: shown };
    }

    self.current()
  }

  /// The visualizer closed: drop the capture (or abandon a pending open) and
  /// forget a failure, so the next visit tries again. True when a capture
  /// was open.
  pub(super) fn close(&mut self) -> bool {
    matches!(
      std::mem::replace(&mut self.slot, Slot::Closed),
      Slot::Open(_)
    )
  }

  /// The capture, once it is open.
  pub(super) fn current(&self) -> Option<&C> {
    match &self.slot {
      Slot::Open(capture) => Some(capture),
      Slot::Closed | Slot::Opening(_) | Slot::Failed { .. } => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn audio_capture_status_fixture_matches_the_english_catalog() {
    assert_eq!(
      AUDIO_CAPTURE_UNAVAILABLE,
      crate::core::i18n::EN.status.app.capture_unavailable
    );
  }
}
