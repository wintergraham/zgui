-- Ask Settings (Settings.tsx), native: the nav with its sliding pill and
-- search, the corner dither that grows in when the page opens, and thirteen
-- sections. Theme and tint changes rebuild the page under a crossfade, so
-- every colour eases at once, as the window's [data-theming] transition does.
local K = require("./kit")
local model = K.model
local S = require("./search")(K)
local sections = {}
for _, file in { "general", "appearance", "tabs", "assistant", "models", "confirmations", "extensions", "about" } do
	for name, build in require("./" .. file)(K) do
		sections[name] = build
	end
end

local reduced = model.reduced
local intro = ui.state("intro", reduced:peek() and 10 or 0)
-- Opening settings grows the window dither in from the bottom-left corner
-- over 10 stepped frames on the 12 fps clock; reduced motion skips to the end.
local function play_intro()
	if reduced:peek() then
		intro:set(10)
		return
	end
	intro:set(0)
	anime.animate(intro, { value = 10, duration = 10 * 1000 / 12, ease = "linear", frameRate = 12, modifier = anime.utils.round(0) })
end
play_intro()
local function ease3(x)
	return 1 - (1 - math.clamp(x, 0, 1)) ^ 3
end

local active = ui.state("search_active", 1)
local query = ui.computed(function()
	return (model.filter:get():match("^%s*(.-)%s*$"))
end)
local results = ui.computed(function()
	return S.search(query:get())
end)
ui.effect(function()
	query:get()
	active:set(1)
end)
local search_el = nil

-- Opening a result goes to its page, then scrolls the setting to the middle
-- of the window and flashes it.
local function jump(name)
	K.after(60, function()
		local el = K.targets[name]
		if not el or not el.mounted then
			return
		end
		local vh = K.viewport:peek().h
		local top = K.scroll:peek() + (el.y or 0) - vh / 2 + math.min(el.height or 0, 200) / 2
		top = math.max(0, top)
		if reduced:peek() then
			K.scroll:set(top)
		else
			anime.animate(K.scroll, { value = top, duration = 520, ease = K.EASE_MOVE })
		end
		K.flash:set(nil)
		K.flash:set(name)
		K.after(1600, function()
			if K.flash:peek() == name then
				K.flash:set(nil)
			end
		end)
	end)
end
local function open_result(x)
	K.close_menu()
	ui.batch(function()
		model.section:set(x.section)
		model.filter:set("")
	end)
	jump(x.target or x.label)
end
local function go(section)
	K.close_menu()
	ui.batch(function()
		model.section:set(section)
		model.filter:set("")
	end)
	K.scroll:set(0)
end
K.go = go

local function nav_icon(icon, size, current)
	if icon == "orb" then
		return K.Orb { size = 14, lit = model.ai }
	end
	return K.icon(icon, size, current and K.C.secondary or K.C.tertiary)
end
local function section_icon(section)
	for _, n in model.NAV do
		if n[1] == section then
			return n[3] == "orb" and K.Orb { size = 13, lit = model.ai } or K.icon(n[3], 13, K.C.muted)
		end
	end
	return K.icon("settings", 13, K.C.muted)
end

