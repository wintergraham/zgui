-- Settings primitives (SettingsKit.tsx + settings.css): palette, dither fills,
-- orb, toggle, select, rows, segmented control, keycaps, tiles, and the
-- popup layer menus and dialogs open in.
local icons = require("./icons")
local shaders = require("./dither")
local logos = require("./logos")
local model = require("./model")
local K = {}

K.SANS, K.MONO = "Geist", "Geist Mono"
K.model = model

---------------------------------------------------------------------------
-- Theme and tint. Every colour is named by its dark value; light swaps each
-- for its counterpart, and a tint keeps OKLCH lightness while moving the hue,
-- with chroma peaking in the mid-tones. The page rebuilds when the palette
-- changes, so colours are plain values everywhere (hover styles included).
---------------------------------------------------------------------------
local LIGHT = {
	["0A0A09"] = "F3F2EE", ["0F0F0E"] = "EAE9E4", ["121211"] = "FFFFFF", ["161615"] = "FFFFFF", ["1A1A18"] = "FFFFFF",
	["1C1C1A"] = "FFFFFF", ["1F1F1D"] = "ECEBE6", ["222220"] = "E2E1DC", ["232321"] = "F9F8F5", ["262521"] = "DDDCD6",
	["262624"] = "E6E5E0", ["2E2D29"] = "D3D2CC", ["33322D"] = "C9C8C1", ["343430"] = "CFCEC8", ["3A3934"] = "BEBDB6",
	["5A5850"] = "A3A199", ["6E6C65"] = "8A8881", ["858379"] = "6E6C65", ["A3A199"] = "55534D", ["D4D2CB"] = "2E2D29",
	["EDEBE5"] = "1A1A18", ["2C2B27"] = "DAD8D1", ["302F2A"] = "DFDDD6", ["3A3933"] = "D0CEC7", ["4C4B45"] = "C4C2BA",
	["181816"] = "FBFAF7", ["4A4943"] = "B5B3AC", ["E8A898"] = "B4432C", ["1C1513"] = "FBEDEA", ["4A2C25"] = "EBC3B9",
	["2A1714"] = "F6DCD5",
}
K.TINTS = { { "stone", "Stone", nil }, { "slate", "Slate", 255 }, { "sage", "Sage", 150 }, { "sand", "Sand", 80 }, { "rose", "Rose", 15 }, { "iris", "Iris", 292 } }
K.THEMES = { { "dark", "Dark" }, { "light", "Light" }, { "system", "System" } }
local TINT_CHROMA, SWATCH_CHROMA = 0.012, 0.045

local function lin(c)
	return c <= 0.04045 and c / 12.92 or ((c + 0.055) / 1.055) ^ 2.4
end
local function gam(c)
	return c <= 0.0031308 and 12.92 * c or 1.055 * c ^ (1 / 2.4) - 0.055
end
local function byte(v)
	return string.format("%02X", math.floor(math.clamp(v, 0, 1) * 255 + 0.5))
end
local function from_oklab(L, A, B)
	local l3 = (L + 0.3963377774 * A + 0.2158037573 * B) ^ 3
	local m3 = (L - 0.1055613458 * A - 0.0638541728 * B) ^ 3
	local s3 = (L - 0.0894841775 * A - 1.291485548 * B) ^ 3
	return byte(gam(4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3))
		.. byte(gam(-1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3))
		.. byte(gam(-0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3))
end
local function tint_hex(hex, hue, chroma)
	local r, g, b = lin(tonumber(hex:sub(1, 2), 16) / 255), lin(tonumber(hex:sub(3, 4), 16) / 255), lin(tonumber(hex:sub(5, 6), 16) / 255)
	local l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ^ (1 / 3)
	local m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ^ (1 / 3)
	local s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ^ (1 / 3)
	local L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
	local c, h = chroma * 4 * L * (1 - L), math.rad(hue)
	return from_oklab(L, c * math.cos(h), c * math.sin(h))
end
-- CSS oklch(L C h / a) as "#rrggbb" or "#rrggbbaa".
function K.oklch(L, c, h, a)
	local hex = "#" .. from_oklab(L, c * math.cos(math.rad(h)), c * math.sin(math.rad(h)))
	return a and hex .. byte(a) or hex
end

local light, hue = false, nil
local cache = {}
-- A token by its dark value ("EDEBE5"), painted for the current theme and
-- tint; `alpha` mixes it toward transparent (color-mix(… N%, transparent)).
local function P(hex, alpha)
	local v = cache[hex]
	if not v then
		local h = light and LIGHT[hex] or hex
		v = "#" .. (hue and tint_hex(h, hue, TINT_CHROMA) or h)
		cache[hex] = v
	end
	return alpha and v .. byte(alpha) or v
end
K.P = P
-- A tint swatch: the hue at the swatches' stronger chroma, always dark.
function K.swatch(hex, h)
	return "#" .. (h and tint_hex(hex, h, SWATCH_CHROMA) or hex)
end

-- Semantic names (settings.css .st-win), refreshed in place by `palette`.
K.C = {}
local C = K.C
local NAMES = {
	window = "0A0A09", inset = "0F0F0E", panel = "121211", page = "161615", card = "181816", elevated = "1A1A18",
	ctl = "1C1C1A", raised = "1F1F1D", subtle = "222220", hover = "232321", line = "262521", selected = "262624",
	ctl_line = "2E2D29", strong = "33322D", active = "343430", hover_line = "3A3934", focus = "5A5850",
	muted = "6E6C65", tertiary = "858379", secondary = "A3A199", soft = "D4D2CB", text = "EDEBE5", faint = "4A4943",
	error = "E8A898", error_bg = "1C1513", error_border = "4A2C25", danger_pill = "2A1714",
}
function K.palette(theme, tint)
	light = theme == "light"
	hue = nil
	for _, t in K.TINTS do
		if t[1] == tint then
			hue = t[3]
		end
	end
	table.clear(cache)
	for name, hex in NAMES do
		C[name] = P(hex)
	end
	C.ink, C.on_ink = C.text, C.panel
	C.clear = "#00000000"
	K.dark = not light
