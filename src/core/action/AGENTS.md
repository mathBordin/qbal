### The shared Action vocabulary

- Every arm delegates to the same ownership-aware `App` method the
  equivalent keybinding uses. Playback starts go through
  `App::start_playback_uris` / `start_playback_context` /
  `start_playback_track_in_context` - never a hand-built
  `IoEvent::StartPlayback` in an arm.
- No catch-all match arm under `src/core/action/` or in `tui/keymap.rs`:
  both deny `clippy::wildcard_enum_match_arm` and
  `clippy::match_wildcard_for_single_variants` (a named binding like
  `_other =>` counts as a wildcard; `matches!` and `Option`/`Result`
  scrutinees are exempt), and `wildcard_arms_in_action_tree` pins the
  action tree at 0 by a raw text scan that includes tests, comments, and
  string literals. CI clippy never compiles tests, so write a test
  catch-all as `_other =>` and only on a `Result`/`Option` scrutinee.
- A new variant needs its arm in `App::apply` (`apply.rs`) and a test in
  `tests.rs`. Feature-gate an arm's body, never the arm: clippy skips a
  match when any arm carries a `#[cfg]`.
- `Action` derives serde (the Lua plugins' wire shape); a payload type added to
  it must stay serde-derivable.
