pub mod requests;
pub mod utils;

use crate::core::app::App;
use anyhow::anyhow;
use std::sync::Arc;
use tokio::sync::Mutex;

use self::utils::UtilsNetwork;

// Most variants are constructed by TUI handlers, so a headless build counts
// them as dead; every tui-enabled CI leg lints the enum in full.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub enum IoEvent {
  /// Context URI (playlist/album), specific playable URIs, start offset.
  StartPlayback(Option<String>, Option<Vec<String>>, Option<usize>),
  Seek(u32),
  NextTrack,
  PreviousTrack,
  ForcePreviousTrack,
  PausePlayback,
  ChangeVolume(u8),
  /// Advance the native queue: play the next queued item, or resume the
  /// suspended playlist when the queue drains. Consumed by
  /// `infra::queue::dispatch::route_queue_event` (wired first in the pump); it
  /// never reaches the network handler.
  AdvanceNativeQueue,
  /// Give up the native queue slot without playing anything else: run the same
  /// teardown a drained queue does (stop the slot, resume the suspended
  /// context). Dispatched by the driver's tick when the slot's output device is
  /// gone and no replacement will open. Consumed by
  /// `infra::queue::dispatch::route_queue_event`; it never reaches the network handler.
  FinishNativeQueue,
  /// Replay the current track of the active decoded source (repeat-one). Consumed
  /// by `route_qobuz_event`; dispatched by the runner tick.
  ReplayCurrentTrack,
  FetchAnnouncements,
  /// Title, artists, duration in seconds, and an optional catalog id.
  GetLyrics(String, Vec<String>, f64),
  /// Aggregate local listening history for the Stats screen
  LoadListeningStats(crate::infra::history::RecapPeriod),
  /// Export the shareable HTML recap and open it in the browser
  GenerateRecap(crate::infra::history::RecapPeriod),
  /// List the Qobuz sidebar rows: favorites, playlists, albums, handled by
  /// `infra::qobuz::dispatch`.
  GetQobuzPlaylists,
  /// List the tracks of a Qobuz sidebar row by its `qobuz:` URI., handled by
  /// `infra::qobuz::dispatch`.
  GetQobuzTracks(String),
  /// Run a Qobuz catalog search and populate `app.search_results`., handled by
  /// `infra::qobuz::dispatch`.
  GetQobuzSearchResults(String),
  /// Play a whole Qobuz album from its first track, by its `qobuz:album:` URI;
  /// handled by `infra::qobuz::dispatch`.
  PlayQobuzAlbum(String),
  /// Start the in-TUI Qobuz browser login (handled by `infra::qobuz::dispatch`).
  QobuzLogin,
  /// Fetch and decode the current track's cover art (album-art URL, source
  /// thumbnail, or a local file's embedded picture). Dispatched by the shared
  /// track-change detector; handled off the `App` lock so the render loop never
  /// blocks on the download/decode.
  #[cfg(feature = "art-decode")]
  FetchCoverArt(crate::core::art::CoverArtRequest),
}

/// Runs the events no source router consumed: the service requests (lyrics,
/// cover art, telemetry, announcements, stats) and the transport events that
/// arrive while nothing is playing.
pub struct Network {
  pub app: Arc<Mutex<App>>,
}

impl Network {
  pub fn new(app: &Arc<Mutex<App>>) -> Self {
    Network {
      app: Arc::clone(app),
    }
  }

