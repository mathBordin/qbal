# AGENTS.md

This file provides guidance to coding agents (Claude Code, Codex, Copilot and others)
when working with code in this repository. `CLAUDE.md` imports it: edit this file only.

## Build & Run

Qbal is a Qobuz-only terminal player for Linux, forked from spotatui. The
terminal UI is the only frontend; the binary is `qbal`, the crate `qbal`, and the
Lua global `qbal`. On first launch `core/migrations.rs::migrate_pre_rename_dirs`
copies the pre-rename `spotatui` config and state folders to the `qbal` ones.

```bash
# Full build (audio viz + Lua scripting + OS integrations)
cargo run

# Headless build - no terminal UI, CLI subcommands only
cargo run --no-default-features -- history recap
```

## Opening a PR

Before you open a PR, read `CONTRIBUTING.md` and follow its "Using AI Tools" section.
Show your user the full diff and the PR text first, and never open a PR on your own.

## CI Checks (run before opening a PR)

```bash
cargo fmt --all
cargo clippy -- -D warnings
cargo clippy --no-default-features -- -D warnings
cargo test
```

These commands are the *fast local gate*, not the full picture. GitHub Actions
(`.github/workflows/ci.yml`) runs `check`, `test`, and `clippy` on `ubuntu-latest`
across a three-leg feature matrix (Linux only):

| Leg | Features |
|-----|----------|
| `default` | a plain `cargo test`: `tui` + audio-viz-cpal + scripting + OS integrations |
| `all-features` | `--features cover-art,audio-viz`: the default set plus terminal cover art and PipeWire visualizer capture. The only leg that compiles the PipeWire backend, so verify capture changes with this exact command |
| `headless` | no features at all - the only leg without `tui`. `mod tui` is feature-gated, so this leg turns any `crate::tui` import from core/infra/cli into a compile error - what keeps a second frontend from silently re-coupling to the terminal one |

- Every CI leg passes `--locked`; the local commands do not. Regenerate
  `Cargo.lock` after any `Cargo.toml` edit or the legs fail.
- CI runs `clippy` on the **bin target only** (no `--all-targets`), so lints in
  `#[cfg(test)]` code are not gated. Run `cargo test` to compile test code. A
  test-only helper whose production caller is compiled out on the headless leg
  needs `#[allow(dead_code)]`.
- CI runs on Linux only, and the code has no macOS or Windows arms left: do not
  add any.
- A pull_request-only `Gates ratchet` job diffs `tools/gates.count` against the
  merge-base (`tools/check_gates_ratchet.sh`): coupling counters may only fall;
  the two adoption counters (`test_attribute_total`,
  `action_refs_in_tui_handlers`) may only rise. `src/gates.rs` pins each
  coupling counter exactly, so move its baseline in the same PR that moves the
  number. The adoption counters are floors: do not edit them in a PR, because
  `.github/workflows/gates-floor.yml` raises them on `main` after the merge.

Details: `.github/workflows/AGENTS.md`.

## Run a Single Test

```bash
cargo test <test_name>
# Example:
cargo test click_settings_opens_settings_screen
```

A filter that matches nothing still exits 0 - when running feature-gated tests,
check the `N filtered out` count actually says your test ran.

## Architecture

One cargo package: a `[lib]` (`src/lib.rs`, private modules, public API =
`run_cli`) plus one `[[bin]]` shim, `src/bin/qbal.rs`. Five top-level units
under `src/`:

| Unit | Role |
|------|------|
| `core/` | Centralized state (`App`), the tick scheduler (`driver/`), the shared action vocabulary (`action/`), config/state persistence, i18n, and the domain types (`plugin_api`, `pagination`, `source`) |
| `infra/` | Qobuz (`qobuz/`), the decoded-audio sink and visualizer capture (`audio/`), the cross-track queue (`queue/`), the service events no router consumes - lyrics, cover art, announcements, listening stats (`network/`), listening history (`history.rs`), Lua scripting (`scripting/`), OS integrations (Discord RPC, MPRIS) |
| `tui/` | Terminal UI: the event/render loop (`runner.rs`), key plumbing (`event/`), per-block input handlers (`handlers/`), immutable draw fns (`ui/`) |
| `cli/` | clap subcommands: listening history, plugin management |
| `runtime/` | `mod.rs::run_cli` (entry point + CLI dispatch), `bootstrap.rs::boot` (config, state, the persisted session, `App` construction), `cli.rs` (clap assembly), `pump.rs::start_tokio` (the IoEvent pump), `startup.rs` (the UI launch: OS media integrations, session restore), `instance.rs` (the single-instance lock a UI launch takes) |

