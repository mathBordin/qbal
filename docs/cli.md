# CLI

Running `qbal` with no subcommand opens the terminal UI. The subcommands below
work without it.

## Top-level flags

| Flag | Purpose |
| --- | --- |
| `-t, --tick-rate <MS>` | UI refresh interval in milliseconds for non-animated screens |
| `-c, --config <PATH>` | Use this `config.yml` instead of `~/.config/qbal/config.yml` |
| `--completions <SHELL>` | Print shell completions: `bash`, `zsh`, `fish`, `powershell` or `elvish` |
| `--debug` | Verbose log for bug reports (same as `QBAL_LOG=debug`); works after a subcommand too |
| `-h, --help` / `-V, --version` | Help and version |

Install completions, for example for bash:

```bash
qbal --completions bash > ~/.local/share/bash-completion/completions/qbal
```

## `history recap`

Make a shareable HTML recap from your local listening history.

```bash
qbal history recap --period 30d --output recap.html
```

| Flag | Purpose |
| --- | --- |
| `--period <PERIOD>` | `7d`, `30d` (default), `month`, `year` or `all` |
| `-o, --output <PATH>` | Output file (default `qbal-recap.html`) |

## `plugin`

Install and manage Lua plugins (needs `git`). Plugins are cloned into
`~/.config/qbal/plugins/<name>/`. See [Scripting](scripting.md).

```bash
qbal plugin add owner/repo [--force]   # or a full git URL
qbal plugin list                       # alias: ls
qbal plugin remove <NAME>              # alias: rm
qbal plugin update [NAME]
qbal plugin new <NAME> [--force]
```

## Environment variables

| Variable | Purpose |
| --- | --- |
| `QBAL_LOG` | Log level: `error`, `warn`, `info` (default), `debug` or `trace` |
| `QBAL_DISCORD_APP_ID` | Discord application id for Rich Presence (see [Configuration](configuration.md#discord-rich-presence)) |
