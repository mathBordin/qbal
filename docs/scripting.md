# Lua scripting

qbal can run user-written Lua plugins. Plugins react to playback events and can drive
playback through a small, curated API. Scripting is compiled in behind the `scripting`
feature, which is enabled in the default build.

## File locations

Plugins are loaded from your app config directory (`$XDG_CONFIG_HOME/qbal`
when `XDG_CONFIG_HOME` is set to an absolute path, or `~/.config/qbal`
when it is unset or not absolute) at startup, in this order:

1. `init.lua`, if present.
2. Single-file plugins: every `plugins/*.lua` file, sorted by filename.
3. Directory plugins: every `plugins/<name>/` folder, sorted by name. The entry point is
   `main.lua`, falling back to `init.lua`. A directory with neither is skipped, and directories
   whose name starts with `.` are ignored.

A directory plugin's own folder is added to Lua's `package.path`, so it can split itself across
files and load them with `require("module")` (resolving to `plugins/<name>/module.lua`).
`package.path` and the module cache are shared across all plugins: if two plugins both
`require("util")`, the first-loaded plugin's `util.lua` is cached under that name and silently
handed to the later plugin as well. Give helper modules distinctive (e.g. plugin-prefixed) names.

Directory plugins are how the `qbal plugin` installer (below) lays out git-cloned plugins.

Missing files or a missing `plugins/` directory are fine. If a file fails to load, the error
is logged and shown as a status message, and the remaining plugins still load.

## Trust and safety

Plugins are not sandboxed. A plugin runs with the same privileges as qbal itself: it has the
full Lua standard library (including filesystem access via `io`/`os`) and can make arbitrary
network requests through `qbal.http_get`/`http_post`. `qbal plugin add` clones a git
repository and runs whatever its `main.lua` contains the next time you start qbal.

Treat installing a plugin like running any other program from the internet: only install plugins
whose source you have read or whose author you trust, and prefer repositories you control. There is
no permission prompt and no isolation between a plugin and your account.

## Installing and managing plugins

A plugin published as a git repository can be installed with the `qbal plugin` command. This
requires `git` on your PATH and does not need a Qobuz login.

```bash
qbal plugin add owner/repo      # GitHub shorthand
qbal plugin add https://gitlab.com/owner/repo.git
qbal plugin add owner/repo --force   # reinstall over an existing copy

qbal plugin list                # show installed plugins
qbal plugin update              # update every plugin to its latest commit
qbal plugin update <name>       # update just one
qbal plugin remove <name>       # uninstall

qbal plugin new <name>          # scaffold a new plugin to start from
```

`add` clones the repository into `plugins/<name>/` under the qbal app config
directory (a shallow clone) and records it in `plugins.lock` in that same
directory. `update` fast-forwards each clone to the remote's latest commit.
Restart qbal after installing or updating for changes to take effect, and
bind any commands the plugin registers under `plugin_commands` in `config.yml`.

Single-file plugins you drop into `plugins/` by hand are not tracked in the lockfile; `plugin list`
shows them under "untracked".

## Publishing a plugin

The quickest start is `qbal plugin new <name>`, which scaffolds a working directory plugin in
your config directory:

```bash
qbal plugin new my-plugin
```

This writes `plugins/my-plugin/main.lua` under the qbal app config directory
(with a `require_api` guard, a sample command, and a suggested key binding) plus
a `README.md`. Edit it, then `git init` and push to share it.

A shareable plugin is a git repository with a `main.lua` (or `init.lua`) entry point at its root:

```
my-plugin/
  main.lua        -- entry point; runs at startup
  lib.lua         -- optional helper module, loaded with require("lib")
  README.md       -- document the command(s) and a suggested key binding
```

The repository name becomes the local plugin name (its last path segment, minus `.git`). Ship a
*suggested* key binding in your README rather than writing to the user's `config.yml`; command
names are decoupled from keys by design.

To help others find it, add the GitHub topic `qbal-plugin` to your repository, and open a pull
request adding it to [`PLUGINS.md`](../PLUGINS.md).

## The `qbal` API

A global table named `qbal` is available in every plugin.

### Constants

- `qbal.api_version` - integer API version (currently `6`).

### Declaring API compatibility

The scripting API is versioned and grows over time. If your plugin uses a feature added in a
particular version, declare it on the first line so users on an older qbal get a clear
message instead of a cryptic `attempt to call nil` error:

