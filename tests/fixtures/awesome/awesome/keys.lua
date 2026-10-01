local modkey = "Mod4"
local globalkeys = gears.table.join(
	awful.key({ modkey }, "i", function()
		require("components/cheatsheet").toggle()
	end, { description = "show keyboard shortcuts", group = "awesome" }),
	awful.key({ modkey, "Shift" }, "c", function(c) c:kill() end, { description = "close", group = "client" }),
	awful.key({ modkey }, "Left", awful.tag.viewprev, { description = "view previous", group = "tag" }),
	awful.key({ modkey, "Control" }, "Return", function() end),
	awful.key({}, "XF86AudioMute", function() end, { description = "mute", group = "volume" })
)
