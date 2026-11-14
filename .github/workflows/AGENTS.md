## CI Checks (run before opening a PR)

- `ci.yml` runs `check`, `test` and `clippy -D warnings` on three Linux legs:
  `default` (`--locked`), `all-features` (`--locked --features
  cover-art,audio-viz`, which needs `libpipewire-0.3-dev libspa-0.2-dev clang`)
  and `headless` (`--locked --no-default-features`).
  Reproduce them locally with `cargo test` / `cargo clippy -- -D warnings` and
  the same commands with `--no-default-features`.
- CI runs on Linux only, the only platform Qbal supports.
- `coverage` runs `cargo llvm-cov` on the default feature set and uploads to
  Codecov; a failed upload never fails the job.
- `gates-ratchet` (pull requests only) compares `tools/gates.count` against the
  merge-base; `gates-floor.yml` raises the adoption floors on `main`.
- There is no release workflow: build binaries locally with
  `cargo build --release`.
