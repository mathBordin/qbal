-- queue-browser: a custom plugin screen that lists the play queue and your
-- Qobuz playlists, showing off custom screens, reads, timers, storage and
-- actions.
--
-- Install (single file; see PLUGINS.md for config path rules):
--   cp queue-browser.lua "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins/"
--
-- Suggested binding, in config.yml:
--   plugin_commands:
--     queue_browser: "ctrl-b"
--
-- Keys inside the screen: j/k move, enter plays the selected queued track,
-- Esc leaves. The screen refreshes itself every second while open.

qbal.require_api(5)

local SCREEN = "queue_browser"

local queued = {} -- { { name = ..., uri = ... }, ... }
local selected = 1
local timer = nil

local function render()
  local queue = qbal.queue()
  queued = {}
  for _, item in ipairs(queue.items or {}) do
    if item.track and item.track.uri then
      queued[#queued + 1] = {
        name = item.track.name .. " - " .. table.concat(item.track.artists, ", "),
        uri = item.track.uri,
      }
    end
  end
  selected = math.max(1, math.min(selected, #queued))

  local now = queue.currently_playing and queue.currently_playing.track
  local pb = qbal.playback()
  local now_name = (now and now.name) or (pb and pb.track and pb.track.name) or "nothing"

  local items = {}
  for _, entry in ipairs(queued) do
    items[#items + 1] = entry.name
  end
  if #items == 0 then
    items = { { text = "(the queue is empty: press z on a track to add it)", italic = true } }
  end

  local playlists = {}
  for _, p in ipairs(qbal.playlists()) do
    playlists[#playlists + 1] = p.name
  end

  qbal.set_screen(SCREEN, {
    {
      type = "paragraph",
      height = 3,
      lines = {
        { text = "j/k move - enter plays the selected track - Esc leaves", italic = true },
        { text = "Now playing: " .. now_name, bold = true, fg = "Green" },
        { text = "opened " .. (qbal.storage_get("opens") or 0) .. " times", fg = "Yellow" },
      },
    },
    { type = "list", title = "Queue", items = items, selected = selected },
    { type = "list", title = "Playlists", items = playlists },
  })
end

qbal.register_screen(SCREEN, {
  title = "Queue Browser",
  on_key = function(key)
    if key == "j" and selected < #queued then
      selected = selected + 1
    elseif key == "k" and selected > 1 then
      selected = selected - 1
    elseif key == "enter" and queued[selected] then
      qbal.play_uri(queued[selected].uri)
      qbal.notify("Playing: " .. queued[selected].name, 3)
    end
    render()
  end,
  on_open = function()
    qbal.storage_set("opens", (qbal.storage_get("opens") or 0) + 1)
    render()
    timer = qbal.set_interval(1000, render)
  end,
  on_close = function()
    if timer then
      qbal.cancel_timer(timer)
      timer = nil
    end
  end,
})

qbal.register_command("queue_browser", function()
  qbal.show_screen(SCREEN)
end)