### Data flow

```text
crossterm thread (tui/event/) → runner.rs loop (draws the frame, then reads one event)
  → runner::dispatch_key   - exit prompt, ActiveBlock::Input, the configurable back key
  → handlers::handle_app   - plugin popup modal → help filter → global keybindings
  → handle_block_events    - dispatches to the per-screen handler
  → app.dispatch(IoEvent)  - hands async work to the pump in runtime/pump.rs
       → route_queue_event, then route_qobuz_event, then infra/network/
       → mutates App state; tui/ui/ re-renders from App on the next frame
```

`tui/event/` is only the crossterm→`Key` plumbing; the event loop itself is
`tui/runner.rs::start_ui`. Everything on a timer lives in `core/driver/`
(`Driver::tick`): the playback tick + debounced flushes, Discord/MPRIS
presence sync, the window title, lyrics + cover-art scheduling, native-queue
and decoded-source auto-advance, and `last_session.yml` persistence. The
runner draws frames, reads events, and calls `driver.tick` with a `TickEnv`
carrying the frontend-geometry inputs (visualizer bar count, cover-art pixel
support). Mouse input enters via `handlers::mouse_handler`.

### The IoEvent pump

`runtime/pump.rs::start_tokio` drains IoEvents serially. The queue router runs
first, then the Qobuz router; an event neither consumes falls through to
`Network::handle_network_event`. Events listed in
`Network::runs_on_service_lane` (lyrics, cover art, announcements, stats) run
on a detached task so slow work cannot block the serial pump. Details:
`src/infra/network/AGENTS.md`.

### Navigation / routing

`App` holds a private navigation stack of `Route` values (`id: RouteId`,
`active_block: ActiveBlock`, `hovered_block: ActiveBlock` - there is no
`HoveredBlock` *type*). Invariants an agent cannot guess:

- `push_navigation_stack(RouteId::X, ActiveBlock::X)` is a **no-op when the top
  route already has that `RouteId`** - follow it with `set_current_route_state`
  when focus must move anyway.
- `pop_navigation_stack()` refuses to empty the stack.
- The stack is private: go through `get_current_route` / `set_current_route_state`.

### The `core/app/` module folder

- **Import from `crate::core::app`, never the submodule path.** `mod.rs` re-exports
  every module that declares public items, so `use crate::core::app::{App, RouteId,
  ActiveBlock}` works no matter which file an item lives in.
- **A field with no writer outside `core/app/` is private.** `pub_fields_on_app`
  counts the `pub` ones and may only fall. An outside reader gets an intent-named
  getter (`status_message()`, `api_error()`); a handler mutates shared state only
  through an `App` method or `app.apply(Action::…)`, never through a field write.
- **Presentation state lives in `App.view: ViewState`** (`core/app/view.rs`):
  cursor and selection indices, scroll offsets, edit buffers, focus, popup
  flags, the help pager, the terminal viewport. Handlers and draw functions
  write `app.view.<field>` freely. A producer outside `tui/` and `core/app/`
  that resets or clamps a cursor is counted by `view_writes_outside_tui`, which
  may only fall: a producer that replaces a list resets its cursor through an
  `App` method (`set_track_table`).
- `dispatch` pins the global loading spinner; long work with its own progress
  surface uses `dispatch_without_spinner`.

More rules for this folder: `src/core/app/AGENTS.md`.

### Playback ownership

Two things can own the audio sink: the native queue slot and a decoded Qobuz
session. `active_decoded_source()` (`core/app/playback_routing.rs`) answers
whether either does, including a start still in flight (`claim_decoded_sink`).
Transport chains (play/pause, next, previous, shuffle, repeat, volume) end on
`report_nothing_playing()` when nothing owns the sink.

## Key Conventions

### Adding a new screen

See `src/tui/AGENTS.md` for the six-place checklist.