```lua
qbal.require_api(6)
```

`qbal.require_api(n)` raises a load error (`requires qbal scripting API v{n} ...`) when
the running build's `api_version` is lower than `n`, which stops that plugin from loading while
leaving the others untouched. Calling it with a version your build supports is a no-op.

### Events

Register a callback with `qbal.on(event, fn)`. Passing an unknown event name raises an
error. Valid events:

| Event | Argument | Fires when |
|-------|----------|------------|
| `start` | none | The app finishes its first render. |
| `quit` | none | The app is shutting down. |
| `track_change` | playback table or nil | The current track identity changes (by uri, or name as a fallback), including the first track. |
| `playback_state_change` | playback table or nil | Playing/paused state changes (no playback counts as not playing). |
| `seek` | playback table or nil | Same track, same play state, and the position jumps back by more than 1.5 s or forward by more than 6.5 s. |
| `volume_change` | playback table or nil | The volume changes. |
| `queue_change` | none | The queue contents change. |
| `shuffle_change` | playback table or nil | Shuffle flips while playback exists on both sides of the tick. |
| `repeat_change` | playback table or nil | The repeat mode changes while playback exists on both sides of the tick. |
| `device_change` | none | Never fires in Qbal; kept so older plugins still load. |
| `search_results` | none | New search results arrive (from the Search screen or `qbal.search`). |
| `route_change` | `{ name = "..." }` | The visible screen changes. Names are listed under Navigation below; plugin screens are `"plugin:<name>"`. |

You can register multiple callbacks for the same event.

### Reads

These return a snapshot of the current state, refreshed before callbacks run.

- `qbal.playback()` - playback table, or `nil` when nothing plays.
- `qbal.current_track()` - track table, or `nil`.
- `qbal.playlists()` - array of your Qobuz playlists in the sidebar.
- `qbal.queue()` - `{ currently_playing = item or nil, items = { ... } }`, Qbal's queue.
  Each item is `{ kind = "track", track = {...} }`.
- `qbal.search_results()` - `{ tracks, albums, artists, playlists, shows }` from the last
  search. Qobuz search fills `tracks`; the other lists are empty.
- `qbal.current_route()` - the current screen name as a string.
- `qbal.config()` - theme and behavior settings (see "Reading configuration" below).
- `qbal.devices()` - always an empty array in Qbal; kept so older plugins still load.

The playback table has these fields:

```
{
  track = {
    uri, name, artists = { ... }, album, duration_ms,
    id, album_id,
    artist_refs = { { id, name }, ... },
    is_playable, is_local, track_number, explicit,
    image_url, release_date,
  } or nil,
  is_playing = bool,
  progress_ms = number,
  shuffle = bool,
  repeat = "off" | "track" | "context",
  volume_percent = number or nil,
  device = nil,
}
```

`repeat` is a Lua reserved word, so index it with `pb["repeat"]`, not `pb.repeat`. (The matching
action is named `set_repeat` for the same reason.)

Track URIs look like `qobuz:track:12345`. `artist_refs` is empty when only the combined
`artists` list is known; `image_url` is the cover URL when Qobuz provides one;
`release_date` is the album's release date when known (`"1997"`, `"1997-01"` or
`"1997-01-20"`), and is always `nil` in the playback table itself.

### Async data reads

- `qbal.get_lyrics(cb)` - `cb({ status, lines }, err)` for the current track, where
  `status` is `"not_started" | "loading" | "found" | "not_found"` and each line is
  `{ time_ms, text }`. Delivered at once when the current track's lyrics are already
  resolved; otherwise it waits for the lookup in flight, and after 15 seconds delivers
  `(nil, "request timed out")`. Calling it from a `track_change` handler is safe: it waits
  for the new track's lyrics rather than returning the previous track's.

```lua
qbal.on("track_change", function()
  qbal.get_lyrics(function(lyrics, err)
    if lyrics and lyrics.status == "found" then
      qbal.log(#lyrics.lines .. " lyric lines")
    end
  end)
end)
```

### Actions

Actions are queued and applied on the app's next tick; they do not return a result. Each one
does exactly what the matching key does.

- `qbal.play()` / `qbal.pause()` - resume or pause. No-op when already in that state.
- `qbal.next()` - skip to the next track.
- `qbal.previous()` - go to the previous track, or restart the current one when more than
  3 seconds in.
