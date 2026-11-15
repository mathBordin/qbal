//! The artist page: the artist's name and two tabs in the title, `Top tracks`
//! and `Albums`, over the list the active tab shows.

use crate::core::app::{ActiveBlock, App};
use crate::core::artist::{discography_rows, DiscographyRow, Release, ReleaseKind};
use crate::core::format::millis_to_minutes;
use crate::tui::theme::EmphasisExt;
use ratatui::{
  layout::{Alignment, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Paragraph},
  Frame,
};

use super::util::{draw_selectable_list_titled, get_color, tab_title};

fn highlight_state(app: &App) -> (bool, bool) {
  let route = app.get_current_route();
  (
    route.active_block == ActiveBlock::Artist,
    route.hovered_block == ActiveBlock::Artist,
  )
}

/// `Daft Punk   Top tracks │ Albums`.
fn title(app: &App, name: &str) -> Line<'static> {
  let screens = &app.strings().screens;
  let state = highlight_state(app);
  let tabs = tab_title(
    app,
    &[screens.artist_top_tracks, screens.albums],
    usize::from(app.artist_shows_albums()),
    state,
  );
  let mut spans = vec![
    Span::styled(
      format!(" {name} "),
      get_color(state, app.user_config.theme)
        .add_modifier(app.user_config.behavior.emphasis(Modifier::BOLD)),
    ),
    Span::raw(" "),
  ];
  spans.extend(tabs.spans);
  Line::from(spans)
}

/// `Discovery (Deluxe Edition) · 2001 · 14 tracks · 24/96`.
fn release_row(app: &App, release: &Release) -> String {
  let mut parts = vec![release.full_title()];
  if let Some(year) = release.year() {
    parts.push(year.to_string());
  }
  if release.tracks_count > 0 {
    parts.push(app.strings().screens.track_count(release.tracks_count));
  }
  if let Some(quality) = release.quality() {
    parts.push(quality);
  }
  format!("  {}", parts.join(" \u{b7} "))
}

fn heading_row(app: &App, kind: ReleaseKind) -> String {
  let screens = &app.strings().screens;
  let label = match kind {
    ReleaseKind::Album => screens.albums,
    ReleaseKind::EpSingle => screens.artist_eps_singles,
    ReleaseKind::Other => screens.artist_other_releases,
  };
  format!("\u{2500}\u{2500} {label} \u{2500}\u{2500}")
}

pub fn draw_artist(f: &mut Frame<'_>, app: &App, area: Rect) {
  let Some(page) = app.artist_page() else {
    return;
  };
  let screens = &app.strings().screens;
  let title = title(app, &page.name);

  let (rows, selected, scroll, empty) = if app.artist_shows_albums() {
    let rows: Vec<String> = discography_rows(&page.releases)
      .into_iter()
      .map(|row| match row {
        DiscographyRow::Heading(kind) => heading_row(app, kind),
        DiscographyRow::Release(index) => release_row(app, &page.releases[index]),
      })
      .collect();
    (
      rows,
      app.view.artist_release_row,
      &app.view.artist_releases_scroll_offset,
      screens.artist_no_releases,
    )
  } else {
    let rows: Vec<String> = page
      .top_tracks
      .iter()
      .enumerate()
      .map(|(index, track)| {
        let mut row = format!("{:>2}. {}", index + 1, track.display_name(app.strings()));
        if !track.album.is_empty() {
          row = format!("{row} \u{b7} {}", track.album);
        }
        if track.duration_ms > 0 {
          row = format!(
            "{row} ({})",
            millis_to_minutes(u128::from(track.duration_ms))
          );
        }
        row
      })
      .collect();
    (
      rows,
      app.view.artist_track_index,
      &app.view.artist_tracks_scroll_offset,
      screens.artist_no_tracks,
    )
  };

  if rows.is_empty() {
    let state = highlight_state(app);
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .title(title)
      .border_style(get_color(state, app.user_config.theme));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let message = if page.loading {
      screens.artist_loading
    } else {
      empty
    };
    if inner.height > 0 {
      f.render_widget(
        Paragraph::new(message)
          .style(Style::default().fg(app.user_config.theme.inactive.into()))
          .alignment(Alignment::Center),
        Rect {
          y: inner.y + inner.height / 2,
          height: 1,
          ..inner
        },
      );
    }
    return;
  }

  draw_selectable_list_titled(
    f,
    app,
    area,
    title,
    &rows,
    highlight_state(app),
    Some(selected),
    scroll,
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::artist::{arrange_releases, ArtistPage, ArtistTab};
  use ratatui::{backend::TestBackend, Terminal};

  fn rendered(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
      .draw(|f| draw_artist(f, app, Rect::new(0, 0, width, height)))
      .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
      .map(|y| {
        (0..width)
          .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
          .collect::<String>()
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn page_app(tab: ArtistTab) -> App {
    let mut app = App::default();
    app.view.artist_tab = tab;
    app.open_artist("qobuz:artist:1".into(), "Daft Punk".into());
    let release = |id: &str, kind, date: &str| Release {
      uri: format!("qobuz:album:{id}"),
      title: id.into(),
      version: None,
      release_date: Some(date.into()),
      tracks_count: 14,
      kind,
      bit_depth: Some(24),
      sample_rate_khz: Some(96.0),
    };
    app.show_artist_page(ArtistPage {
      uri: "qobuz:artist:1".into(),
      name: "Daft Punk".into(),
      loading: false,
      top_tracks: Vec::new(),
      releases: arrange_releases(vec![
        release("Discovery", ReleaseKind::Album, "2001-03-12"),
        release("Get Lucky", ReleaseKind::EpSingle, "2013-04-19"),
      ]),
    });
    app
  }

  #[test]
  fn the_albums_tab_lists_releases_under_their_group_headings() {
    let content = rendered(&page_app(ArtistTab::Albums), 80, 8);
    assert!(content.contains("Daft Punk"), "{content}");
    assert!(content.contains("Top tracks"), "{content}");
    assert!(
      content.contains("\u{2500}\u{2500} Albums \u{2500}\u{2500}"),
      "{content}"
    );
    assert!(
      content.contains("Discovery \u{b7} 2001 \u{b7} 14 tracks \u{b7} 24/96"),
      "{content}"
    );
    assert!(content.contains("EPs & singles"), "{content}");
  }

  #[test]
  fn an_empty_tab_says_so_and_a_loading_page_says_it_is_loading() {
    let content = rendered(&page_app(ArtistTab::TopTracks), 80, 8);
    assert!(content.contains("No popular tracks found"), "{content}");

    let mut app = App::default();
    app.open_artist("qobuz:artist:1".into(), "Daft Punk".into());
    let content = rendered(&app, 80, 8);
    assert!(content.contains("Loading"), "{content}");
  }
}