end
K.palette("dark", "stone")

---------------------------------------------------------------------------
-- Motion: the lab's durations and curves (tokens.css, motion.ts).
---------------------------------------------------------------------------
K.EASE_OUT = "cubicBezier(.2,0,0,1)"
K.EASE_ENTER = "cubicBezier(.16,1,.3,1)"
K.EASE_EXIT = "cubicBezier(.4,0,1,1)"
K.EASE_MOVE = "cubicBezier(.65,0,.35,1)"
K.SPRING = "cubicBezier(.16,1,.3,1)"
K.FAST, K.BASE, K.MEDIUM, K.EXIT, K.SPRING_MS = 90, 140, 200, 100, 420
function K.reduced()
	return model.reduced:peek()
end
-- A property that eases to `fn()` whenever what it reads changes. Moves
-- (transforms, sizes) jump under reduced motion; colours and opacity keep
-- their timing, as the lab's reduced mode does.
-- Derived state that holds its value from the start: effects made while a
-- part is built run when the build finishes, so a plain ui.computed reads
-- nil until then. Call it while building (it keeps per-instance state).
function K.computed(fn)
	local s = ui.use_state(fn())
	ui.effect(function()
		s:set(fn())
	end)
	return s
end
-- Tween bindings ease numbers; a colour follows its source directly.
function K.tween(fn, ms, ease, move)
	if type(fn()) ~= "number" then
		return fn
	end
	return ui.tween(K.computed(fn), { duration = (move and K.reduced() and 0 or ms or K.MEDIUM) / 1000, ease = ease or K.EASE_OUT })
end
function K.after(ms, f)
	return anime.createTimer { duration = ms, onComplete = f }
end

---------------------------------------------------------------------------
-- Text, icons, layout helpers.
---------------------------------------------------------------------------
local function merge(node, props)
	if props then
		for k, v in props do
			node[k] = v
		end
	end
	return node
end
K.merge = merge
-- `color = false` inherits the parent's (a button's hover colour).
function K.text(content, size, color, props)
	-- Left-aligned: a label stretched wider than its text keeps it at the start.
	local node = merge({ content, font = K.SANS, text_size = size or 13, align = "left" }, props)
	if color ~= false then
		node.color = color or C.text
	end
	return ui.text(node)
end
function K.mono(content, size, color, props)
	return ui.text(merge({ content, font = K.MONO, text_size = size or 10, color = color or C.muted, align = "left" }, props))
end
-- Geist Mono section labels: 10px, 1.2 tracking.
function K.label(content, color, props)
	return K.mono(content, 10, color or C.muted, merge({ letter_spacing = 1.2 }, props))
end
-- Wrapping text. A function's text is laid out as plain text, rebuilt when
-- the string changes (computed text measures before it knows its width).
function K.para(content, size, color, line_height, props)
	local function node(text)
		return ui.text(merge({ text, align = "left", font = K.SANS, text_size = size, line_height = line_height, color = color, wrap = true }, props))
	end
	if type(content) ~= "function" then
		return node(content)
	end
	return ui.presence(content, node, { w_full = true })
end
-- A show/presence region in a row takes the row's free room; this keeps one
-- at its content's size instead.
function K.fixed(node, props)
	return ui.div(merge({ shrink = 0, node }, props))
end
function K.spacer()
	return ui.div { grow = 1, shrink = 1, min_w = 0 }
end

local svgs = {}
function K.icon(name, size, color, props)
	local code = svgs[name]
	if not code then
		assert(icons[name], "missing icon " .. name)
		code = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#fff" '
			.. 'stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' .. icons[name] .. "</svg>"
		svgs[name] = code
	end
	size = size or 15
	return ui.svg(merge({ code = code, w = size, h = size, tint = color or C.tertiary, shrink = 0 }, props))
end
function K.image(src, size, props)
	return ui.image(merge({ src = "assets/" .. src .. ".png", w = size, h = size, fit = "contain", shrink = 0 }, props))
end
function K.favicon(name, size, props)
	return K.image("favicons/" .. name, size or 16, merge({ rounded = 4 }, props))
end

-- Press feedback (`:active { transform: scale(var(--press-scale)) }`).
function K.press(props, scale)
	local ref = props.ref or ui.ref()
	props.ref = ref
	props.cursor = props.cursor or "pointer"
	local press, release = props.on_press, props.on_release
	props.on_press = function()
		if not K.reduced() then
			anime.animate(ref, { scale = scale or 0.97, duration = K.FAST, ease = K.EASE_OUT })
		end
		if press then
			press()
		end
	end
	props.on_release = function()
		anime.animate(ref, { scale = 1, duration = K.FAST, ease = K.EASE_OUT })
		if release then
			release()
		end
	end
	return props
end
-- A bare button: no engine padding, fill, border or focus look.
function K.button(props, scale)
	props.p = props.p or 0
	props.bg = props.bg or C.clear
	props.border = props.border or 0
	props.hover = props.hover or {}
	props.focus = props.focus or { border_color = C.secondary }
	props.direction = props.direction or "row"
	props.items = props.items or "center"
	if scale == false then
		props.cursor = props.cursor or "pointer"
		return ui.button(props)
	end
	return ui.button(K.press(props, scale))
