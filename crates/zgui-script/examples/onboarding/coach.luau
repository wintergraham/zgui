-- "Onboarding · Keychain coach": shown when a Chromium browser's saved
-- passwords are next, over macOS's own Keychain prompt (a picture of it),
-- pointing at what to do there. The prompt's field and buttons work: typing
-- counts characters only (the password itself is never kept), and the
-- buttons answer for the import.
local K = require("./kit")
local D = require("./data")
local S = require("./state")
local I = require("./import")
local Sh = require("./shaders")
local C = K.C
local text, icon, spacer = K.text, K.icon, K.spacer
local Coach = {}

local INK = "#14130F" -- the step markers' numbers, on the highlight

local function browser_name(): string
	local id = S.coach_for:get()
	local b = D.by_id[id]
	return if b then b.name else "your browser"
end

local field = ui.ref()

local function shake()
	anime.animate(field, { x = { 0, -7, 6, -4, 3, 0 }, duration = 340, ease = "outQuad" })
end

-- Step 1's way out: a typed password moves on to Always Allow.
local function submit()
	if S.coach_typed:peek() == 0 then
		shake()
		local el = field:peek()
		if el then
			el:focus()
		end
		return
	end
	S.coach_step:set(2)
end

local function answer(kind: string)
	if kind ~= "denied" and S.coach_step:peek() == 1 then
		submit()
		if S.coach_step:peek() == 1 then
			return
		end
	end
	I.keychain(kind)
end

local function marker(n: number)
	local state = function()
		local step = S.coach_step:get()
		return if step == n then "active" elseif step > n then "done" else "next"
	end
	return ui.row {
		gap = 8,
		items = "center",
		ui.overlay {
			w = 18,
			h = 18,
			rounded = 9,
			items = "center",
			justify = "center",
			border = function()
				return if state() == "active" then 0 else 1
			end,
			border_color = C.line_strong,
			bg = function()
				return if state() == "active" then C.highlight else C.clear
			end,
			ui.show(function()
				return state() == "done"
			end, icon("check", 10, C.muted), text(tostring(n), 10, function()
				return if state() == "active" then INK else C.muted
			end, { weight = 600 })),
		},
		text(if n == 1 then "Type your Mac password" else "Click Always Allow", 13, function()
			return if state() == "active" then C.text else C.muted
		end, { weight = 500 }),
	}
end

local function badge(n: number, left: number, top: number)
	return ui.overlay {
		absolute = true,
		left = left,
		top = top,
		w = 16,
		h = 16,
		rounded = 8,
		items = "center",
		justify = "center",
		bg = C.highlight,
		pointer_events = "none",
		on_mount = function(el)
			if not S.reduce_motion:peek() then
				anime.animate(el, { scale = { 0.4, 1 }, opacity = { 0, 1 }, duration = 320, ease = "outBack(2.2)" })
			end
		end,
		text(tostring(n), 9, INK, { weight = 700 }),
	}
end

-- A button drawn in the prompt's picture: a hover wash where it is.
local function hotspot(id: string, left: number, top: number, w: number, h: number, round: number, run: () -> ())
	return K.button({
		id = id,
		absolute = true,
		left = left,
		top = top,
		w = w,
		h = h,
		rounded = round,
		hover = { bg = "#FFFFFF14" },
		on_click = run,
	}, 0.97)
end

local function prompt()
	local step2 = function()
		return S.coach_step:get() == 2
	end
	return ui.overlay {
		relative = true,
		w = 480,
		h = 232,
		ui.image { src = "assets/keychain.png", w = 480, h = 232, fit = "cover" },
		-- The password field: focusable, counting keys.
		ui.row {
			id = "keychain-password",
			ref = field,
			absolute = true,
			left = 188,
			top = 113,
			w = 250,
			h = 26,
			px = 9,
			rounded = 6,
			items = "center",
			focusable = true,
			focus = {},
			cursor = "text",
			border = 1,
			border_color = function()
				return if step2() then C.clear else C.highlight
			end,
			bg = function()
				return if step2() then C.clear else C.highlight_bg
			end,
			on_click = function()
				local el = field:peek()
				if el then
					el:focus()
				end
			end,
			on_key = function(e)
				if S.coach_step:peek() ~= 1 then
					return false
				end
				if e.key == "Enter" then
					submit()
				elseif e.key == "Backspace" then
					S.coach_typed:set(math.max(0, S.coach_typed:peek() - 1))
				elseif utf8.len(e.key) == 1 and not e.meta and not e.ctrl then
					S.coach_typed:set(math.min(32, S.coach_typed:peek() + 1))
				else
					return false
				end
				return true
			end,
			text(function()
				return string.rep("●", S.coach_typed:get())
			end, 7, "#E6E6E6", { letter_spacing = 1.5 }),
			-- The caret blinks only while the coach shows step 1.
			ui.show(function()
				return S.coach_open:get() and not step2()
			end, ui.div {
				w = 1,
				h = 14,
				ml = 1,
				bg = "#E6E6E6",
				on_mount = function(el)
					Sh.keep(el, anime.animate(el, { opacity = { 1, 0 }, duration = 530, ease = "steps(1)", loop = true, alternate = true }))
				end,
			}),
			spacer(),
			ui.show(step2, icon("check-circle", 12, C.done)),
		},
		-- macOS's buttons, in the picture.
		hotspot("keychain-help", 44, 160, 22, 22, 11, function()
			S.coach_help:toggle()
		end),
		hotspot("keychain-deny", 266, 157, 82, 26, 7, function()
			answer("denied")
		end),
		hotspot("keychain-allow", 358, 157, 79, 26, 7, function()
			answer("once")
		end),
		ui.overlay {
			absolute = true,
			left = 118,
			top = 157,
			w = 105,
			h = 26,
			rounded = 7,
			border = 1,
			border_color = function()
				return if step2() then C.highlight else C.clear
			end,
			bg = function()
				return if step2() then C.highlight_bg else C.clear
			end,
			hotspot("keychain-always", 0, 0, 105, 26, 7, function()
				answer("always")
			end),
		},
		-- The step badges, over everything and letting the pointer through.
		ui.overlay {
			absolute = true,
			top = 0,
			left = 0,
			right = 0,
			bottom = 0,
			pointer_events = "none",
			ui.show(function()
				return not step2()
			end, badge(1, 431, 104), badge(2, 216, 148)),
		},
	}
