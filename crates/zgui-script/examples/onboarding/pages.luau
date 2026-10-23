-- The board's screens, each the content between the title bar and the
-- footer over its own background: 00 Splash to 07 Ready.
local K = require("./kit")
local D = require("./data")
local S = require("./state")
local I = require("./import")
local N = require("./nav")
local Sh = require("./shaders")
local C = K.C
local text, mono, icon, spacer = K.text, K.mono, K.icon, K.spacer
local P = {}

-- A step: its content laid out under the title bar, sliding in from the
-- side it was reached from.
local function page(props: { [any]: any })
	props.grow = 1
	props.min_h = 0
	props.items = props.items or "stretch"
	props.on_mount = function(el)
		if not S.reduce_motion:peek() then
			anime.animate(el, { x = { S.dir:peek() * 18, 0 }, duration = 420, ease = "outQuint" })
		end
	end
	return ui.overlay {
		w_full = true,
		h_full = true,
		ui.column { pt = 50, pb = 68, items = "stretch", ui.column(props) },
	}
end

----------------------------------------------------------------------------
-- 00 Splash
----------------------------------------------------------------------------

local SPACE = { 3, 0.22, 0.5, 3 } -- pixel, tint, stars, the wordmark's cell
-- Where the wordmark's dither sits in the window, on the scene's 3-point grid.
local MARK_X, MARK_Y, MARK_W, MARK_H, TEXT_TOP = 234, 57, 411, 252, 21

function P.splash()
	return ui.overlay {
		id = "splash",
		w_full = true,
		h_full = true,
		cursor = "pointer",
		on_click = function()
			N.go("welcome")
		end,
		on_mount = function()
			ui.after(3200, function()
				if S.page:peek() == "splash" then
					N.go("welcome")
				end
			end)
		end,
		ui.div {
			absolute = true,
			top = 0,
			left = 0,
			right = 0,
			bottom = 0,
			bg_shader = Sh.SPACE,
			shader_params = { { 0, 0, 0, 0 }, SPACE },
			on_mount = Sh.drift,
		},
		-- "ask.", solid, under the scene drawn again wherever its own dither
		-- is off: the board's dithered wordmark.
		ui.overlay {
			absolute = true,
			left = MARK_X,
			top = MARK_Y,
			w = MARK_W,
			h = MARK_H,
			on_mount = function(el)
				anime.animate(el, { opacity = { 0, 1 }, scale = { 0.985, 1 }, duration = 1100, ease = "outCubic" })
			end,
			ui.row {
				absolute = true,
				left = 0,
				right = 0,
				top = TEXT_TOP,
				h = 210,
				justify = "center",
				items = "center",
				text("ask.", 210, "#EDEBE5", { weight = 500, letter_spacing = -7, line_height = 210 }),
			},
			ui.div {
				absolute = true,
				top = 0,
				left = 0,
				right = 0,
				bottom = 0,
				bg_shader = Sh.SPACE,
				shader_params = { { MARK_X, MARK_Y, 1, TEXT_TOP }, SPACE },
				on_mount = Sh.drift,
			},
		},
		ui.row {
			absolute = true,
			left = 0,
			right = 0,
			top = 306,
			justify = "center",
			on_mount = function(el)
				anime.animate(el, { opacity = { 0, 1 }, delay = 350, duration = 900, ease = "outCubic" })
			end,
			mono("THE BROWSER THAT DOES THE WORK", 12, "#A3A199", { letter_spacing = 2.4 }),
		},
		-- The top strip moves the window; a click anywhere else moves on.
		ui.row {
			absolute = true,
			left = 0,
			right = 0,
			top = 0,
			h = 46,
			pt = 20,
			pl = 28,
			pr = 23,
			window_drag = true,
			mono("MERIDIAN SOFTWARE", 10, "#6E6C65", { letter_spacing = 1.2 }),
			spacer(),
			mono("FIRST LAUNCH", 10, "#6E6C65", { letter_spacing = 1.2 }),
		},
	}
end

----------------------------------------------------------------------------
-- 01 Welcome
----------------------------------------------------------------------------

function P.welcome()
	return page({
		items = "center",
		justify = "center",
		gap = 20,
		px = 72,
		pb = 40,
		ui.column {
			gap = 10,
			items = "center",
			text("Welcome to Ask", 30, C.text, { weight = 600, letter_spacing = -0.6 }),
			text("A browser with an agent that works alongside you.", 15, C.secondary),
		},
	})
