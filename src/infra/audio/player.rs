//! Audio playback engine.
//!
//! [`LocalPlayer`] decodes an audio file or stream and plays it through the
//! system's default output device. The decoded audio reaches the same default
//! device the visualizer's monitor captures, so the visualizer needs no extra
//! wiring.
//!
//! ## Encapsulation
//!
//! All `rodio` types are contained inside this module. The public surface is a
//! small transport API (`play_file`/`pause`/`resume`/`seek`/`position`/
//! `is_finished`/`set_volume`) that speaks `std` types only. This keeps the
//! runtime free of `rodio`.
//!
//! ## Losing the output device
//!
//! Losing the device mid-track takes two shapes, and only one of them is an
//! error anybody reports. cpal notices the device being *removed* and reports
//! `DeviceNotAvailable`. It cannot notice the far more common case: the OS
//! moving its **default output** somewhere else — headphones unplugged, AirPods
//! back in their case — which leaves the stream happily bound to a device
//! nobody is listening to. So the sink also remembers its device's id,
//! and [`device_lost`](LocalPlayer::device_lost) compares that against the
//! current default.
//!
//! Either way no audio callback runs again, and that matters far beyond
//! silence: rodio's
//! `clear` and `try_seek` *wait on that callback* (`sleep_until_end`, and the
//! seek feedback channel, neither with a timeout), and the transport methods
//! here are called straight from the serial IoEvent pump. A blocked one wedges
//! the pump, and with it every unrelated thing behind it — searches included.
//!
//! So every method that would wait on the audio thread refuses once the device
//! is gone, and — because a detector that misses one day would freeze the app
//! again — the wait itself is bounded: an audio thread that does not answer
//! *is* the last-resort detector. The driver's tick notices, calls
//! [`LocalPlayer::reopen`] to rebuild the output on the new default device, and
//! restages the track there.
//!
//! ## Threading
//!
//! `rodio::MixerDeviceSink` is `!Send`, so it cannot live on the shared player
//! struct (which is held behind an `Arc` across async tasks). Instead a
//! dedicated thread owns the `MixerDeviceSink` and keeps it alive; the player
//! holds only the `Send + Sync` [`rodio::Player`] plus a keepalive channel whose
//! drop tells the thread to release the device.

use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rodio::{Decoder, Player};

use super::volume_gain;
use crate::core::stream_format::OutputFormat;

/// An audio player driving the system default output device.
///
/// Cheap to hold behind an `Arc`: the heavy `MixerDeviceSink` lives on a
/// dedicated thread, and the `Player` here is a lightweight `Arc`-backed handle.
pub struct LocalPlayer {
  /// Replaced wholesale by [`reopen`](LocalPlayer::reopen). Only ever locked to
  /// clone the `Arc` out, never held across a rodio call: `clear` waits on the
  /// audio thread while the render tick reads `position()` from another one.
  sink: Mutex<Sink>,
  /// The last percent handed to [`set_volume`](LocalPlayer::set_volume),
  /// re-applied to a reopened sink so a device change is not also a volume jump.
  volume_percent: AtomicU8,
  /// The cached answer of the default-output compare in
  /// [`device_lost`](LocalPlayer::device_lost).
  device_check: Mutex<Option<DeviceCheck>>,
  /// Reopen attempts since the device was lost.
  reopen: Mutex<ReopenState>,
  /// Bumped when a stage starts, so a stage that a later one overtook never
  /// touches the sink after it: two fetches can share one player and finish
  /// in the wrong order.
  stage_generation: AtomicU64,
  /// Serializes a stage's two sink transitions (the clear, and the append)
  /// with the generation check in front of each, so an overtaken stage can
  /// neither clear a newer track nor append behind it. The decode between
  /// them runs outside it.
  stage_transition: Mutex<()>,
}

/// One default-output read, keyed on the opened device's id so a swapped
/// sink is never answered from the old sink's cache.
struct DeviceCheck {
  checked_at: Instant,
  opened: String,
  moved: bool,
}

