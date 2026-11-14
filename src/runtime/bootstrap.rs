//! The frontend-neutral bootstrap: logging, the panic hook, the ALSA error
//! silencer, config/state loading with migrations, Spotify authentication,
//! and `App` construction. `boot()` is the one entry point; `run_cli()` and
//! `run_gui()` call it.

use crate::core::app::App;
use crate::core::limits::MAX_PLAYBAR_ROWS;
use crate::core::migrations::{
  apply_legacy_client_config_migration, apply_legacy_config_runtime_state_migration,
};
use crate::core::onboarding::Onboarding;
use crate::core::state::{PersistedRuntimeState, RuntimeState};
use crate::core::user_config::{
  validate_tick_rate_milliseconds, BehaviorConfig, StartupBehavior, UserConfig, UserConfigPaths,
};
use crate::infra::network::IoEvent;
use anyhow::Result;
use backtrace::Backtrace;
use log::info;
use std::{io::Write, panic, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub(super) fn init_audio_backend() {}

pub(super) fn setup_logging(
  resolved_level: log::LevelFilter,
  target_levels: &[(&'static str, log::LevelFilter)],
) -> anyhow::Result<()> {
  let log_dir = crate::core::paths::app_log_dir();
  let log_path = crate::core::paths::app_log_path();

  // Owner-only, not plain create_dir_all: this sits in the shared OS temp
  // directory under a predictable name, so the default mode would leave the
  // logs readable by every other local user.
  crate::core::paths::ensure_private_dir(&log_dir).map_err(|e| {
    anyhow::anyhow!(
      "Failed to create log directory {}: {}",
      log_dir.display(),
      e
    )
  })?;
  // The global level never goes above `Info`: dependencies (reqwest, hyper,
  // rustls, h2, ...) are deliberately never pinned individually with
  // `level_for`, so the global default is what keeps them quiet at
  // `--debug`/`SPOTATUI_LOG=trace` without a pin list that has to be kept in
  // sync as dependencies change. Below `Info` (e.g. `SPOTATUI_LOG=warn`) the
  // requested level applies globally instead, since nothing needs raising.
  let global_level = resolved_level.min(log::LevelFilter::Info);
  let mut dispatch = fern::Dispatch::new()
    .format(|out, message, record| {
      out.finish(format_args!(
        "{}[{}][{}] {}",
        chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
        record.target(),
        record.level(),
        message
      ))
    })
    .level(global_level);
  for &(target, level) in target_levels {
    dispatch = dispatch.level_for(target, level);
  }
  dispatch
    .chain(fern::log_file(&log_path)?) // Use the dynamic path
    .apply()
    .map_err(|e| anyhow::anyhow!("Failed to initialize logger: {}", e))?;

  // Print the location of log for user reference — on stderr, not stdout.
  //
  // stdout belongs to program output: `spotatui history recap` writes HTML there
  // and `spotatui mcp` writes JSON-RPC there, where the MCP spec is explicit
  // that a server "MUST NOT write anything to its stdout that is not a valid MCP
  // message" (a stray line makes every client fail to start). A diagnostic notice
  // is exactly what stderr is for, and it stays visible on a terminal either way.
  eprintln!("Logging to: {}", log_path.display());

  Ok(())
}

pub(super) fn install_panic_hook() {
  let default_hook = panic::take_hook();
  panic::set_hook(Box::new(move |info| {
    let is_audio_backend_panic = info
      .location()
      .map(|location| {
        let file = location.file();
        file.contains("audio_backend/portaudio.rs") || file.contains("audio_backend/rodio.rs")
      })
      .unwrap_or(false);

    if is_audio_backend_panic {
      eprintln!(
        "Recoverable audio backend panic detected. Playback may pause while the output device changes."
      );
      return;
    }

    // The keepawake crate unwraps its D-Bus release; that panic ends only its own worker thread.
    if info
      .location()
      .is_some_and(|location| location.file().contains("keepawake"))
    {
      log::warn!("keepawake: the sleep inhibitor panicked: {info}");
      return;
    }

    #[cfg(feature = "tui")]
    ratatui::restore();
    let panic_log_path =
      crate::core::paths::app_state_dir().map(|dir| dir.join("spotatui_panic.log"));

    if let Some(path) = panic_log_path.as_ref() {
      if let Some(parent) = path.parent() {
        // Owner-only like every other state-dir creation: a panic can be the
        // first thing that ever creates the directory.
        let _ = crate::core::paths::ensure_private_dir(parent);
      }
      if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
      {
        let _ = writeln!(f, "\n==== spotatui panic ====");
        let _ = writeln!(f, "{}", info);
        let _ = writeln!(f, "{:?}", Backtrace::new());
        // Only the tail (at most the last 64 KiB / 200 lines) is read, never
        // the whole file: the running log can be megabytes by the time a
        // panic happens. `read_log_tail` never panics (every fallible step
        // returns `None`), which matters here — a panic inside a panic hook
        // aborts the process with no message at all.
        let log_tail =
          super::logging::read_log_tail(&crate::core::paths::app_log_path(), 64 * 1024, 200);
        if let Some(tail) = log_tail {
          if !tail.is_empty() {
            let _ = writeln!(f, "\n==== log tail (up to the last 200 lines) ====");
            let _ = writeln!(f, "{tail}");
          }
        }
      }
      eprintln!(
        "A crash log was written to: {} (rerun with --debug for a much more detailed log)",
        path.to_string_lossy()
      );
    }
    default_hook(info);

    if cfg!(debug_assertions) && std::env::var_os("RUST_BACKTRACE").is_none() {
      eprintln!("{:?}", Backtrace::new());
    }

    if cfg!(target_os = "windows") && std::env::var_os("SPOTATUI_PAUSE_ON_PANIC").is_some() {
      eprintln!("Press Enter to close...");
      let mut s = String::new();
      let _ = std::io::stdin().read_line(&mut s);
    }
  }));
}

fn apply_configured_runtime_defaults(
  runtime_state: &mut RuntimeState,
  persisted_state: &PersistedRuntimeState,
  behavior: &BehaviorConfig,
) -> PersistedRuntimeState {
  let mut applied_defaults = PersistedRuntimeState::default();

  if persisted_state.volume_percent.is_none() {
    if let Some(volume_percent) = behavior.volume_percent {
      runtime_state.volume_percent = volume_percent.min(100);
      applied_defaults.volume_percent = Some(runtime_state.volume_percent);
    }
  }
  if persisted_state.sidebar_width_percent.is_none() {
    if let Some(sidebar_width_percent) = behavior.sidebar_width_percent {
      runtime_state.sidebar_width_percent = sidebar_width_percent.min(100);
      applied_defaults.sidebar_width_percent = Some(runtime_state.sidebar_width_percent);
    }
  }
  if persisted_state.playbar_height_rows.is_none() {
    if let Some(playbar_height_rows) = behavior.playbar_height_rows {
      runtime_state.playbar_height_rows = playbar_height_rows.min(MAX_PLAYBAR_ROWS);
      applied_defaults.playbar_height_rows = Some(runtime_state.playbar_height_rows);
    }
  }
  if persisted_state.library_height_percent.is_none() {
    if let Some(library_height_percent) = behavior.library_height_percent {
      runtime_state.library_height_percent = library_height_percent.min(100);
      applied_defaults.library_height_percent = Some(runtime_state.library_height_percent);
    }
  }

  applied_defaults
}

/// Everything the shared bootstrap hands to whichever consumer runs next
/// (CLI dispatch or a frontend launch). Several fields are read by only one
/// consumer; each `cfg_attr` allowance names the build that leaves the field
/// unread.
pub(super) struct Boot {
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) app: Arc<Mutex<App>>,
  /// Consumed by the IoEvent pump. CLI mode holds it (unread) for the length
  /// of the command so an `App::dispatch` during CLI handling is silently
  /// queued rather than logged as a send error.
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) sync_io_rx: std::sync::mpsc::Receiver<IoEvent>,
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) user_config: UserConfig,
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) restore_playback: Option<crate::core::persisted_playback::PersistedPlayback>,
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) restore_queue: Vec<crate::core::plugin_api::TrackInfo>,
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) initial_shuffle_enabled: bool,
  #[cfg_attr(not(feature = "tui"), allow(dead_code))]
  pub(super) initial_startup_behavior: StartupBehavior,
}
/// The launch inputs `boot` reads, built from the command line by `cli::boot_options`.
pub(super) struct BootOptions {
  pub(super) system_language: Option<crate::core::i18n::Language>,
  pub(super) config_path: Option<PathBuf>,
  pub(super) tick_rate: Option<u64>,
  pub(super) subcommand: Option<String>,
}

