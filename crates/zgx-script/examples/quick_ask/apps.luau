-- The apps under the panel: Chrome, VS Code, Mail and Finder, drawn from the
-- board's "App window" frames. Finder's files can be picked and dragged.
local K = require("./kit")
local icon, text, mono, spacer = K.icon, K.text, K.mono, K.spacer

local M = {}

local function lights()
	return ui.row {
		gap = 7, items = "center", shrink = 0,
		ui.div { w = 11, h = 11, rounded = 6, bg = "#FF5F57" },
		ui.div { w = 11, h = 11, rounded = 6, bg = "#FEBC2E" },
		ui.div { w = 11, h = 11, rounded = 6, bg = "#28C840" },
	}
end

local function chrome()
	return ui.column {
		w_full = true, h_full = true, bg = "#DEE1E6",
		ui.row {
			w_full = true, h = 38, px = 12, gap = 8, items = "end",
			ui.row { h_full = true, pl = 2, pr = 10, items = "center", lights() },
			ui.row {
				w = 220, h = 30, px = 10, gap = 8, bg = "#FFFFFF", corners = { 8, 8, 0, 0 }, items = "center",
				ui.div { w = 14, h = 14, rounded = 3, bg = "#A0522D" },
				text("Reverse-seared ribeye", 12, "#1F1F1F"),
			},
		},
		ui.row {
			w_full = true, h = 38, px = 12, gap = 12, bg = "#FFFFFF", items = "center",
			icon("arrow-left", 14, "#5F6368"), icon("arrow-right", 14, "#5F6368"), icon("rotate-cw", 14, "#5F6368"),
			ui.row {
				grow = 1, h = 28, px = 12, gap = 8, rounded = 14, bg = "#F1F3F4", items = "center",
				icon("lock", 11, "#5F6368"),
				text("kitchennotebook.com/technique/reverse-sear-ribeye", 12.5, "#202124"),
			},
		},
		ui.column {
			w_full = true, grow = 1, bg = "#F4F1EA", overflow = "hidden",
			ui.row {
				w_full = true, h = 56, px = 56, gap = 28, items = "center",
				border = 1, border_sides = { 0, 0, 1, 0 }, border_color = "#E2DDD2",
				text("The Kitchen Notebook", 19, "#1C1B18", { weight = 600 }),
				spacer(),
				text("Recipes", 13, "#57534B"), text("Techniques", 13, "#57534B"),
				text("Gear", 13, "#57534B"), text("Newsletter", 13, "#57534B"),
			},
			ui.column {
				w_full = true, max_w = 760, self = "center", px = 24, pt = 44, gap = 16,
				mono("TECHNIQUE · BEEF", 11, "#A0522D", { letter_spacing = 1 }),
				text("Reverse-seared ribeye, the patient way", 40, "#1C1B18",
					{ weight = 600, letter_spacing = -0.8, line_height = 44, wrap = true }),
				text("By Mara Ellison · 12 min read", 13, "#7A756B"),
				ui.div {
					w_full = true, h = 240, rounded = 6,
					gradient = { angle = 160, stops = { "#3B2A20", "#7A4B2E", "#C9935E" } },
				},
				text("Low heat first, then a hard sear. Starting the steak in a 250°F oven brings it to "
					.. "temperature evenly from edge to edge, so the final minute in a smoking pan only has to "
					.. "build the crust.", 16, "#3A3731", { wrap = true, line_height = 26 }),
			},
		},
	}
end

local CODE = {
	{ 12, "export function parseDueDate(input: string) {" },
	{ 13, "  // Accepts 2026-10-17 or 2026-10-17T09:41:00Z", "#6A9955" },
	{ 14, "  const ISO = /^(\\d{4})-(\\d{2})-(\\d{2})(?:T(\\d{2}):(\\d{2}))?/;", nil, true },
	{ 15, "  const match = input.trim().match(ISO);", nil, true },
	{ 16, "  if (!match) throw new InvalidDateError(input);", nil, true },
	{ 17, "  const [, y, m, d] = match.map(Number);" },
	{ 18, "  return new Date(Date.UTC(y, m - 1, d));" },
	{ 19, "}" },
	{ 20, " " },
	{ 21, "export const isOverdue = (due: Date) =>" },
	{ 22, "  due.getTime() < Date.now();" },
}