#[derive(Default)]
struct ReopenState {
  attempts: u8,
  last_attempt: Option<Instant>,
}

/// What [`LocalPlayer::recover_device`] did this call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reopen {
  /// A fresh sink is in place, paused and empty: restage onto it.
  Reopened,
  /// An attempt just failed; another follows after [`DEVICE_REOPEN_RETRY_AFTER`].
  Retrying,
  /// Nothing happened: the next attempt is not due yet, or the device was
  /// already given up on.
  Waiting,
  /// The last attempt failed. Reported once; the caller ends the session.
  GaveUp,
}

/// One open output: rodio's control handle, the audio thread's keepalive, and
/// the flag cpal's error callback raises when the device disappears.
struct Sink {
  player: Arc<Player>,
  /// Dropping this sender signals the audio thread to drop its `OutputStream`
  /// and release the audio device. Held for the sink's lifetime.
  _keepalive: mpsc::Sender<()>,
  /// Raised by our cpal error callback when the device is *removed*.
  lost: Arc<AtomicBool>,
  /// The key identifying the device this sink was opened on (its name - see
  /// `device_key`), so a default-output change can be
  /// spotted (see module docs). `None` when the key could not be read, which
  /// only disables the comparison — never fakes a change.
  device_name: Option<String>,
  /// The format the device was opened with.
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  output: OutputFormat,
}

/// How often a wait on the audio thread stops to re-ask whether the device is
/// still there. Well above a healthy `clear` (one buffer, tens of milliseconds),
/// short enough that a real disconnect is noticed while the user still connects
/// it to what they just did.
const AUDIO_THREAD_POLL: Duration = Duration::from_secs(3);

/// The absolute ceiling on such a wait. Above Qobuz's 60s stream stall so a
/// slow network is never mistaken for a dead device, and finite so an audio
/// thread that wedges for a reason nothing here can name still cannot take the
/// app with it.
const AUDIO_THREAD_CEILING: Duration = Duration::from_secs(90);

/// How long to wait for the audio thread to hand back an opened device.
/// `reopen` runs from the driver's tick, on the UI thread under the `App` lock,
/// so this wait cannot be unbounded either. A healthy open is tens of
/// milliseconds; this is a backstop, not a deadline anybody should reach.
const DEVICE_OPEN_TIMEOUT: Duration = Duration::from_secs(5);

/// How often [`LocalPlayer::device_lost`] re-reads the default output device.
/// The tick asks on every frame, and the read talks to the OS's audio APIs.
const DEVICE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// The spacing between reopen attempts, and how many are made before the
/// device is given up on. A Bluetooth output takes a few seconds to negotiate
/// after it comes back, so one failed open is not the answer.
const DEVICE_REOPEN_RETRY_AFTER: Duration = Duration::from_secs(2);
const DEVICE_REOPEN_ATTEMPTS: u8 = 5;

/// Whether another reopen attempt is due: not within
/// [`DEVICE_REOPEN_RETRY_AFTER`] of the last one, and never past
/// [`DEVICE_REOPEN_ATTEMPTS`].
fn reopen_due(attempts: u8, since_last: Option<Duration>) -> bool {
  attempts < DEVICE_REOPEN_ATTEMPTS && since_last.is_none_or(|d| d >= DEVICE_REOPEN_RETRY_AFTER)
}

/// A decoded stream, ready for [`LocalPlayer::play_prepared`].
pub struct PreparedStream(Box<dyn rodio::Source + Send>);

