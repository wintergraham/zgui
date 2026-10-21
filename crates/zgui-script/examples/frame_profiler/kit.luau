-- Tokens and small controls shared by the profiler and its menus.
local icons = require("./icons")
local K = {}

K.SANS, K.MONO = "Geist", "Geist Mono"
K.C = {
	window = "#121211",
	body = "#161615",
	inset = "#0F0F0E",
	menu = "#1A1A18",
	tip = "#1F1F1D",
	raised = "#1F1F1D",
	selected = "#262624",
	line = "#262521",
	subtle = "#222220",
	strong = "#33322D",
	text = "#EDEBE5",
	body_text = "#D4D2CB",
	secondary = "#A3A199",
	tertiary = "#858379",
	muted = "#6E6C65",
	faint = "#4A4943",
	coral = "#E8A898",
	gold = "#E2C98F",
	warn = "#D9A23B",
	info = "#A9BABF",
	live = "#C3CEB2",
	clear = "#00000000",
}
local C = K.C

-- Motion: the design system's springs and fades, in ms.
K.FAST, K.FADE, K.MOVE = 120, 160, 220
function K.spring(stiffness, damping)
	return anime.createSpring { stiffness = stiffness or 520, damping = damping or 34 }
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

-- A plain button: no default padding or fill, a hover wash.
function K.button(props, scale)
	props.p = props.p or 0
	props.bg = props.bg or C.clear
	props.border = props.border or 0
	props.hover = props.hover or { bg = C.raised }
	-- The design has no focus ring: a pressed control keeps its own look.
	props.focus = props.focus or {}
	props.direction = props.direction or "row"
	props.items = props.items or "center"
	return ui.button(K.press(props, scale))
end

function K.icon_button(id, name, size, on_click, active)
	return K.button {
		id = id,
		w = 28,
		h = 28,
		rounded = 7,
		justify = "center",
		on_click = on_click,
		bg = active and function()
			return active() and C.selected or C.clear
		end or nil,
		K.icon(name, size or 14, C.secondary),
	}
end

-- A row of options over a pill that slides (and resizes) to the selected one.
-- `build(i, active)` returns option i's contents; `active()` is reactive.
function K.segmented(id, selected, count, build, style)
	local geom = ui.state(id .. "_geom", {})
	local row
	local items = {}
	local function measure()
		if not row then
			return
		end
		local g = {}
		for i, el in items do
			g[i] = { x = el.x - row.x, w = el.width }
		end
		geom:set(g)
	end
	local function at(field)
		return ui.spring(
			ui.computed(function()
				local g = geom:get()[selected:get()]
				return g and g[field] or 0
			end),
			{ stiffness = 420, damping = 34 }
		)
	end
	local options = {}
	for i = 1, count do
		local active = function()
			return selected:get() == i
		end
		options[i] = K.button({
			id = id .. "_" .. i,
			h = style.h,
			w = style.w,
			px = style.px or 8,
			gap = style.gap or 6,
			rounded = style.r or 5,
			bg = C.clear,
			hover = { bg = C.clear },
			justify = "center",
			on_click = function()
				selected:set(i)
			end,
			on_resize = function(_, el)
				items[i] = el
				measure()
			end,
			build(i, active),
		}, 0.96)
	end
	return ui.overlay {
		id = id,
		h = style.h + 2 * (style.p or 2),
		p = style.p or 2,
		bg = style.bg,
		rounded = style.outer or 7,
		on_resize = function(_, el)
			row = el
			measure()
		end,
		ui.div {
			absolute = true,
			top = style.p or 2,
			left = 0,
			h = style.h,
			rounded = style.r or 5,
			bg = style.pill or C.selected,
			x = at("x"),
			w = at("w"),
			opacity = function()
				return #geom:get() > 0 and 1 or 0
			end,
		},
		ui.row { gap = style.spacing or 2, items = "center", options },
	}
end

-- The design's switch: 32×18, a dark knob on a light track when on.
function K.toggle(id, value)
	local knob = ui.spring(
		ui.computed(function()
			return value:get() and 14 or 0
		end),
		{ stiffness = 700, damping = 36 }
	)
	local wash = ui.tween(
		ui.computed(function()
			return value:get() and 1 or 0
		end),
		{ duration = 140 }
	)
	return K.button({
		id = id,
		w = 32,
		h = 18,
		rounded = 9,
		bg = "#343430",
		hover = { bg = "#3A3935" },
		on_click = function()
			value:toggle()
		end,
		ui.overlay {
			w = 32,
			h = 18,
			ui.div { absolute = true, top = 0, left = 0, w = 32, h = 18, rounded = 9, bg = C.text, opacity = wash },
			ui.div {
				absolute = true,
				top = 2,
				left = 2,
				w = 14,
				h = 14,
				rounded = 7,
				x = knob,
				bg = function()
					return value:get() and C.window or C.tertiary
				end,
			},
		},
	}, 0.94)
end

-- A bordered key, as tooltips and hints show shortcuts.
function K.kbd(label)
	return ui.row {
		h = 18,
		px = 5,
		rounded = 4,
		border = 1,
		border_color = C.strong,
		items = "center",
		K.mono(label, 10, C.secondary, { letter_spacing = 4 }),
	}
end

-- A small key cap: icons and/or text.
function K.keys(parts, color)
	local nodes = {}
	for _, part in parts do
		if icons[part] then
			table.insert(nodes, K.icon(part, 11, color or C.muted))
		else
			table.insert(nodes, K.mono(part, 10.5, color or C.muted))
		end
	end
	return ui.row { gap = 3, items = "center", h = 14, nodes }
end

return K