end

-- .st-btn: the bordered button. o: label, icon, after (icon after the
-- label), small, primary, id, on_click, w, keys (a trailing keycap).
function K.btn(o)
	local small, primary = o.small, o.primary
	local fg = primary and C.on_ink or C.text
	local children = {}
	if o.icon then
		table.insert(children, K.icon(o.icon, 13, fg))
	end
	table.insert(children, ui.text { o.label, id = o.id and o.id .. "_label", font = K.SANS, text_size = small and 12 or 12.5, color = fg, weight = primary and 500 or nil })
	if o.after then
		table.insert(children, K.icon(o.after, 12, fg))
	end
	if o.keys then
		table.insert(children, ui.div { ml = 4, K.keycap(o.keys, "sm") })
	end
	return K.button(merge({
		id = o.id, h = small and 26 or 30, px = small and 10 or 12, gap = small and 5 or 6, rounded = 7, shrink = 0,
		bg = primary and C.ink or C.ctl, border = 1, border_color = primary and C.ink or C.ctl_line,
		hover = primary and { bg = C.soft, border_color = C.soft } or { bg = C.hover, border_color = C.hover_line },
		on_click = o.on_click, justify = o.justify,
		children,
	}, o.props))
end
-- .st-link: underlined text button.
function K.link(label, on_click, props)
	return K.button(merge({
		on_click = on_click, hover = { color = C.text }, color = C.secondary, shrink = 0,
		ui.column { gap = 1, K.text(label, 12, false), ui.div { h = 1, w_full = true, bg = C.strong } },
	}, props), false)
end

---------------------------------------------------------------------------
-- Dither fills (SettingsKit Dither, scaled, amount).
---------------------------------------------------------------------------
K.GRAIN = { fine = 0.5, default = 1, coarse = 2 }
K.U = {
	page = { ink = "#2C2B27FF", pixel = 2, mode = 1, cx = 0, cy = 1, radius = 0.3, from = 0.3, to = 0, curve = 1.6, noise = 0.6, scale = 3, seed = 7 },
	drops = { ink = "#302F2AFF", pixel = 2, mode = 3, from = 0, to = 0.32, curve = 3.6, scale = 2.65, seed = 21, fade = 0.62 },
	esc = { ink = "#4C4B45FF", pixel = 2, mode = 0, angle = 90, from = 0, to = 0.6, curve = 1.8 },
	load = { ink = "#3A3933FF", pixel = 1.5, mode = 0, angle = 90, from = 0.75, to = 0.1, curve = 0.9, noise = 0.2, scale = 3 },
	focus = { ink = "#EDEBE5FF", pixel = 1.5, mode = 1, cx = 0.5, cy = 0.5, radius = 0.55, from = 0.42, to = 0, curve = 1.3, noise = 0.5, scale = 5 },
	track = { ink = "#A3A199FF", pixel = 1.5, mode = 0, angle = 90, from = 0, to = 0.9, curve = 1.1 },
}
-- How strongly a surface dithers: the master intensity times its own amount.
function K.amount(surface)
	local d = model.dither:get()
	return d.master * d[surface]
end
function K.scaled(u, k, grain)
	local out = table.clone(u)
	out.from = (u.from or 0) * k
	out.to = math.min(1, (u.to or 1) * k)
	out.pixel = (u.pixel or 2) * K.GRAIN[grain or model.dither:get().grain]
	return out
end
local function rgba(ink, raw)
	local hex = ink:sub(2, 7)
	local col = raw and "#" .. hex or P(hex)
	return {
		tonumber(col:sub(2, 3), 16) / 255, tonumber(col:sub(4, 5), 16) / 255, tonumber(col:sub(6, 7), 16) / 255,
		#ink >= 9 and tonumber(ink:sub(8, 9), 16) / 255 or 1,
	}
end
local function bayer_params(u)
	local mode = u.mode or 0
	return {
		rgba(u.ink or "#ECEAE4FF", u.raw),
		{ mode, u.pixel or 2, u.angle or 0, u.cx or 0.5 },
		{ u.cy or 0.5, u.radius or 0.5, u.from or 0, u.to or 1 },
		{ u.curve or 1, mode > 2.5 and (u.fade or 0) or (u.noise or 0), u.scale or 4, u.seed or 0 },
	}
