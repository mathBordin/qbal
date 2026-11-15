# Configuration

Qbal reads `~/.config/qbal/config.yml` (or `$XDG_CONFIG_HOME/qbal/config.yml`).
Point it at another file with `--config <path>`.

Every field is optional; anything you leave out keeps its default. A commented
example lives in [`examples/config.example.yml`](../examples/config.example.yml).

Most simple values (numbers, toggles, icons, positions, colours, keys) can also
be changed in the **Settings** screen (`Alt+,`). Press `/` there to filter the
rows by name or by their key in this file. Saving from Settings rewrites the
`behavior`, `theme` and `keybindings` sections; your `format`, `tables` and
`plugin_commands` sections are kept as they are. Edit the file while Qbal is
closed.

Values Qbal changes as it runs (volume, shuffle, pane sizes, seen
announcements) are not in this file: they live in
`~/.local/state/qbal/state.yml`, next to the listening history and the last
session.

## Safe by default

An invalid value logs a warning and falls back to its default; a misspelled key
is ignored. Warnings go to the log file whose path Qbal prints at startup
(see [Logging](#logging)).

Qbal refuses to start only on a YAML syntax error, or on a value that is out of
range in a way it cannot guess around: `volume_increment` outside 0–100, or a
tick rate outside 1–999 ms.

## Interface language

```yaml
behavior:
  language: auto   # auto | en | pt-BR | fr
```

`auto` (the default) follows the first non-empty `LC_ALL`, `LC_MESSAGES` or
`LANG`: a Portuguese locale picks Portuguese (Brazil), a French one picks
French, anything else English. Spellings such as `pt_BR`, `pt` or `fr_FR` are
accepted. Change it live in Settings; the system locale is read once at
startup.

Translations keep product names, key names, configuration keys and your music's
titles as they are. Command-line output and logs stay in English.

## Behavior

```yaml
behavior:
  seek_milliseconds: 5000            # seek step for < and >
  volume_increment: 10               # step for + and - (0..=100)
  volume_percent: 100                # starting volume, until Qbal has saved one
  tick_rate_milliseconds: 250        # UI event-loop cadence (1..=999)
  animation_tick_rate_milliseconds: 16  # animation cadence (1..=999)
  status_message_ttl_percent: 100    # how long status messages stay (10..=1000)
  table_scroll_padding: 5            # rows kept visible below the cursor
```

| Key | Default | What it does |
| --- | --- | --- |
| `enable_text_emphasis` | `true` | Bold and italic text. |
| `show_loading_indicator` | `true` | Show a spinner while something loads. |
| `disable_mouse_inputs` | `false` | Keyboard only. |
| `set_window_title` | `true` | Put the playing track in the terminal title (see [`window_title`](#window_title)). |
| `enable_lyrics` | `true` | Look up lyrics on [LRCLIB](https://lrclib.net), which receives the title, artist and length. |
| `enable_announcements` | `true` | Show one-time notices from the project's announcement feed. |
| `announcement_feed_url` | unset | Read announcements from another HTTPS URL. |
| `enable_monthly_recap_prompt` | `true` | Offer your monthly listening recap once a month. |
| `keepawake_enabled` | `true` | Keep the computer awake while music plays. |
| `enable_media_keys` | `true` | Let media keys and `playerctl` control Qbal (MPRIS). |
| `enforce_wide_search_bar` | `false` | Always give the search box its own full-width row. |
| `banner_gradient` | `true` | Animated colour gradient on the welcome banner; `false` uses the theme's banner colour. The Terminal (ANSI) theme defaults to `false`. |
| `startup_behavior` | `continue` | When the last session is restored: `continue` keeps it playing or paused as it was, `play` always plays, `pause` always pauses. |

## Qobuz

The first time Qbal needs your library, it opens the Qobuz sign-in page in your
browser (or shows its address if no browser opens). The token is kept in
`~/.config/qbal/qobuz_credentials.yml`.

```yaml
behavior:
  qobuz_quality: 27  # 5 = MP3 320, 6 = FLAC 16/44.1, 7 = FLAC 24/96, 27 = FLAC 24/192 (the most a track has)
```

This is the quality Qbal asks for. Qobuz serves some tracks in a lower one, so
the playbar shows what it actually delivered, for example `FLAC 16/44.1`.

Tracks play while they download, and a seek past the downloaded part continues
the download from there.

### Best audio quality

Qbal plays FLAC without changing it: the samples are decoded exactly, nothing
processes them, and at 100% volume none of them is scaled. It also opens the
sound output at each track's own sample rate, so it never resamples.

What happens next is up to PipeWire. By default it runs at 48 kHz and resamples
anything else (with a good resampler). To let your sound card follow each
track's rate instead (44.1, 96, 192 kHz…), allow those rates:

```bash
mkdir -p ~/.config/pipewire/pipewire.conf.d
cat > ~/.config/pipewire/pipewire.conf.d/qbal-rates.conf <<'CONF'
context.properties = {
    default.clock.allowed-rates = [ 44100 48000 88200 96000 176400 192000 ]
}
CONF
systemctl --user restart pipewire pipewire-pulse wireplumber
```

The playbar's output line then shows the card at the track's rate, for example
`Output: 32-bit float · 96 kHz · 2 ch → USB DAC: 24-bit · 96 kHz`. PipeWire only
switches the rate while nothing else is playing, and the card keeps to the
rates it supports. Bluetooth is different: the codec (LDAC, aptX, AAC, SBC)
decides the quality there.

Environment variables:

- `QBAL_QOBUZ_TOKEN` replaces the saved token.
- `QBAL_QOBUZ_APP_ID`, `QBAL_QOBUZ_APP_SECRET` and `QBAL_QOBUZ_OAUTH_KEY`
  (all three together) replace the values Qbal otherwise reads from the Qobuz
  web player. Use them if Qobuz changes its web player and signing in stops
  working.

## Startup screen and sorting

```yaml
behavior:
  startup_route: home                    # home | stats
  default_sort_playlist_tracks: default  # see below
```

`default_sort_playlist_tracks` is `"<field>"` for ascending or `"<field>:desc"`
for descending. Fields: `default` (the playlist's own order), `name`, `artist`,
`album`, `duration`, `release_date`. Press `,` in a track list to sort it
differently for the session.

## Resuming the last session

When you quit while something plays, Qbal restores that playlist, track and
position on the next launch, following `startup_behavior`.

## Discord Rich Presence

```yaml
behavior:
  enable_discord_rpc: true
  discord_rpc_client_id: "your_application_id"
```

Discord shows the name of the Discord application Qbal reports to. The built-in
one belongs to spotatui, so your status says "spotatui" until you create your own
application in the [Discord Developer Portal](https://discord.com/developers/applications),
name it Qbal, and put its application id in `discord_rpc_client_id` (or in the
`QBAL_DISCORD_APP_ID` environment variable).

## Audio visualizer

Press `v` for the visualizer and `V` to switch between Cava, Bar Graph, Head
Unit, Analog VU, Winamp, Phosphor Scope, Outrun, Rack EQ and Cassette Deck.

```yaml
behavior:
  visualizer_style: Cava   # Cava, BarGraph, HeadUnit, AnalogVu, Winamp, PhosphorScope, Outrun, RackEq, Cassette
```

The visualizer listens to what your speakers play, so it also reacts to videos
and other sounds. A build with the `audio-viz` feature reads it from PipeWire;
otherwise Qbal uses an ALSA or PulseAudio monitor device (see
[Installation](installation.md#build-features)).

## Cover art

Needs a build with the `cover-art` feature.

| Key | Default | What it does |
| --- | --- | --- |
| `draw_cover_art` | `true` | Show the cover in the playbar and the cover view (`G`). |
| `draw_cover_art_forced` | `false` | Draw it with coloured characters on terminals that cannot show images. |
| `playbar_cover_art_size_percent` | `100` | Size of the playbar cover (up to `200`; the playbar height still limits it). |
| `cover_art_theme` | `false` | Tint the interface with the cover's colours, fading on track change. |
| `cover_art_dither` | `false` | Draw the cover as a two-colour dithered image. |
| `cover_art_dither_algorithm` | `stucki` | `stucki`, `bayer8x8` or `atkinson`. |
| `cover_art_dither_pixel_scale` | `1` | `1` to `3`; higher values make larger visual pixels. |

The dither colour is set under `theme`:

```yaml
theme:
  cover_art_dither_color: auto            # follows the progress-bar colour
  # cover_art_dither_color: '120, 160, 220'
```

## Listening history

Qbal keeps a local play history in `~/.local/state/qbal/history/listens.jsonl`.
It powers the Stats screen and `qbal history recap`. Short or skipped plays are
stored but left out of the totals.

## Layout

```yaml
behavior:
  sidebar_position: left      # left | right | hidden
  playbar_position: bottom    # bottom | top
  sidebar_width_percent: 20   # starting width, until you resize it
  library_height_percent: 30  # starting library height, until you resize it
  playbar_height_rows: 6      # starting playbar height (0..=50, 0 hides it)
  small_terminal_width: 150
  small_terminal_height: 45
```

- `hidden` gives the content the full width; the sidebar comes back while the
  Library or Playlists panel has focus.
- Resize with `{` / `}` (sidebar), `(` / `)` (playbar, or the library split
  while hovering Library or Playlists) and `|` to reset. Sizes you set this way
  are saved and win over the values above.
- Below `small_terminal_width` columns the search box gets its own row.

### Detached panes

The lyrics, visualizer, cover art, queue and mini player can stay on screen
beside the main layout. Open one of them and press `Alt+d`.

| Key | Does |
| --- | --- |
| `Alt+d` | Detach the open view into a pane, or put the focused pane back to full screen |
| `Alt+x` | Close the focused pane, or hide the main layout when it has focus |
| `Alt+h` / `Alt+j` / `Alt+k` / `Alt+l` (or `Alt` + arrow) | Focus the pane to the left, below, above, to the right |
| `Alt+H` / `Alt+J` / `Alt+K` / `Alt+L` (or `Alt+Shift` + arrow) | Swap the focused pane with that neighbour |
| `Alt+r` | Switch the focused split between side by side and stacked |
| `Alt+=` / `Alt+-` | Grow or shrink the focused pane |
| `Alt+0` | Close every pane |

A focused pane gets its view's keys (`V` still cycles visualizer styles); `Esc`
or the back key returns focus to the main layout. With the mouse, a click
focuses a pane, `[x]` closes it, and dragging the line between two panes
resizes them. The layout is saved as `pane_layout` in `state.yml`.

## Icons

All under `behavior`, all editable in Settings.

```yaml
behavior:
  playing_icon: "▶"         # marks the playing row in track lists
  gauge_filled_icon: "⣿"    # progress and volume bar fill
  gauge_unfilled_icon: "⣉"  # progress and volume bar background
  sort_ascending_icon: "↑"
  sort_descending_icon: "↓"
  list_highlight_icon: "▶"  # cursor in the sidebar and menus
```

Every icon except `list_highlight_icon` must be exactly one terminal column
wide; a wider one is rejected with a warning. Some glyphs (`↑ ↓ ▶`) are drawn
two columns wide by terminals set to treat "ambiguous width" characters as
wide; switch that terminal option to narrow, or pick other glyphs.

## Playbar control labels

```yaml
behavior:
  playbar_control_labels:
    prev: "⏮"
    play_pause: "[PLAY/PAUSE]"
    next: "⏭"
    shuffle: "shfl"
    repeat: "rpt"
    vol_down: "vol-"
    vol_up: "vol+"
```

File only. Leave a key out (or empty) to keep its default; mouse targets resize
to fit.

## Format templates

```yaml
format:
  playbar_status: "{state} ({source}{queue}{shuffle}{repeat} | Volume: {volume}%)"
  window_title: "{title}{artist}"
```

Templates use `{key}` placeholders; write a literal brace as `{{` or `}}`. An
unknown key or unbalanced brace falls back to the default with a warning. With
the default templates, the labels follow the interface language.

### `playbar_status`

| Key | Renders as |
| --- | --- |
| `{state}` | `Playing` / `Paused`, padded to 7 characters |
| `{source}` | `Qobuz` |
| `{queue}` | ` \| 3/12`, the position in the playlist, or empty |
| `{shuffle}` | ` \| Shuffle: On ` / ` \| Shuffle: Off`, or empty while a queued track plays |
| `{repeat}` | ` \| Repeat: All  ` and so on, or empty while a queued track plays |
| `{volume}` | the volume number; add your own `%` |

`{queue}`, `{shuffle}` and `{repeat}` bring their own ` | Label: ` prefix so the
whole segment disappears when it does not apply. Don't write your own label
around them.

### `window_title`

Used when `set_window_title` is on. `{title}` is the track; `{artist}` renders
as ` — Artist`, or nothing when the artist is unknown.

## Tables

`tables.songs` sets the columns of every track list.

```yaml
tables:
  songs:
    - { id: title, width_percent: 40 }
    - { id: artist, header: "Band", width_percent: 30 }
    - { id: album }
    - { id: length }
```

| Field | Meaning |
| --- | --- |
| `id` | `index`, `title`, `artist`, `album`, `length` or `year` |
| `header` | your own column title |
| `width_percent` | width as a share of the table (above 0, up to 100) |
| `width` | width in terminal cells |

The default is `title, artist, album, length`. A column takes `width_percent`
or `width`, not both, and the percentages may not add up past 100. A table with
an unknown or repeated id, or none at all, falls back to the default with a
warning. The playing marker goes on the `title` column, or the first column if
there is none.

## Keybindings, theme and plugins

- `keybindings:` rebinds the named actions (`back: q`, `next_track: n`,
  `open_settings: alt-,`). The names and defaults are in
  [keybindings.md](keybindings.md).
- `theme:` picks a preset and any of its colour slots: see
  [themes.md](themes.md).
- `plugin_commands:` binds keys to Lua plugin commands: see
  [scripting.md](scripting.md).

## Logging

Every run writes a log to `$TMPDIR/qbal_logs/qballog<pid>` (`/tmp/qbal_logs/...`
when `TMPDIR` is unset). Qbal prints the path when it starts and when it exits.

| | Effect |
| --- | --- |
| `--debug` | Raises Qbal's own messages to `debug` |
| `QBAL_LOG=<level>` | `error`, `warn`, `info` (default), `debug` or `trace` |

When both are given, the more detailed one wins. Libraries Qbal uses (HTTP,
TLS) stay at `info`.

Qbal does not log your Qobuz token or request headers. Error messages can
include the address of a failed request, so read the log before attaching it
to a public issue.

If Qbal crashes it writes `~/.local/state/qbal/qbal_panic.log` with the error,
a backtrace and the end of the log. That file is usually enough for a bug
report.
