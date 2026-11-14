### CLI subcommands

Subcommands are assembled in `src/runtime/cli.rs::build_clap_app` and
dispatched in `src/runtime/mod.rs::run_cli`: `history` and `plugin` return
before config load and before the single-instance lock matters. Place new
subcommands deliberately. stdout is output-only (`history recap` writes HTML) -
diagnostics go to stderr; never add a `println!` on a path a subcommand can
reach.