end
-- A box filled by `shader` with params from `get` (rerun when what it reads
-- changes). `clock` is the shader time's rate per second while dither
-- animation is on (React's 12 fps tick); `running` gates it further.
function K.fill(shader, get, props, clock, running)
	props = props or {}
	local ref = props.ref or ui.ref()
	props.ref = ref
	props.bg_shader = shader
	props.shader_params = get()
	props.pointer_events = "none"
	local mounted, anim = false, nil
	local function apply(p)
		local set = {}
		for i = 0, 15 do
			set["shaderParam" .. i] = p[i // 4 + 1][i % 4 + 1]
		end
		anime.utils.set(ref, set)
	end
	ui.effect(function()
		local p = get()
		if mounted then
			apply(p)
		end
	end)
	local function tick()
		if not anim then
			return
		end
		local on = model.dither:get().animate and not model.reduced:get() and (not running or running())
		if on then
			anim:play()
		else
			anim:pause()
			anime.utils.set(ref, { shaderTime = 0 })
		end
	end
	if clock then
		ui.effect(tick)
	end
	local on_mount = props.on_mount
	props.on_mount = function(el)
		mounted = true
		if clock then
			anim = anime.animate(el, { shaderTime = { 0, clock * 600 }, duration = 600000, loop = true, ease = "linear", frameRate = 12, autoplay = false })
			ui.batch(tick)
		end
		if on_mount then
			on_mount(el)
		end
	end
	return ui.div(props)
end
-- A Bayer fill over its box. `u` is a uniforms table or a function returning
-- one (React's Dither props); raw keeps the ink out of the palette.
function K.dither(u, props, clock, running)
	local get = type(u) == "function" and u or function()
		return u
	end
	return K.fill(shaders.bayer, function()
		return bayer_params(get())
	end, merge({ absolute = true, top = 0, left = 0, right = 0, bottom = 0 }, props), clock, running)
end

---------------------------------------------------------------------------
-- Orb: the sphere dither over #1F1E1B. Switching it off steps the light down
-- over the 6 dither exit frames; active orbs churn on the 12 fps clock.
---------------------------------------------------------------------------
local ORB = { ink = "#EDEBE5FF", pixel = 1, mode = 2, angle = 225, radius = 0.5, to = 1, curve = 1.1 }
local ACTIVE, PAUSED = { ink = "#9EE07EFF", fill = "#18221A" }, { ink = "#5A5850FF", fill = "#1A1A18" }
local function truth(v)
	if type(v) == "function" then
		return v()
	elseif type(v) == "table" or type(v) == "userdata" then
		return v:get()
	end
	return v
end
K.truth = truth
K.Orb = ui.component(function(props)
	local size, active = props.size, props.active
	local frame = ui.use_state(truth(props.lit) and 6 or 0)
	ui.effect(function()
		local goal = truth(props.lit) and 6 or 0
		local now = frame:peek()
		if goal == now then
			return
		end
		if K.reduced() then
			frame:set(goal)
			return
		end
		anime.animate(frame, { value = goal, duration = math.abs(goal - now) * 1000 / 12, ease = "linear", frameRate = 12, modifier = anime.utils.round(0) })
	end)
	local function palette()
		return active and (truth(props.lit) and ACTIVE or PAUSED) or nil
	end
	local function light_level()
		return 0.12 + 0.88 * frame:get() / 6
	end
	local solid = ui.div {
		absolute = true, top = 0, left = 0, right = 0, bottom = 0, rounded = size / 2,
		hidden = function()
			return K.amount("orbs") > 0
		end,
		bg = function()
			local p = palette()
			return (p and p.ink or ORB.ink):sub(1, 7)
		end,
		opacity = light_level,
	}
	local dithered = K.fill(shaders.orb, function()
		local p = palette()
		local u = K.scaled({ pixel = size > 20 and 1.5 or 1, to = light_level() }, K.amount("orbs"))
		return {
			rgba(p and p.ink or ORB.ink, true),
			{ u.pixel, u.to, ORB.curve, active and 0.35 or 0 },
			{ active and 1 or 0, 3, 0, 0 },
			{ 0, 0, 0, 0 },
		}
	end, { absolute = true, top = 0, left = 0, right = 0, bottom = 0, hidden = function()
		return K.amount("orbs") == 0
	end }, active and 1 or nil, function()
		return truth(props.lit)
	end)
	return ui.overlay {
		id = props.id, w = size, h = size, rounded = size / 2, shrink = 0, overflow = "hidden",
		bg = function()
			local p = palette()
			return p and p.fill or "#1F1E1B"
		end,
		dithered, solid,
	}
end)

---------------------------------------------------------------------------
-- Toggle: the lab's blob switch, a stiff lead and a soft tail on springs.
---------------------------------------------------------------------------
local KNOB = { r = 7, off = 9, on = 23, press = 4 }
local function hull(x1, r1, x2, r2, y)
	if x2 < x1 then
		x1, r1, x2, r2 = x2, r2, x1, r1
	end
	local dist = x2 - x1
	local path = ui.path()
	if dist <= math.abs(r1 - r2) + 0.01 then
		local r, x = math.max(r1, r2), r1 >= r2 and x1 or x2
		return path:arc(x, y, r, 0, 360):close()
	end
	local th = math.acos((r1 - r2) / dist)
	local deg = math.deg(th)
	-- The outer tangent from the top of the right circle round to its bottom,
	-- then back along the bottom tangent and round the left circle.
	path:move(x1 + r1 * math.cos(th), y - r1 * math.sin(th))
	path:line(x2 + r2 * math.cos(th), y - r2 * math.sin(th))
	path:arc(x2, y, r2, -deg, 2 * deg)
	path:line(x1 + r1 * math.cos(th), y + r1 * math.sin(th))
	path:arc(x1, y, r1, deg, 360 - 2 * deg)
	return path:close()
end
-- props: on (state or function), set(v), disabled (function), label, id.
K.Toggle = ui.component(function(props)
	local on = function()
		return truth(props.on)
	end
	local disabled = function()
		return props.disabled and truth(props.disabled) or false
	end
	local start = on() and KNOB.on or KNOB.off
	local lead, tail = ui.use_state(start), ui.use_state(start)
	local pressed, hovered = ui.use_state(false), ui.use_state(false)
	ui.effect(function()
		local target = on() and KNOB.on or KNOB.off
		local goal = target + (pressed:get() and (target == KNOB.on and -KNOB.press or KNOB.press) or 0)
		if K.reduced() then
			lead:set(target)
			tail:set(target)
			return
		end
		anime.animate(lead, { value = goal, duration = 700, ease = anime.createSpring { stiffness = 900, damping = 42 } })
		anime.animate(tail, { value = target, duration = 900, ease = anime.createSpring { stiffness = 300, damping = 22 } })
	end)
	return ui.button {
		id = props.id, w = 32, h = 18, p = 0, rounded = 9, border = 0, shrink = 0, focusable = true,
		cursor = function()
			return disabled() and "default" or "pointer"
		end,
		disabled = disabled,
		bg = K.tween(function()
			if on() then
				return C.ink
			end
			return hovered:get() and not disabled() and C.focus or C.active
		end, K.BASE),
		hover = {}, focus = {},
		on_hover = function(inside)
			hovered:set(inside)
			if not inside then
				pressed:set(false)
			end
		end,
		on_press = function()
			pressed:set(true)
		end,
		on_release = function()
			pressed:set(false)
		end,
		on_click = function()
			if not disabled() then
				props.set(not on())
			end
		end,
		ui.canvas { w = 32, h = 18, pointer_events = "none", draw = function(c)
			local a, b = lead:get(), tail:get()
			local dist = math.abs(a - b)
			c:fill(hull(a, KNOB.r - math.min(1, dist * 0.07), b, KNOB.r - math.min(3.2, dist * 0.32), 9), on() and C.on_ink or C.muted)
		end },
	}
end)

---------------------------------------------------------------------------
-- Popup layer: menus anchor to a trigger's right edge (.st-menu right:0),
-- follow the page as it scrolls and close on Escape or a click outside.
-- Dialogs centre over a scrim. Both draw in one layer over the page.
---------------------------------------------------------------------------
K.menu = ui.state("menu", nil)
K.modal = ui.state("modal", nil)
K.scroll = ui.state("page_scroll", 0)
K.viewport = ui.state("viewport", { w = 1280, h = 820 })
local builders = {}
-- Builders live in this run; a reload starts with nothing open.
K.menu:set(nil)
K.modal:set(nil)
-- An anchor: pass `anchor.props` into the trigger; `anchor.el` is its element.
function K.anchor()
	local a = { el = nil }
	a.mount = function(el)
		a.el = el
	end
	return a
end
-- Opens menu `id`, built by `build()`, under the trigger element `el`.
-- opts: align ("end" or "start"), gap.
function K.open_menu(id, el, build, opts)
	opts = opts or {}
	builders[id] = build
	K.menu:set {
		id = id, x = el.x or 0, y = el.y or 0, w = el.width or 0, h = el.height or 0,
		off = K.scroll:peek(), align = opts.align or "end", gap = opts.gap or 6,
	}
end
function K.toggle_menu(id, el, build, opts)
	local m = K.menu:peek()
	if m and m.id == id then
		K.menu:set(nil)
	else
		K.open_menu(id, el, build, opts)
	end
end
function K.menu_open(id)
	local m = K.menu:get()
	return m ~= nil and m.id == id
end
function K.close_menu()
	K.menu:set(nil)
end
function K.open_modal(id, build)
	builders[id] = build
	K.menu:set(nil)
	K.modal:set(id)
end
function K.close_modal()
	K.modal:set(nil)
end

local function menu_layer()
	return ui.presence(K.menu, function(m)
		if not m or not builders[m.id] then
			return ui.div {}
		end
		local height = ui.use_state(0)
		local function top()
			local shift = K.scroll:get() - m.off
			local below = m.y + m.h + m.gap - shift
			local h, vh = height:get(), K.viewport:get().h
			if h > 0 and below + h > vh - 8 and m.y - shift - m.gap - h >= 8 then
				return m.y - shift - m.gap - h
			end
			return below
		end
		local panel = builders[m.id]()
		return ui.overlay {
			absolute = true, top = 0, left = 0, right = 0, bottom = 0,
			ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, on_press = K.close_menu },
			ui.div {
				absolute = true, top = top,
				left = m.align == "start" and m.x or nil,
				right = m.align == "end" and function()
					return K.viewport:get().w - (m.x + m.w)
				end or nil,
				origin = { m.align == "end" and 1 or 0, 0 }, opacity = 0, scale = 0.97,
				on_resize = function(size)
					height:set(size.h)
				end,
				on_mount = function(el)
					anime.animate(el, { opacity = 1, scale = 1, duration = K.MEDIUM, ease = K.EASE_ENTER })
				end,
				panel,
			},
		}
	end)
end
local function modal_layer()
	return ui.presence(K.modal, function(id)
		if not id or not builders[id] then
			return ui.div {}
		end
		local dialog = ui.ref()
		local function vw()
			return K.viewport:get().w
		end
		local function vh()
			return K.viewport:get().h
		end
		-- The layer scrolls when the dialog is taller than the window.
		return ui.overlay {
			absolute = true, top = 0, left = 0, w = vw, h = vh, bg = "#0000008C",
			on_mount = function(el)
				anime.animate(el, { opacity = { 0, 1 }, duration = K.MEDIUM, ease = K.EASE_ENTER })
				if not K.reduced() then
					anime.animate(dialog, { scale = { 0.97, 1 }, y = { 4, 0 }, duration = K.MEDIUM, ease = K.EASE_ENTER })
				end
			end,
			ui.scroll {
				w = vw, h = vh,
				ui.overlay {
					w = vw, min_h = vh,
					ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, on_press = K.close_modal },
					ui.column {
						w = vw, min_h = vh, p = 24, items = "center", justify = "center", pointer_events = "none",
						-- Presses on the dialog's own blank space stay in it.
						ui.div { ref = dialog, on_press = function() end, builders[id]() },
					},
				},
			},
		}
	end, { exit = { opacity = 0, duration = 120, ease = K.EASE_EXIT } })
end
function K.layer()
	return ui.overlay {
		absolute = true, top = 0, left = 0, right = 0, bottom = 0, pointer_events = "none",
		menu_layer(), modal_layer(),
	}
end
-- Escape closes the open menu, then the open dialog.
function K.escape()
	if K.menu:peek() then
		K.menu:set(nil)
		return true
	end
	if K.modal:peek() then
		K.modal:set(nil)
		return true
	end
	return false
end

-- The shared menu surface (.st-menu).
function K.menu_panel(props, children)
	return ui.column(merge({
		min_w = 200, p = 4, gap = 1, bg = C.elevated, border = 1, border_color = C.strong, rounded = 10,
		shadow = { { x = 0, y = 12, blur = 28, color = "#00000066" }, { x = 0, y = 2, blur = 6, color = "#00000040" } },
		children,
	}, props))
end
-- A menu row (.st-mi): o.lead (node), label, desc (tall), meta (mono),
-- checked, on_click, id, tall.
function K.menu_item(o)
	local tall = o.tall or o.desc ~= nil
	return K.button({
		id = o.id, w_full = true, h = not tall and 30 or nil, py = tall and 7 or 0, px = 8, gap = 9, rounded = 6,
		items = tall and not o.center and "start" or "center",
		bg = o.selected and C.selected or C.clear, hover = { bg = C.selected },
		on_click = o.on_click,
		o.lead,
		o.desc and ui.column { gap = 2, shrink = 1, min_w = 0,
			K.text(o.label, 12.5),
			K.text(o.desc, 11, C.muted, { wrap = true }),
		} or K.text(o.label, 12.5, nil, { shrink = 1, truncate = true }),
		K.spacer(),
		o.meta and K.mono(o.meta, 10.5, C.muted) or nil,
		o.trail,
		-- `checked` may be a function: the check follows it while the menu stays open.
		o.checked and K.icon("check", 13, C.secondary, { mt = tall and not o.center and 2 or 0, hidden = type(o.checked) == "function" and function()
			return not o.checked()
		end or nil }) or nil,
	}, false)
end

---------------------------------------------------------------------------
-- Select: a value pill and its menu (Pen "Value" + the shared menu surface).
-- o: id, value (fn), label (fn, shown instead), options (list or fn),
-- pick(v), orb, multi (fn returning a map of option -> on), lead (fn ->
-- node in the pill), width.
---------------------------------------------------------------------------
function K.Select(o)
	local a = K.anchor()
	local function options()
		return type(o.options) == "function" and o.options() or o.options
	end
	local function build()
		local items = {}
		for i, opt in options() do
			items[i] = K.menu_item {
				id = o.id .. "_option_" .. i, label = opt,
				lead = o.orb and K.Orb { size = 14, lit = true } or o.item_lead and o.item_lead(opt) or nil,
				checked = o.multi and function()
					return o.multi()[opt] == true
				end or opt == o.value(),
				on_click = function()
					o.pick(opt)
					if not o.multi then
						K.close_menu()
					end
				end,
			}
		end
		return K.menu_panel({ min_w = o.width or 200 }, items)
	end
	return K.button {
		id = o.id, h = 30, px = 10, gap = 6, rounded = 7, shrink = 0,
		bg = function()
			return K.menu_open(o.id) and C.hover or C.ctl
		end,
		border = 1,
		border_color = function()
			return K.menu_open(o.id) and C.hover_line or C.ctl_line
		end,
		hover = { bg = C.hover, border_color = C.hover_line },
		on_mount = a.mount,
		on_click = function()
			K.toggle_menu(o.id, a.el, build)
		end,
		o.orb and K.Orb { size = 12, lit = true } or nil,
		o.lead and o.lead() or nil,
		ui.text { function()
			return o.label and o.label() or o.value()
		end, id = o.id .. "_value", font = K.SANS, text_size = 12.5, color = C.soft },
		K.icon("chevrons-up-down", 12, C.muted),
	}
end

---------------------------------------------------------------------------
-- Search targets: rows and cards register under their setting name, so an
-- opened search result can scroll its setting into view and flash it.
---------------------------------------------------------------------------
K.targets = {}
K.flash = ui.state("flash", nil)
-- Props that make a node a search target: registers it and draws the flash
-- ring (.st-flash) over it while it is the one jumped to.
function K.target(name, props)
	props.on_mount = function(el)
		K.targets[name] = el
	end
	return props
end
function K.flash_ring(name, radius)
	local ref = ui.ref()
	ui.effect(function()
		if K.flash:get() == name then
			anime.animate(ref, { opacity = { 1, 0 }, delay = K.reduced() and 0 or 450, duration = K.reduced() and 600 or 1050, ease = K.EASE_OUT })
		end
	end)
	return ui.div {
		ref = ref, absolute = true, top = 0, left = 0, right = 0, bottom = 0, rounded = radius or 10, pointer_events = "none", opacity = 0,
		bg = P("EDEBE5", 0.05), border = 2, border_color = P("EDEBE5", 0.45),
	}
end
-- Typing in a field keeps "/" for the field rather than the search box.
K.editing = ui.state("editing", false)

---------------------------------------------------------------------------
-- Rows and sections.
---------------------------------------------------------------------------
-- .st-row. o: label, desc (string or fn), accent (bool or fn), disabled
-- (fn), setting (search target, the label by default), id, child.
function K.Row(o)
	local accent = o.accent
	local function lit()
		return accent and truth(accent) or false
	end
	local function off()
		return o.disabled and truth(o.disabled) or false
	end
	local name = o.setting or o.label
	return ui.overlay(merge(K.target(name, {
		id = o.id, w_full = true,
		K.flash_ring(name),
		ui.div {
			absolute = true, top = 0, left = 0, right = 0, bottom = 0, rounded = 10, overflow = "hidden",
			hidden = function()
				return not lit()
			end,
			bg = C.elevated, border = 1, border_color = P("EDEBE5", 0.67),
			K.dither(function()
				return K.scaled(K.U.esc, K.amount("highlight"))
			end),
		},
		ui.row {
			w_full = true, p = 15, gap = 24, items = "center",
			opacity = K.tween(function()
				return off() and 0.45 or 1
			end, K.BASE),
			ui.column {
				grow = 1, shrink = 1, min_w = 0, gap = 4,
				K.text(o.label, 13.5, C.text, { id = o.id and o.id .. "_label" }),
				K.para(o.desc, 12, C.tertiary, 17.4),
			},
			o.child,
		},
	}), o.props))
end
-- A switch row for a model flag. Assistant rows go inert, and say why,
-- while AI features are off.
function K.flag(name, desc, disabled, id)
	if name == "Reduce motion" then
		return K.Row { label = name, desc = desc, child = K.Toggle { id = id, on = model.reduced, label = name, set = function(v)
			model.reduced:set(v)
		end } }
	end
	local function off()
		return not model.ai:get() and model.AI_ROWS[name] == true or (disabled and truth(disabled) or false)
	end
	return K.Row {
		label = name, disabled = off,
		desc = function()
			return (not model.ai:get() and model.AI_ROWS[name]) and "Needs AI features. " .. desc or desc
		end,
		child = K.Toggle { id = id, label = name, disabled = off, on = function()
			return model.flag(name) and not off()
		end, set = function(v)
			model.set_flag(name, v)
		end },
	}
end
-- The AI master switch: outlined while off.
function K.AiSwitch(id)
	return K.Row {
		label = "AI features",
		desc = "The assistant, agent actions and page reading. When off, Ask is a plain browser and nothing is sent to a model.",
		accent = function()
			return not model.ai:get()
		end,
		child = K.Toggle { id = id or "ai_toggle", on = model.ai, label = "AI features", set = function(v)
			model.ai:set(v)
		end },
	}
end
-- .st-section: spacing above the label marks a new section; `first` (right
-- under a header) drops the margin.
function K.Section(label, first, props)
	return ui.div(merge({ pt = first and 22 or 36, pb = 8, px = 2, K.label(label) }, props))
end
-- .md-sec: a section label with controls at its end. The margin sits outside
-- the row, so the controls centre on the label.
function K.SectionRow(label, trail, first)
	return ui.row { w_full = true, mt = first and 0 or 14, gap = 12, items = "center", K.Section(label, true), K.spacer(), trail }
end
function K.Header(title, desc, lead)
	return ui.row {
		w_full = true, items = "center", gap = 16, pb = 24,
		lead,
		ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
			K.text(title, 22, C.text, { weight = 500, letter_spacing = -0.3, line_height = 29 }),
			ui.text { desc, align = "left", font = K.SANS, text_size = 13, color = C.tertiary, wrap = true },
		},
	}
