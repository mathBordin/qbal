# Key bindings

Press `?` in Qbal for the full list, in your language and with your own
bindings. Type `/` there to filter it.

## Defaults

The third column is the name to use under `keybindings:` in `config.yml`.

### Playback

| Action | Key | Config name |
| --- | --- | --- |
| Play / pause | `Space` | `toggle_playback` |
| Next track | `n` | `next_track` |
| Previous track (or restart the current one) | `p` | `previous_track` |
| Previous track, always | `P` | `force_previous_track` |
| Seek back / forward | `<` / `>` | `seek_backwards` / `seek_forwards` |
| Volume down / up | `-` / `+` | `decrease_volume` / `increase_volume` |
| Shuffle | `Ctrl-s` | `shuffle` |
| Repeat (off, all, one) | `Ctrl-r` | `repeat` |

### Moving around

| Action | Key | Config name |
| --- | --- | --- |
| Up / down / left / right | `k` / `j` / `h` / `l` (or the arrows) | `move_up` / `move_down` / `move_left` / `move_right` |
| Open / play | `Enter` | `submit` |
| Back (quits from the first screen) | `q` | `back` |
| Next / previous page | `Ctrl-d` / `Ctrl-u` | `next_page` / `previous_page` |
| Start / end of the list | `Ctrl-a` / `Ctrl-e` | `jump_to_start` / `jump_to_end` |
| Search Qobuz | `/` | `search` |
| Help | `?` | `help` |
| Settings | `Alt-,` | `open_settings` |
| Save settings | `Alt-s` | `save_settings` |

### Views

| Action | Key | Config name |
| --- | --- | --- |
| Visualizer | `v` | `audio_analysis` |
| Lyrics | `B` | `lyrics_view` |
| Cover art (cover-art builds) | `G` | `cover_art_view` |
| Mini player | `T` | `miniplayer_view` |
| Queue | `Q` | `show_queue` |
| Add the selected track to the queue | `z` | `add_item_to_queue` |
| Remove from the queue (in the queue) | `x` | `remove_from_queue` |
| Generate your listening recap | `R` | `generate_recap` |

### Panes

See [Detached panes](configuration.md#detached-panes).

| Action | Key | Config name |
| --- | --- | --- |
| Detach the open view into a pane, or put it back | `Alt-d` | `detach_pane` |
| Close the focused pane | `Alt-x` | `close_pane` |
| Focus the pane left / below / above / right | `Alt-h` / `Alt-j` / `Alt-k` / `Alt-l` | `focus_pane_left` / `focus_pane_down` / `focus_pane_up` / `focus_pane_right` |
| Swap with the pane left / below / above / right | `Alt-H` / `Alt-J` / `Alt-K` / `Alt-L` | `move_pane_left` / `move_pane_down` / `move_pane_up` / `move_pane_right` |
| Switch the split direction | `Alt-r` | `rotate_pane` |
| Grow / shrink the focused pane | `Alt-=` / `Alt--` | `grow_pane` / `shrink_pane` |
| Close every pane | `Alt-0` | `reset_panes` |

`Alt` with an arrow key also focuses panes, and `Alt+Shift` with an arrow swaps
them.

### Fixed keys

These have no config name:

| Key | Where | Action |
| --- | --- | --- |
| `H` / `M` / `L` | lists | jump to the top, middle, bottom |
| `S` | a playlist | play it from a random track |
| `,` | a playlist | sort it |
| `V` | visualizer | next visualizer style |
| `[` / `]` | Stats | previous / next period |
| `J` / `K` | queue | move the selected track |
| `{` / `}` | anywhere | narrower / wider sidebar |
| `(` / `)` | anywhere | shorter / taller playbar |
| `\|` | anywhere | reset the layout |

## Changing a binding

In Settings, open the **Keybindings** tab, select an action, press `Enter`, then
press the new key; `Alt-s` saves. Or edit `config.yml`:

```yaml
keybindings:
  back: "esc"
  next_track: "ctrl-n"
  open_settings: "alt-o"
```

### Key format

- A single character: `"a"`, `"/"`, `" "` (or `"space"`). A capital letter means
  Shift: `"A"`.
- Ctrl: `"ctrl-q"`, `"ctrl--"` (Ctrl and `-`).
- Alt: `"alt-,"`, `"alt-H"` (Alt+Shift+h), `"alt-left"`, `"alt-shift-left"`.
- Special keys: `"enter"`, `"tab"`, `"esc"`, `"backspace"`, `"del"`, `"left"`,
  `"right"`, `"up"`, `"down"`, `"pageup"`, `"pagedown"`, `"home"`, `"end"`,
  `"ins"`, `"f0"` to `"f12"`. Names are case-insensitive.

Three-key combinations such as `ctrl-alt-q` are not supported. Note that
`"delete"` means **Backspace** and `"del"` means the Delete key.

`H`, `M`, `L`, the arrows, `backspace` and `enter` are reserved: Settings
refuses them, and a plugin command bound to one is skipped with a warning.

A value that cannot be parsed keeps the default and logs a warning. A name that
is not in the tables above is ignored without a warning, so check the spelling.
