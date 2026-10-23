-- Tokens and small helpers for the sidebar, from ask.pen's variables and the
-- "Sidebar · Navigation & PiP" board.
local icons = require("./icons")
local shaders = require("./dither")
local K = {}

K.SANS, K.MONO = "Geist", "Geist Mono"
K.C = {
	window = "#0A0A09", swatch_line = "#1F1E1B", panel = "#121211", tray = "#151514", tray_line = "#2A2925",
	field = "#1F1F1D", raised = "#1F1F1D", selected = "#262624", line = "#262521", strong = "#33322D",
	group = "#1D1D1B", tab_on = "#343430", tab_on_line = "#46463F", pin_hover = "#22211E", focus = "#C9C6BD",
	text = "#EDEBE5", soft = "#C9C6BE", tab = "#B7B5AC", secondary = "#A3A199", asleep = "#929087",
	tertiary = "#858379", muted = "#6E6C65", faint = "#4A4943", unread = "#A9BABF", audio = "#C2C5A5",
	gold = "#E2C98F", gold_bg = "#17150F", gold_line = "#4D4130", gold_button = "#1F1B14", done = "#C3CEB2",
	pip = "#141413", pip_line = "#2E2D29", mini = "#1B1B19", orb_bg = "#1F1E1B", tooltip = "#262624",
	clear = "#00000000",
}
local C = K.C

function K.after(ms, f)
	return anime.createTimer { duration = ms, onComplete = f }
end
function K.spring(stiffness, damping)
	return anime.createSpring { stiffness = stiffness or 520, damping = damping or 34 }
end

-- "#rrggbb" and an alpha as the four numbers a shader parameter takes.
function K.rgba(hex, a)
	local n = tonumber(hex:sub(2, 7), 16)
	return { bit32.rshift(n, 16) / 255, bit32.band(bit32.rshift(n, 8), 255) / 255, bit32.band(n, 255) / 255, a or 1 }
end

local svgs = {}
function K.icon(name, size, color, props)
	local code = svgs[name]
	if not code then
		assert(icons[name], "missing icon " .. name)
		code = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#fff" '
			.. 'stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'
			.. icons[name]
			.. "</svg>"
		svgs[name] = code
	end
	local node = { code = code, w = size, h = size, tint = color, shrink = 0 }
	for k, v in props or {} do
		node[k] = v
	end
	return ui.svg(node)
end

function K.favicon(name, size, props)
	local node = { src = "favicons/" .. name .. ".png", w = size, h = size, rounded = size <= 14 and 3 or math.floor(size * 0.22), fit = "cover", shrink = 0 }
	for k, v in props or {} do
		node[k] = v
	end
	return ui.image(node)
end

local function textNode(font, content, size, color, props)
	-- Left-aligned: a label stretched wider than its text keeps it at the start.
	local node: { [any]: any } = { content, font = font, text_size = size, color = color, align = "left" }
	for k, v in props or {} do
		node[k] = v
	end
	return ui.text(node)
end
function K.text(content, size, color, props)
	return textNode(K.SANS, content, size, color or C.text, props)
end
function K.mono(content, size, color, props)
	return textNode(K.MONO, content, size, color or C.muted, props)
end
function K.spacer()
	return ui.div { grow = 1, shrink = 1, min_w = 0 }
end

-- Bayer-dithered fills (bayer-dither.glsl). `mode` 0 linear, 1 radial,
-- 2 lit sphere, 3 noise; the rest are the shader's uniforms.
function K.bayer(o)
	return {
		bg_shader = shaders.bayer,
		shader_params = {
			K.rgba(o.ink or C.text, o.alpha or 1),
			{ o.mode or 0, o.pixel or 1, o.angle or 0, o.cx or 0.5 },
			{ o.cy or 0.5, o.radius or 0.5, o.from or 0, o.to or 1 },
			{ o.curve or 1, o.noise or 0, o.scale or 4, o.seed or 0 },
		},
	}
end

-- The Ask orbs, one per agent state, as the board draws them.
K.ORBS = {
	working = { bg = C.orb_bg, ink = C.text, mode = 1, radius = 0.5, from = 0.05, to = 1, curve = 2.2 },
	reading = { bg = C.orb_bg, ink = C.text, mode = 0, angle = 90, from = 1, to = 0.06, curve = 1.4, noise = 0.25, scale = 3, seed = 9 },
	waiting = { bg = "#25211A", ink = C.gold, mode = 2, angle = 225, radius = 0.5, to = 1, curve = 1.1 },
	done = { bg = "#1C1F18", ink = C.done, mode = 2, angle = 225, radius = 0.5, to = 1, curve = 1.1 },
	chat = { bg = C.orb_bg, ink = C.text, mode = 2, angle = 225, radius = 0.5, to = 1, curve = 1.1 },
}
-- `live` turns the shader's clock, so reading and working orbs move.
function K.orb(kind, size, props)
	local o = K.ORBS[kind]
	local node = K.bayer(o)
	node.w, node.h, node.rounded, node.bg, node.shrink = size, size, size / 2, o.bg, 0
	for k, v in props or {} do
		node[k] = v
	end
	if kind ~= "done" and kind ~= "waiting" then
		local on_mount = node.on_mount
		node.on_mount = function(el)
			anime.animate(el, { shaderTime = { 0, 12 }, duration = 12000, loop = true, ease = "linear", frameRate = 12 })
			if on_mount then
				on_mount(el)
			end
		end
	end
	return ui.div(node)
end

-- Press feedback: a quick dip, then a spring back.
function K.press(props, scale)
	local ref = ui.ref()
	props.ref = ref
	props.cursor = props.cursor or "pointer"
	local press, release = props.on_press, props.on_release
	props.on_press = function()
		anime.animate(ref, { scale = scale or 0.96, duration = 70, ease = "outQuad" })
		if press then
			press()
		end
	end
	props.on_release = function()
		anime.animate(ref, { scale = 1, duration = 380, ease = K.spring(600, 28) })
		if release then
			release()
		end
	end
	return props
end

-- A plain button: no default padding, fill or focus ring.
function K.button(props, scale)
	props.p = props.p or 0
	props.bg = props.bg or C.clear
	props.border = props.border or 0
	-- No engine hover or focus look: rows draw their own from state.
	props.hover = props.hover or {}
	props.focus = props.focus or {}
	if props.focusable == nil then
		props.focusable = false
	end
	props.direction = props.direction or "row"
	props.items = props.items or "center"
	return ui.button(K.press(props, scale))
end

-- A 22-point square icon button, the PiP's and the topbar's.
function K.icon_button(id, name, size, color, on_click, props)
	local node = {
		id = id, w = size + 10, h = size + 10, rounded = 5, justify = "center",
		hover = { bg = C.raised }, on_click = on_click,
		K.icon(name, size, color),
	}
	for k, v in props or {} do
		node[k] = v
	end
	return K.button(node, 0.92)
end

return K