local function code(selected)
	local lines = {}
	for i, l in CODE do
		lines[i] = ui.row {
			w_full = true, h = 22, px = 16, gap = 18, items = "center",
			bg = l[4] and function() return selected() and "#264F78" or "#00000000" end or nil,
			mono(tostring(l[1]), 12, "#6E7681", { w = 18 }),
			mono(l[2], 12.5, l[3] or "#D4D4D4"),
		}
	end
	local files = {}
	for i, f in { { "src" }, { "   invoices.ts" }, { "   parse.ts", true }, { "   format.ts" }, { "tests" }, { "package.json" } } do
		files[i] = mono(f[1], 12, f[2] and "#FFFFFF" or "#CCCCCC99")
	end
	return ui.column {
		w_full = true, h_full = true, bg = "#1F1F1F",
		ui.row {
			w_full = true, h = 34, px = 12, bg = "#181818", items = "center",
			border = 1, border_sides = { 0, 0, 1, 0 }, border_color = "#2B2B2B",
			ui.row { w = 200, lights() },
			text("parse.ts — billing-service", 12, "#9D9D9D"),
		},
		ui.row {
			w_full = true, grow = 1, items = "stretch",
			ui.column {
				w = 200, py = 12, px = 14, gap = 6, bg = "#181818",
				border = 1, border_sides = { 0, 1, 0, 0 }, border_color = "#2B2B2B",
				mono("EXPLORER", 10.5, "#9D9D9D", { letter_spacing = 0.6 }),
				files,
			},
			ui.column { grow = 1, py = 14, lines },
		},
	}
end

local MAILS = {
	{ "Priya Shah", "Re: Offsite — Lisbon in March?", "Works for me if the weather holds. Could you check…" },
	{ "Linear", "Weekly digest", "12 issues closed in Billing this week" },
	{ "Dan Okafor", "Q4 planning doc", "Left a few comments on the hiring section" },
	{ "Stripe", "Your October invoice", "Invoice INV-2291 is ready to view" },
}

local function mail(pasted)
	local rows = {}
	for i, m in MAILS do
		local on = i == 1
		rows[i] = ui.column {
			w_full = true, px = 14, py = 10, gap = 2, rounded = 6, bg = on and "#2F6FEB" or nil,
			text(m[1], 12.5, on and "#FFFFFF" or "#1D1D1F", { weight = 600 }),
			text(m[2], 12, on and "#FFFFFFE6" or "#1D1D1F", { truncate = true }),
			text(m[3], 11.5, on and "#FFFFFFB3" or "#86868B", { truncate = true }),
		}
	end
	return ui.row {
		w_full = true, h_full = true, items = "stretch", bg = "#FFFFFF",
		ui.column {
			w = 300, p = 8, gap = 2, bg = "#F5F5F7",
			border = 1, border_sides = { 0, 1, 0, 0 }, border_color = "#E5E5EA",
			ui.row { h = 30, pl = 6, items = "center", lights() },
			rows,
		},
		ui.column {
			grow = 1, shrink = 1, min_w = 0, px = 36, pt = 52, gap = 14,
			text("Re: Offsite — Lisbon in March?", 20, "#1D1D1F", { weight = 600 }),
			ui.row {
				gap = 10, items = "center",
				ui.div { w = 32, h = 32, rounded = 16, bg = "#E8B4A0", items = "center", justify = "center",
					text("PS", 12, "#5A2E1F", { weight = 600 }) },
				ui.column { gap = 2, text("Priya Shah", 13, "#1D1D1F", { weight = 600 }), text("To: you · 9:12", 11.5, "#86868B") },
			},
			text("Hi! Works for me if the weather holds. Could you check what Lisbon is actually like in early "
				.. "March? I'd love a rooftop dinner on the first night, but not if we're going to freeze. "
				.. "Let me know and I'll book.", 14, "#1D1D1F", { wrap = true, line_height = 22 }),
			-- What "Paste into Mail" writes: a reply draft under the message.
			ui.show(function() return pasted:get() ~= nil end, ui.column {
				w_full = true, mt = 8, p = 14, gap = 8, rounded = 10, bg = "#F5F8FF",
				border = 1, border_color = "#C9D8FF",
				text("Reply draft · pasted from Ask", 11.5, "#2F6FEB", { weight = 600 }),
				ui.text { function() return pasted:get() or "" end, text_size = 13.5, color = "#1D1D1F", wrap = true, line_height = 20 },
			}),
		},
	}
