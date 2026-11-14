mod history;
#[cfg(feature = "scripting")]
mod plugin;

pub use self::history::{handle_history_matches, history_subcommand};
#[cfg(feature = "scripting")]
pub use self::plugin::{handle_plugin_command, plugin_subcommand};
