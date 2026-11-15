# Changelog

Qbal is a fork of [spotatui](https://github.com/LargeModGames/spotatui) v0.48.0.
Releases before 0.1.0 are spotatui's; see its repository for that history.

## [Unreleased]

### Added

- **Album search**: `Tab` on the search screen switches between songs and
  albums. `Enter` on an album plays it from the first track; `o` opens its
  track list instead. One search fills both lists, so switching is instant.
- **Audio format in the playbar**: the track's codec, bit depth, sample rate
  and average bitrate (`FLAC · 24-bit · 96 kHz · 3104 kbps`) on the title row,
  shortened to fit narrow terminals.
- **Output format in the playbar**: the format Qbal hands to the sound system,
  and the format the sound card runs at, read from `/proc/asound`
  (`Output: 32-bit float · 48 kHz · 2 ch → Card: 24-bit · 96 kHz`). When the two
  differ, PipeWire or PulseAudio is converting the audio.
- **Lua plugins**: `qbal.play_uri` also takes a `qobuz:album:` uri and plays
  the whole album.

## [0.1.0] 2026-11-14

### Changed

- **Qobuz only, in the terminal**: Qbal plays Qobuz in a terminal UI. Spotify,
  local files, Subsonic, internet radio, YouTube, Apple Music and the Soloist
  engine are gone, and so are the browser GUI, the desktop app and the mobile app.
- **Renamed to Qbal**: the command is `qbal`, settings live in `~/.config/qbal`
  and state in `~/.local/state/qbal`. On the first launch, Qbal copies the old
  `spotatui` folders there, so your settings, listening history and Qobuz login
  carry over. The old folders are left in place.
- **Plugin API renamed**: Lua plugins call `qbal.*` instead of `spotatui.*`, and
  environment variables start with `QBAL_` instead of `SPOTATUI_`.
- **The search screen is one list of songs**: Qobuz search returns tracks, so
  the empty album, artist, playlist and podcast panes are gone.
- **English, Portuguese (Brazil) and French**: the interface ships in these
  three languages and follows your system language by default.
- **Linux only**: the macOS and Windows integrations are removed.
- **Sorting works on Qobuz playlists**: `,` in a playlist sorts its tracks by
  name, artist, album, duration or release date, and `default_sort_playlist_tracks`
  applies when a playlist opens.
- **The playing track is marked** with `playing_icon` in track lists.
- **Lua plugins**: `qbal.queue()`, `qbal.playlists()` and `qbal.search_results()`
  return live data again, `qbal.search()` searches Qobuz, and `qbal.set_repeat()`
  sets the playlist's repeat mode. Functions for removed features
  (`get_saved_tracks`, `add_to_queue`, `transfer_playback`, …) are gone.
- **Stats**: Enter on a top track plays it.
- **Queue screen** shows what a playlist is playing, not only queued tracks.
- **The error screen** gives Qobuz advice instead of Spotify's.
- **The "Spotify" theme preset is now "Green"**; configs that name the old
  preset still load it.
- **French settings** use French punctuation (`Langue : …`), arrow keys are
  named in each language in the help, and the compact settings box fits its
  translated title.

### Removed

- The AI DJ and MCP server, the listening party, playlist sync, telemetry and
  the global song counter, and self-update.
- Settings that did nothing without Spotify: the shuffle, repeat, paused and
  active-source icons, `group_folders_first`, `stop_after_current_track` and the
  player-bar visualizer. The `album_tracks`, `albums`, `recently_played`,
  `podcasts` and `episodes` tables and the always-empty "liked" column.
- `format.playbar_status_source`: `format.playbar_status` is the one playbar
  template, and `{device}` and `{party}` are gone.
