-- Tokens and small controls for Quick ask, from ask.pen's variables and the
-- "Ask · Quick ask · Ask from anywhere" board.
local icons = require("./icons")
local K = {}

K.SANS, K.MONO = "Geist", "Geist Mono"
K.C = {
	window = "#0A0A09", panel = "#121211", elevated = "#1A1A18", raised = "#1F1F1D", selected = "#262624",
	line = "#262521", strong = "#33322D", text = "#EDEBE5", body = "#D4D2CB", secondary = "#A3A199",
	tertiary = "#858379", muted = "#6E6C65", faint = "#4A4943", done = "#C3CEB2", error = "#E8A898",
	hint = "#8F8D85", keycap = "#B5B2A9", soft = "#C9C6BE", ink = "#121211",
	-- The glass panel's own washes.
	glass = "#141413C2", glass_line = "#FFFFFF1F", dock = "#FFFFFF08", dock_line = "#FFFFFF1A",
	wash = "#FFFFFF14", well = "#0A0A0980", key_line = "#FFFFFF1F",
	clear = "#00000000",
}
local C = K.C

K.FAST, K.FADE, K.MOVE = 120, 160, 220
function K.spring(stiffness, damping)
	return anime.createSpring { stiffness = stiffness or 520, damping = damping or 34 }
end
function K.after(ms, f)
	anime.createTimer { duration = ms, onComplete = f }
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

local function textNode(font, content, size, color, props)
	local node: { [any]: any } = { content, font = font, text_size = size, color = color }
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

-- Press feedback: a quick dip, then a spring back.
function K.press(props, scale)
	local ref = ui.ref()
	props.ref = ref
	props.cursor = props.cursor or "pointer"
	local press, release = props.on_press, props.on_release
	props.on_press = function()
		anime.animate(ref, { scale = scale or 0.95, duration = 70, ease = "outQuad" })
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

-- A plain button: no default padding, fill or focus ring, a hover wash.
function K.button(props, scale)
	props.p = props.p or 0
	props.bg = props.bg or C.clear
	props.border = props.border or 0
	props.hover = props.hover or { bg = C.wash }
	props.focus = props.focus or {}
	props.focusable = false
	props.direction = props.direction or "row"
	props.items = props.items or "center"
	return ui.button(K.press(props, scale))
end

-- The board's key cap: a hairline box around Geist Mono.
function K.kbd(label, opts)
	opts = opts or {}
	return ui.row {
		h = opts.h or 18, px = opts.px or 5, rounded = opts.rounded or 4, corners = opts.corners,
		border = 1, border_color = opts.line or C.key_line, items = "center", shrink = 0,
		K.mono(label, opts.size or 10, opts.color or C.keycap, { letter_spacing = 0.5 }),
	}
end

return K
