### The `core/app/` module folder

The struct stays **flat** - every field is declared once in
`src/core/app/mod.rs`, plus the presentation fields grouped in `App.view` - and
its methods are split across sibling modules by concern, each with its own
`impl App` block. The boundaries are organizational, not architectural.

Rules when working in here:

- **Child modules open with `use super::*;`** and declare no other top-level
  imports; all external imports live in `mod.rs`. When a type is only importable
  under a feature that does not gate the function, use a **function-local** `use`
  instead of adding a top-level import.
- **A private helper called from a sibling module needs `pub(super)`**; one reached
  from `tui/` or `infra/` needs `pub(crate)`. Private *fields* need nothing - they
  are declared in `mod.rs` and visible to all descendants.
- New `impl App` methods go to the concern module that owns that state, not `mod.rs`.

Tests are colocated (`#[cfg(test)] mod tests`) in most - not all - modules. Shared
fixtures are `pub(super) fn`s in `test_support.rs`, imported as
`use crate::core::app::test_support::*;`.

### Playback ownership

- Two owners can hold the rodio sink: the native queue slot
  (`queue_now`) and a Qobuz session (`qobuz_playback`). The queue slot wins:
  while it plays, `qobuz_playback` is a suspended context, not the owner.
- A Qobuz start (`start_qobuz_queue`) and the queue router both claim the sink
  (`App::claim_decoded_sink`) before they start the player, and
  `active_decoded_source()` reads the claim, so the owner holds through the
  download, a failed start or a lost output device,
  until the queue runs out (the driver's teardown, the queue's
  exhausted-context resume) and `release_decoded_sink_claim` runs.
- `resume_or_finish` releases the claim only where no decoded context resumes.
  A resumed decoded context keeps it: only `start_*_queue` sets the claim,
  `play_index` does not.
- A queue restored from an older session can still hold `spotify:` or other
  non-Qobuz URIs; the queue skips them (`core::queue::is_qobuz_uri`). Never
  assume every queued URI is `qobuz:`. A `last_session.yml` from another
  source fails to load and is ignored.

### Errors

- Nothing else clears `api_error`. `update_on_tick` retires it once the
  lifetime passes, handing the text to the status bar only when the error frame
  is the current screen, so a frontend with no dismissal gesture does not latch
  the first failure forever. The CLI never ticks, so its latch is intact.
- Demote a site to `set_error_status_message` only when all three hold: it
  fires from the tick or a self-refreshing retry loop (so every retry restamps
  the lifetime and the backstop can never win), it is provably unreachable as a
  CLI exit signal, and the failed operation is bookkeeping rather than the thing
  the user asked for. One site qualifies today: `flush_state_save`, which
  latches its report to once per failure run. Repeating that report at the
  retry rate would hold `status_message_is_error` and silently drop every
  ordinary status message for the rest of the session.
