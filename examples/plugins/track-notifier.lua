-- track-notifier: show a "Now playing" toast and a playbar segment on every track change.
--
-- Install (single file; see PLUGINS.md for config path rules):
--   cp track-notifier.lua "${XDG_CONFIG_HOME:-$HOME/.config}/qbal/plugins/"
-- Then restart qbal.

qbal.on("track_change", function(pb)
  if pb and pb.track then
    local artists = table.concat(pb.track.artists, ", ")
    qbal.notify("Now playing: " .. pb.track.name .. " - " .. artists, 4)
    qbal.set_playbar(pb.track.name)
  else
    -- Nothing playing: clear our playbar segment.
    qbal.set_playbar(nil)
  end
end)

qbal.on("start", function()
  qbal.log("track-notifier loaded (api version " .. qbal.api_version .. ")")
end)