end

----------------------------------------------------------------------------
-- 02 Import · Choose browsers
----------------------------------------------------------------------------

local function is_picked(id: string): boolean
	if id == "file" then
		return S.file:get()
	end
	for _, x in S.picked:get() do
		if x == id then
			return true
		end
	end
	return false
end

local function toggle_source(id: string)
	ui.batch(function()
		S.skip_import:set(false)
		if id == "file" then
			S.file:toggle()
			return
		end
		local was = is_picked(id)
		local out = {}
		for _, b in D.browsers do
			local on = if b.id == id then not was else table.find(S.picked:peek(), b.id) ~= nil
			if on then
				table.insert(out, b.id)
			end
		end
		S.picked:set(out)
	end)
end

local function tile(b: any)
	local on = function()
		return is_picked(b.id)
	end
	local file = b.id == "file"
	return K.button({
		id = "tile:" .. b.id,
		grow = 1,
		basis = 0,
		h = 53,
		gap = 10,
		pt = 10,
		pb = 10,
		pl = 10,
		pr = 12,
		rounded = 9,
		border = 1,
		bg = function()
			return if on() then C.elevated elseif file then C.clear else C.panel
		end,
		border_color = function()
			return if on() then C.line_strong else C.line
		end,
		hover = { bg = C.elevated },
		opacity = function()
			return if I.started() then 0.5 else 1
		end,
		on_click = function()
			if not I.started() then
				toggle_source(b.id)
			end
		end,
		K.logo(b, 32),
		ui.column {
			grow = 1,
			shrink = 1,
			min_w = 0,
			gap = 2,
			text(if file then "From a file" else b.name, 13, if file then C.secondary else C.text, { weight = 500 }),
			text(function()
				if file then
					return if S.file:get() then "Bookmarks.html" else "HTML or CSV"
				end
				return b.meta
			end, 11, C.muted),
		},
		if file then ui.show(on, K.check(on)) else K.check(on),
	}, 0.98)
end

