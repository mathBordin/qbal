-- accent-cycler: a `cycle_accent` command that rotates the theme accent color at runtime.
--
-- Theme overrides from set_theme are runtime-only; they reset when qbal restarts.
--
-- Install (single file; see PLUGINS.md for config path rules):
--   cp accent-cycler.lua "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins/"
--
-- Suggested binding, in config.yml in the qbal app config directory:
--   plugin_commands:
--     cycle_accent: "ctrl-y"

local accents = { "Magenta", "Cyan", "Green", "Yellow", "Red", "Blue" }
local index = 0

qbal.register_command("cycle_accent", function()
  index = (index % #accents) + 1
  local color = accents[index]
  qbal.set_theme({
    playbar_progress = color,
    hint = color,
    selected = color,
  })
  qbal.notify("Accent: " .. color, 2)
end)
