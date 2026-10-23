-- Tabs & navigation: one live window (SettingsSections.tsx NavPreview). It
-- is laid out at the app's real size and scaled to the card; parts glide
-- between layouts on the spring, hovering a row lifts the part it changes,
-- and the page scrolls on a loop so "Collapse while scrolling" shows the
-- floating bar tucking into its pill.
return function(K)
	local model = K.model
	local C = K.C

	local VW, VH, STRIP, SIDE, RAIL = 980, 400, 42, 232, 56
	local PINS = { { "gmail", "234,67,53" }, { "gcal", "66,133,244" }, { "linear", "94,106,210" }, { "spotify", "29,185,84" } }
	local ACTIVE = { "opentable", "The River Café · OpenTable" }
	local GROUP = { { "pen-dev", "ask.pen" }, { "github-mark", "Pull requests · ask" }, { "figma", "Sidebar v4 – Figma" } }
	local LOOSE = { { "theverge", "Ask review: the agent lives in your tabs" }, { "wikipedia", "Ordered dithering" } }
	local CHATS = { { "Dinner for four on Friday", "9:26" }, { "Compare noise-cancelling headphones", "Tue" }, { "Summarise the PR review", "Mon" }, { "Lisbon itinerary", "Sep 28" } }
	local MARK = { ink = "#EDEBE5FF", raw = true, pixel = 1, mode = 2, angle = 225, cx = 0.5, cy = 0.55, radius = 0.42, to = 1, curve = 1 }

	local function state()
		local f = model.flags:get()
		return { tabs = model.tabs:get(), navbar = model.navbar:get(), sidebar = f["Sidebar open"], collapse = f["Collapse while scrolling"], fullUrl = f["Show full URL"] }
	end
	local function layout(s, scrolled)
		local vertical, inset = s.tabs == "vertical", s.navbar == "inset"
		local sideW = vertical and (s.sidebar and SIDE or RAIL) or (s.sidebar and SIDE or 0)
		local top = vertical and 0 or STRIP
		local side = { x = sideW > 0 and 0 or -SIDE, y = top, w = sideW > 0 and sideW or SIDE, h = VH - top }
		local cx, cy = sideW > 0 and sideW or 8, vertical and 8 or top
		local cw = VW - cx - 8
		local strip = { x = 0, y = vertical and -STRIP - 4 or 0, w = VW, h = STRIP }
		local pill = not inset and s.collapse and scrolled
		local bw = inset and cw or pill and 168 or math.min(cw - 96, 760)
		local bh = (not inset and pill) and 30 or 40
		local bar = { x = cx + (cw - bw) / 2, y = inset and cy or cy + (pill and 13 or 8), w = bw, h = bh }
		local page = inset and { x = cx, y = cy + 40, w = cw, h = VH - cy - 40 - 8 } or { x = cx, y = cy, w = cw, h = VH - cy - 8 }
		return { side = side, strip = strip, bar = bar, page = page, pill = pill, sideOn = sideW > 0, rail = vertical and not s.sidebar }
	end

	local function fav(icon, size)
		return K.favicon(icon, size or 16)
	end
	local function kc(key, size)
		return ui.row { gap = 2, items = "center", K.icon("command", size or 11, C.muted), K.mono(key, 10.5, C.muted) }
	end
	local function ib(icon)
		return ui.row { w = 28, h = 28, rounded = 7, items = "center", justify = "center", shrink = 0, K.icon(icon, 15, C.secondary) }
	end
	local function divider()
		return ui.div { w = 1, h = 18, bg = C.line, shrink = 0 }
	end
	-- The active tab's concave feet, where it runs down into the page.
	local function foot(side)
		return ui.canvas {
			absolute = true, bottom = 0, w = 9, h = 9, left = side == "left" and -9 or nil, right = side == "right" and -9 or nil,
			draw = function(c)
				local cx = side == "left" and 0 or 9
				local fill = ui.path():move(side == "left" and 9 or 0, 0):line(side == "left" and 9 or 0, 9):line(cx, 9)
				fill:arc(cx, 0, 9, 90, side == "left" and -90 or 90):close()
				c:fill(fill, C.page)
				c:stroke(ui.path():arc(cx, 0, 8.5, 90, side == "left" and -90 or 90), C.line, 1)
			end,
		}
	end
	local function HTab(t, on, basis)
		return ui.overlay {
			h = on and 35 or 30, w = on and 196 or basis or 170, shrink = on and 0 or 1, min_w = 34, self = on and "end" or nil, mb = on and -5 or 0,
			corners = on and { 9, 9, 0, 0 } or nil, rounded = on and nil or 8, bg = on and C.page or C.clear,
			border = on and 1 or 0, border_sides = on and { 1, 1, 0, 1 } or nil, border_color = C.line,
			on and foot("left") or nil, on and foot("right") or nil,
			ui.row { h = 30, pl = 9, pr = 8, gap = 8, items = "center", w_full = true,
				fav(t[1]), K.text(t[2], 12.5, on and C.text or C.secondary, { weight = on and 500 or nil, truncate = true, shrink = 1, grow = 1, min_w = 0 }),
				on and K.icon("x", 12, C.muted) or nil,
			},
		}
	end
	local function VTab(t, on)
		return ui.row {
			w_full = true, h = 28, px = 7, gap = 8, items = "center", rounded = 6, shrink = 0, bg = on and C.active or C.clear,
			ui.row { w = 18, h = 18, items = "center", justify = "center", fav(t[1], 14) },
			K.text(t[2], 12.5, on and C.text or C.secondary, { weight = on and 500 or nil, truncate = true, grow = 1, shrink = 1, min_w = 0 }),
			on and K.icon("x", 12, C.muted) or nil,
		}
	end
	local function vpin(p, w)
		local r, g, b = p[2]:match("(%d+),(%d+),(%d+)")
		local brand = string.format("#%02X%02X%02X", tonumber(r), tonumber(g), tonumber(b))
		return ui.overlay {
			grow = w and 0 or 1, w = w, h = 36, rounded = 8, overflow = "hidden", bg = C.panel, border = 1, border_color = "#262521", shrink = 0,
			items = "center", justify = "center",
			ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, gradient = { angle = 180, stops = { brand .. "38", brand .. "08" } } },
			ui.div { absolute = true, top = 0, left = 6, right = 6, h = 1, bg = brand .. "66" },
			fav(p[1]),
		}
	end
	local function head(label, trail)
		return ui.row { h = 24, px = 6, gap = 8, items = "center", shrink = 0,
			K.label(label, C.tertiary), K.spacer(), trail,
		}
	end

	local Preview = ui.component(function(props)
		local focus = props.focus
		local width = ui.use_state(0)
		local scrolled = ui.use_state(false)
		local s = state
		local function g()
			return layout(state(), scrolled:get())
		end
		local function vertical()
			return s().tabs == "vertical"
		end
		local function dim(part)
			return K.tween(function()
				local f = focus:get()
				return (f ~= nil and f ~= part) and 0.32 or 1
			end, K.MEDIUM)
		end
		local function box(name, props)
			props.absolute = true
			props.top, props.left = 0, 0
			props.x = K.tween(function()
				return g()[name].x
			end, K.SPRING_MS, K.SPRING, true)
			props.y = K.tween(function()
				return g()[name].y
			end, K.SPRING_MS, K.SPRING, true)
			props.w = K.tween(function()
				return g()[name].w
			end, K.SPRING_MS, K.SPRING, true)
			props.h = K.tween(function()
				return g()[name].h
			end, K.SPRING_MS, K.SPRING, true)
			return ui.overlay(props)
		end
		-- An outline 4pt outside the part being changed.
		local function ring(part, radius)
			return ui.div {
				absolute = true, top = -4, left = -4, right = -4, bottom = -4, rounded = radius + 4, pointer_events = "none",
				border = 2, border_color = K.P("EDEBE5", 0.55),
				hidden = function()
					local f = focus:get()
					return not (f == part or (f == "tabs" and part == "side" and vertical()) or (f == "sidebar" and part == "side" and not vertical()))
				end,
			}
		end
		local count = 1 + #GROUP + #LOOSE

		local pins = {}
		for i, p in PINS do
			pins[i] = ui.row { w = 32, h = 30, rounded = 7, bg = C.panel, border = 1, border_color = C.line, items = "center", justify = "center", fav(p[1]) }
		end
		local group_tabs = { HTab(GROUP[1], false, 150), HTab(GROUP[2], false, 150) }
		local strip = box("strip", {
			opacity = K.tween(function()
				if vertical() then
					return 0
				end
				local f = focus:get()
				return (f ~= nil and f ~= "tabs") and 0.32 or 1
			end, K.MEDIUM),
			ui.row { w_full = true, h_full = true, pl = 12, pr = 10, gap = 6, items = "center",
				ui.row { h = 30, pr = 6, gap = 8, items = "center", shrink = 0,
					ui.overlay { w = 22, h = 22, rounded = 6, overflow = "hidden", bg = C.panel, border = 1, border_color = C.line, border_over = true,
						K.dither(function()
							return K.scaled(MARK, K.amount("orbs"))
						end, { hidden = function()
							return K.amount("orbs") == 0
						end }),
						ui.div { absolute = true, top = 5, left = 5, right = 5, bottom = 5, rounded = 3, bg = C.secondary, hidden = function()
							return K.amount("orbs") > 0
						end },
					},
					K.text("Work", 13.5, C.text, { weight = 600 }),
					K.icon("chevron-down", 12, C.secondary),
				},
				ib("panel-left"), divider(),
				ui.row { gap = 4, shrink = 0, pins },
				ui.row { gap = 6, items = "center", self = "end", h = 38, pb = 4, shrink = 1, min_w = 0,
					HTab(ACTIVE, true),
					ui.row { h = 30, gap = 2, items = "center", rounded = 8, bg = C.elevated, shrink = 1, min_w = 0,
						ui.row { h = 30, px = 8, gap = 6, items = "center", shrink = 0, K.icon("folder", 13, C.secondary), K.text("Project · Ask", 12, C.secondary) },
						group_tabs,
					},
					HTab(LOOSE[2], false),
					ib("plus"),
				},
				K.spacer(),
				ui.row { h = 28, px = 8, gap = 6, rounded = 7, items = "center", border = 1, border_color = C.line, shrink = 0,
					K.icon("search", 13, C.secondary), K.mono(count .. " tabs", 10, C.tertiary),
				},
				ui.row { h = 28, pl = 8, pr = 6, gap = 7, rounded = 7, items = "center", bg = C.elevated, border = 1, border_color = C.line, shrink = 0,
					K.Orb { size = 14, lit = model.ai }, K.text("Assistant", 12.5, C.text, { weight = 500 }),
					ui.row { h = 20, px = 6, gap = 4, rounded = 4, items = "center", border = 1, border_color = C.line, kc("J") },
				},
			},
			ring("tabs", 0),
		})

		local vpins = {}
		for i, p in PINS do
			vpins[i] = vpin(p)
		end
		local chats = {}
		for i, ch in CHATS do
			chats[i] = ui.row {
				w_full = true, h = 28, px = 8, gap = 10, items = "center", rounded = 8, shrink = 0,
				bg = i == 1 and C.ctl or C.clear, border = i == 1 and 1 or 0, border_color = "#262521",
				ui.div { w = 3, h = 12, rounded = 2, mr = -6, bg = i == 1 and C.text or C.clear },
				K.text(ch[1], 12.5, i == 1 and C.text or C.secondary, { weight = i == 1 and 500 or nil, truncate = true, grow = 1, shrink = 1, min_w = 0 }),
				K.mono(ch[2], 10, C.tertiary),
			}
		end
		local vtabs = ui.column { w_full = true, gap = 4,
			head("TABS", ui.row { gap = 8, items = "center", K.mono(tostring(count), 10, C.secondary), K.icon("plus", 14, C.muted) }),
			VTab(ACTIVE, true),
			ui.column { w_full = true, gap = 2, p = 4, rounded = 8, bg = C.ctl, shrink = 0,
				ui.row { h = 24, px = 4, gap = 7, items = "center", K.icon("folder", 13, C.secondary), K.text("Project · Ask", 12, C.secondary), K.spacer(), K.icon("chevron-up", 10, C.secondary) },
				ui.column { w_full = true, gap = 2, pl = 8, VTab(GROUP[1]), VTab(GROUP[2]), VTab(GROUP[3]) },
			},
			VTab(LOOSE[1]), VTab(LOOSE[2]),
			ui.div { h = 1, mx = 4, my = 6, bg = C.line, shrink = 0 },
		}
		local full = ui.column {
			absolute = true, top = 0, left = 0, bottom = 0, w = 232, gap = 8, pt = 8, pr = 4, pl = 8,
			opacity = K.tween(function()
				return g().rail and 0 or 1
			end, K.MEDIUM),
			ui.row { w_full = true, h = 28, pl = 7, pr = 9, gap = 8, items = "center", rounded = 6, bg = C.inset, border = 1, border_color = "#262521", shrink = 0,
				K.icon("search", 14, C.secondary), K.text("Search anything", 12.5, C.secondary), K.spacer(), kc("K", 10),
			},
			ui.show(vertical, ui.row { w_full = true, gap = 6, shrink = 0, vpins }),
			ui.column { w_full = true, grow = 1, shrink = 1, min_h = 0, overflow = "hidden", gap = 4,
				ui.show(vertical, vtabs),
				head("CHATS", K.icon("square-pen", 12, C.muted)),
				chats,
			},
			ui.row { w_full = true, h = 38, pt = 2, px = 6, pb = 8, gap = 4, items = "center", shrink = 0,
				K.icon("download", 14, C.tertiary), K.spacer(),
				ui.row { w = 28, h = 28, rounded = 8, items = "center", justify = "center", bg = C.ctl, border = 1, border_color = "#262521", K.icon("briefcase", 14, C.text) },
				ui.row { w = 28, h = 28, items = "center", justify = "center", K.icon("house", 14, C.tertiary) },
				ui.row { w = 28, h = 28, items = "center", justify = "center", K.icon("flask-conical", 14, C.tertiary) },
				K.spacer(), K.icon("panel-left", 14, C.tertiary),
			},
		}
		local rail_tabs = {}
		for i, t in { ACTIVE, GROUP[1], GROUP[2], GROUP[3], LOOSE[1], LOOSE[2] } do
			rail_tabs[i] = ui.row { w = 40, h = 32, rounded = 8, items = "center", justify = "center", shrink = 0, bg = i == 1 and C.active or C.clear, fav(t[1], 14) }
		end
		local rail_pins = {}
		for i, p in PINS do
			rail_pins[i] = vpin(p, 40)
		end
		local rail = ui.column {
			absolute = true, top = 0, left = 0, bottom = 0, w = 56, pt = 8, pl = 8, gap = 4, items = "center",
			opacity = K.tween(function()
				return g().rail and 1 or 0
			end, K.MEDIUM),
			rail_pins, ui.div { w = 40, h = 1, my = 6, bg = C.line }, rail_tabs,
		}
		local side = box("side", {
			opacity = K.tween(function()
				if not g().sideOn then
					return 0
				end
				local f = focus:get()
				local mine = vertical() and "tabs" or "sidebar"
				return (f ~= nil and f ~= mine) and 0.32 or 1
			end, K.MEDIUM),
			ui.overlay { w_full = true, h_full = true, overflow = "hidden", full, rail },
			ring("side", 0),
		})

		local times = { ui.text { "Friday, 4 people", font = K.SANS, text_size = 12, color = C.secondary, mr = 6 } }
		for i, t in { "18:30", "19:00", "19:45", "20:15", "21:00" } do
			times[#times + 1] = ui.div { px = 10, py = 5, rounded = 7, bg = i == 1 and C.ink or C.clear, border = i == 1 and 0 or 1, border_color = C.ctl_line,
				K.mono(t, 11.5, i == 1 and C.on_ink or C.secondary) }
		end
		local doc = ui.column {
			w_full = true, max_w = 640, self = "center", gap = 6, px = 32,
			pt = K.tween(function()
				return s().navbar == "inset" and 32 or 76
			end, K.SPRING_MS, K.SPRING, true),
			y = K.tween(function()
				return scrolled:get() and -150 or 0
			end, 900, K.EASE_MOVE, true),
			ui.row { gap = 8, items = "center", fav("opentable", 14), K.mono("OPENTABLE · LONDON", 11, C.muted, { letter_spacing = 1 }) },
			K.text("The River Café", 30, C.text, { weight = 500, letter_spacing = -0.5, line_height = 36, mt = 4 }),
			K.text("Italian · Hammersmith · £££ · 4.8 (2,316 reviews)", 12, C.muted, { mb = 12 }),
			ui.row { gap = 6, items = "center", mb = 14, times },
			ui.div { w_full = true, h = 150, rounded = 10, mb = 14, border = 1, border_color = C.subtle, gradient = { angle = 135, stops = { C.raised, C.selected } } },
			ui.text { "Seasonal Italian cooking on the Thames since 1987. The menu changes twice a day around what arrives from Italy each morning, cooked in the wood oven or on the chargrill.", font = K.SANS, text_size = 14, line_height = 23, color = C.secondary, wrap = true, mb = 12 },
			ui.text { "Tables on the terrace are released 30 days ahead. Groups of seven or more book through the restaurant directly; a deposit applies to parties of six.", font = K.SANS, text_size = 14, line_height = 23, color = C.secondary, wrap = true, mb = 12 },
		}
		local page = box("page", {
			opacity = dim("page"), rounded = 12, bg = C.page,
			corners = function()
				return s().navbar == "inset" and { 0, 0, 12, 12 } or { 12, 12, 12, 12 }
			end,
			ui.column { w_full = true, h_full = true, overflow = "hidden", rounded = 12, doc },
			ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, pointer_events = "none", border = 1, border_color = C.line,
				corners = function()
					return s().navbar == "inset" and { 0, 0, 12, 12 } or { 12, 12, 12, 12 }
				end },
			ring("page", 12),
		})

		local function url()
			return ui.presence(function()
				return s().fullUrl
			end, function(full)
				if full then
					return ui.rich { font = K.SANS, text_size = 12.5, color = C.secondary, truncate = true,
						ui.span { "https://www.", color = C.faint }, ui.span { "opentable.com", color = C.text, weight = 500 }, ui.span { "/r/the-river-cafe-london" } }
				end
				return K.text("opentable.com", 12.5, C.text, { weight = 500 })
			end)
		end
		local function tb(icon, off)
			return ui.row { w = 26, h = 26, rounded = 6, items = "center", justify = "center", shrink = 0, opacity = off and 0.35 or 1, K.icon(icon, 15, C.secondary) }
		end
		local function floating()
			return s().navbar == "floating"
		end
		local bar = box("bar", {
			opacity = dim("bar"),
			ui.overlay {
				w_full = true, h_full = true, overflow = "hidden",
				corners = function()
					if floating() then
						return g().pill and { 10, 10, 10, 10 } or { 12, 12, 12, 12 }
					end
					return { 12, 12, 0, 0 }
				end,
				bg = function()
					return floating() and K.P("1C1C1A", 0.92) or C.page
				end,
				border = 1, border_color = function()
					return floating() and C.strong or C.line
				end,
				border_sides = function()
					return floating() and { 1, 1, 1, 1 } or { 1, 1, 0, 1 }
				end,
				backdrop_blur = function()
					return floating() and 24 or 0
				end,
				shadow = function()
					return floating() and { x = 0, y = 10, blur = 28, color = "#00000073" } or false
				end,
				ui.row {
					absolute = true, top = 0, left = 0, right = 0, bottom = 0, gap = 6, items = "center",
					px = function()
						return floating() and 6 or 8
					end,
					opacity = K.tween(function()
						return g().pill and 0 or 1
					end, K.BASE),
					ui.row { gap = 2, items = "center", tb("arrow-left"), tb("arrow-right", true), tb("rotate-cw") },
					divider(),
					ui.row { grow = 1, shrink = 1, min_w = 0, h = 28, pl = 10, pr = 3, gap = 8, items = "center", rounded = 8, bg = C.panel, border = 1, border_color = C.line,
						fav("opentable", 14),
						ui.div { grow = 1, shrink = 1, min_w = 0, overflow = "hidden", url() },
						ui.row { w = 22, h = 22, rounded = 6, items = "center", justify = "center", K.icon("star", 14, C.tertiary) },
					},
					divider(),
					ui.row { gap = 2, items = "center", tb("puzzle"), tb("ellipsis") },
				},
				ui.row {
					absolute = true, top = 0, left = 0, right = 0, bottom = 0, gap = 8, items = "center", justify = "center",
					opacity = K.tween(function()
						return g().pill and 1 or 0
					end, K.BASE),
					fav("opentable", 13), K.text("opentable.com", 12, C.text, { weight = 500 }),
				},
			},
			ring("bar", 12),
		})

		local names = { tabs = "TABS", sidebar = "SIDEBAR", bar = "ADDRESS BAR", page = "PAGE" }
		return ui.overlay {
			id = "nav_preview", w_full = true, aspect = VW / VH, overflow = "hidden", bg = C.window,
			on_resize = function(size)
				width:set(size.w)
			end,
			-- The page scrolls down for 2.2 s, then back for 1.6 s; held still under reduced motion.
			ui.div {
				w = 1, h = 1, opacity = 0, absolute = true,
				on_mount = function(el)
					if K.reduced() then
						return
					end
					anime.animate(el, { x = { 0, 1 }, duration = 3800, delay = 900, loop = true, frameRate = 10, ease = "linear",
						onUpdate = function(a)
							scrolled:set(a.iterationProgress < 2200 / 3800)
						end })
				end,
			},
			ui.overlay {
				absolute = true, top = 0, left = 0, w = VW, h = VH, origin = { 0, 0 },
				scale = function()
					return math.max(0.01, width:get() / VW)
				end,
				page, side, strip, bar,
			},
			ui.div {
				absolute = true, left = 12, bottom = 10, px = 6, py = 3, rounded = 5, bg = K.P("121211", 0.8), border = 1, border_color = C.strong,
				hidden = function()
					return focus:get() == nil
				end,
				ui.text { function()
					return names[focus:get() or "page"]
				end, id = "preview_tag", font = K.MONO, text_size = 10, letter_spacing = 1.2, color = C.secondary },
			},
		}
	end)

	local Page = ui.component(function()
		local focus = ui.use_state(nil)
		local function hint(part, node)
			return ui.div { w_full = true, on_hover = function(inside)
				if inside then
					focus:set(part)
				elseif focus:peek() == part then
					focus:set(nil)
				end
			end, node }
		end
		local function vertical()
			return model.tabs:get() == "vertical"
		end
		local function floating()
			return model.navbar:get() == "floating"
		end
		return ui.column {
			w_full = true,
			K.page_header("Tabs & navigation"),
			ui.div { w_full = true, mb = 4, rounded = 12, overflow = "hidden", bg = C.inset, border = 1, border_color = C.line, Preview { focus = focus } },
			K.Section("TABS"),
			hint("tabs", K.Row {
				label = "Tab layout",
				desc = function()
					return vertical() and "Tabs stack in the sidebar, with groups and pinned tabs" or "Tabs run across the top of the window"
				end,
				child = K.Segmented { id = "tab_layout", options = { { "vertical", "Vertical" }, { "horizontal", "Horizontal" } }, value = function()
					return model.tabs:get()
				end, pick = function(v)
					model.tabs:set(v)
				end },
			}),
			hint("sidebar", K.Row {
				label = "Show sidebar",
				desc = function()
					return vertical() and "When off, the sidebar narrows to a rail of favicons" or "Spaces and chats. When off, it hides until you hover the left edge"
				end,
				child = K.Toggle { id = "show_sidebar", label = "Show sidebar", on = function()
					return model.flag("Sidebar open")
				end, set = function(v)
					model.set_flag("Sidebar open", v)
				end },
			}),
			K.Section("ADDRESS BAR"),
			hint("bar", K.Row {
				label = "Position",
				desc = function()
					return floating() and "Floats over the top of the page" or "Sits in its own row above the page"
				end,
				child = K.Segmented { id = "bar_position", options = { { "floating", "Floating" }, { "inset", "Inset" } }, value = function()
					return model.navbar:get()
				end, pick = function(v)
					model.navbar:set(v)
				end },
			}),
			hint("bar", K.Row {
				label = "Collapse while scrolling",
				disabled = function()
					return not floating()
				end,
				desc = function()
					return floating() and "Tucks into a pill as you scroll down; scroll up or hover to bring it back" or "Only a floating bar collapses"
				end,
				child = K.Toggle { id = "collapse_scrolling", label = "Collapse while scrolling",
					disabled = function()
						return not floating()
					end,
					on = function()
						return model.flag("Collapse while scrolling") and floating()
					end, set = function(v)
						model.set_flag("Collapse while scrolling", v)
					end },
			}),
			hint("bar", K.Row {
				label = "Show full URL", desc = "Always show the whole address instead of just the site",
				child = K.Toggle { id = "show_full_url", label = "Show full URL", on = function()
					return model.flag("Show full URL")
				end, set = function(v)
					model.set_flag("Show full URL", v)
				end },
			}),
		}
	end)

	return {
		["Tabs & navigation"] = function()
			return { Page {} }
		end,
	}
end
