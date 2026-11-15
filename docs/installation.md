# Installation

Qbal runs on Linux and is built from source.

## 1. Install Rust

You need Rust 1.90 or newer. Check with `cargo --version`, or install it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## 2. Install the system libraries

| Distribution | Command |
| --- | --- |
| Fedora | `sudo dnf install pkgconf-pkg-config openssl-devel alsa-lib-devel pipewire-devel clang` |
| Debian / Ubuntu | `sudo apt install pkg-config libssl-dev libasound2-dev libpipewire-0.3-dev libspa-0.2-dev clang` |
| Arch | `sudo pacman -S pkgconf openssl alsa-lib pipewire clang` |

The PipeWire packages and `clang` are only needed for the `audio-viz` feature
below. If a PipeWire build step complains about `libclang`, also install
`clang-devel` (Fedora) or `libclang-dev` (Debian/Ubuntu).

To name the output in the playbar (speakers, headphones, a USB DAC, HDMI, or a
Bluetooth device and its codec), Qbal runs PipeWire's `pw-dump` while it plays.
Fedora installs it by default; elsewhere it is in `pipewire-utils` (Fedora,
openSUSE), `pipewire-bin` (Debian/Ubuntu) or `pipewire` (Arch). Without it the
playbar still shows the sound card's format, labelled "Card".

## 3. Build

```bash
git clone https://github.com/mathBordin/ghosbuz
cd ghosbuz
cargo build --release --features cover-art,audio-viz
```

The program is `target/release/qbal`. To run it from anywhere:

```bash
cp target/release/qbal ~/.local/bin/
```

### Build features

Features are chosen at build time with `--features`; to change them, rebuild.

| Feature | Default | What it does |
| --- | --- | --- |
| `cover-art` | off | Draws the album cover in the terminal. Terminals that show real images (Kitty, WezTerm, Ghostty, Konsole, foot…) get a sharp picture; others get a picture made of coloured characters. |
| `audio-viz` | off | Lets the visualizer read your speakers' audio from PipeWire. Without it, the visualizer looks for an ALSA monitor device and, if it finds none, falls back to your default input, which can be the microphone. |
| `audio-viz-cpal` | on | The ALSA capture the visualizer uses without PipeWire. |
| `mpris` | on | Media keys and the desktop's "now playing" widget (MPRIS). |
| `discord-rpc` | on | Shows what you play in your Discord status. |
| `scripting` | on | Lua plugins. |

Check whether you run PipeWire with `pactl info | grep "Server Name"`.

## Update

```bash
cd ghosbuz
git pull
cargo build --release --features cover-art,audio-viz
cp target/release/qbal ~/.local/bin/
```

## Where Qbal keeps its files

| Path | Contents |
| --- | --- |
| `~/.config/qbal/config.yml` | Settings, key bindings and theme |
| `~/.config/qbal/qobuz_credentials.yml` | Your Qobuz login |
| `~/.config/qbal/plugins/` | Installed Lua plugins |
| `~/.local/state/qbal/` | Listening history, last session, window layout |

`$XDG_CONFIG_HOME` and `$XDG_STATE_HOME` are honoured when set.

Coming from spotatui? On its first launch Qbal copies `~/.config/spotatui` and
`~/.local/state/spotatui` to the folders above, so your settings, history and
Qobuz login carry over. The old folders are left untouched; delete them once
you no longer need them.

## Uninstall

```bash
rm ~/.local/bin/qbal
rm -rf ~/.config/qbal ~/.local/state/qbal   # also removes your settings and login
```