end

---------------------------------------------------------------------------
-- Segmented control: one pill glides under the selected option. A danger
-- option tints itself and the pill red. o: id, value (fn), options {{v,
-- label}}, pick(v), small, danger.
---------------------------------------------------------------------------
K.Segmented = ui.component(function(o)
	local spots = ui.use_state({})
	local function current()
		local v = o.value()
		for i, opt in o.options do
			if opt[1] == v then
				return i
			end
		end
		return nil
	end
	local function spot()
		local i = current()
		return i and spots:get()[tostring(i)] or nil
	end
	local function danger()
		return o.danger ~= nil and o.value() == o.danger
	end
	local h = o.small and 22 or 26
	local buttons = {}
	for i, opt in o.options do
		local v, label = opt[1], opt[2]
		local bad = o.danger ~= nil and v == o.danger
		local function on()
			return o.value() == v
		end
		buttons[i] = K.button({
			id = o.id and o.id .. "_" .. tostring(v), h = h, px = o.small and 9 or 12, rounded = 6, justify = "center",
			bg = bad and function()
				return on() and C.clear or P("E8A898", 0.07)
			end or C.clear,
			on_resize = function(size, el)
				local next = table.clone(spots:peek())
				next[tostring(i)] = { x = el.x, w = size.w }
				spots:set(next)
			end,
			on_click = function()
				o.pick(v)
			end,
			on_key = function(e)
				local step = ({ ArrowRight = 1, ArrowDown = 1, ArrowLeft = -1, ArrowUp = -1 })[e.key]
				if not step then
					return false
				end
				local n = #o.options
				o.pick(o.options[((current() or 1) - 1 + step) % n + 1][1])
				return true
			end,
			-- Reserve the medium-weight width so the selected label never shifts the row.
			ui.div {
				K.text(label, o.small and 11.5 or 12, C.text, { weight = 500, invisible = true }),
				ui.text {
					label, absolute = true, top = 0, left = 0, right = 0, align = "center",
					font = K.SANS, text_size = o.small and 11.5 or 12,
					weight = function()
						return on() and 500 or 400
					end,
					color = K.tween(function()
						if bad then
							return on() and C.error or P("E8A898", 0.78)
						end
						return on() and C.text or C.secondary
					end, K.FAST),
				},
			},
		}, false)
	end
	local origin = ui.use_state(0)
	return ui.overlay {
		id = o.id, shrink = 0, p = 2, rounded = 8, bg = C.panel, border = 1, border_color = C.line,
		on_resize = function(_, el)
			origin:set(el.x)
		end,
		ui.div {
			absolute = true, top = 2, left = 0, h = h, rounded = 6,
			hidden = function()
				return spot() == nil
			end,
			x = K.tween(function()
				local s = spot()
				return s and s.x - origin:get() - 1 or 0
			end, K.MEDIUM, K.EASE_OUT, true),
			w = K.tween(function()
				local s = spot()
				return s and s.w or 0
			end, K.MEDIUM, K.EASE_OUT, true),
			bg = K.tween(function()
				return danger() and C.danger_pill or C.selected
			end, K.MEDIUM),
			border = 1,
			border_color = function()
				return danger() and C.error_border or C.clear
			end,
			shadow = { x = 0, y = 1, blur = 2, color = "#0000002E" },
		},
		ui.row { gap = 2, buttons },
	}
end)

