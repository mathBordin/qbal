# Contributing to Qbal

Bug reports, fixes, translations and themes are all welcome.

- [Report a bug](#report-a-bug)
- [Set up](#set-up)
- [Where things live](#where-things-live)
- [Before you open a PR](#before-you-open-a-pr)
- [Adding interface text](#adding-interface-text)
- [If the gates test fails](#if-the-gates-test-fails)
- [Using AI Tools](#using-ai-tools)

## Report a bug

Open an [issue](https://github.com/mathBordin/qbal/issues) with what you did,
what you expected and what happened. Attach the log: run `qbal --debug`,
reproduce the problem, and attach the file whose path Qbal prints when it exits
(read it first; see [Logging](docs/configuration.md#logging)). After a crash,
attach `~/.local/state/qbal/qbal_panic.log`.

## Set up

Install Rust and the system libraries from
[docs/installation.md](docs/installation.md), then:

```bash
git clone https://github.com/mathBordin/qbal
cd qbal
cargo run                               # default build
cargo run --features cover-art,audio-viz  # everything
cargo run --no-default-features -- history recap   # headless, CLI only
```

## Where things live

| Folder | What is in it |
| --- | --- |
| `src/core/` | App state, the tick scheduler, actions, config and state files, translations |
| `src/infra/` | Qobuz, the audio player and visualizer capture, the queue, lyrics and cover art, listening history, Lua scripting, MPRIS and Discord |
| `src/tui/` | The terminal interface: event loop, key handlers, drawing |
| `src/cli/` | Command-line subcommands |
| `src/runtime/` | Start-up and the background task pump |
| `docs/` | User documentation |
| `examples/` | Example config and Lua plugins |

[`AGENTS.md`](AGENTS.md) describes the architecture and the rules the code
follows; the `AGENTS.md` files in subfolders cover each area.

## Before you open a PR

Run what CI runs:

```bash
cargo fmt --all
cargo clippy -- -D warnings
cargo clippy --no-default-features -- -D warnings
cargo clippy --features cover-art,audio-viz -- -D warnings
cargo test
```

CI builds and tests three feature sets on Linux: the default build, the full
build (`--features cover-art,audio-viz`) and the headless build
(`--no-default-features`); see [`ci.yml`](.github/workflows/ci.yml).

Add or update tests when you change behaviour, and add a line under
`## [Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md) for a change users notice.

PR titles follow `type(scope): subject`, for example
`fix(queue): keep the order after a skip`. Write `Fixes #N` only when the PR
fixes the whole issue.

## Adding interface text

The interface ships in English, Portuguese (Brazil) and French. Each group of
strings in `src/core/i18n/` has an `EN` table plus `PT_BR` and `FR`; the large
settings group keeps them in `src/core/i18n/settings/{en,pt_br,fr}.rs`.

- Add a field to the group and to `EN`, then read it through `app.strings()`.
- Add the Portuguese and French text too. A table can fall back to English with
  `..EN`, but a new string should not stay English for long.
- Keep placeholders named (`{count}`, `{name}`), with a small typed method that
  calls `format_template`, and add a sentinel call for it in
  `template_tests.rs`. Never build a sentence by joining translated pieces.
- Leave key names, config values, product names and the user's own content
  (track titles and so on) untranslated.

`cargo test reports_entries_still_in_english -- --nocapture` lists what is
still in English. The tests that fail the build check that no entry is empty
and that every translation keeps the English placeholder names.

## If the gates test fails

`cargo test` includes a gates test. It counts how often code reaches across the
app's layers, for example a key handler that writes a field of `App` directly,
and keeps those counts in [`tools/gates.count`](tools/gates.count). The counts
may only go down.

When your change makes one go up, change the code rather than the number: use
an `App` method, `app.apply(Action::...)`, or `app.view` for presentation
state. The failure names the counter; [`src/gates.rs`](src/gates.rs) says what
each one measures. When a count goes down, lower its baseline in the same PR.

The last two counters, `test_attribute_total` and `action_refs_in_tui_handlers`,
work the other way: they may only go up, and a workflow raises them on `main`
after a merge, so leave them alone in a PR.

### Using AI Tools

AI tools are welcome. How you wrote the code matters less than whether you
checked it. You are the author of your PR, whatever wrote the first draft:

- Read your whole diff before you open the PR, and be ready to explain any line
  of it.
- Run the checks yourself and paste their output in the PR. Do not claim a check
  you did not run.
- Change only what the PR is about. No unrelated rewrites or deletions.
- Write the description yourself: what changed and why.

PRs that delete unrelated content or claim checks that were never run are
closed without a detailed review.