- `qbal.seek(ms)` - seek to a position in milliseconds.
- `qbal.set_volume(pct)` - set the volume, clamped to 0-100.
- `qbal.shuffle(on)` - turn shuffle on or off.
- `qbal.set_repeat(mode)` - `"off"`, `"track"` or `"context"`.
- `qbal.cycle_repeat()` - cycle repeat like the repeat key.
- `qbal.play_uri(uri)` - play one `qobuz:track:` uri. Anything else raises.
- `qbal.search(query)` - search Qobuz and show the results.
- `qbal.notify(msg, ttl_secs?)` - show a status message (default 4 seconds).

Arguments are checked when you call (non-empty, a well-formed uri, numbers in range); an
invalid one raises a Lua error.

### Timers

- `qbal.set_timeout(ms, fn) -> handle` - run `fn` once after roughly `ms` milliseconds.
- `qbal.set_interval(ms, fn) -> handle` - run `fn` repeatedly every roughly `ms`
  milliseconds (`ms` must be at least 1).
- `qbal.cancel_timer(handle)` - cancel a pending timeout or interval. Unknown or expired
  handles are a no-op.

Timers fire from the app's tick loop, so their real resolution is the UI tick rate
(`behavior.tick_rate_milliseconds`, at most 999ms) -- a 10ms timeout still waits for the next
tick. If the app stalls past several interval periods, the interval fires once and reschedules
(no catch-up burst). An interval whose callback errors is removed after the first failure.

### Navigation

- `qbal.navigate(target)` - open a screen, exactly like its key. Targets: `home`, `queue`,
  `settings`, `help`, `lyrics`, `analysis`, `miniplayer`. Unknown targets raise.
- `qbal.back()` - go back, like the back key.
- `qbal.current_route()` - the current screen name. The same names come with the
  `route_change` event: `home`, `search`, `track_table`, `queue`, `settings`, `help`,
  `lyrics`, `cover_art`, `miniplayer`, `analysis`, `stats`, `announcement`,
  `recap_prompt`, `exit_prompt`, `error`, and `plugin:<name>` for plugin screens.

### Persistent storage

Each plugin gets a private key-value store persisted as plain JSON at
`plugin-data/<plugin>.json` under the qbal app config directory. Values must
be JSON-serializable (tables, strings, numbers, booleans); functions and userdata
raise.

- `qbal.storage_get(key)` - the stored value, or `nil`.
- `qbal.storage_set(key, value)` - store a value. `nil` removes the key.
- `qbal.storage_remove(key)` - remove a key.
- `qbal.storage_keys()` - array of stored key names.

Writes are flushed to disk in the background (roughly every 3 seconds) and always on quit.
The files are plain JSON on disk -- other programs (and other plugins, via `io`) can read
them, so do not store secrets. If two qbal instances run at once, the last writer wins.
A missing or corrupt file starts the plugin with an empty store (corruption is logged).

```lua
local plays = (qbal.storage_get("play_count") or 0) + 1
qbal.storage_set("play_count", plays)
```

### Reading configuration

`qbal.config()` returns the live user configuration:

```
{
  theme = { active = "0, 180, 180", playbar_text = "Reset", ... },
  behavior = {
    seek_milliseconds, volume_increment,
    tick_rate_milliseconds, animation_tick_rate_milliseconds,
    playing_icon,
    enable_text_emphasis, show_loading_indicator, enforce_wide_search_bar,
    set_window_title, shuffle_enabled,
    sidebar_width_percent, playbar_height_rows, library_height_percent,
    active_source,
  },
}
```

Theme values use the same strings as `config.yml` (a colour name or `"r, g, b"`), so they
round-trip through `qbal.set_theme`. The Qobuz token and the Discord application id are never
exposed. Read it after the `start` event; before that it holds defaults.

### Logging

- `qbal.log(msg)` - write an info-level line to the app log.

### JSON utilities

- `qbal.json_decode(json)` - parse a JSON string into Lua tables, strings, numbers,
  booleans, and nil-compatible values. Invalid JSON raises a Lua error.
- `qbal.json_encode(value)` - serialize a Lua value to a compact JSON string. Values that
  cannot be represented as JSON, such as functions or userdata, raise a Lua error.

JSON `null` decodes to a light userdata sentinel, not Lua `nil`, and the sentinel is truthy in
Lua. To detect it, compare against a known null value:

```lua
local NULL = qbal.json_decode("null")
local decoded = qbal.json_decode('{"artist":null}')
if decoded.artist == NULL then
  -- field was present but null
end
```

```lua
local body = qbal.json_encode({
  track = "qobuz:track:12345",
  rating = 5,
})

local decoded = qbal.json_decode('{"ok":true,"items":[1,2]}')
qbal.log("first item: " .. decoded.items[1])
```

### HTTP requests

HTTP runs asynchronously. Calls return immediately; the callback runs on a later UI tick after
the response arrives. Only `http://` and `https://` URLs are accepted.

- `qbal.http_get(url, callback)` - send a GET request.
- `qbal.http_post(url, body, headers, callback)` - send a POST request. `body` is a string.
  `headers` must be a table of string keys and string values, or `nil` for no headers. The
  four-argument form is required, so pass `nil` when you do not need headers.

Callbacks receive `callback(resp, err)`:

- Success: `resp = { status = number, ok = bool, body = string }`, `err = nil`.
- Transport failure such as DNS, a timeout (requests time out after 30 seconds), or a
  connection failure: `resp = nil`, `err = string`.
- HTTP 4xx and 5xx responses are not transport failures. They call the success path with
  `resp.ok = false`.

Response bodies are decoded with lossy UTF-8 conversion. In-flight requests are dropped when
qbal exits.

```lua
qbal.on("track_change", function(pb)
  if not pb or not pb.track then
    return
  end

  local url = "https://example.com/lyrics?uri=" .. pb.track.uri
  qbal.http_get(url, function(resp, err)
    if err then
      qbal.notify("lyrics fetch failed: " .. err, 4)
      return
    end
    if resp.ok then
      local parsed = qbal.json_decode(resp.body)
      qbal.popup("Lyrics", parsed.lines)
    else
      qbal.notify("lyrics service returned " .. resp.status, 4)
    end
  end)
end)
```

```lua
local body = qbal.json_encode({ event = "track_started" })

qbal.http_post(
  "https://example.com/webhook",
  body,
  { ["content-type"] = "application/json" },
  function(resp, err)
    if err then
      qbal.log("webhook failed: " .. err)
    elseif not resp.ok then
      qbal.log("webhook returned " .. resp.status)
    end
  end
)
```

## Commands and keybindings

`qbal.register_command(name, fn)` registers a named, callable action. The name must be a
non-empty string with no whitespace. Registering the same name twice (from any plugin) raises a
Lua error at load time.

```lua
qbal.register_command("toggle_lyrics", function()
  qbal.notify("lyrics toggled", 3)
end)
```

To bind a command to a key, add a `plugin_commands` section to `config.yml`:

```yaml
plugin_commands:
  toggle_lyrics: "ctrl-l"
  show_stats: "ctrl-k"
```

Each entry maps a command name to a key string. The key string uses the same format as the
built-in keybindings (e.g. `ctrl-l`, `alt-x`, `f1`, `space`). Entries are skipped when
the key string is invalid, the key is one of the reserved navigation keys (`H`, `M`, `L` for the
top/middle/bottom list jumps, and `up`, `down`, `left`, `right`, `backspace`, `enter`), or the
key already has a named action bound to it. The remaining entries are loaded normally, and each
skipped entry logs a warning.

Press `?` in a running Qbal to see which keys are already taken.

When the bound key is pressed, the corresponding command callback fires after the current key
handler returns. An error in the callback is reported as a highlighted status message (6-second
ttl) and logged, but the command stays registered -- a transient failure does not permanently
unbind a key.

Plugin authors should document a suggested binding in their plugin rather than shipping one
in config. Command names are decoupled from keys by design: the user decides which key to use.

## UI extension

### Playbar segment

`qbal.set_playbar(text)` sets a persistent text segment for the calling plugin, shown in
the playbar title as `" | {text}"` after any status message. Each plugin has its own segment
slot; calling `set_playbar` again replaces it. Pass `nil` to clear the segment.

```lua
qbal.on("track_change", function(pb)
  if pb and pb.track then
    qbal.set_playbar(pb.track.name)
  else
    qbal.set_playbar(nil)
  end
end)
```

The segment persists until the plugin explicitly clears it. Multiple plugins each show their
own segment in alphabetical plugin-name order.

### Popup