/// The shared bootstrap sequence: user config, runtime state, the persisted
/// playback session, and `App` construction. Frontend-neutral: every
/// interactive step goes through `onboarding`.
pub(super) async fn boot(options: BootOptions, onboarding: Arc<dyn Onboarding>) -> Result<Boot> {
  let mut user_config = UserConfig::new();
  if let Some(config_file_path) = options.config_path {
    let path = UserConfigPaths { config_file_path };
    user_config.path_to_config.replace(path);
  }
  user_config.load_config()?;
  let system_language = if options.subcommand.is_some() {
    crate::core::i18n::Language::En
  } else {
    options
      .system_language
      .unwrap_or_else(crate::core::i18n::system_language)
  };
  let language = if options.subcommand.is_some() {
    crate::core::i18n::Language::En
  } else {
    user_config.behavior.language.resolve(system_language)
  };
  onboarding.set_language(language);
  let mut runtime_state = RuntimeState::default();
  let mut persisted_state = PersistedRuntimeState::default();
  let mut state_path = None;
  let mut should_save_initial_state = false;
  let mut can_save_initial_state = false;
  let legacy_client_path = crate::core::paths::legacy_client_config_path();
  match crate::core::state::default_state_path() {
    Ok(path) => {
      let state_file_exists = path.exists();
      match crate::core::state::load(&path) {
        Ok(state) => {
          persisted_state = state;
          runtime_state.apply_persisted(&persisted_state);
          if !state_file_exists {
            should_save_initial_state = true;
          }
          can_save_initial_state = true;
          state_path = Some(path);
        }
        Err(e) => {
          log::warn!("[state] ignoring unreadable runtime state: {e}");
        }
      }
    }
    Err(e) => {
      log::warn!("[state] runtime state path is unavailable: {e}");
    }
  }
  let default_fields =
    apply_configured_runtime_defaults(&mut runtime_state, &persisted_state, &user_config.behavior);
  let legacy_runtime_fields = match user_config
    .path_to_config
    .as_ref()
    .map(|paths| paths.config_file_path.clone())
  {
    Some(config_path) => match apply_legacy_config_runtime_state_migration(
      &config_path,
      &mut runtime_state,
      &persisted_state,
    ) {
      Ok(fields) => fields,
      Err(e) => {
        log::warn!("[state] failed to read legacy runtime fields from config.yml: {e}");
        PersistedRuntimeState::default()
      }
    },
    None => PersistedRuntimeState::default(),
  };
  let legacy_client_fields = match (
    legacy_client_path.as_deref(),
    user_config
      .path_to_config
      .as_ref()
      .map(|paths| paths.config_file_path.clone()),
  ) {
    (Some(client_path), Some(config_path)) => match apply_legacy_client_config_migration(
      client_path,
      &config_path,
      &mut user_config.behavior,
      &mut runtime_state,
      &persisted_state,
    ) {
      Ok(fields) => fields,
      Err(e) => {
        log::warn!("[state] failed to read the legacy client.yml: {e}");
        PersistedRuntimeState::default()
      }
    },
    _ => PersistedRuntimeState::default(),
  };
  let state = if should_save_initial_state {
    runtime_state.to_persisted()
  } else {
    let mut state = default_fields;
    state.merge_patch(&legacy_runtime_fields);
    state.merge_patch(&legacy_client_fields);
    state
  };
  if can_save_initial_state && !state.is_empty() {
    if let Some(path) = &state_path {
      if let Err(e) = crate::core::state::save(path, &state) {
        log::warn!("[state] failed to save initial runtime state: {e}");
      }
    }
  }
  info!("user config loaded successfully");

  let initial_shuffle_enabled = runtime_state.shuffle_enabled;
  let initial_startup_behavior = user_config.behavior.startup_behavior;

  // Load the persisted non-Spotify playback session so the last song can resume
  // on launch. The file's mere existence means a non-Spotify source was playing
  // at the last save: the runner clears it whenever playback stops or switches
  // to Spotify, so a present file is always something worth resuming — even when
  // the browse source was later switched to Spotify while the song kept playing
  // (browse-source and playback-source are deliberately decoupled). A session
  // whose source feature isn't compiled into this build is a no-op on restore.
  let restore_session: Option<crate::core::persisted_playback::PersistedSession> =
    match crate::core::persisted_playback::default_session_path()
      .and_then(|path| crate::core::persisted_playback::load(&path))
    {
      Ok(session) => session,
      Err(e) => {
        log::warn!("[session] ignoring unreadable playback session: {e}");
        None
      }
    };
  // Split the session: the playback (if any) drives the source resume, while the
  // native queue is restored into app state regardless of whether a source is
  // resumed (a queue-only session must not suppress Spotify's device transfer).
  let (restore_playback, restore_queue): (
    Option<crate::core::persisted_playback::PersistedPlayback>,
    Vec<crate::core::plugin_api::TrackInfo>,
  ) = match restore_session {
    Some(s) => (s.playback, s.queue),
    None => (None, Vec::new()),
  };

  if let Some(tick_rate) = options.tick_rate {
    user_config.behavior.tick_rate_milliseconds =
      validate_tick_rate_milliseconds(tick_rate, "Tick rate")?;
  }

  let (sync_io_tx, sync_io_rx) = std::sync::mpsc::channel::<IoEvent>();
  info!("app state initialized");

  // Initialise app state
  let app = Arc::new(Mutex::new(App::new_with_state(
    sync_io_tx,
    user_config.clone(),
    runtime_state.clone(),
    state_path.clone(),
    system_language,
  )));

  if options.subcommand.is_some() {
    app.lock().await.use_english_messages();
  }

  Ok(Boot {
    app,
    sync_io_rx,
    user_config,
    restore_playback,
    restore_queue,
    initial_shuffle_enabled,
    initial_startup_behavior,
  })
}

#[cfg(test)]
mod tests {}
