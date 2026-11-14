//! The serial IoEvent pump: source routing by URI scheme and the service lane.
//! Every frontend drives `App` through this.

use crate::infra::network::{IoEvent, Network};

/// Signalled after the pump runs an event, so a frontend that pushes its
/// changes (the GUI) does not wait for its next tick to show the result.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub(super) fn events_handled() -> &'static tokio::sync::Notify {
  static HANDLED: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();
  HANDLED.get_or_init(tokio::sync::Notify::new)
}

// CLI mode never starts the pump; only a frontend launch does.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub(super) async fn start_tokio(io_rx: std::sync::mpsc::Receiver<IoEvent>, network: &mut Network) {
  // Bridge the sync dispatch channel onto the async runtime: a dedicated
  // thread does blocking recv() and forwards into a tokio channel the pump can
  // await.
  let (bridge_tx, mut bridge_rx) = tokio::sync::mpsc::unbounded_channel::<IoEvent>();
  std::thread::spawn(move || {
    while let Ok(io_event) = io_rx.recv() {
      if bridge_tx.send(io_event).is_err() {
        break;
      }
    }
  });

  while let Some(io_event) = bridge_rx.recv().await {
    // Source-agnostic service events (lyrics, cover art, telemetry,
    // announcements, stats/recap) only touch `App` and their own HTTP clients,
    // so they run concurrently on a detached task instead of queueing behind
    // transport events on this serial pump.
    if Network::runs_on_service_lane(&io_event) {
      let mut service_network = Network::new(&network.app);
      tokio::spawn(async move {
        service_network.handle_network_event(io_event).await;
        events_handled().notify_one();
      });
      continue;
    }

    // The queue router runs first: it owns `AdvanceNativeQueue` and the queue
    // slot's transport controls, and relinquishes the slot on an unrelated
    // `StartPlayback` (returning false so the Qobuz start still runs).
    let handled_queue =
      crate::infra::queue::dispatch::route_queue_event(&network.app, &io_event).await;
    let handled_qobuz = !handled_queue
      && crate::infra::qobuz::dispatch::route_qobuz_event(&network.app, &io_event).await;
    if !handled_queue && !handled_qobuz {
      network.handle_network_event(io_event).await;
    } else {
      // A router consumed the event and returned without touching
      // `is_loading`, which `App::dispatch` set to true. Only
      // `handle_network_event` resets it, so clear it here — otherwise loading
      // Qobuz content leaves the UI stuck on the loading indicator.
      let mut app = network.app.lock().await;
      app.is_loading = false;
      app.note_display_changes();
    }
    events_handled().notify_one();
  }
}