impl LocalPlayer {
  /// Open the default audio output device and return a ready player.
  ///
  /// **Blocking:** waits for the audio thread to open the device. Call once at
  /// setup, off any latency-sensitive path. Returns an error if no output
  /// device is available (e.g. headless CI).
  pub fn new() -> Result<Self> {
    let sink = open_sink()?;
    // Start silent; nothing is queued until the first `play_file`.
    sink.player.pause();
    Ok(Self {
      sink: Mutex::new(sink),
      volume_percent: AtomicU8::new(100),
      device_check: Mutex::new(None),
      reopen: Mutex::new(ReopenState::default()),
      stage_generation: AtomicU64::new(0),
      stage_transition: Mutex::new(()),
    })
  }

  /// The format the output device was opened with. Changes when the device is
  /// reopened on another one.
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub fn output_format(&self) -> OutputFormat {
    self.sink.lock().unwrap().output.clone()
  }

  /// Start a stage: the generation it must still hold at each sink transition.
  fn begin_stage(&self) -> u64 {
    self.stage_generation.fetch_add(1, Ordering::SeqCst) + 1
  }

  /// Clear the sink for `stage`, unless a later stage started since it began.
  fn clear_for_stage(&self, stage: u64, sink: &Arc<Player>) -> Result<()> {
    let _transition = self.stage_transition.lock().unwrap();
    if self.stage_generation.load(Ordering::SeqCst) != stage {
      anyhow::bail!("superseded by a later track");
    }
    let clearing = Arc::clone(sink);
    if self.bounded(move || clearing.clear()).is_none() {
      anyhow::bail!("audio output device stopped responding");
    }
    Ok(())
  }

  /// Append `source` for `stage`, unless a later stage started since it began
  /// or the driver's tick reopened the device under it: either way the sink
  /// it cleared is no longer the one to play through.
  fn append_for_stage(
    &self,
    stage: u64,
    sink: &Arc<Player>,
    source: impl rodio::Source + Send + 'static,
  ) -> Result<()> {
    let _transition = self.stage_transition.lock().unwrap();
    if !Arc::ptr_eq(sink, &self.player()) {
      anyhow::bail!("audio output device changed");
    }
    if self.stage_generation.load(Ordering::SeqCst) != stage {
      anyhow::bail!("superseded by a later track");
    }
    // The clear paused the sink; `append` leaves it so.
    sink.append(source);
    Ok(())
  }

  /// The rodio handle, for the calls that cannot block (they only touch
  /// atomics and short-lived mutexes, never the audio thread).
  fn player(&self) -> Arc<Player> {
    Arc::clone(&self.sink.lock().unwrap().player)
  }

  /// The rodio handle for calls that *do* wait on the audio thread, refusing
  /// once the device is gone — see the module docs: waiting there never
  /// returns, and would wedge the serial IoEvent pump rather than fall silent.
  fn live_player(&self) -> Result<Arc<Player>> {
    let sink = self.sink.lock().unwrap();
    if sink.lost.load(Ordering::Relaxed) {
      anyhow::bail!("audio output device disconnected");
    }
    Ok(Arc::clone(&sink.player))
  }

  /// Whether cpal reported the device *removed* (headphones unplugged, a USB
  /// DAC pulled), as against the OS merely moving its default output somewhere
  /// else. Recovery pauses only for removal: a device the user just *plugged
  /// in* should keep playing.
  pub fn device_removed(&self) -> bool {
    self.sink.lock().unwrap().lost.load(Ordering::Relaxed)
  }

  /// Whether the sink is still connected to something the user can hear. The
  /// driver's tick polls this to decide when to [`reopen`](Self::reopen).
  ///
  /// Two ways to fail (see module docs): the device was removed and cpal told
  /// us, or the OS quietly moved its default output elsewhere and nobody did.
  pub fn device_lost(&self) -> bool {
    // Read the sink's answer and release its lock: the default-output read
    // below queries the OS, and the render path waits on this mutex.
    let (removed, opened) = {
      let sink = self.sink.lock().unwrap();
      (sink.lost.load(Ordering::Relaxed), sink.device_name.clone())
    };
    if removed {
      return true;
    }
    // Only an id read successfully on *both* sides is evidence: a missing one
    // means "cannot tell", which must not read as "changed" — mid-switch there
    // is briefly no default device at all, and tearing playback down for that
    // would be worse than the silence this exists to catch.
    let Some(opened) = opened else {
      return false;
    };
    let mut check = self.device_check.lock().unwrap();
    if let Some(cached) = check.as_ref() {
      if cached.opened == opened && cached.checked_at.elapsed() < DEVICE_CHECK_INTERVAL {
        return cached.moved;
      }
    }
    let moved = default_output_key().is_some_and(|current| current != opened);
    *check = Some(DeviceCheck {
      checked_at: Instant::now(),
      opened,
      moved,
    });
    moved
  }

