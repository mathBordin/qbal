# Example plugins

Small, self-contained Lua plugins that demonstrate the qbal plugin API. Copy one into your
config directory and restart qbal to try it. See [`docs/scripting.md`](../../docs/scripting.md)
for the full API reference. These examples are first-party and auditable; plugins run with full app
privileges, so read anything you install from elsewhere (see
[Trust and safety](../../docs/scripting.md#trust-and-safety)).

| Plugin | What it shows |
|--------|---------------|
| [`track-notifier.lua`](track-notifier.lua) | Events, `notify`, `set_playbar` |
| [`track-info-popup.lua`](track-info-popup.lua) | `register_command`, reads, `popup` |
| [`accent-cycler.lua`](accent-cycler.lua) | `register_command`, `set_theme` |
| [`now-playing-webhook.lua`](now-playing-webhook.lua) | `http_post`, `json_encode` |
| [`queue-browser.lua`](queue-browser.lua) | Custom screens, reads, timers, storage, `play_uri` |
| [`now-playing.lua`](now-playing.lua) | `cover_art` widget, `row` layout, synced lyrics |
| [`session-stats/`](session-stats) | A directory plugin with a `require`-d helper module |

## Installing

Single-file plugins go straight into `plugins/` under the qbal app config
directory (see [`PLUGINS.md`](../../PLUGINS.md) for config path rules):

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins"
cp track-notifier.lua "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins/"
```

Directory plugins (a folder with a `main.lua` entry point) are copied as a whole:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins"
cp -r session-stats "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins/"
```

Restart qbal after installing. Plugins that register commands need a key binding; add one to
`config.yml` in the qbal app config directory under `plugin_commands` (each
plugin documents a suggested key in its header comment).

To install a plugin published as a git repository, use the built-in installer instead:

```bash
qbal plugin add owner/repo
```