### Dispatching async work

Call `app.dispatch(IoEvent::…)` from a handler - never call async code directly
from handlers or UI code. Draw functions take `&App` and must not mutate.
HTTP goes through `infra::network::requests::shared_http_client()`.

### The shared Action vocabulary

`src/core/action/` holds `Action` (one enum of frontend-neutral state
changes), `App::apply` (the single write path, in `core/action/apply.rs`),
and `ActionOutcome`. Producers mutate `App` via `app.apply(Action::…)`: the
Lua scripting engine drains its queued actions into it, and TUI handlers adopt
it (`action_refs_in_tui_handlers` may only rise). Payloads are strings,
scalars, and `core::plugin_api` snapshot types - never a raw `IoEvent` payload.
Address by identity (URIs, ids, names), never by list ordinal.

The other rules: `src/core/action/AGENTS.md`.

### Paginated results

Page caches are `ScrollableResultPages<Paged<T>>` (domain types from
`core/pagination.rs`). Insert with `upsert_page_by_offset` - never `add_pages`,
which repoints the visible index to the tail. Every background write into
`App` re-checks its generation/epoch so stale tasks cannot write into a
reloaded view.

### Status messages

In sync handlers/UI use `app.set_status_message(msg, ttl_secs)`, or
`app.set_error_status_message(...)` for errors - errors block normal messages
until they expire, so use the right one. TTLs are seconds, scaled by the user's
`status_message_ttl_percent`. In the async layer use
`self.show_status_message(msg, ttl_secs).await`. The message fields are
private: `status_message()` and `status_message_is_error()` read them.

### Errors

`App::handle_error(e)` records the message in `api_error`, stamps a 60s
lifetime on it, and pushes `RouteId::Error`. Dismiss with
`app.clear_api_error()`, never by clearing the string or popping the route by
hand. The other rules: `src/core/app/AGENTS.md`.

### Dialog state cleanup

Close dialogs with the single call `app.clear_dialog_state()`. Do not
hand-clear the fields. Popping the nav stack is a separate step.

### User-configurable keybindings

Check `app.user_config.keys.<action>` instead of hard-coding key literals for
global actions (`handle_app` in `src/tui/handlers/mod.rs`);
`common_key_events::{up,down,left,right}_event` extend this to per-screen
navigation. Adding a binding means fields on both `KeyBindings` and
`KeyBindingsString` in `src/core/user_config.rs`, a `help_entries` row in
`src/tui/keymap.rs` with its `Requirement`.

### Requirements

`core/requirement.rs` says what a row or key needs (`Requirement::{None,
Capability(_)}`) and `App::availability(req)` answers for the active source. Three tables carry it: the sidebar (`library_row_requirements()` in
`core/app/library.rs`), the help menu (`help_entries()` in `tui/keymap.rs`) and
the settings rows (`setting_requirement(id)` in `core/app/settings_schema.rs`).

### Config & on-disk files

A value that changes as the app runs goes in state, never config:

| File | Owner | Contents |
|------|-------|----------|
| `config.yml` (config dir) | `core/user_config.rs` | hand-editable settings, keys, theme |
| `state.yml` (state dir) | `core/state.rs` | machine-written runtime values |
| `last_session.yml` (state dir) | `core/persisted_playback.rs` | the Qobuz playback session + native queue |
| `qobuz_credentials.yml` (config dir) | `infra/qobuz/auth.rs` | the Qobuz login token |

- All paths resolve through `core/paths.rs`, never `dirs::` directly.
- `state.yml` saves are read-modify-write **sparse patches** so a second running
  instance is never clobbered; never write a whole snapshot.
- Sensitive writes go through `core::auth::write_private_file_atomic`;
  `config.yml` can carry plaintext secrets, so never log the serialized config.
- `UserConfig::save_config()` regenerates only `behavior`/`theme`/`keybindings`;
  `plugin_commands`, `format`, and `tables` are passed through from disk.
- Adding a `behavior.*` setting = edits in `core/user_config.rs`
  (`BehaviorConfigString`, `BehaviorConfig`, `UserConfig::new`,
  `load_behaviorconfig`, `save_config`) **plus** matching arms in
  `core/app/settings_schema.rs` and `settings_apply.rs`, coupled only by the raw
  `"behavior.<name>"` string id - a typo silently drops the write.