  /// Wait on the audio thread, but never forever.
  ///
  /// rodio's `clear` and `try_seek` block until the audio callback answers, and
  /// they run on the serial IoEvent pump: one that never returns takes every
  /// unrelated request behind it down too. `device_lost` normally catches a
  /// dead device before we get here — this is the backstop for a way of losing
  /// one that neither cpal nor the default-output check saw.
  ///
  /// A plain timeout would be wrong: a dead device and a source stalled on the
  /// network look identical from here (Qobuz gives its stream 60 seconds), and
  /// cutting the second one short would break slow playback to fix a freeze. So
  /// the wait re-*asks the device* every [`AUDIO_THREAD_POLL`] instead of
  /// guessing, gives up at once when it really went away, and only past
  /// [`AUDIO_THREAD_CEILING`] — nothing identifiably wrong, still no answer —
  /// declares it lost anyway, because a pump that never returns is worse than
  /// a track that never plays.
  ///
  /// A call that truly never returns leaves its worker parked for the life of
  /// the process. That is the cheap half of this trade.
  fn bounded<T: Send + 'static>(&self, op: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    // The sink this wait belongs to. `reopen` swaps in a whole new `Sink`, so
    // every check below must be about *this* one: a poll that read the fresh
    // sink would find it healthy, wait out the full ceiling, and then mark the
    // live sink lost.
    let (waited_on, lost) = {
      let sink = self.sink.lock().unwrap();
      (Arc::clone(&sink.player), Arc::clone(&sink.lost))
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
      let _ = tx.send(op());
    });

    let started = std::time::Instant::now();
    loop {
      match rx.recv_timeout(AUDIO_THREAD_POLL) {
        Ok(value) => return Some(value),
        // The worker vanished without answering; nothing left to wait for.
        Err(mpsc::RecvTimeoutError::Disconnected) => return None,
        Err(mpsc::RecvTimeoutError::Timeout) => {
          // Reopened under us: the caller's result would land on a sink nobody
          // plays through any more, and the driver is already restaging.
          if !Arc::ptr_eq(&waited_on, &self.player()) {
            return None;
          }
          if self.device_lost() {
            return None;
          }
          if started.elapsed() >= AUDIO_THREAD_CEILING {
            lost.store(true, Ordering::Relaxed);
            return None;
          }
        }
      }
    }
  }

  /// Rebuild the output on the *current* default device and drop the dead one.
  ///
  /// The new sink starts paused and empty, at the volume the old one had, so
  /// the caller restages whatever was playing (see the driver's tick). Dropping
  /// the old `Sink` releases its keepalive, which lets the old audio thread
  /// exit; `Player`'s own `Drop` only sets a flag, so nothing here blocks.
  ///
  /// **Blocking:** opens a device, like [`new`](Self::new).
  pub fn reopen(&self) -> Result<()> {
    let fresh = open_sink()?;
    // Configure before publishing: once swapped in, other threads can see it.
    fresh.player.pause();
    fresh
      .player
      .set_volume(volume_gain(self.volume_percent.load(Ordering::Relaxed)));
    *self.sink.lock().unwrap() = fresh;
    *self.device_check.lock().unwrap() = None;
    Ok(())
  }

  /// [`reopen`](Self::reopen) with retries, for the driver's tick: an attempt
  /// every [`DEVICE_REOPEN_RETRY_AFTER`] up to [`DEVICE_REOPEN_ATTEMPTS`], so a
  /// device that is still negotiating gets its chance, and one that never
  /// comes back is given up on exactly once.
  ///
  /// **Blocking:** an attempt opens a device, like [`new`](Self::new).
  pub fn recover_device(&self) -> Reopen {
    let attempts = {
      let mut state = self.reopen.lock().unwrap();
      if !reopen_due(state.attempts, state.last_attempt.map(|t| t.elapsed())) {
        return Reopen::Waiting;
      }
      state.attempts += 1;
      state.last_attempt = Some(Instant::now());
      state.attempts
    };
    match self.reopen() {
      Ok(()) => {
        *self.reopen.lock().unwrap() = ReopenState::default();
        Reopen::Reopened
      }
      Err(e) => {
        log::warn!("[audio] reopen attempt {attempts}: {e:#}");
        if attempts >= DEVICE_REOPEN_ATTEMPTS {
          Reopen::GaveUp
        } else {
          Reopen::Retrying
        }
      }
    }
  }

  /// [`play_file`](Self::play_file) without the final play: the track is
  /// queued and the sink left paused, so a caller that must seek first, or
  /// stay paused, never plays a burst of the track's start.
  pub fn stage_file(&self, path: &Path) -> Result<()> {
    let stage = self.begin_stage();
    let sink = self.live_player()?;
    // Stop whatever is currently playing *before* any fallible step (open or
    // decode), so a failure here can never leave the previous track audible. A
    // manual Next/Previous into a missing or undecodable file must fall silent:
    // `play_index`'s failure arm relies on the sink draining here so the runner
    // tick's `is_finished()` fires and auto-advance skips past the bad file
    // instead of dead-ending on a stale, still-playing track.
    self.clear_for_stage(stage, &sink)?;

    let file = std::fs::File::open(path)
      .with_context(|| format!("opening audio file {}", path.display()))?;

    // On decode error we return with the sink already empty (no old track
    // playing); on success we append the new source below. The decode is
    // long enough for the tick to have reopened the device, or for a later
    // stage to have taken the sink; `append_for_stage` refuses both.
    let decoder = Decoder::new(BufReader::new(file))
      .with_context(|| format!("decoding audio file {}", path.display()))?;

    self.append_for_stage(stage, &sink, decoder)
  }

  /// Build the decoder for a stream without a sink change, so the caller can
  /// decide under its own lock if the stream is still wanted.
  ///
  /// With `byte_len` the reader is seekable and the decoder knows the total
  /// size (a progressive download of a known file). Without it the reader is
  /// treated as non-seekable: the decoder is built with `with_seekable(false)`
  /// so the symphonia probe never issues the `Seek` that breaks on an infinite
  /// HTTP stream. The `Seek` bound is only there to satisfy
  /// rodio's type signature. A stream has no filename, so format detection is
  /// primed from `mime_type` (e.g. `"audio/mpeg"`) when available.
  ///
  /// **Blocking:** the probe reads from the network reader; call it off the
  /// async runtime (e.g. `spawn_blocking`) like `play_file`.
  pub fn prepare_stream<R>(
    reader: R,
    mime_type: Option<&str>,
    byte_len: Option<u64>,
  ) -> Result<PreparedStream>
  where
    R: std::io::Read + std::io::Seek + Send + Sync + 'static,
  {
    let mut builder = Decoder::builder()
      .with_data(reader)
      .with_seekable(byte_len.is_some());
    if let Some(len) = byte_len {
      builder = builder.with_byte_len(len);
    }
    if let Some(mime) = mime_type {
      builder = builder.with_mime_type(mime);
    }
    let decoder = builder
      .build()
      .map_err(|e| anyhow::anyhow!("decoding audio stream: {e}"))?;
    Ok(PreparedStream(Box::new(decoder)))
  }

  /// [`play_prepared`](Self::play_prepared) without the final play, like
  /// [`stage_file`](Self::stage_file).
  pub fn stage_prepared(&self, stream: PreparedStream) -> Result<()> {
    let stage = self.begin_stage();
    let sink = self.live_player()?;
    self.clear_for_stage(stage, &sink)?;
    self.append_for_stage(stage, &sink, stream.0)
  }

  /// Pause playback, keeping the current position.
  pub fn pause(&self) {
    self.player().pause();
  }

  /// Resume playback from the current position.
  pub fn resume(&self) {
    self.player().play();
  }

  /// Whether playback is currently paused.
  pub fn is_paused(&self) -> bool {
    self.player().is_paused()
  }

  /// Stop playback and discard the current source.
  ///
  /// After this, [`is_finished`](Self::is_finished) reports `true`. A no-op
  /// once the device is gone — the sink it would drain is already dead.
  pub fn stop(&self) {
    let Ok(sink) = self.live_player() else { return };
    self.bounded(move || sink.clear());
  }

  /// Stop on the blocking pool and return at once. `stop` waits for the audio
  /// thread, which a stalled network source holds until its stall timeout.
  pub fn stop_detached(self: std::sync::Arc<Self>) {
    tokio::task::spawn_blocking(move || self.stop());
  }

  /// [`stop_detached`](Self::stop_detached) with `session` kept alive until
  /// the stop returns, so a tempfile the sink still reads is not deleted
  /// under it.
  pub fn stop_detached_holding<S: Send + 'static>(self: std::sync::Arc<Self>, session: S) {
    tokio::task::spawn_blocking(move || {
      self.stop();
      drop(session);
    });
  }

  /// Set the output volume from the user's percent, on the logarithmic curve
  /// of [`volume_gain`](super::volume_gain).
  pub fn set_volume(&self, percent: u8) {
    self.volume_percent.store(percent, Ordering::Relaxed);
    self.player().set_volume(volume_gain(percent));
  }

  /// The playback position of the current source.
  pub fn position(&self) -> Duration {
    self.player().get_pos()
  }

  /// Whether the sink has no source playing — either nothing was ever played,
  /// or the current track played to completion (used to advance to the next
  /// track).
  pub fn is_finished(&self) -> bool {
    self.player().empty()
  }

  /// Seek to an absolute position within the current source.
  pub fn seek(&self, pos: Duration) -> Result<()> {
    let sink = self.live_player()?;
    match self.bounded(move || sink.try_seek(pos)) {
      Some(result) => result.map_err(|e| anyhow::anyhow!("seeking local audio: {e}")),
      None => anyhow::bail!("audio output device stopped responding"),
    }
  }
}

