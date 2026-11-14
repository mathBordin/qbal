use super::*;

impl App {
  /// The listening streak and its note for the Home page; `None` without a streak.
  pub fn welcome_streak(&self) -> Option<(String, String)> {
    let streaks = self.listening_streaks.as_ref()?;
    if streaks.current_days == 0 {
      return None;
    }
    let screens = &self.strings().screens;
    let note = if streaks.today_ms > 0 {
      screens.listened_today(&crate::infra::history::format_duration_with(
        streaks.today_ms,
        self.strings(),
      ))
    } else {
      screens.streak_keep.to_string()
    };
    Some((screens.listening_streak(streaks.current_days), note))
  }

  /// Open a Welcome page link in the system browser; any other target is ignored.
  pub(crate) fn open_changelog_link(&mut self, url: &str) {
    if !crate::core::changelog::opens_link(url) {
      log::warn!("ignored a Welcome page link that is not in the changelog");
      return;
    }
    self.open_external_url(url);
  }
}

#[cfg(test)]
mod tests {}