  /// Events that run on the concurrent service lane in `start_tokio`. Their
  /// handlers touch only `self.app` (plus their own HTTP clients /
  /// `spawn_blocking`), so they can execute on a detached task with a
  /// throwaway `Network` instead of head-of-line-blocking the serial pump.
  pub fn runs_on_service_lane(io_event: &IoEvent) -> bool {
    #[cfg(feature = "art-decode")]
    if matches!(io_event, IoEvent::FetchCoverArt(_)) {
      return true;
    }
    matches!(io_event, |IoEvent::FetchAnnouncements| IoEvent::GetLyrics(
      ..
    )
      | IoEvent::LoadListeningStats(_)
      | IoEvent::GenerateRecap(_))
  }

  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      // Transport and Qobuz events are consumed by the queue and Qobuz routers
      // in the pump; they only land here when nothing is playing.
      IoEvent::FetchAnnouncements => {
        self.fetch_announcements().await;
      }
      IoEvent::GetLyrics(track, artists, duration) => {
        self.get_lyrics(track, artists, duration).await;
      }
      #[cfg(feature = "art-decode")]
      IoEvent::FetchCoverArt(request) => {
        self.fetch_cover_art(request).await;
      }
      IoEvent::LoadListeningStats(period) => {
        self.load_listening_stats(period).await;
      }
      IoEvent::GenerateRecap(period) => {
        self.generate_recap(period).await;
      }
      // Transport, Qobuz and native-queue events are consumed by the routers
      // in the pump before they ever reach this handler.
      _ => {}
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  #[cfg(feature = "art-decode")]
  async fn fetch_cover_art(&mut self, request: crate::core::art::CoverArtRequest) {
    use crate::core::art::{fetch_and_decode, CoverArtRequest, CoverArtStatus};

    let key = request.key().to_string();

    // Skip the download/decode when we already hold art for this exact key
    // (e.g. consecutive tracks that share an album cover): just mark it loaded.
    {
      let mut app = self.app.lock().await;
      if app.desired_cover_art_key.as_deref() != Some(key.as_str()) {
        return;
      }
      if app.cover_art.key() == Some(key.as_str()) {
        app.cover_art.status = CoverArtStatus::Loaded;
        return;
      }
    }

    let result = match request {
      CoverArtRequest::Url(url) => fetch_and_decode(&url).await,
    };

    // Dominant-color extraction for the adaptive theme: one pass over a 64x64
    // thumbnail, but still done here, off the App lock, like the decode above.
    let palette = result
      .as_ref()
      .ok()
      .and_then(crate::core::cover_theme::extract_palette);

    let mut app = self.app.lock().await;
    if app.desired_cover_art_key.as_deref() != Some(key.as_str()) {
      return;
    }
    match result {
      Ok(img) => {
        app.store_cover_art(key, img, palette);
        app.cover_art.status = CoverArtStatus::Loaded;
      }
      Err(err) => {
        log::warn!("cover art load failed: {err}");
        // Drop any stale art so the pane shows the "unavailable" placeholder
        // rather than the previous track's image.
        app.clear_cover_art();
        app.cover_art.status = CoverArtStatus::Failed;
      }
    }
  }
  async fn load_listening_stats(&mut self, period: crate::infra::history::RecapPeriod) {
    use crate::infra::history;

    let strings = self.strings().await;
    let result = tokio::task::spawn_blocking(move || {
      let listens = history::load_listens()?;
      let filtered = history::filter_listens_for_period(&listens, period);
      Ok::<_, anyhow::Error>((
        history::build_stats_data_with(&filtered, strings),
        history::compute_streaks(&listens),
      ))
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|result| result);

    let mut app = self.app.lock().await;
    // A language change queues a fresh load. Neither data nor failure from
    // the superseded load may finish that new request's loading lifecycle.
    if !std::ptr::eq(strings, app.strings()) {
      return;
    }
    match result {
      Ok((stats, streaks)) => {
        app.listening_streaks = Some(streaks);
        app.land_listening_stats(period, stats);
      }
      Err(error) => {
        app.fail_listening_stats();
        {
          let strings = app.strings();
          app.handle_error_with_message(
            anyhow!("failed to load listening history: {}", error),
            strings.status.infra.history_failed(&error.to_string()),
          )
        };
      }
    }
  }

  /// Export the shareable HTML recap off the runtime threads and open it in
  /// the browser; reads only local history.
  async fn generate_recap(&mut self, period: crate::infra::history::RecapPeriod) {
    use crate::infra::history;

    let strings = crate::infra::app_strings(&self.app).await;
    let result = tokio::task::spawn_blocking(move || {
      let output_path = history::recap_output_path()?;
      let count = history::export_history_recap_with(period, &output_path, strings)?;
      Ok::<_, anyhow::Error>((output_path, count))
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|result| result);

    match result {
      Ok((output_path, count)) => {
        if let Err(e) = open::that_detached(&output_path) {
          log::warn!("failed to open recap in browser: {}", e);
        }
        let mut app = self.app.lock().await;
        let strings = app.strings();
        app.set_status_message(
          strings
            .status
            .infra
            .recap_created(&output_path.display().to_string(), count),
          5,
        );
      }
      Err(error) => {
        let mut app = self.app.lock().await;
        let strings = app.strings();
        app.set_status_message(strings.status.infra.recap_failed(&error.to_string()), 5);
      }
    }
  }

  /// Snapshot the active table after releasing the app lock.
  async fn strings(&self) -> &'static crate::core::i18n::Strings {
    crate::infra::app_strings(&self.app).await
  }
}
