## CI Checks (run before opening a PR)

- `ci.yml` runs `check`, `test` and `clippy -D warnings` on two Linux legs:
  `default` (`--locked`) and `headless` (`--locked --no-default-features`).
  Reproduce them locally with `cargo test` / `cargo clippy -- -D warnings` and
  the same commands with `--no-default-features`.
- The `macos` job runs `check` + `clippy` with
  `tui,cover-art,audio-viz-cpal,macos-media,discord-rpc,scripting`. It is the
  only place the `#[cfg(target_os = "macos")]` arms compile, so an item used
  only by the macOS media-key handler looks dead on Linux without `mpris`.
- `coverage` runs `cargo llvm-cov` on the default feature set and uploads to
  Codecov; a failed upload never fails the job.
- `gates-ratchet` (pull requests only) compares `tools/gates.count` against the
  merge-base; `gates-floor.yml` raises the adoption floors on `main`.
- There is no release workflow: build binaries locally with
  `cargo build --release`.