`qbal.popup(title, lines)` opens a centered modal dialog. The dialog overlays every
screen, including the help menu and queue. Press `j`/Down to scroll down, `k`/Up to scroll
up, and `Esc` or `q` to close. All other keys are swallowed while the popup is open.

`lines` can be:
- A single string.
- An array where each item is a string or a table `{ text, fg?, bold?, italic? }`.
  - `fg` is a color string in the same format as `config.yml` theme values (e.g. `"Red"`,
    `"Magenta"`, `"0, 200, 200"`).
  - `bold` and `italic` are booleans (default `false`).
  - Missing `text`, an unparseable color, or a non-string/non-table item raises a Lua error.

```lua
qbal.popup("Track info", {
  { text = "Now playing", bold = true },
  { text = "Song title here", fg = "Cyan" },
  "",
  "Press Esc to close",
})
```

### Theme overrides

`qbal.set_theme(tbl)` applies runtime color overrides to the active theme. Keys are
theme field names and values are color strings. Changes are applied immediately and affect all
subsequent renders. They are never written back to `config.yml` -- they are runtime-only and
reset on app restart.

Valid field names: `active`, `banner`, `error_border`, `error_text`, `hint`, `hovered`,
`inactive`, `playbar_background`, `playbar_progress`, `playbar_progress_text`, `playbar_text`,
`selected`, `text`, `background`, `header`, `highlighted_lyrics`, `analysis_bar`,
`analysis_bar_text`.

Color string format is the same as in `config.yml` (named ANSI color or `"r, g, b"`).

An unknown field name or an invalid color raises a Lua error.

```lua
qbal.set_theme({
  playbar_text = "Magenta",
  hint = "0, 200, 0",
})
```

### Custom screens

Plugins can register full-screen views. Screens are retained-mode: you publish content with
`set_screen`, the app renders it from its own state, and you update it again whenever you want
the view to change (typically from `on_key`, a timer, or a data callback). There is no
per-frame draw callback.

- `qbal.register_screen(name, spec)` - register a screen. `name` must be non-empty with no
  whitespace and unique across all plugins. `spec` is a table:
  - `title` (optional string) - the window title (defaults to the screen name).
  - `on_key(key)` (required) - called for every key pressed while the screen is focused. `key`
    is a `config.yml`-style string (`"a"`, `"ctrl-x"`, `"enter"`, `"up"`, `"space"`, ...).
  - `on_open()` / `on_close()` (optional) - called when the screen gains/loses the route.
- `qbal.set_screen(name, widgets)` - publish the screen's content (see widgets below).
- `qbal.show_screen(name)` - navigate to the screen.
- `qbal.close_screen(name)` - leave the screen if it is currently shown.

`set_screen`/`show_screen`/`close_screen` verify the screen is registered and owned by the
calling plugin.

`widgets` is an array; each entry is a table with a `type`:

- `{ type = "paragraph", lines = <lines>, scroll? }` - styled text. `lines` uses the same
  format as `qbal.popup`. `scroll` defaults to `true`; set it to `false` on a fixed
  header so `PageUp`/`PageDown` leave it alone.
- `{ type = "list", items = <lines>, title?, selected? }` - a bordered list.
  `selected` is 1-based (like Lua arrays) and highlights that item.
- `{ type = "gauge", ratio, label? }` - a progress bar; `ratio` is clamped to 0..1.
- `{ type = "cover_art", source?, fit? }` - the current track's cover art (API v6).
- `{ type = "row", children = { ... } }` / `{ type = "column", children = { ... } }` -
  layout containers (API v6).

#### Sizing

Any widget may carry a size hint on either axis:

| Field | Meaning |
|---|---|
| `height` / `width` | an absolute number of cells; `0` hides the widget |
| `height_percent` / `width_percent` | `1`..`100` percent of the parent |

The top level is an implicit vertical stack, a `column` stacks vertically and a `row` stacks
horizontally. Only the hint matching the parent's axis is used - `width` is ignored at the top
level, `height` is ignored inside a `row`. A widget with no matching hint shares the remaining
space evenly with the other unsized widgets. Setting both `height` and `height_percent` (or
both width forms) is an error, as is a percentage outside `1`..`100`. A cell count of `0` is
not an error - it collapses the widget to nothing, which is a way to hide one conditionally.

A `gauge` with no `height` takes 3 rows, as before. Since API v6 an explicit `height` on a
gauge is honored instead of ignored.

