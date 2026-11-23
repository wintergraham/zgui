-- The titlebar, across the window: the traffic lights, the sidebar's
-- toggle, the title over the panels (it moves with them as the sidebar
-- opens and closes) and the tools, left of the window's buttons where they
-- sit at the right (Windows). Empty space moves the window.
local theme = require("../theme")
local icons = require("../icons")
local state = require("../state")
local actions = require("../actions")
local controls = require("./controls")
local layout = require("@lib/layout")

-- The traffic lights' room, and the toggle's.
local LIGHTS, TOGGLE = 78, 28

-- The window's own buttons' width where they sit at the title bar's right
-- (Windows' caption buttons, a Linux desktop's), or nil where they are at
-- its left (macOS's traffic lights) or the platform does not say.
local function right_buttons(): number?
	local area, buttons = ui.window.titlebar_area:get(), ui.window.controls:get()
	if area and buttons and buttons.x >= area.x + area.width then
		return buttons.width
	end
	return nil
end

-- The room before the toggle: the traffic lights', or an inset where the
-- buttons are at the right.
local function lead(): number
	return if right_buttons() then theme.inset + 2 else LIGHTS
end

return function(): Node
	return ui.row {
		w_full = true,
		h = theme.titlebar,
		shrink = 0,
		-- Clear of the window's buttons where they are at the right.
		pr = function()
			local buttons = right_buttons()
			return if buttons then buttons + 8 else theme.inset + 2
		end,
		gap = 2,
		items = "center",
		controls.drag_space { w = lead, h_full = true, shrink = 0 },
		ui.button {
			w = TOGGLE,
			h = TOGGLE,
			p = 0,
			rounded = 8,
			items = "center",
			justify = "center",
			shrink = 0,
			bg = theme.clear,
			hover = { bg = "#ffffff14" },
			active = { bg = "#ffffff1f" },
			focus = { border = 0 },
			on_click = actions.toggle_sidebar,
			controls.sidebar_glyph(function()
				return state.sidebar_open:get()
			end, 17, theme.secondary),
		},
		controls.drag_space {
			h_full = true,
			grow = 1,
			min_w = 0,
			direction = "row",
			items = "center",
			gap = 10,
			-- The title starts over the panels while the sidebar shows, just
			-- after the toggle while it does not.
			ui.div {
				shrink = 0,
				w = function()
					return math.max(state.sidebar_shown:get() + theme.inset - lead() - TOGGLE - 2, 0) + 8
				end,
			},
			ui.text { "Monitor", text_size = 13, weight = 600, color = theme.text, shrink = 0 },
			ui.text {
				function()
					local error = state.error:get()
					if error ~= "" then
						return error
					end
					local tree = state.root:get()
					local n = if tree then #layout.leaves(tree) else 0
					return `{n} panel{if n == 1 then "" else "s"} · every {state.interval / 1000} s · the last minute`
				end,
				shrink = 1,
				min_w = 0,
				truncate = true,
				text_size = 12,
				color = function()
					return if state.error:get() ~= "" then theme.exited else theme.tertiary
				end,
			},
		},
		controls.tool(icons.split_right, function()
			actions.split("row")
		end),
		controls.tool(icons.split_down, function()
			actions.split("column")
		end),
	}
end
