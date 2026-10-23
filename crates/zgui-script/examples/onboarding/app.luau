-- Ask · Onboarding, from ask.pen's board of the same name: first-run setup
-- in its own 880 × 580 window, before the browser opens. Every step on the
-- board is here and works: the splash, Welcome, choosing browsers and what
-- to bring, the import running in the background (with the Keychain coach
-- when a Chromium browser's passwords come up), Appearance, the agent's
-- autonomy and its per-action rules, and Ready. Nothing is read from the
-- disk or sent anywhere; the import is played.
local K = require("./kit")
local D = require("./data")
local S = require("./state")
local I = require("./import")
local N = require("./nav")
local P = require("./pages")
local Coach = require("./coach")
local C = K.C
local text, mono, icon, spacer = K.text, K.mono, K.icon, K.spacer

-- A host may open on a step (`ONBOARDING_STEP=agent`) for screenshots.
local booted = ui.state("booted", false)
if not booted:peek() then
	booted:set(true)
	local first = ui.shared("onboarding_step", "splash"):peek()
	-- `coach` opens the Keychain coach over the import, for Chrome.
	if first == "coach" then
		S.page:set("importing")
		S.reached:set(2)
		I.start()
		S.import_state:set("waiting")
		S.coach_open:set(true)
	elseif D.steps[first] and first ~= "splash" then
		S.page:set(first)
		S.reached:set(D.steps[first].n)
		if first == "importing" or first == "ready" then
			I.start()
		end
	end
end
-- The theme the Appearance step chose, kept across reloads.
P.set_theme(S.theme:peek())

----------------------------------------------------------------------------
-- Title bar: close, the five-step strip, Skip setup
----------------------------------------------------------------------------

local function segment(i: number)
	local n = function()
		return D.steps[S.page:get()].n
	end
	local reached = function()
		return i <= S.reached:get()
	end
	return K.button({
		id = "segment:" .. i,
		py = 5,
		hover = {},
		cursor = function()
			return if reached() and i ~= n() then "pointer" else "default"
		end,
		on_click = function()
			if reached() and i ~= n() then
				local target = D.segments[i]
				if target == "browsers" and I.started() then
					target = "importing"
				end
				N.go(target)
			end
		end,
		ui.div {
			w = 20,
			h = 3,
			rounded = 2,
			bg = function()
				if i == n() then
					return C.tertiary
				elseif reached() then
					return C.faint
				end
				return C.line_strong
			end,
		},
	}, 1)
end

local function title_bar()
	local segments = {}
	for i = 1, #D.segments do
		table.insert(segments, segment(i))
	end
	return ui.row {
		h = 50,
		px = 14,
		items = "center",
		window_drag = true,
		ui.row {
			grow = 1,
			basis = 0,
			items = "center",
			K.button({
				id = "close",
				w = 26,
				h = 26,
				rounded = 7,
				justify = "center",
				hover = { bg = C.hover },
				on_click = N.close,
				icon("x", 14, C.tertiary),
			}, 0.92),
		},
		ui.row { gap = 4, items = "center", segments },
		ui.row {
			grow = 1,
			basis = 0,
			justify = "end",
			items = "center",
			ui.presence(function()
				local page = S.page:get()
				return if page == "welcome" or page == "ready" then nil else "skip"
			end, function()
				return K.button({
					id = "skip-setup",
					h = 22,
					px = 6,
					rounded = 5,
					hover = { bg = C.hover },
					on_click = function()
						N.go("ready")
					end,
					text("Skip setup", 13, C.muted),
				}, 0.97)
			end, { enter = { opacity = { 0, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.FAST } }),
		},
	}
end

----------------------------------------------------------------------------
-- Footer: the step, the import's quiet status, and the step's buttons
----------------------------------------------------------------------------

local function quiet_status()
	local waiting = function()
		return S.import_state:get() == "waiting"
	end
	local done = function()
		return S.import_state:get() == "done"
	end
	return K.button({
		id = "import-status",
		h = 20,
		gap = 8,
		px = 4,
		rounded = 5,
		hover = {},
		cursor = function()
			return if waiting() then "pointer" else "default"
		end,
		on_click = function()
			if waiting() then
				I.resume_coach()
			end
		end,
		ui.overlay {
			w = 56,
			h = 3,
			rounded = 2,
			bg = C.active,
			overflow = "hidden",
			items = "start",
			ui.div {
				h = 3,
				rounded = 2,
				bg = function()
					return if waiting() then C.waiting elseif done() then C.done else C.ink
				end,
				w = function()
					return (I.progress() * 100) .. "%"
				end,
			},
		},
		text(function()
			return if waiting() then "Waiting for Keychain" elseif done() then "Imported" else "Importing"
		end, 11, function()
			return if waiting() then C.waiting elseif done() then C.done else C.tertiary
		end),
		mono(I.percent, 9.5, C.muted),
	}, 1)
end

local function footer_actions(page: string)
	local a = N.actions(page)
	return ui.row {
		gap = 8,
		items = "center",
		if a.skip then K.action(a.skip.label, "ghost", a.skip.run, { id = "footer-skip" }) else nil,
		if a.back then K.action(a.back.label, "secondary", a.back.run, { id = "back" }) else nil,
		K.action(a.primary.label, "primary", a.primary.run, { id = "primary", disabled = a.primary.disabled }),
	}
end

-- As tall as a button, so the row keeps its place while one step's buttons
-- give way to the next's. The strip in the title bar already says the step.
local function footer()
	return ui.row {
		h = 32,
		mx = 14,
		mb = 14,
		items = "center",
		ui.presence(function()
			return if I.started() and S.page:get() ~= "ready" then "on" else nil
		end, quiet_status, { enter = { opacity = { 0, 1 }, x = { -6, 0 }, duration = K.SLOW }, exit = { opacity = 0, duration = K.FAST } }),
		spacer(),
		ui.presence(function()
			return S.page:get()
		end, footer_actions, { mode = "wait", enter = { opacity = { 0, 1 }, duration = K.BASE }, exit = { opacity = 0, duration = 80 } }),
	}
end

----------------------------------------------------------------------------
-- The window
----------------------------------------------------------------------------

local BUILD = {
	splash = P.splash,
	welcome = P.welcome,
	browsers = P.browsers,
	data = P.data,
	importing = P.importing,
	appearance = P.appearance,
	agent = P.agent,
	per_action = P.per_action,
	ready = P.ready,
}

local function on_key(e)
	if S.coach_open:peek() then
		if e.key == "Escape" then
			S.coach_open:set(false)
			return true
		end
		return false
	end
	local page = S.page:peek()
	if e.key == "Enter" then
		if page == "splash" then
			N.go("welcome")
		else
			N.primary()
		end
		return true
	elseif e.key == "Escape" and page ~= "splash" and page ~= "welcome" then
		N.back()
		return true
	elseif page == "agent" and (e.key == "ArrowLeft" or e.key == "ArrowRight") then
		local at = 2
		for i, l in D.levels do
			if l.id == S.level:peek() then
				at = i
			end
		end
		at = math.clamp(at + (if e.key == "ArrowLeft" then -1 else 1), 1, #D.levels)
		N.set_level(D.levels[at].id)
		return true
	end
	return false
end

local on_splash = ui.computed(function()
	return S.page:get() == "splash"
end)
local root_el = nil
return ui.overlay {
	id = "window",
	w_full = true,
	h_full = true,
	bg = C.window,
	overflow = "hidden",
	focusable = true,
	focus = {},
	on_key = on_key,
	on_mount = function(el)
		root_el = el
		el:focus()
	end,
	on_press = function()
		-- Clicks on blank space give keys back to the window.
		if root_el and not S.coach_open:peek() then
			root_el:focus()
		end
	end,
	opacity = ui.tween(S.leaving, { from = 1, to = 0, duration = 360, ease = "inQuad" }),
	scale = ui.tween(S.leaving, { from = 1, to = 0.97, duration = 360, ease = "inQuad" }),
	ui.presence(function()
		return S.page:get()
	end, function(page: string)
		return BUILD[page]()
	end, {
		w_full = true,
		h_full = true,
		enter = { opacity = { 0, 1 }, duration = 320, ease = "outCubic" },
		exit = { opacity = 0, duration = 160, ease = "outQuad" },
	}),
	-- The title bar and footer over every step but the splash. The layer
	-- lets the pointer through to the step under it, but for its controls.
	ui.column {
		items = "stretch",
		pointer_events = "none",
		hidden = on_splash,
		opacity = ui.tween(on_splash, { from = 1, to = 0, duration = 420, ease = "outCubic" }),
		title_bar(),
		spacer(),
		footer(),
	},
	Coach.view(),
}