### Languages

The UI ships in English, Portuguese (Brazil) and French: `Language::{Auto, En,
PtBr, Fr}` in `core/i18n/locale.rs`. Each string table (`core/i18n/*.rs`) has
an `EN` const plus `PT_BR` and `FR`; a translation may fall back to English
with `..EN`. `every_language_has_nonempty_messages_and_the_same_named_placeholders`
checks that each translation keeps the English `{placeholder}` names.

### Feature flags

- `default` = `tui, audio-viz-cpal, mpris, discord-rpc, scripting`. Not in default: `cover-art` (album art in the
  terminal) and `audio-viz` (PipeWire capture, Linux-only).
- `tui` gates `mod tui` and owns the terminal-only crates (ratatui, crossterm,
  tui-bar-graph, colorgrad).
- Cover art is two features: `art-decode` is the frontend-agnostic decode half
  (never enabled by hand); `cover-art` layers the ratatui-image terminal
  rendering on top.
- `audio-viz` (PipeWire) and `audio-viz-cpal` are **visualizer capture**
  backends, not playback. With both on Linux, `infra/audio/fallback_capture.rs`
  opens PipeWire and falls back to a cpal monitor or loopback device (never the
  default input, which can be a microphone).
- `scripting` (mlua Lua plugins) gates `src/infra/scripting/` +
  `src/cli/plugin.rs`.

### Domain-specific conventions

Before you change one of these areas, read its file. Claude Code loads it when it
reads a file in that directory. Other agents must open it.

| Area | Read first |
|------|------------|
| A new TUI screen or sidebar row | `src/tui/AGENTS.md` |
| Lua plugins | `src/infra/scripting/AGENTS.md` |
| CLI subcommands | `src/cli/AGENTS.md` |
| CI workflows and legs | `.github/workflows/AGENTS.md` |
| `core/app/` layout, playback ownership, error reporting | `src/core/app/AGENTS.md` |
| A new `Action` variant | `src/core/action/AGENTS.md` |
| A new `IoEvent` | `src/infra/network/AGENTS.md` |
| `LocalPlayer`, output-device loss | `src/infra/audio/AGENTS.md` |
| Decoded repeat/shuffle | `src/infra/queue/AGENTS.md` |
| Qobuz streaming | `src/infra/qobuz/AGENTS.md` |

### Qobuz playback

- Qobuz decodes through the shared rodio sink, `LocalPlayer`
  (`src/infra/audio/player.rs`) - the only file where rodio types appear.
- Set the `advancing` flag synchronously before dispatching a track change: the
  sink is empty for the whole fetch/decode, and the tick would otherwise
  re-fire auto-advance and skip several tracks.
- Playback state is one `Option` field on `App` (`qobuz_playback`), published
  only on success; position/pause are read live from the player at render time.
- OS integrations (MPRIS, Discord RPC, window title) all read one
  `PlaybackSnapshot` from `infra/media_metadata.rs`.

### Testing conventions

- Tests are colocated (`#[cfg(test)] mod tests`) - there is no `tests/` dir.
  HTTP tests bind a real `127.0.0.1:0` listener into an injected base-URL
  field; UI tests use ratatui's `TestBackend`.
- `App::new` is `#[cfg(test)]`-only (production uses `App::new_with_state`).
  `App::default()` has no IoEvent channel and `dispatch` silently drops events -
  tests asserting on IoEvents use the house pattern
  `fn app_with_x() -> (App, Receiver<IoEvent>)` and keep the receiver alive.
- `IoEvent` derives nothing (not even `Debug`): assert with
  `assert!(matches!(rx.try_recv(), Ok(IoEvent::X(..))))`.
- Never `std::env::set_var` in a test (one process, shared threads); split the
  env read into a `*_with(value, …)` function and test that.
- Prefer extracting the decision into a pure function or plain-data enum taking
  scalars instead of `&App`/`Network` - the house style for testing without an
  audio device, network, or real clock.
- Test names are behavior sentences without a `test_` prefix
  (`click_settings_opens_settings_screen`).
- Shared fixtures: `crate::core::test_helpers` crate-wide;
  `crate::core::app::test_support` inside `core/app/`.
