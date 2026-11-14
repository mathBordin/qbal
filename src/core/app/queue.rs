use super::*;

/// Domain-level representation of the playback queue.
/// Replaces `rspotify::model::CurrentUserQueue` in `App` state.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueState {
  pub currently_playing: Option<PlayableInfo>,
  pub queue: Vec<PlayableInfo>,
}

impl App {
  /// Add a track to the native cross-source queue.
  ///
  /// Rejects tracks with no URI and radio streams (a live stream is not a finite
  /// track). Spotify tracks on an external Connect device keep today's Web-API
  /// queue behavior (there is no native sink to play them through); everything
  /// else is pushed onto [`Self::native_queue`].
  pub fn add_track_to_native_queue(&mut self, track: TrackInfo) {
    // Music keeps its own queue, which spotatui can't reach.

    let Some(uri) = track.uri.clone() else {
      self.set_status_message(self.strings().status.app.queue_no_uri, 3);
      return;
    };

    let name = track.display_name(self.strings()).to_owned();
    self.native_queue.push(track);
    info!(
      "queued {uri} at position {} of the native queue",
      self.native_queue.len()
    );
    self.set_status_message(self.strings().status.app.queued(&name), 3);
  }

  /// The position while its URI still matches, else the first item with that
  /// URI (the queue can hold duplicates).
  fn queue_position_for(&self, uri: &str, position: usize) -> Option<usize> {
    if self
      .native_queue
      .get(position)
      .and_then(|t| t.uri.as_deref())
      == Some(uri)
    {
      return Some(position);
    }
    self
      .native_queue
      .iter()
      .position(|t| t.uri.as_deref() == Some(uri))
  }

  /// Jump to a queued item: drop everything before it and hand the sink to
  /// the queue, suspending a playing context mid-track first.
  pub(crate) fn play_queue_item(&mut self, uri: &str, position: usize) {
    let Some(skip) = self.queue_position_for(uri, position) else {
      return;
    };
    // Drop the items before the selected one so it becomes the queue head.
    self.native_queue.drain(..skip);
    if !self.queue_owns_playback() {
      self.suspend_active_decoded_context_mid_track();
    }
    self.dispatch(IoEvent::AdvanceNativeQueue);
  }

  /// Remove one native-queue item; a no-op when it is not in the queue.
  pub(crate) fn remove_queue_item(&mut self, uri: &str, position: usize) {
    if let Some(idx) = self.queue_position_for(uri, position) {
      self.native_queue.remove(idx);
    }
  }

  /// Move one native-queue item to index `to`; a no-op when out of range.
  pub(crate) fn move_queue_item(&mut self, uri: &str, from: usize, to: usize) {
    let Some(from) = self.queue_position_for(uri, from) else {
      return;
    };
    if to >= self.native_queue.len() {
      return;
    }
    let item = self.native_queue.remove(from);
    self.native_queue.insert(to, item);
  }

  /// Empty the native queue; the playing slot and a suspended context stay.
  pub(crate) fn clear_native_queue(&mut self) {
    self.native_queue.clear();
  }

  /// Whether the native queue's playback slot currently owns the output (either a
  /// decoded queued track or a native-streamed Spotify one). The single gated
  /// entry point for every queue-ownership check; reduces to `false` in a build
  /// where the slot cannot exist (slim, or one whose only decoded source is
  /// internet radio, which is never queueable).
  pub fn queue_owns_playback(&self) -> bool {
    {
      self.queue_now.is_some()
    }
  }

  /// Whether the queue slot is playing a Spotify track via native streaming.
  /// While true, librespot owns the sink and any still-`Some` decoded
  /// `*_playback` struct is a *suspended* context that must stay invisible to
  /// the decoded-ownership predicates (`active_decoded_source`,
  /// `active_decoded_player`, transport routing) — otherwise a space-bar toggle
  /// or media key would resume the suspended player on top of librespot.
  pub(crate) fn queue_now_is_spotify(&self) -> bool {
    {
      false
    }
  }

  /// The queue slot's player when it is playing a *decoded* queued track (local /
  /// Subsonic / Qobuz / YouTube). `None` for a Spotify slot or an empty slot. Gated
  /// on exactly those four sources: they are the decoded ones a queue item can
  /// name, so a build whose only decoded source is internet radio has no
  /// decoded slot to look up.
  pub fn queue_now_decoded_player(&self) -> Option<&Arc<crate::infra::audio::LocalPlayer>> {
    match self.queue_now.as_ref()? {
      QueueNowPlaying::Decoded(d) => Some(&d.player),
    }
  }

  /// Take the queue slot, returning its player when it was a decoded track (so
  /// the caller can stop it). Clears [`Self::queue_now`] either way.
  pub fn take_queue_now_decoded_player(&mut self) -> Option<Arc<crate::infra::audio::LocalPlayer>> {
    match self.queue_now.take() {
      Some(QueueNowPlaying::Decoded(d)) => Some(d.player),
      _ => None,
    }
  }

  /// The track currently playing through the queue slot, if any. Used by
  /// persistence to prepend it back onto the saved queue so a mid-queue quit
  /// doesn't lose the in-flight track.
  pub(super) fn queue_now_track(&self) -> Option<&TrackInfo> {
    {
      match self.queue_now.as_ref()? {
        QueueNowPlaying::Decoded(d) => Some(&d.track),
      }
    }
  }

  /// A `"{name} - {artists}"` label for the track playing through the queue slot,
  /// for the Queue screen's "Now playing" row. `None` when the slot is empty.
  pub fn queue_now_display(&self) -> Option<String> {
    let track = self.queue_now_track()?;
    Some(format!(
      "{} - {}",
      track.display_name(self.strings()),
      track.artists.join(", ")
    ))
  }
}

#[cfg(test)]
mod tests {}
