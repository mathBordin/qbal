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
}

#[cfg(test)]
mod tests {}
