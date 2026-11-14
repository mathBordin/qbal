//! The command-line surface: clap assembly.

use super::bootstrap::BootOptions;
use crate::cli;
use crate::core::banner::BANNER;
use clap::{builder::PossibleValue, Arg, ArgMatches, Command as ClapApp};
use std::path::PathBuf;

pub(super) fn boot_options(matches: &ArgMatches) -> BootOptions {
  BootOptions {
    system_language: None,
    config_path: matches.get_one::<String>("config").map(PathBuf::from),
    tick_rate: matches.get_one::<u64>("tick-rate").copied(),
    subcommand: matches.subcommand_name().map(str::to_owned),
  }
}

pub(super) fn build_clap_app() -> ClapApp {
  // `mut` is only exercised by the feature-gated subcommand additions below.
  #[cfg_attr(not(feature = "scripting"), allow(unused_mut))]
  let mut clap_app = ClapApp::new(env!("CARGO_PKG_NAME"))
      .version(env!("CARGO_PKG_VERSION"))
      .author(env!("CARGO_PKG_AUTHORS"))
      .about(env!("CARGO_PKG_DESCRIPTION"))
      .override_usage("Press `?` while running the app to see keybindings")
      .before_help(BANNER)
      .arg(
        Arg::new("tick-rate")
          .short('t')
          .long("tick-rate")
          .value_parser(clap::value_parser!(u64))
          .help("Set the normal UI tick rate in milliseconds.")
          .long_help(
            "Specify the normal UI tick rate in milliseconds. Lower values refresh non-animated \
screens more often and cost more CPU. Animation-heavy views keep their separate animation tick rate.",
          ),
      )
      .arg(
        Arg::new("config")
          .short('c')
          .long("config")
          .help("Specify configuration file path."),
      )
      .arg(
        Arg::new("completions")
          .long("completions")
          .help("Generates completions for your preferred shell")
          .value_parser([
            PossibleValue::new("bash"),
            PossibleValue::new("zsh"),
            PossibleValue::new("fish"),
            PossibleValue::new("powershell").alias("power-shell"),
            PossibleValue::new("elvish"),
          ])
          .value_name("SHELL"),
      )
      .arg(
        Arg::new("debug")
          .long("debug")
          .global(true)
          .action(clap::ArgAction::SetTrue)
          .help("Enable verbose debug logging for bug reports (also QBAL_LOG=debug)")
          .long_help(
            "Enable verbose debug logging (equivalent to QBAL_LOG=debug). At this level the \
log records identifying details such as playlist ids. QBAL_LOG=trace goes further. Access \
tokens and request headers are never logged, but read through the file before posting it \
publicly.",
          ),
      )
      .subcommand(cli::history_subcommand());

  #[cfg(feature = "scripting")]
  {
    clap_app = clap_app.subcommand(cli::plugin_subcommand());
  }

  clap_app
}

#[cfg(test)]
mod tests {
  use super::*;

  /// `--debug` is the flag every bug report in issue #566 is going to be asked
  /// for, and the help text is the only place a user learns that the log can
  /// carry personal data. Renaming either is a silent regression otherwise:
  /// nothing else in the suite builds the clap app.
  #[test]
  fn debug_flag_parses_and_defaults_to_off() {
    let app = build_clap_app();
    let matches = app
      .clone()
      .try_get_matches_from(["qbal", "--debug"])
      .expect("--debug must parse");
    assert!(matches.get_flag("debug"));

    let matches = app
      .try_get_matches_from(["qbal"])
      .expect("no args must parse");
    assert!(!matches.get_flag("debug"));
  }

  /// `--debug` needs `global(true)` to parse after a subcommand
  /// (`qbal history recap --debug`), not just before one. `run_cli`
  /// reads it off the root `ArgMatches` (`matches.get_flag("debug")` in
  /// `runtime/mod.rs`), and clap copies a global flag's value back up to
  /// every parent level it was propagated through, so that read keeps working
  /// once the flag is placed on the subcommand line instead of the root one.
  #[test]
  fn debug_flag_parses_after_a_subcommand() {
    let matches = build_clap_app()
      .try_get_matches_from(["qbal", "history", "recap", "--debug"])
      .expect("--debug after a subcommand must parse");
    assert!(matches.get_flag("debug"));
  }

  #[test]
  fn debug_flag_long_help_warns_about_personal_data() {
    let help = build_clap_app()
      .get_arguments()
      .find(|arg| arg.get_id() == "debug")
      .and_then(|arg| arg.get_long_help().or_else(|| arg.get_help()))
      .map(ToString::to_string)
      .expect("--debug must document itself");
    let help = help.to_lowercase();
    assert!(
      help.contains("before posting"),
      "the long help must tell the user to read the log before posting it:\n{help}"
    );
    assert!(
      help.contains("playlist ids"),
      "the long help must name what the log records:\n{help}"
    );
  }

  #[test]
  fn tick_rate_flag_rejects_invalid_values() {
    for bad in ["fast", "250ms", "1.5", "-5"] {
      let err = build_clap_app()
        .try_get_matches_from(["qbal", &format!("--tick-rate={bad}")])
        .expect_err("invalid tick-rate must be rejected at parse time");
      assert!(
        err.to_string().contains("tick-rate"),
        "error must name the flag: {err}"
      );
    }
  }

  #[test]
  fn out_of_range_tick_rate_still_reaches_boot_options() {
    // The 1..=999 range check stays in `bootstrap.rs`, shared with `config.yml`
    // loading, so an out-of-range value must still parse here and reach boot.
    let matches = build_clap_app()
      .try_get_matches_from(["qbal", "-t", "5000"])
      .expect("out-of-range tick-rate must parse; the range check lives in bootstrap");
    assert_eq!(boot_options(&matches).tick_rate, Some(5000));
  }

  #[test]
  fn completions_accepts_powershell() {
    let matches = build_clap_app()
      .try_get_matches_from(["qbal", "--completions", "powershell"])
      .unwrap();
    assert_eq!(
      matches.get_one::<String>("completions").map(String::as_str),
      Some("powershell")
    );
  }

  #[test]
  fn completions_still_accepts_the_old_power_shell_spelling() {
    let matches = build_clap_app()
      .try_get_matches_from(["qbal", "--completions", "power-shell"])
      .unwrap();
    assert_eq!(
      matches.get_one::<String>("completions").map(String::as_str),
      Some("power-shell")
    );
  }

  #[test]
  fn completions_rejects_an_unknown_shell() {
    assert!(build_clap_app()
      .try_get_matches_from(["qbal", "--completions", "nushell"])
      .is_err());
  }
}