#### Containers

`row` and `column` are pure layout: they draw no border and no title, they only split their
area among `children`. Nest them to build a grid:

```lua
qbal.set_screen("now_playing", {
  { type = "paragraph", lines = { "q to close" }, height = 1, scroll = false },
  {
    type = "row",
    children = {
      { type = "cover_art", width_percent = 40 },
      { type = "paragraph", lines = lyrics, width_percent = 60 },
    },
  },
})
```

Trees are capped at 8 levels of nesting and 256 widgets per screen.

#### Cover art

`{ type = "cover_art" }` renders the artwork qbal has *already* downloaded and decoded for
the current track - there is no second download, and the app keeps ownership of the terminal
graphics protocol, resizing, placement and cleanup.

- `source` - optional; only `"current"` is accepted today. Any other value is an error, so a
  future source fails loudly on an old build rather than showing the wrong image.
- `fit` - `"contain"` (default) fits the image inside its area keeping the aspect ratio and
  never upscaling; `"scale"` also upscales an image smaller than its area. The image is
  centered in whatever space the layout gives it.

At most **one** `cover_art` widget may appear anywhere in a screen's tree; a second one is a
`set_screen` error.

When no image can be shown the widget draws a centered status line instead ("Loading cover
art...", "No cover art for this source", ...). That is what you get when the track has no
artwork, when the build was compiled without the `cover-art` feature, when
`behavior.draw_cover_art` is off, or when the terminal has no graphics protocol and
`behavior.draw_cover_art_forced` is off. A Kitty-, iTerm2- or Sixel-capable terminal shows
graphics art by default; set `draw_cover_art_forced` for the basic-terminal fallback.

When `behavior.cover_art_dither` is enabled, plugin cover-art widgets use the same optional
two-color dither style as the playbar and full-screen art view. The accent comes from
`theme.cover_art_dither_color`, or from the live playbar progress color when that setting is
`auto`; the widget uses the current theme background for its other color. The mask is made
from the original decoded image at the widget's output resolution, including for small slots.

#### Keys

Keys reach `on_key` only after the global keybindings have run, so keys like the back key or
volume keys keep their normal meaning. `Esc` (and the back key) leave the screen, and
`PageUp`/`PageDown` scroll paragraph text; everything else is forwarded. An `on_key` that
errors is disabled after the first failure (one strike).

`PageUp`/`PageDown` apply one shared scroll offset to every paragraph with `scroll = true`, so
a header with `scroll = false` stays pinned while the rest of the screen moves.

A minimal interactive screen:

```lua
qbal.require_api(5)

local selected = 1
local names = {}

local function render()
  qbal.set_screen("my_playlists", {
    { type = "paragraph", lines = { { text = "j/k to move, Esc to leave", italic = true } }, height = 2 },
    { type = "list", title = "Playlists", items = names, selected = selected },
  })
end

qbal.register_screen("my_playlists", {
  title = "My Playlists",
  on_key = function(key)
    if key == "j" and selected < #names then selected = selected + 1 end
    if key == "k" and selected > 1 then selected = selected - 1 end
    render()
  end,
  on_open = function()
    names = {}
    for _, p in ipairs(qbal.playlists()) do
      names[#names + 1] = p.name
    end
    render()
  end,
})

qbal.register_command("my_playlists", function()
  qbal.show_screen("my_playlists")
end)
```

Bind `my_playlists` under `plugin_commands` in `config.yml` to open it with a key.

## Error behavior

Plugin code can never crash the app. If a callback raises an error or panics, the error is
logged, a highlighted status message is shown in the playbar, and that one callback is
disabled (one strike). Other callbacks, including other callbacks for the same event, keep
running.

Plugin errors are shown using the theme's error color and stay visible for 6 seconds.
Normal notifications (e.g. a "Now playing" message from `qbal.notify`) cannot overwrite
a live plugin error -- the error is shown first, and the notification takes effect only after
the error expires. A later plugin error always replaces an earlier one immediately.

## Sample init.lua

```lua
qbal.on("track_change", function(pb)
  if pb and pb.track then
    qbal.notify("Now playing: " .. pb.track.name .. " by " .. table.concat(pb.track.artists, ", "), 4)
  end
end)

qbal.on("start", function()
  qbal.log("plugins loaded, api version " .. qbal.api_version)
end)
```
