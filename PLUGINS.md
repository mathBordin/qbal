# Plugins

Qbal runs user-written Lua plugins. They react to playback events, add commands and key
bindings, draw popups and playbar segments, restyle the theme, and make async HTTP requests.
See [`docs/scripting.md`](docs/scripting.md) for the full API and
[`examples/plugins/`](examples/plugins) for runnable examples.

## Installing a plugin

Plugins published as git repositories install with one command (requires `git`):

```bash
qbal plugin add owner/repo     # clone + record in the lockfile
qbal plugin list               # show installed plugins
qbal plugin update             # update all to their latest commit
qbal plugin remove <name>      # uninstall
qbal plugin new <name>         # scaffold a new plugin to start from
```

Plugins are cloned into `~/.config/qbal/plugins/<name>/` and loaded at startup.
Restart Qbal after installing, and bind any commands the plugin registers
under `plugin_commands` in `config.yml`.

Plugins are not sandboxed and run with full app privileges and network access, so only install
ones you trust. See [Trust and safety](docs/scripting.md#trust-and-safety).

You can also drop a single `.lua` file into the app config directory's `plugins/`
folder by hand. Manual examples use `${XDG_CONFIG_HOME:-$HOME/.config}` for
brevity. That matches Qbal when `XDG_CONFIG_HOME` is unset or absolute; if
it is relative, Qbal ignores it and loads plugins from
`$HOME/.config/qbal/plugins`, so copy there instead.

## First-party examples

These ship in this repo under [`examples/plugins/`](examples/plugins):

- **track-notifier** - "Now playing" toast and playbar segment on every track change.
- **track-info-popup** - a command that pops up details of the current track.
- **accent-cycler** - a command that rotates the theme accent color.
- **now-playing-webhook** - POSTs a JSON payload to a webhook on track change.
- **session-stats** - a directory plugin (with a `require`-d helper) that tracks session plays.
- **queue-browser** - a custom screen listing the queue and your playlists, with timers and storage.
- **now-playing** - a custom screen with cover art and synced lyrics in a row layout.
  Art only draws in builds with the `cover-art` feature.

## Sharing your own plugin

Run `qbal plugin new <name>` to scaffold a starting point. A shareable plugin is just a git
repository with a `main.lua` (or `init.lua`) entry point at its root. Helper modules sit alongside
it and load via `require("module")`. Document any command and a suggested key binding in your
README, but ship the binding as a suggestion, not a hard-coded key.

Tag your repository with the GitHub topic `qbal-plugin` so it's discoverable, and open a pull
request adding it to this list - a short description and the `owner/repo` install line is all it
takes.