---------------------------------------------------------------------------
-- Keycap (Pen "Kbd"): 20px, strong border, Geist Mono 10.5; ⌘ ⌥ and Return
-- draw as icons. size "sm" is the 18px hint size.
---------------------------------------------------------------------------
local KEY_ICONS = { ["⌘"] = "command", ["⌥"] = "option", ["⏎"] = "corner-down-left", ["↵"] = "corner-down-left" }
local function split_keys(keys)
	local parts = {}
	for _, k in (type(keys) == "table" and keys or { keys }) do
		local rest = k
		while #rest > 0 do
			local found, at, len = nil, nil, 0
			for sym in KEY_ICONS do
				local i = string.find(rest, sym, 1, true)
				if i and (not at or i < at) then
					found, at, len = sym, i, #sym
				end
			end
			if not found then
				table.insert(parts, rest)
				break
			end
			if at > 1 then
				table.insert(parts, rest:sub(1, at - 1))
			end
			table.insert(parts, found)
			rest = rest:sub(at + len)
		end
	end
	return parts
end
function K.keycap(keys, size, props)
	local children = {}
	for _, p in split_keys(keys) do
		table.insert(children, KEY_ICONS[p] and K.icon(KEY_ICONS[p], 11, C.secondary) or K.mono(p, 10.5, C.secondary))
	end
	local sm = size == "sm"
	return ui.row(merge({
		h = sm and 18 or 20, min_w = sm and 18 or 20, px = sm and 4 or 5, gap = 2, rounded = 4, shrink = 0,
		items = "center", justify = "center", border = 1, border_color = C.strong,
		children,
	}, props))
