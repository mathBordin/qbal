<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->
## Table of Contents

- [Qbal](#qbal)
  - [Install](#install)
  - [Use](#use)
  - [Documentation](#documentation)
  - [Credits](#credits)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

# Qbal

A fast [Qobuz](https://www.qobuz.com) player for the Linux terminal, written in Rust.

Browse your Qobuz favorites, playlists and albums, search the catalog, and play
in lossless quality without leaving the terminal. The playbar shows the format
of the track (codec, bit depth, sample rate, bitrate) and what reaches your
ears: the format of your speakers, headphone jack, USB DAC or HDMI, or the codec
of your Bluetooth device. Qbal adds synced lyrics, an
audio visualizer, album art, a play queue, listening statistics, media keys,
Discord status, and Lua plugins.

The interface is available in English, Portuguese (Brazil) and French.

> Qobuz streaming needs a paid Qobuz subscription.

## Install

Qbal is built from source. You need [Rust](https://rustup.rs) 1.90 or newer and
a few system libraries:

```bash
# Fedora
sudo dnf install pkgconf-pkg-config openssl-devel alsa-lib-devel pipewire-devel clang
# Debian / Ubuntu
sudo apt install pkg-config libssl-dev libasound2-dev libpipewire-0.3-dev libspa-0.2-dev clang
# Arch
sudo pacman -S pkgconf openssl alsa-lib pipewire clang
```

Then build and install:

```bash
git clone https://github.com/mathBordin/qbal
cd qbal
cargo build --release --features cover-art,audio-viz
cp target/release/qbal ~/.local/bin/
```

See [docs/installation.md](docs/installation.md) for what each build feature
does, updating, and uninstalling.

## Use

Run `qbal`. The first time you browse or search, Qbal opens the Qobuz sign-in
page in your browser.

| Key | Action |
| --- | --- |
| `?` | Every key binding |
| `/` | Search Qobuz |
| `Tab` | Switch the search between songs and albums |
| `Enter` | Play / open |
| `Space` | Play / pause |
| `n` / `p` | Next / previous track |
| `v` | Visualizer |
| `Alt+,` | Settings, including the interface language |
| `q` | Back / quit |

## Documentation

- [Installation](docs/installation.md)
- [Key bindings](docs/keybindings.md)
- [Configuration](docs/configuration.md)
- [Themes](docs/themes.md)
- [Lua plugins](docs/scripting.md)
- [Command line](docs/cli.md)

## Credits

Qbal is a fork of [spotatui](https://github.com/LargeModGames/spotatui) by
LargeModGames and its contributors, itself built on
[spotify-tui](https://github.com/Rigellute/spotify-tui) by Alexander Keliris.
It keeps their MIT license.