// ---------------------------------------------------------------------------
// Platform-specific output construction
// ---------------------------------------------------------------------------

/// Open the default output device on a dedicated thread and return the live
/// [`Sink`] (dropping its keepalive releases the device).
fn open_sink() -> Result<Sink> {
  use rodio::cpal::traits::HostTrait;
  use rodio::cpal::StreamError;
  use rodio::DeviceSinkBuilder;

  // Pick the device here rather than letting rodio do it, so the sink can
  // remember which one it got and notice the OS moving on from it later.
  let device = rodio::cpal::default_host()
    .default_output_device()
    .context("no audio output device available")?;
  let device_key = device_key(&device);
  let device_label = device_key.clone();

  let lost = Arc::new(AtomicBool::new(false));
  let on_error = {
    let lost = Arc::clone(&lost);
    move |err: StreamError| {
      // Ours only because rodio's default callback `eprintln!`s (its `tracing`
      // feature is off here) and raw stderr corrupts the TUI. The one error
      // worth recording is the device going away: cpal pauses the stream and
      // reports `DeviceNotAvailable`, after which no audio callback runs again
      // and every rodio call that waits on one would hang (see module docs).
      if matches!(err, StreamError::DeviceNotAvailable) {
        lost.store(true, Ordering::Relaxed);
      }
    }
  };

  let (init_tx, init_rx) = mpsc::channel::<std::result::Result<(Player, OutputFormat), String>>();
  let (keepalive_tx, keepalive_rx) = mpsc::channel::<()>();

  std::thread::Builder::new()
    .name("qbal-local-audio".to_string())
    .spawn(move || {
      // `open_default_sink()` would install rodio's `eprintln!` callback and
      // pick the device itself, so build the same thing by hand: the default
      // device, falling back to its other supported configs. Unlike rodio's
      // helper this does not then sweep every *other* output device — a machine
      // whose default output cannot be opened gets a clear error instead of
      // audio from a surprise device.
      let opened = DeviceSinkBuilder::from_device(device)
        .map(|builder| builder.with_error_callback(on_error))
        .and_then(|builder| builder.open_sink_or_fallback());
      match opened {
        Ok(mut stream) => {
          // rodio prints a drop warning for the `MixerDeviceSink` to stderr by
          // default (it has no `tracing` feature enabled here). Raw stderr
          // output corrupts the TUI, and the drop is deliberate anyway (device
          // handoff between sources tears the player down) — silence it.
          stream.log_on_drop(false);
          let sink = Player::connect_new(stream.mixer());
          let config = stream.config();
          let (bits, float) = sample_format_parts(config.sample_format());
          let output = OutputFormat {
            sample_rate: config.sample_rate().get(),
            channels: config.channel_count().get(),
            bits,
            float,
            device: device_label,
          };
          if init_tx.send(Ok((sink, output))).is_err() {
            return; // player was dropped before init completed
          }
          // Keep `stream` (and the device) alive until the player drops its
          // keepalive sender, at which point `recv` returns `Err` and we fall
          // through, dropping `stream` and releasing the device.
          let _ = keepalive_rx.recv();
        }
        Err(e) => {
          let _ = init_tx.send(Err(e.to_string()));
        }
      }
    })
    .context("spawning local audio output thread")?;

  let (player, output) = init_rx
    .recv_timeout(DEVICE_OPEN_TIMEOUT)
    .context("local audio output thread did not open a device")?
    .map_err(|e| anyhow::anyhow!("opening default audio output device: {e}"))?;

  Ok(Sink {
    player: Arc::new(player),
    _keepalive: keepalive_tx,
    lost,
    device_name: device_key,
    output,
  })
}