local function Nav()
	local C = K.C
	local current = function()
		return query:get() == "" and model.section:get() or nil
	end
	local counts = K.computed(function()
		local c = {}
		for _, x in results:get() do
			c[x.section] = (c[x.section] or 0) + 1
		end
		return c
	end)
	-- Where each item sits in the list: tops step by the item heights and the 2pt gap.
	local spots, y = {}, 0
	for i, n in model.NAV do
		local sub = n[4] ~= nil
		spots[n[1]] = { x = sub and 24 or 0, y = y, w = sub and 196 or 220, h = sub and 28 or 32 }
		y += (sub and 28 or 32) + 2
	end
	local function spot()
		local s = current()
		return s and spots[s] or nil
	end
	local items = {}
	for i, n in model.NAV do
		local label, route, icon, parent = n[1], n[2], n[3], n[4]
		local sub = parent ~= nil
		local function on()
			return current() == label
		end
		items[i] = K.button({
			id = "nav_" .. route, h = sub and 28 or 32, ml = sub and 24 or 0, pl = sub and 9 or 10, pr = 10,
			gap = sub and 8 or 10, rounded = 8, w = sub and 196 or 220, shrink = 0,
			color = function()
				return on() and C.text or C.secondary
			end,
			hover = { bg = C.page, color = C.text },
			opacity = K.tween(function()
				local q = query:get()
				return (q ~= "" and not counts:get()[label]) and 0.38 or 1
			end, K.FAST),
			on_click = function()
				go(label)
			end,
			K.fixed(ui.show(on, nav_icon(icon, sub and 14 or 15, true), nav_icon(icon, sub and 14 or 15, false))),
			K.text(label, sub and 12.5 or 13, false, { truncate = true, shrink = 1 }),
			K.spacer(),
			K.fixed(ui.show(function()
				return query:get() ~= "" and counts:get()[label] ~= nil
			end, ui.div {
				min_w = 18, h = 16, px = 5, rounded = 8, bg = C.selected, items = "center", justify = "center",
				ui.text { function()
					return tostring(counts:get()[label] or "")
				end, font = K.MONO, text_size = 10, weight = 500, color = C.text },
			}, route == "assistant" and ui.show(function()
				return not model.ai:get()
			end, ui.div {
				px = 5, py = 1, rounded = 4, border = 1, border_color = C.strong,
				K.mono("Off", 10, C.muted, { letter_spacing = 0.6 }),
			}) or nil)),
		}, false)
	end
	return ui.column {
		id = "nav", w = 220, shrink = 0,
		K.text("Settings", 22, C.text, { weight = 500, letter_spacing = -0.3, line_height = 29, mb = 16 }),
		K.search_box {
			id = "search", value = model.filter, placeholder = "Search settings", label = "Search settings",
			props = { mb = 12 },
			on_mount = function(el)
				search_el = el
			end,
			on_key = function(e)
				local list = results:peek()
				if e.key == "ArrowDown" or e.key == "ArrowUp" then
					if #list > 0 then
						active:set((active:peek() - 1 + (e.key == "ArrowDown" and 1 or -1)) % #list + 1)
					end
					return true
				end
				if e.key == "Enter" and list[active:peek()] then
					open_result(list[active:peek()])
					return true
				end
				if e.key == "Escape" and model.filter:peek() ~= "" then
					model.filter:set("")
					return true
				end
				return false
			end,
			trail = ui.show(function()
				return query:get() ~= ""
			end, K.button {
				id = "search_clear", w = 18, h = 18, rounded = 5, justify = "center", bg = C.selected, hover = { bg = C.strong },
				on_click = function()
					model.filter:set("")
				end,
				K.icon("x", 12, C.secondary),
			}, ui.div {
				w = 16, h = 16, rounded = 4, border = 1, border_color = C.strong, items = "center", justify = "center", shrink = 0,
				K.mono("/", 10, C.muted),
			}),
		},
		ui.overlay {
			w_full = true,
			ui.div {
				id = "nav_pill", absolute = true, top = 0, left = 0, rounded = 8, bg = C.hover, border = 1, border_color = C.ctl_line,
				pointer_events = "none",
				hidden = function()
					return spot() == nil
				end,
				x = K.tween(function()
					local s = spot()
					return s and s.x or 0
				end, K.MEDIUM, K.EASE_OUT, true),
				y = K.tween(function()
					local s = spot()
					return s and s.y or 0
				end, K.MEDIUM, K.EASE_OUT, true),
				w = K.tween(function()
					local s = spot()
					return s and s.w or 220
				end, K.MEDIUM, K.EASE_OUT, true),
				h = K.tween(function()
					local s = spot()
					return s and s.h or 32
				end, K.MEDIUM, K.EASE_OUT, true),
			},
			ui.column { w_full = true, gap = 2, items },
		},
	}
end

local function Content()
	return ui.presence(function()
		return query:get() ~= "" and "search" or model.section:get()
	end, function(key)
		if key == "search" then
			return ui.column {
				id = "content", w_full = true,
				ui.presence(function()
					return query:get()
				end, function(q)
					return ui.column {
						w_full = true,
						S.Results {
							q = q, results = S.search(q), active = active, open = open_result, icon = section_icon,
							suggest = function(s)
								model.filter:set(s)
							end,
						},
					}
				end),
			}
		end
		local build = sections[key] or sections.Generic
		return ui.column { id = "content", w_full = true, on_mount = function(el)
			K.content_el = el
		end, build(key) }
	end)
end

local function Page()
	local C = K.C
	local wide = K.computed(function()
		return K.viewport:get().w > 820
	end)
	local function layout_w()
		return math.min(1180, K.viewport:get().w * 0.94)
	end
	local glow = function()
		return ease3(intro:get() / 10)
	end
	local drops = function()
		return ease3((intro:get() - 2) / 8)
	end
	return ui.overlay {
		id = "settings", w_full = true, h_full = true, bg = C.window,
		ui.overlay {
			absolute = true, top = 0, left = 0, right = 0, bottom = 0, pointer_events = "none",
			K.dither(function()
				local u = table.clone(K.U.page)
				u.radius = K.U.page.radius * (0.35 + 0.65 * glow())
				return K.scaled(u, K.amount("background") * glow())
			end),
			ui.overlay {
				absolute = true, top = 0, left = 0, right = 0, h = 130,
				K.dither(function()
					return K.scaled(K.U.drops, K.amount("background") * drops())
				end),
			},
		},
		ui.column { w_full = true, h_full = true,
			ui.scroll {
			id = "page_scroll", offset = K.scroll, grow = 1, shrink = 1, min_h = 0, w_full = true,
			ui.column {
				w_full = true, pt = 48, pb = 40,
				ui.row {
					w = "94%", max_w = 1180, self = "center", gap = 56, items = "start",
					ui.div { w = 220, shrink = 0, hidden = function()
						return not wide:get()
					end },
					ui.column {
						grow = 1, shrink = 1, min_w = 0,
						max_w = function()
							return query:get() == "" and model.section:get() == "Models" and 904 or 684
						end,
						Content(),
					},
				},
			},
			},
		},
		-- The nav stays put while the page scrolls (position: sticky; top: 48px).
		ui.show(wide, ui.div {
			absolute = true, top = 48,
			left = function()
				return (K.viewport:get().w - layout_w()) / 2
			end,
			on_wheel = function(e)
				K.scroll:set(math.max(0, K.scroll:peek() + e.dy))
			end,
			Nav(),
		}),
		K.layer(),
	}
end

-- "/" focuses the search box from anywhere on the page.
local function keys(e)
	-- A control recording a shortcut takes the next key.
	if K.capture then
		K.capture(e)
		return true
	end
	if e.key == "Escape" and K.escape() then
		return true
	end
	if e.key == "/" and not K.editing:peek() and not e.meta and not e.ctrl and search_el then
		search_el:focus()
		return true
	end
	return false
end

return ui.overlay {
	id = "settings_root", w_full = true, h_full = true, bg = K.C.window,
	on_resize = function(size)
		K.viewport:set { w = size.w, h = size.h }
	end,
	on_key = keys,
	ui.presence(function()
		return model.theme:get() .. "/" .. model.tint:get()
	end, function(key)
		local theme, tint = key:match("^(.-)/(.*)$")
		K.palette(theme, tint)
		return ui.overlay {
			w = function()
				return K.viewport:get().w
			end,
			h = function()
				return K.viewport:get().h
			end,
			Page(),
		}
	end, {
		enter = { opacity = { 0, 1 }, duration = 260, ease = K.EASE_OUT },
		exit = { opacity = 0, duration = 260, ease = K.EASE_OUT },
	}),
}