function P.browsers()
	local rows = {}
	for r = 0, 2 do
		local cells = {}
		for c = 1, 4 do
			table.insert(cells, tile(D.browsers[r * 4 + c] or D.file))
		end
		table.insert(rows, ui.row { gap = 8, cells })
	end
	local skip = function()
		return S.skip_import:get()
	end
	return page({
		gap = 20,
		pt = 20,
		px = 72,
		pb = 24,
		K.heading("Bring your browsing with you"),
		ui.column { gap = 8, items = "stretch", rows },
		ui.presence(function()
			return if I.started() then "locked" else "open"
		end, function(state)
			if state == "locked" then
				return ui.row {
					gap = 8,
					items = "center",
					icon("lock", 13, C.muted),
					text("Importing now", 13, C.tertiary),
				}
			end
			return K.button({
				id = "skip-import",
				gap = 8,
				self = "start",
				hover = {},
				on_click = function()
					ui.batch(function()
						S.skip_import:set(true)
						S.picked:set({})
						S.file:set(false)
					end)
				end,
				K.radio(skip),
				text("Skip importing", 13, function()
					return if skip() then C.text else C.secondary
				end),
			}, 0.99)
		end, { enter = { opacity = { 0, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.FAST } }),
	})
end

----------------------------------------------------------------------------
-- 03 Import · Choose data
----------------------------------------------------------------------------

local function totals()
	local t = { bookmarks = {}, pages = 0, logins = 0, tabs = 0, windows = 0, ext = 0, compatible = 0, chromium = {}, others = {} }
	for _, b in I.sources() do
		if b.bookmarks > 0 then
			table.insert(t.bookmarks, b)
		end
		t.pages += b.pages
		t.logins += b.logins
		t.tabs += b.tabs
		t.windows += b.windows
		if b.extensions > 0 then
			t.ext += b.extensions
			t.compatible += b.compatible
			table.insert(t.chromium, b.name)
		elseif b.id ~= "file" then
			table.insert(t.others, b.name)
		end
	end
	return t
end

-- What each part of the import comes to, or nil when nothing chosen has it.
local function kind_meta(id: string): string?
	local t = totals()
	if id == "bookmarks" then
		if #t.bookmarks == 0 then
			return nil
		end
		local bits = {}
		for i, b in t.bookmarks do
			if i <= 2 then
				table.insert(bits, D.count(b.bookmarks) .. " from " .. (if b.id == "file" then "the file" else b.name))
			end
		end
		if #t.bookmarks > 2 then
			table.insert(bits, (#t.bookmarks - 2) .. " more")
		end
		return table.concat(bits, " · ")
	elseif id == "history" then
		if t.pages == 0 then
			return nil
		end
		return "Last 90 days · about " .. D.count(math.max(100, math.floor(t.pages / 1000 + 0.5) * 1000)) .. " pages"
	elseif id == "passwords" then
		if t.logins == 0 then
			return nil
		end
		return D.count(t.logins) .. " logins · stored in your macOS Keychain"
	elseif id == "tabs" then
		if t.tabs == 0 then
			return nil
		end
		return t.tabs .. " tabs in " .. t.windows .. (if t.windows == 1 then " window" else " windows") .. ", opened as tab groups"
	elseif id == "extensions" then
		if t.compatible == 0 then
			return nil
		end
		local whose = if #t.chromium == 1 then t.chromium[1] .. " " else ""
		return t.compatible .. " of " .. t.ext .. " " .. whose .. "extensions are compatible"
	end
	return nil
end

local function kind_row(k: any)
	local available = function()
		return kind_meta(k.id) ~= nil
	end
	local on = function()
		return available() and S.kinds:get()[k.id] == true
	end
	local locked = function()
		return I.started() or not available()
	end
	return K.button({
		id = "kind:" .. k.id,
		h = 60,
		py = 12,
		gap = 12,
		hover = {},
		cursor = function()
			return if locked() then "default" else "pointer"
		end,
		opacity = function()
			return if available() then (if I.started() then 0.6 else 1) else 0.45
		end,
		on_click = function()
			if not locked() then
				S.kinds:set_key(k.id, not S.kinds:peek()[k.id])
			end
		end,
		icon(k.icon, 16, C.secondary),
		ui.column {
			grow = 1,
			shrink = 1,
			min_w = 0,
			gap = 2,
			text(k.title, 14, C.text),
			text(function()
				return kind_meta(k.id) or ""
			end, 12, C.muted, { truncate = true }),
		},
		K.switch(on),
	}, 0.995)
end

function P.data()
	local rows = {}
	for _, k in D.kinds do
		table.insert(rows, kind_row(k))
	end
	return page({
		gap = 20,
		pt = 20,
		px = 72,
		pb = 24,
		ui.row {
			items = "center",
			K.heading("Choose what to bring"),
			spacer(),
			ui.row {
				items = "center",
				ui.each(ui.computed(function()
					local out = {}
					for i, b in I.sources() do
						if i <= 4 then
							table.insert(out, b.id)
						end
					end
					return out
				end), function(id: string, i: number)
					local b = if id == "file" then D.file else D.by_id[id]
					return ui.div { ml = if i > 1 then -4 else 0, K.logo(b, 36) }
				end, {
					key = function(id)
						return id
					end,
					animate = true,
					enter = { opacity = { 0, 1 }, scale = { 0.7, 1 }, duration = K.SLOW, ease = "outBack(1.6)" },
					exit = { opacity = 0, scale = 0.7, duration = K.BASE },
				}),
			},
		},
		ui.column { items = "stretch", rows },
	})
end

----------------------------------------------------------------------------
-- 04 Import · Importing
----------------------------------------------------------------------------

local function job_of(id: string)
	for _, job in S.jobs:get() do
		if job.id == id then
			return job
		end
	end
	return nil
end

local function spinner(name: string, size: number, color: any)
	return icon(name, size, color, {
		on_mount = function(el)
			Sh.keep(el, anime.animate(el, { rotate = 360, duration = 1100, ease = "linear", loop = true }))
		end,
	})
end

local function job_row(id: string, last: boolean)
	local b = if id == "file" then D.file else D.by_id[id]
	local job = function()
		return job_of(id)
	end
	local active = function()
		local j = job()
		return j ~= nil and not j.finished and (j.part > 1 or j.done > 0)
	end
	local waiting = function()
		local j = job()
		return j ~= nil
			and not j.finished
			and S.import_state:get() == "waiting"
			and j.parts[j.part] == "passwords"
			and j.chromium
			and not S.keychain:get()[id]
	end
	local status = function()
		local j = job()
		if not j then
			return "queued"
		elseif j.finished then
			return "done"
		elseif waiting() then
			return "waiting"
		elseif active() then
			return "active"
		end
		return "queued"
	end
	return ui.row {
		id = "job:" .. id,
		gap = 14,
		py = 16,
		items = "center",
		border_sides = { 0, 0, if last then 0 else 1, 0 },
		border_color = C.line,
		K.logo(b, 40),
		ui.column {
			grow = 1,
			shrink = 1,
			min_w = 0,
			gap = 6,
			items = "stretch",
			ui.row {
				items = "center",
				h = 18,
				text(if id == "file" then "Bookmarks.html" else b.name, 14, C.text, { weight = 500 }),
				spacer(),
				ui.presence(status, function(s: string)
					if s == "done" then
						return ui.row { gap = 6, items = "center", icon("check", 14, C.done), mono("Done", 12, C.done) }
					elseif s == "waiting" then
						return K.button({
							id = "allow:" .. id,
							gap = 6,
							px = 6,
							h = 20,
							rounded = 5,
							on_click = I.resume_coach,
							icon("key", 12, C.waiting),
							text("Allow access", 12, C.waiting),
						})
					elseif s == "active" then
						return mono(function()
							local j = job()
							return if j then math.floor(I.job_progress(j) * 100) .. "%" else ""
						end, 12, C.secondary)
					end
					return mono("Queued", 12, C.muted)
				end, { enter = { opacity = { 0, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.FAST } }),
			},
			ui.presence(function()
				return if status() == "active" or status() == "waiting" then "track" else nil
			end, function()
				return ui.overlay {
					h = 3,
					rounded = 2,
					bg = C.active,
					overflow = "hidden",
					items = "start",
					ui.div {
						h = 3,
						rounded = 2,
						bg = function()
							return if waiting() then C.waiting else C.ink
						end,
						w = function()
							local j = job()
							return (if j then I.job_progress(j) * 100 else 0) .. "%"
						end,
					},
				}
			end, { layout = "height", enter = { opacity = { 0, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.MEDIUM } }),
			text(function()
				local j = job()
				return if j then I.status(j) else ""
			end, 12, function()
				return if waiting() then C.waiting else C.tertiary
			end, { truncate = true }),
		},
	}
end

function P.importing()
	local ids = {}
	for _, job in S.jobs:peek() do
		table.insert(ids, job.id)
	end
	local rows = {}
	for i, id in ids do
		table.insert(rows, job_row(id, i == #ids))
	end
	local done = function()
		return S.import_state:get() == "done"
	end
	return page({
		gap = 28,
		pt = 20,
		px = 72,
		pb = 24,
		K.heading(function()
			return if done() then "Import complete" else "Importing in the background"
		end),
		-- More than three sources scroll, fading at the bottom.
		if #rows > 3
			then ui.scroll { h = 240, fade_overflow = true, fade = { 0, 16 }, ui.column { items = "stretch", rows } }
			else ui.column { items = "stretch", rows },
	})
end

----------------------------------------------------------------------------
-- 05 Appearance
----------------------------------------------------------------------------

local function pref(label: string, control: any, opts: { [string]: any }?)
	opts = opts or {}
	return ui.row {
		h = 48,
		px = 2,
		gap = 24,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = C.line_subtle,
		opacity = opts.opacity,
		text(label, 13.5, C.text, { grow = 1, shrink = 1, min_w = 0 }),
		control,
	}
end

local function set_theme(option: string)
	S.theme:set(option)
	if option == "Dark" then
		ui.theme.set("ask-dark")
	elseif option == "Light" then
		ui.theme.set("ask-light")
	else
		ui.theme.follow_system { dark = "ask-dark", light = "ask-light" }
	end
end
P.set_theme = set_theme

local function tint_choices()
	local out = {}
	for _, t in D.tints do
		local on = function()
			return S.tint:get() == t.id
		end
		table.insert(
			out,
			K.button({
				id = "tint:" .. t.id,
				w = 24,
				h = 24,
				rounded = 12,
				justify = "center",
				border = 1,
				bg = function()
					return if on() then C.selected else C.panel
				end,
				border_color = function()
					return if on() then C.secondary else C.line
				end,
				hover = { border_color = C.tertiary },
				on_click = function()
					S.tint:set(t.id)
				end,
				ui.div { w = 14, h = 14, rounded = 7, bg = t.color },
			}, 0.88)
		)
	end
	return ui.row { gap = 8, shrink = 0, out }
end

local function preview()
	local booked = ui.state("preview_booked", false)
	return ui.column(Sh.live {
		id = "preview",
		w = 248,
		h = 369,
		p = 16,
		gap = 22,
		shrink = 0,
		rounded = 12,
		border = 1,
		border_color = C.line,
		bg = C.window,
		overflow = "hidden",
		bg_shader = Sh.DITHER,
		shader_params = function()
			return Sh.dither { ink = "#5A5850", light = "#C2BFB6", mode = 1, cx = 0.5, cy = 1.1, radius = 0.8, from = 0.5, to = 0, curve = 1.4, noise = 0.5, scale = 3, seed = 3 }
		end,
		ui.row {
			gap = 6,
			ui.row {
				gap = 5,
				px = 8,
				py = 6,
				rounded = 5,
				bg = C.raised,
				items = "center",
				icon("globe", 10, C.secondary),
				text("opentable.com", 10, C.secondary),
			},
			ui.row {
				gap = 5,
				px = 8,
				py = 6,
				rounded = 5,
				bg = C.panel,
				items = "center",
				spinner("loader-circle", 10, C.secondary),
				text("Loading…", 10, C.secondary),
			},
		},
		ui.column {
			gap = 7,
			px = 9,
			items = "start",
			ui.div { w = 96, h = 4, rounded = 2, bg = C.line_strong },
			ui.div { w = 164, h = 4, rounded = 2, bg = C.line_strong },
			ui.div { w = 142, h = 4, rounded = 2, bg = C.line_strong },
		},
		ui.row(Sh.live {
			self = "start",
			gap = 5,
			p = 8,
			rounded = 10,
			items = "center",
			bg_shader = Sh.DITHER,
			shader_params = function()
				return Sh.dither { ink = "#EDEBE5", light = "#1B1A17", mode = 0, from = 0.22, to = 0.22 }
			end,
			K.button({
				id = "reserve",
				px = 12,
				py = 7,
				rounded = 5,
				bg = C.raised,
				hover = { bg = C.selected },
				on_click = function()
					booked:toggle()
				end,
				text(function()
					return if booked:get() then "Booked · 8:00" else "Reserve · 8:00"
				end, 11, C.text),
			}),
			icon("mouse-pointer-2", 15, C.text, {
				on_mount = function(el)
					Sh.keep(el, anime.animate(el, { x = { -2, 3 }, y = { 1, -2 }, duration = 1400, ease = "inOutSine", loop = true, alternate = true }))
				end,
			}),
		}),
		spacer(),
		ui.row {
			gap = 6,
			items = "center",
			ui.div(Sh.live {
				w = 18,
				h = 18,
				rounded = 9,
				bg = C.raised,
				bg_shader = Sh.DITHER,
				shader_params = function()
					return Sh.dither { ink = "#EDEBE5", light = "#1B1A17", mode = 2, angle = 225, radius = 0.5, to = 1, curve = 1.35, pixel = 1, fixed = true }
				end,
			}),
			text(function()
				return if booked:get() then "Table booked" else "Reading page"
			end, 10, C.secondary),
		},
	})
end

function P.appearance()
	local off = function()
		return S.intensity:get() == "Off"
	end
	return page({
		gap = 22,
		pt = 4,
		px = 40,
		pb = 12,
		K.heading("Make it yours"),
		ui.row {
			gap = 32,
			items = "start",
			ui.column {
				w = 520,
				items = "stretch",
				pref("Theme", K.segmented("theme", { "Dark", "Light", "System" }, function()
					return S.theme:get()
				end, set_theme)),
				pref("Tint", tint_choices()),
				pref("Reduce motion", K.switch(function()
					return S.reduce_motion:get()
				end, function()
					S.reduce_motion:toggle()
				end, { id = "reduce-motion" })),
				ui.row { pt = 20, pb = 6, px = 2, K.eyebrow("DITHER", 10) },
				pref("Intensity", K.segmented("intensity", { "Off", "Subtle", "Default", "Rich" }, function()
					return S.intensity:get()
				end, function(v)
					S.intensity:set(v)
				end)),
				pref("Grain", K.segmented("grain", { "Fine", "Default", "Coarse" }, function()
					return S.grain:get()
				end, function(v)
					S.grain:set(v)
				end), {
					opacity = function()
						return if off() then 0.45 else 1
					end,
				}),
			},
			preview(),
		},
	})
end

----------------------------------------------------------------------------
-- 06 Agent · Autonomy
----------------------------------------------------------------------------

local function level_card(l: any)
	local selected = function()
		return not S.ai_off:get() and S.level:get() == l.id
	end
	local unguarded = l.id == "unguarded"
	local segments = {}
	for i = 1, 4 do
		table.insert(
			segments,
			ui.div {
				grow = 1,
				basis = 0,
				h = 4,
				rounded = 2,
				bg = function()
					if unguarded then
						return C.unguarded
					end
					if i <= l.meter then
						return if selected() then C.text else C.muted
					end
					return C.line_subtle
				end,
			}
		)
	end
	return K.button({
		id = "level:" .. l.id,
		grow = 1,
		basis = 0,
		h = 78,
		direction = "column",
		items = "stretch",
		gap = 12,
		pt = 12,
		px = 14,
		pb = 14,
		rounded = 8,
		border = 1,
		bg = function()
			return if selected() then C.raised else C.clear
		end,
		border_color = function()
			return if selected() then C.line_strong else C.line_subtle
		end,
		hover = { bg = C.wash },
		opacity = function()
			return if S.ai_off:get() then 0.45 else 1
		end,
		on_click = function()
			N.set_level(l.id)
		end,
		ui.row { gap = 3, segments },
		ui.row {
			items = "center",
			ui.column {
				gap = 2,
				text(l.name, 14, function()
					return if unguarded then C.warning elseif selected() then C.text else C.secondary
				end, {
					weight = function()
						return if selected() then 600 else 500
					end,
				}),
				text(l.hint, 12, function()
					return if selected() then C.secondary else C.muted
				end),
			},
			spacer(),
			K.radio(selected),
		},
	}, 0.98)
end

-- What the agent does unasked and what it stops for, from the rules (tracked).
local function breakdown()
	local rules = S.rules:get()
	local by_id = {}
	for _, r in D.rules do
		by_id[r.id] = r
	end
	local does = { "Reads, scrolls and clicks through pages" }
	for _, id in D.does_order do
		if rules[id] == "Don’t ask" then
			table.insert(does, by_id[id].does)
		end
	end
	local asks = {}
	for _, id in D.asks_order do
		if rules[id] ~= "Don’t ask" then
			table.insert(asks, by_id[id].asks .. (if rules[id] == "Once per site" then " (once per site)" else ""))
		end
	end
	local function cap(list)
		if #list <= 5 then
			return list
		end
		local out = table.move(list, 1, 4, 1, {})
		table.insert(out, "and " .. (#list - 4) .. " more")
		return out
	end
	return cap(does), cap(asks)
end

local function breakdown_column(heading: string, which: number, mark: string, color: any)
	return ui.column {
		w = 356,
		gap = 10,
		K.eyebrow(heading),
		ui.each(ui.computed(function()
			local does, asks = breakdown()
			local list = if which == 1 then does else asks
			if #list == 0 then
				return { "—" }
			end
			return list
		end), function(line: string)
			if line == "—" then
				return text("Nothing — it never stops to check", 13, C.muted, { h = 17 })
			end
			local more = line:sub(1, 4) == "and "
			return ui.row {
				gap = 10,
				h = 17,
				items = "center",
				if more then ui.div { w = 14 } else icon(mark, 14, color),
				text(line, 13, if more then C.muted else C.secondary),
			}
		end, {
			key = function(line)
				return line
			end,
			animate = { duration = 260, ease = "outQuint" },
			enter = { opacity = { 0, 1 }, x = { -6, 0 }, duration = K.SLOW },
			exit = { opacity = 0, duration = K.FAST },
		}),
	}
end

function P.agent()
	local cards = {}
	for _, l in D.levels do
		table.insert(cards, level_card(l))
	end
	return page({
		gap = 24,
		pt = 8,
		px = 72,
		pb = 24,
		K.heading("How much can Ask do for you?"),
		ui.column {
			gap = 24,
			pt = 8,
			items = "stretch",
			ui.row { gap = 8, cards },
			ui.presence(function()
				return if S.ai_off:get() then "off" else "on"
			end, function(state: string)
				if state == "off" then
					return ui.column { h = 149, K.eyebrow("THE AGENT IS OFF") }
				end
				return ui.row {
					gap = 24,
					h = 149,
					items = "start",
					breakdown_column("DOES ON ITS OWN", 1, "check", C.done),
					breakdown_column("ASKS YOU FIRST", 2, "pause-circle", C.tertiary),
				}
			end, { enter = { opacity = { 0, 1 }, duration = K.SLOW }, exit = { opacity = 0, duration = K.FAST } }),
			ui.row {
				gap = 8,
				items = "center",
				K.button({
					id = "customize",
					h = 30,
					px = 10,
					gap = 7,
					rounded = 8,
					border = 1,
					border_color = function()
						return if S.level:get() == "custom" then C.line_strong else C.line
					end,
					on_click = function()
						N.go("per_action")
					end,
					icon("sliders", 13, C.secondary),
					text(function()
						return if S.level:get() == "custom" and not S.ai_off:get() then "Custom rules" else "Customize per action"
					end, 12, C.text, { weight = 500 }),
					icon("chevron-right", 12, C.muted),
				}, 0.97),
				K.button({
					id = "ai-off",
					h = 30,
					px = 10,
					gap = 7,
					rounded = 8,
					border = 1,
					border_color = C.line,
					on_click = function()
						S.ai_off:toggle()
					end,
					icon("power", 13, function()
						return if S.ai_off:get() then C.done else C.secondary
					end),
					text(function()
						return if S.ai_off:get() then "Turn AI on" else "Turn AI off"
					end, 12, C.text, { weight = 500 }),
					icon("chevron-right", 12, C.muted),
				}, 0.97),
			},
		},
	})
end

----------------------------------------------------------------------------
-- 06B Agent · Choose per action
----------------------------------------------------------------------------

local LEVEL_NAMES = { navigate = "Navigate", assist = "Assist", delegate = "Delegate", unguarded = "Unguarded" }
local LEVEL_IDS = { Navigate = "navigate", Assist = "assist", Delegate = "delegate", Unguarded = "unguarded" }

-- A rule changed by hand: the level whose preset it now matches, or custom.
local function set_rule(id: string, value: string)
	ui.batch(function()
		S.rules:set_key(id, value)
		local rules = S.rules:peek()
		local match = "custom"
		for _, l in D.levels do
			local same = true
			for rule, v in D.presets[l.id] do
				if rules[rule] ~= v then
					same = false
				end
			end
			if same then
				match = l.id
			end
		end
		S.level:set(match)
		S.ai_off:set(false)
	end)
end

local function rule_row(r: any, last: boolean)
	local open = ui.state("menu:" .. r.id, false)
	local value = function()
		return S.rules:get()[r.id] or "Always ask"
	end
	local items = {}
	for _, f in D.FREQUENCIES do
		table.insert(
			items,
			ui.menu_item {
				f,
				on_click = function()
					set_rule(r.id, f)
				end,
			}
		)
	end
	return ui.row {
		h = 42,
		px = 2,
		gap = 24,
		items = "center",
		border_sides = { 0, 0, if last then 0 else 1, 0 },
		border_color = C.line_subtle,
		text(r.label, 13.5, C.text, { grow = 1, shrink = 1, min_w = 0 }),
		ui.menu {
			open = open,
			label = r.label,
			w = 160,
			p = 4,
			rounded = 8,
			bg = C.raised,
			border = 1,
			border_color = C.line,
			anchor = K.button({
				id = "rule:" .. r.id,
				w = 124,
				h = 26,
				px = 10,
				gap = 6,
				rounded = 7,
				bg = C.field,
				border = 1,
				border_color = function()
					return if open:get() then C.line_strong else C.field_line
				end,
				hover = { border_color = C.line_strong },
				on_click = function()
					open:toggle()
				end,
				text(value, 12.5, function()
					return if value() == "Always ask" then C.body else C.secondary
				end),
				spacer(),
				icon("chevrons-down", 12, C.muted),
			}, 0.98),
			items,
		},
	}
end

function P.per_action()
	local rows = {}
	for i, r in D.rules do
		table.insert(rows, rule_row(r, i == #D.rules))
	end
	return page({
		gap = 22,
		pt = 4,
		px = 40,
		pb = 12,
		K.heading("Choose per action"),
		ui.column {
			items = "stretch",
			ui.row {
				h = 36,
				px = 2,
				pb = 6,
				items = "center",
				border_sides = { 0, 0, 1, 0 },
				border_color = C.line_subtle,
				K.eyebrow("ASK BEFORE", 10),
				spacer(),
				K.segmented("preset", { "Navigate", "Assist", "Delegate", "Unguarded" }, function()
					if S.ai_off:get() then
						return nil
					end
					return LEVEL_NAMES[S.level:get()]
				end, function(name)
					N.set_level(LEVEL_IDS[name])
				end, { Unguarded = C.waiting }),
			},
			rows,
		},
	})
end

----------------------------------------------------------------------------
-- 07 Ready
----------------------------------------------------------------------------

local function sources_label(): string
	local names = {}
	for _, job in S.jobs:get() do
		table.insert(names, if job.id == "file" then "your file" else job.name)
	end
	if #names <= 2 then
		return table.concat(names, " & ")
	end
	return names[1] .. " & " .. (#names - 1) .. " more"
end

local function summary_import()
	return ui.presence(function()
		return S.import_state:get()
	end, function(state: string)
		if state == "idle" then
			return ui.row { gap = 8, items = "center", icon("x", 13, C.muted), text("Nothing imported", 12, C.tertiary, { weight = 500 }) }
		elseif state == "done" then
			return ui.row { gap = 8, items = "center", icon("check", 13, C.done), text(function()
				return "Imported " .. sources_label()
			end, 12, C.text, { weight = 500 }) }
		elseif state == "waiting" then
			return K.button({
				id = "summary-keychain",
				gap = 8,
				px = 4,
				h = 22,
				rounded = 5,
				on_click = I.resume_coach,
				icon("key", 13, C.waiting),
				text("Waiting for Keychain", 12, C.waiting, { weight = 500 }),
			})
		end
		return ui.row {
			gap = 8,
			items = "center",
			spinner("loader", 13, C.tertiary),
			text(function()
				return "Importing " .. sources_label()
			end, 12, C.text, { weight = 500 }),
			text(I.percent, 12, C.tertiary, { weight = 500 }),
		}
	end, { enter = { opacity = { 0, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.FAST } })
end

function P.ready()
	return page({
		gap = 28,
		px = 72,
		pb = 32,
		items = "center",
		justify = "center",
		ui.column {
			w = 420,
			items = "center",
			ui.overlay {
				w = 120,
				h = 120,
				items = "center",
				justify = "center",
				ui.overlay {
					w = 96,
					h = 96,
					rounded = 26,
					items = "center",
					justify = "center",
					bg = C.page,
					border = 1,
					border_color = C.wash_line,
					shadow = { { x = 0, y = 0, blur = 48, color = "#ECEAE41F" }, { x = 0, y = 1, blur = 3, color = "#0000004D" } },
					ui.div(Sh.live {
						w = 58,
						h = 58,
						rounded = 29,
						bg = "#2B2A27",
						bg_shader = Sh.DITHER,
						shader_params = function()
							return Sh.dither { ink = "#ECEAE4", mode = 2, angle = 225, radius = 0.5, to = 1, curve = 1.35, pixel = 1.33, fixed = true }
						end,
					}),
				},
			},
			ui.div { h = 28 },
			text("You're all set.", 64, C.text, { weight = 600, letter_spacing = -2.6, line_height = 67 }),
		},
		ui.row {
			gap = 20,
			pt = 8,
			pb = 8,
			pr = 8,
			pl = 16,
			items = "center",
			rounded = 11,
			bg = C.panel,
			border = 1,
			border_color = C.line,
			summary_import(),
			ui.row {
				gap = 8,
				items = "center",
				ui.show(function()
					return S.ai_off:get()
				end, icon("power", 13, C.muted), icon("check", 13, C.done)),
				text(function()
					if S.ai_off:get() then
						return "Agent off"
					elseif S.level:get() == "custom" then
						return "Agent on custom rules"
					end
					return "Agent on " .. (LEVEL_NAMES[S.level:get()] or "Assist")
				end, 12, C.text, { weight = 500 }),
			},
			ui.presence(function()
				return if S.made_default:get() then "made" else "ask"
			end, function(state: string)
				if state == "made" then
					return ui.row { h = 24, px = 6, gap = 6, items = "center", icon("check", 13, C.done), text("Ask is your default", 12, C.done, { weight = 500 }) }
				end
				return K.button({
					id = "make-default",
					h = 24,
					px = 10,
					rounded = 6,
					bg = C.wash,
					border = 1,
					border_color = C.wash_line,
					hover = { bg = C.hover },
					on_click = function()
						S.made_default:set(true)
					end,
					text("Make Ask default", 12, C.text, { weight = 500 }),
				}, 0.96)
			end, { enter = { opacity = { 0, 1 }, scale = { 0.95, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.FAST } }),
		},
	})
end

return P