/// Bits per sample and whether they are floating point, for a cpal sample type.
fn sample_format_parts(format: rodio::cpal::SampleFormat) -> (u8, bool) {
  let bits = u8::try_from(format.bits_per_sample()).unwrap_or(u8::MAX);
  (bits, format.is_float())
}

/// The device's description; reading it has no side effect.
fn device_key(device: &rodio::Device) -> Option<String> {
  use rodio::DeviceTrait;
  device.description().ok().map(|d| d.name().to_string())
}

/// The identifying key of the output device the OS currently calls its
/// default, which is what a sink opened earlier is compared against.
fn default_output_key() -> Option<String> {
  use rodio::cpal::traits::HostTrait;
  rodio::cpal::default_host()
    .default_output_device()
    .as_ref()
    .and_then(device_key)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_first_reopen_is_due_at_once() {
    assert!(reopen_due(0, None));
  }

  #[test]
  fn sample_formats_report_their_width_and_kind() {
    use rodio::cpal::SampleFormat;
    assert_eq!(sample_format_parts(SampleFormat::F32), (32, true));
    assert_eq!(sample_format_parts(SampleFormat::I16), (16, false));
    assert_eq!(sample_format_parts(SampleFormat::I32), (32, false));
  }

  #[test]
  fn reopens_are_spaced_out() {
    assert!(!reopen_due(1, Some(DEVICE_REOPEN_RETRY_AFTER / 2)));
    assert!(reopen_due(1, Some(DEVICE_REOPEN_RETRY_AFTER)));
  }

  #[test]
  fn reopens_stop_after_the_attempt_budget() {
    let long_ago = Some(Duration::from_secs(60));
    assert!(reopen_due(DEVICE_REOPEN_ATTEMPTS - 1, long_ago));
    assert!(!reopen_due(DEVICE_REOPEN_ATTEMPTS, long_ago));
  }

  /// The detector that matters most in practice. cpal reports nothing when the
  /// OS moves its default output elsewhere — the stream stays bound to a device
  /// nobody can hear — so comparing against the current default is what catches
  /// headphones unplugged or AirPods going back in their case.
  #[test]
  #[ignore = "requires an audio output device"]
  fn a_default_output_change_reads_as_a_lost_device() {
    let player = LocalPlayer::new().expect("open default output device");
    assert!(
      !player.device_lost(),
      "a sink freshly opened on the current default is not lost"
    );

    // Stand in for the OS switching away: the sink holds the id of a device
    // that is no longer the default.
    player.sink.lock().unwrap().device_name = Some("a device that went away".to_string());
    assert!(
      player.device_lost(),
      "a default that moved is a lost device"
    );

    // An id we could not read means "cannot tell", never "changed": mid-switch
    // there is briefly no default at all, and tearing playback down for that
    // would be worse than the silence this exists to catch.
    player.sink.lock().unwrap().device_name = None;
    assert!(!player.device_lost(), "an unreadable id is not evidence");

    player.reopen().expect("reopen on the default device");
    assert!(
      !player.device_lost(),
      "reopen re-anchors the sink to the new default"
    );
  }

  /// The backstop, on the path that matters: the audio thread stops answering
  /// *because* the device went away. The wait must notice at its first check
  /// rather than sit out a stall timeout meant for slow networks — this is the
  /// difference between a stutter and the frozen app the user reported.
  #[test]
  #[ignore = "requires an audio output device"]
  fn a_wait_gives_up_as_soon_as_the_device_is_gone() {
    let player = LocalPlayer::new().expect("open default output device");
    // The device moved out from under the sink, as it does when headphones go.
    player.sink.lock().unwrap().device_name = Some("a device that went away".to_string());

    let started = std::time::Instant::now();
    let answered = player.bounded(|| std::thread::sleep(Duration::from_secs(120)));

    assert!(answered.is_none(), "the wait must give up, not block");
    assert!(
      started.elapsed() < AUDIO_THREAD_POLL + Duration::from_secs(2),
      "it must give up at the first check, got {:?}",
      started.elapsed()
    );
  }

  /// The reopen window. `reopen` swaps in a whole `Sink`, and a wait already
  /// running belongs to the old one: it must give up at once rather than sit
  /// out the ceiling and then mark the *live* sink lost — the driver is
  /// already restaging onto that sink.
  #[test]
  #[ignore = "requires an audio output device"]
  fn a_wait_gives_up_when_the_sink_is_reopened_under_it() {
    let player = Arc::new(LocalPlayer::new().expect("open default output device"));

    let waiting = Arc::clone(&player);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
      let gave_up = waiting
        .bounded(|| std::thread::sleep(Duration::from_secs(120)))
        .is_none();
      let _ = tx.send(gave_up);
    });

    // Reopen while that wait is parked, before its first poll.
    std::thread::sleep(AUDIO_THREAD_POLL / 2);
    player.reopen().expect("reopen on the default device");

    match rx.recv_timeout(AUDIO_THREAD_POLL + Duration::from_secs(2)) {
      Ok(gave_up) => assert!(gave_up, "a wait on a replaced sink must give up"),
      Err(_) => panic!("the wait outlived the sink it belonged to"),
    }
    assert!(
      !player.device_lost(),
      "the ceiling must never be charged to the fresh sink"
    );
  }
}
