use super::*;

/// One finished play since this process started, for the Session screen.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct SessionPlay {
  /// Unix milliseconds.
  pub started_at_ms: u64,
  pub ended_at_ms: u64,
  pub listened_ms: u64,
  /// 0 for a live stream.
  pub duration_ms: u32,
  pub title: String,
  pub artists: Vec<String>,
  pub album: String,
  /// The scheme names the source; `None` when the track has no URI.
  pub uri: Option<String>,
  pub image_url: Option<String>,
}

/// A process left running for weeks keeps its last plays only.
const SESSION_KEEP: usize = 500;

impl App {
  pub(crate) fn record_session_play(&mut self, play: SessionPlay) {
    if self.session_plays.len() == SESSION_KEEP {
      self.session_plays.remove(0);
    }
    self.session_plays.push(play);
  }
}
