//! Bootstrap and the processes every frontend shares.
//!
//! `run_cli()` is the console entry point: it wires logging, parses the
//! command line, runs the shared `bootstrap::boot()` sequence (config, state,
//! auth, `App`), and then either executes one CLI subcommand or launches the
//! terminal UI. The IoEvent pump every frontend drives lives in `pump`, and
//! the native-streaming startup every frontend shares lives in `streaming`.

mod bootstrap;
mod cli;
#[cfg(feature = "tui")]
mod instance;
mod logging;
mod pump;
#[cfg(feature = "tui")]
mod startup;

use crate::core::migrations::apply_legacy_state_file_migrations;
use anyhow::{anyhow, Result};
use clap_complete::{generate, Shell};
use log::info;
use std::io;
use std::sync::Arc;

/// Console implementation of the first-launch surface for builds without the
/// terminal frontend. There is no interactive picker to show here: the only
/// source is Qobuz, and a headless build has no UI launch to run one from.
#[cfg(not(feature = "tui"))]
struct HeadlessOnboarding;

#[cfg(not(feature = "tui"))]
impl crate::core::onboarding::Onboarding for HeadlessOnboarding {}

pub async fn run_cli() -> Result<()> {
  let result = run_cli_inner().await;
  // A failing run is the one that gets reported, so it needs the log path
  // most — and `?` inside carries every failure straight past the notice at
  // the bottom. Checked rather than assumed: `setup_logging` is itself one of
  // the steps that can fail, and pointing at a file that was never created
  // sends the reporter looking for something that is not there.
  if result.as_ref().is_err_and(|e| !is_instance_refusal(e)) {
    let path = crate::core::paths::app_log_path();
    if path.is_file() {
      eprintln!(
        "{}",
        logging::exit_log_notice(&path, log::max_level() >= log::LevelFilter::Debug)
      );
    }
  }
  result
}

/// Opens the log at the level `--debug` or `SPOTATUI_LOG` asks for, then writes the startup header.
fn start_logging(debug_flag: bool) -> Result<()> {
  let env_log_value = std::env::var("SPOTATUI_LOG").ok();
  let (log_level, log_level_warning) =
    logging::resolve_log_level(debug_flag, env_log_value.as_deref());
  bootstrap::setup_logging(log_level, &logging::target_levels(log_level))?;
  if let Some(warning) = log_level_warning {
    log::warn!("{warning}");
  }

  // Always on, so a bug report has version/platform/build context in every
  // log, not just `--debug` ones.
  info!(
    "{}",
    logging::startup_header(
      env!("CARGO_PKG_VERSION"),
      std::env::consts::OS,
      std::env::consts::ARCH,
      &logging::compiled_features(),
      std::env::var("TERM").ok().as_deref(),
      std::env::var("TERM_PROGRAM").ok().as_deref(),
      std::env::var_os("WT_SESSION").is_some(),
    )
  );
  Ok(())
}

async fn run_cli_inner() -> Result<()> {
  let mut clap_app = cli::build_clap_app();

  let matches = clap_app.clone().get_matches();

  // Logging depends on the parsed flags (`--debug`), so it moves here from
  // being the very first statement. Accepted consequence: a clap usage error
  // above (bad flag, `--help`, `--version`) exits before any log file exists.
  start_logging(matches.get_flag("debug"))?;

  info!("spotatui {} starting up", env!("CARGO_PKG_VERSION"));
  bootstrap::init_audio_backend();
  info!("audio backend initialized");

  bootstrap::install_panic_hook();
  info!("panic hook configured");

  // Shell completions don't need any spotify work
  if let Some(s) = matches.get_one::<String>("completions") {
    let shell =
      completion_shell(s).ok_or_else(|| anyhow!("no completions available for '{}'", s))?;
    generate(shell, &mut clap_app, "spotatui", &mut io::stdout());
    return Ok(());
  }

  // Held until this function returns, so a second UI launch is refused while
  // this one runs.
  #[cfg(feature = "tui")]
  let _instance_lock = if instance::takes_lock(matches.subcommand_name()) {
    instance::acquire()?
  } else {
    None
  };

  if let Err(e) = apply_legacy_state_file_migrations() {
    log::warn!("[state] failed to migrate legacy app data files: {e}");
  }

  if let Some(history_matches) = matches.subcommand_matches("history") {
    println!("{}", crate::cli::handle_history_matches(history_matches)?);
    return Ok(());
  }

  // Plugin management is pure git + filesystem work; it must not require Spotify auth.
  #[cfg(feature = "scripting")]
  if let Some(plugin_matches) = matches.subcommand_matches("plugin") {
    crate::cli::handle_plugin_command(plugin_matches)?;
    return Ok(());
  }

  // Without the terminal frontend there is no interactive UI to launch; only
  // the CLI subcommands work. Bail before any first-run prompt would fire.
  #[cfg(not(feature = "tui"))]
  if matches.subcommand_name().is_none() {
    return Err(anyhow!(
      "this spotatui build has no terminal UI (compiled without the `tui` feature); run a CLI subcommand instead"
    ));
  }

  // The console implementation of the first-launch surface; core and infra
  // only ever see the `Onboarding` trait. `Arc` so the blocking streaming
  // credential task in the UI launch can hold its own handle.
  #[cfg(feature = "tui")]
  let onboarding: Arc<dyn crate::core::onboarding::Onboarding> =
    Arc::new(crate::tui::onboarding::ConsoleOnboarding::default());
  #[cfg(not(feature = "tui"))]
  let onboarding: Arc<dyn crate::core::onboarding::Onboarding> = Arc::new(HeadlessOnboarding);

  let boot = bootstrap::boot(cli::boot_options(&matches), onboarding).await?;

  // Every subcommand returned above; what is left is the UI launch.
  #[cfg(feature = "tui")]
  {
    startup::launch_ui(boot).await?;
    // On stderr, same reasoning as the "Logging to:" notice in `setup_logging`:
    // stdout is reserved for program output (history HTML). `--completions`,
    // `update`, `history`, `plugin`, and the headless no-subcommand error all
    // return earlier and skip it. The failure case is covered by `run_cli`.
    eprintln!(
      "{}",
      logging::exit_log_notice(
        &crate::core::paths::app_log_path(),
        log::max_level() >= log::LevelFilter::Debug,
      )
    );
    Ok(())
  }
  #[cfg(not(feature = "tui"))]
  {
    let _ = boot;
    unreachable!("headless builds reject a UI launch before boot");
  }
}

/// A second UI launch refused by the instance lock is expected, not a failure to report.
fn is_instance_refusal(error: &anyhow::Error) -> bool {
  #[cfg(feature = "tui")]
  {
    error.is::<instance::AlreadyRunning>()
  }
  #[cfg(not(feature = "tui"))]
  {
    let _ = error;
    false
  }
}

/// The shell for a `--completions` value. clap passes the value as typed, so
/// the old `power-shell` alias arrives here unchanged and needs its own arm.
fn completion_shell(name: &str) -> Option<Shell> {
  match name {
    "fish" => Some(Shell::Fish),
    "bash" => Some(Shell::Bash),
    "zsh" => Some(Shell::Zsh),
    "powershell" | "power-shell" => Some(Shell::PowerShell),
    "elvish" => Some(Shell::Elvish),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn completion_shell_maps_both_powershell_spellings() {
    assert_eq!(completion_shell("powershell"), Some(Shell::PowerShell));
    assert_eq!(completion_shell("power-shell"), Some(Shell::PowerShell));
    assert_eq!(completion_shell("nushell"), None);
  }
}