end

local function note(name: string, words: string)
	return ui.row {
		gap = 8,
		icon(name, 13, C.muted),
		text(words, 12, C.muted, { wrap = true, grow = 1, shrink = 1, line_height = 17 }),
	}
end

function Coach.view()
	-- A layer the pointer passes through while the coach is away.
	return ui.overlay {
		absolute = true,
		top = 0,
		left = 0,
		right = 0,
		bottom = 0,
		pointer_events = "none",
		ui.show(function()
		return S.coach_open:get()
	end, ui.overlay {
		id = "coach",
		absolute = true,
		top = 0,
		left = 0,
		right = 0,
		bottom = 0,
		items = "center",
		justify = "center",
		-- The dimmed window behind it: a click there puts the coach away
		-- (the import waits, and the row and footer can bring it back).
		ui.div {
			absolute = true,
			top = 0,
			left = 0,
			right = 0,
			bottom = 0,
			bg = "#0000008C",
			on_click = function()
				S.coach_open:set(false)
			end,
			on_mount = function(el)
				anime.animate(el, { opacity = { 0, 1 }, duration = K.MEDIUM, ease = "outQuad" })
			end,
		},
		ui.column {
		items = "stretch",
		w = 560,
		rounded = 10,
		bg = C.elevated,
		border = 1,
		border_color = C.line,
		overflow = "hidden",
		shadow = { x = 0, y = 24, blur = 64, color = C.shadow },
		-- Clicks on the panel stay on it, off the backdrop.
		on_click = function() end,
		on_mount = function(el)
			if not S.reduce_motion:peek() then
				anime.animate(el, { opacity = { 0, 1 }, scale = { 0.97, 1 }, y = { 8, 0 }, duration = 320, ease = "outQuint" })
			end
		end,
		ui.column {
			items = "stretch",
			gap = 8,
			pt = 20,
			px = 24,
			pb = 16,
			ui.row {
				gap = 6,
				items = "center",
				icon("key", 12, C.waiting),
				text(function()
					return "One more step for " .. browser_name()
				end, 11, C.waiting, { letter_spacing = 0.2 }),
			},
			text(function()
				return "macOS will ask before Ask can read your " .. browser_name() .. " data"
			end, 16, C.text, { weight = 500 }),
			ui.row {
				gap = 10,
				pt = 6,
				items = "center",
				marker(1),
				ui.div { w = 24, h = 1, bg = C.line_strong },
				marker(2),
			},
			ui.presence(function()
				return if S.coach_help:get() then "help" else nil
			end, function()
				return text(
					"This is macOS's own prompt, not Ask's. It appears once for each browser that keeps passwords behind a Safe Storage key.",
					12,
					C.tertiary,
					{ wrap = true, line_height = 17, pt = 4 }
				)
			end, { layout = "height", enter = { opacity = { 0, 1 }, duration = K.MEDIUM }, exit = { opacity = 0, duration = K.FAST } }),
		},
		ui.column {
			py = 24,
			items = "center",
			bg = C.inset,
			border = 1,
			border_sides = { 1, 0, 1, 0 },
			border_color = C.line_subtle,
			prompt(),
		},
		ui.column {
			items = "stretch",
			gap = 10,
			pt = 14,
			px = 24,
			pb = 18,
			note("lock", "Use your Mac login password — not your Chrome or Google password."),
			ui.row {
				gap = 6,
				pt = 4,
				items = "center",
				text("Clicked Allow or Deny?", 12, C.muted),
				K.button({
					id = "keychain-again",
					hover = {},
					on_click = function()
						ui.batch(function()
							S.coach_step:set(1)
							S.coach_typed:set(0)
						end)
						local el = field:peek()
						if el then
							el:focus()
						end
					end,
					text("Ask again", 12, C.text),
				}, 0.97),
			},
		},
		},
	}),
	}
end

-- A closed coach lets go of its caret's clock.
ui.effect(function()
	if not S.coach_open:get() then
		ui.after(200, Sh.sweep)
	end
end)

-- Focus the password field whenever step 1 shows.
ui.effect(function()
	local el = field:get()
	if el and S.coach_open:get() and S.coach_step:get() == 1 then
		el:focus()
	end
end)

return Coach