end

-- A document icon: a white page with a coloured band, as the board draws them.
local function page(tone, picked)
	return ui.column {
		w = 34, h = 42, rounded = 4, bg = "#FFFFFF", justify = "end", pb = 8, px = 5,
		border = 1, border_color = picked and "#2F6FEB66" or "#00000014",
		shadow = { y = 1, blur = 3, color = "#0000001F" },
		ui.div { w_full = true, h = 4, rounded = 1, bg = tone },
	}
end

-- `ctx`: { files, picked (state: list of names), drag = fn(name) -> payload,
-- drag_preview = fn(payload) -> node, on_drag_start }.
local function finder(ctx)
	local tiles = {}
	for i, f in ctx.files do
		local function picked()
			return table.find(ctx.picked:get(), f.name) ~= nil
		end
		tiles[i] = ui.column {
			id = "finder_file_" .. i, w = 104, gap = 8, items = "center", py = 6, rounded = 8, cursor = "pointer",
			hover = { bg = "#0000000A" },
			on_click = function()
				local list = table.clone(ctx.picked:peek())
				local at = table.find(list, f.name)
				if at then
					table.remove(list, at)
				else
					table.insert(list, f.name)
				end
				ctx.picked:set(list)
			end,
			on_drag = function()
				return ctx.drag(f.name)
			end,
			drag_preview = ctx.drag_preview,
			on_drag_end = ctx.on_drag_end,
			ui.div {
				w = 52, h = 56, items = "center", justify = "center",
				f.kind == "Folder" and icon("folder", 44, "#5AA9F5") or page(f.tone, false),
			},
			ui.div {
				px = 6, py = 2, rounded = 4,
				bg = function() return picked() and "#2F6FEB" or "#00000000" end,
				ui.text { f.name, text_size = 11.5, color = function() return picked() and "#FFFFFF" or "#1D1D1F" end },
			},
		}
	end
	local side = {}
	for i, s in { { "clock", "Recents" }, { "folder", "Documents", true }, { "download", "Downloads" }, { "monitor", "Desktop" } } do
		side[i] = ui.row {
			w_full = true, h = 26, px = 8, gap = 8, rounded = 6, items = "center", bg = s[3] and "#0000000F" or nil,
			icon(s[1], 13, "#2F6FEB"), text(s[2], 12.5, "#1D1D1F"),
		}
	end
	return ui.row {
		w_full = true, h_full = true, items = "stretch", bg = "#FFFFFF",
		ui.column {
			w = 180, p = 10, gap = 2, bg = "#ECECEC",
			border = 1, border_sides = { 0, 1, 0, 0 }, border_color = "#DADADA",
			ui.row { h = 30, pl = 4, items = "center", lights() },
			text("Favorites", 10.5, "#86868B", { weight = 600, mt = 8, mb = 4, ml = 8 }),
			side,
		},
		ui.column {
			grow = 1, shrink = 1, min_w = 0,
			ui.row {
				w_full = true, h = 52, px = 20, items = "center",
				border = 1, border_sides = { 0, 0, 1, 0 }, border_color = "#E5E5EA",
				text("Apartment", 14, "#1D1D1F", { weight = 600 }),
				spacer(),
				text("Drag files onto Ask, or ⌥Space with them selected", 11.5, "#86868B"),
			},
			ui.row { px = 28, pt = 26, gap = 26, items = "start", tiles },
		},
	}
end

-- The app window for scene `id`.
function M.view(id, ctx)
	if id == "chrome" then
		return chrome()
	elseif id == "code" then
		return code(ctx.selection_on)
	elseif id == "mail" then
		return mail(ctx.pasted)
	end
	return finder(ctx)
end

-- The window's own fill and edge, by app.
M.frame = {
	chrome = { bg = "#DEE1E6", line = "#00000033" },
	code = { bg = "#1F1F1F", line = "#FFFFFF1A" },
	mail = { bg = "#FFFFFF", line = "#00000033" },
	finder = { bg = "#FFFFFF", line = "#00000033" },
}

return M