end

---------------------------------------------------------------------------
-- Provider tiles: brand colours in every theme and tint.
---------------------------------------------------------------------------
function K.tile(logo, size, props)
	local l = logos.tiles[logo] or logos.tiles.openai
	local inner = math.floor(size * 0.58 + 0.5)
	local code = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="' .. l.d .. '" fill="' .. l.ink .. '" fill-rule="evenodd"/>'
		.. (l.dot and '<path d="' .. l.dot .. '" fill="#1783FF"/>' or "") .. "</svg>"
	return ui.div(merge({
		w = size, h = size, rounded = math.floor(size * 0.29 + 0.5), bg = l.tile, shrink = 0,
		items = "center", justify = "center", border = 1, border_color = "#FFFFFF1F",
		ui.svg { code = code, w = inner, h = inner },
	}, props))
end

---------------------------------------------------------------------------
-- Inputs: a field with its placeholder drawn under the editor.
-- o: id, value (state), placeholder, size, mono, h, props, on_key,
-- on_focus, on_blur, multiline.
---------------------------------------------------------------------------
function K.input(o)
	return ui.overlay(merge({
		grow = 1, shrink = 1, min_w = 0, h = o.h or "100%",
		ui.text {
			o.placeholder or "", absolute = true, left = 0, top = o.multiline and 0 or nil,
			font = o.mono and K.MONO or K.SANS, text_size = o.size or 12, color = o.placeholder_color or C.muted,
			self = "center", wrap = o.multiline,
			hidden = function()
				return o.value:get() ~= ""
			end,
		},
		ui.input {
			id = o.id, label = o.label or o.placeholder or "", value = o.value, multiline = o.multiline,
			w_full = true, h_full = true, p = 0, border = 0, bg = C.clear,
			-- The field around it draws focus; the editor draws none.
			focus = { border = 0, border_color = C.clear }, hover = {},
			font = o.mono and K.MONO or K.SANS, text_size = o.size or 12, color = C.text,
			on_mount = o.on_mount,
			-- Escape still closes an open menu or dialog from inside a field.
			on_key = function(e)
				if o.on_key and o.on_key(e) then
					return true
				end
				if e.key == "Escape" then
					return K.escape()
				end
				return false
			end,
			on_focus = function()
				K.editing:set(true)
				if o.on_focus then
					o.on_focus()
				end
			end,
			on_blur = function()
				K.editing:set(false)
				if o.on_blur then
					o.on_blur()
				end
			end,
		},
	}, o.props))
end
-- The bordered search box (.st-nav-search).
function K.search_box(o)
	local focused = o.focused or ui.state(o.id .. "_focused", false)
	return ui.row(merge({
		h = 30, px = 10, gap = 8, items = "center", rounded = 7, bg = C.inset, border = 1,
		border_color = K.tween(function()
			return focused:get() and C.focus or C.line
		end, K.FAST),
		K.icon("search", 13, nil, { tint = K.tween(function()
			return focused:get() and C.secondary or C.muted
		end, K.FAST) }),
		K.input {
			id = o.id, value = o.value, placeholder = o.placeholder, size = 12, label = o.label, on_key = o.on_key,
			on_mount = o.on_mount,
			on_focus = function()
				focused:set(true)
			end,
			on_blur = function()
				focused:set(false)
			end,
		},
		o.trail and K.fixed(o.trail) or nil,
	}, o.props))
end

return K
