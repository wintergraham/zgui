-- ask.pen's variables as two themes (the board's own dark, and a light one
-- made to match it), and the small controls every step shares.
local icons = require("./icons")
local K = {}

K.SANS, K.MONO = "Geist", "Geist Mono"

-- The board's variables (bg-window, text-primary, …), dark as ask.pen has them.
K.DARK = {
	window = "#0A0A09", panel = "#121211", page = "#161615", elevated = "#1A1A18", raised = "#1F1F1D",
	selected = "#262624", active = "#343430", inset = "#0F0F0E", line_subtle = "#222220", line = "#262521",
	line_strong = "#33322D", text = "#EDEBE5", body = "#D4D2CB", secondary = "#A3A199", tertiary = "#858379",
	muted = "#6E6C65", faint = "#4A4943", ink = "#EDEBE5", on_ink = "#121211", done = "#C3CEB2",
	waiting = "#E2C98F", warning = "#D9A23B", highlight = "#F2C94C", highlight_bg = "#F2C94C26",
	field = "#1C1C1A", field_line = "#2E2D29", wash = "#FFFFFF0A", wash_line = "#FFFFFF24", hover = "#FFFFFF0D",
	unguarded = "#E5A53A80", shadow = "#00000080", pressed = "#FFFFFF14", text_selection = "#EDEBE533",
}
K.LIGHT = {
	window = "#F6F5F1", panel = "#FFFFFF", page = "#EFEEE9", elevated = "#FFFFFF", raised = "#ECEBE6",
	selected = "#E3E1DB", active = "#D6D4CD", inset = "#EFEEEA", line_subtle = "#E7E5DF", line = "#DEDCD5",
	line_strong = "#CCC9C1", text = "#1B1A17", body = "#2E2D29", secondary = "#5F5D56", tertiary = "#77756D",
	muted = "#8B8981", faint = "#B4B1A8", ink = "#1B1A17", on_ink = "#F6F5F1", done = "#5E7A45",
	waiting = "#8F6410", warning = "#A8700C", highlight = "#D9A21B", highlight_bg = "#F2C94C33",
	field = "#FFFFFF", field_line = "#DAD8D1", wash = "#0000000A", wash_line = "#00000021", hover = "#0000000A",
	unguarded = "#C98A1A99", shadow = "#00000026", pressed = "#00000014", text_selection = "#1B1A1726",
}
-- Every color is a token, so a theme switch restyles the window in place.
K.C = {} :: { [string]: string }
for name in K.DARK do
	K.C[name] = "$ask." .. name
end
K.C.clear = "#00000000"
local C = K.C

local function theme(name, kind, base, colors)
	ui.theme.register {
		name = name,
		type = kind,
		extends = base,
		tokens = {
			ask = colors,
			background = colors.window,
			text = { primary = colors.text, secondary = colors.secondary, tertiary = colors.tertiary },
			surface = { overlay = colors.raised, raised = colors.raised },
			menu = { bg = colors.raised, hover = colors.selected },
			border = colors.line_strong,
			["border.subtle"] = colors.line_subtle,
			["border.strong"] = colors.line_strong,
			-- zgui's own interactive colors are blue; these keep presses,
			-- selections and focus in the board's warm neutrals.
			accent = colors.ink,
			["accent.hover"] = colors.body,
			["accent.text"] = colors.on_ink,
			selection = colors.text_selection,
			["selection.item"] = colors.selected,
			["focus.ring"] = colors.line_strong,
			hover = colors.hover,
			pressed = colors.pressed,
			["scrollbar.thumb"] = colors.line_strong,
			["scrollbar.thumb.active"] = colors.faint,
		},
	}
end
ui.theme.tokens { ask = K.DARK }
theme("ask-dark", "dark", "zgui-dark", K.DARK)
theme("ask-light", "light", "zgui-light", K.LIGHT)
ui.theme.default("ask-dark")

-- Whether the light theme is showing (tracked).
function K.light(): boolean
	return ui.theme.current:get() == "ask-light"
end

-- The board's durations (dur-fast, dur-base, dur-medium, dur-slow).
K.FAST, K.BASE, K.MEDIUM, K.SLOW = 90, 140, 200, 280

function K.icon(name: string, size: number, color: any, props: { [string]: any }?)
	assert(icons[name], "missing icon " .. name)
	local node: { [string]: any } = { code = icons[name], w = size, h = size, tint = color, shrink = 0 }
	for k, v in props or {} do
		node[k] = v
	end
	return ui.svg(node)
end

local function text_node(font, content, size, color, props)
	local node: { [any]: any } = { content, font = font, text_size = size, color = color }
	for k, v in props or {} do
		node[k] = v
	end
	return ui.text(node)
end
function K.text(content: any, size: number, color: any?, props: { [string]: any }?)
	return text_node(K.SANS, content, size, color or C.text, props)
end
function K.mono(content: any, size: number, color: any?, props: { [string]: any }?)
	return text_node(K.MONO, content, size, color or C.muted, props)
end
function K.spacer()
	return ui.div { grow = 1, shrink = 1, min_w = 0 }
end

-- Press feedback: a quick dip, then a spring back.
function K.press(props: { [string]: any }, scale: number?)
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
		anime.animate(ref, { scale = 1, duration = 360, ease = anime.createSpring { stiffness = 600, damping = 28 } })
		if release then
			release()
		end
	end
	return props
end

-- A plain button: no default padding, fill or focus ring; a hover wash.
function K.button(props: { [string]: any }, scale: number?)
	props.p = props.p or 0
	props.bg = props.bg or C.clear
	props.border = props.border or 0
	props.hover = props.hover or { bg = C.hover }
	props.focus = props.focus or {}
	props.focusable = false
	props.direction = props.direction or "row"
	props.items = props.items or "center"
	return ui.button(K.press(props, scale))
end

-- The board's Button: 32 tall, 12.5 medium. `kind` is "primary" (ink),
-- "secondary" (raised, outlined) or "ghost".
function K.action(label: any, kind: string, on_click: () -> (), opts: { [string]: any }?)
	opts = opts or {}
	local disabled = opts.disabled
	local fg = kind == "primary" and C.on_ink or kind == "secondary" and C.text or C.secondary
	local props: { [string]: any } = {
		id = opts.id,
		h = 32,
		px = 12,
		gap = 7,
		rounded = 8,
		shrink = 0,
		justify = "center",
		bg = kind == "primary" and C.ink or kind == "secondary" and C.raised or C.clear,
		border = kind == "secondary" and 1 or 0,
		border_color = C.line_strong,
		hover = kind == "primary" and { opacity = 0.88 } or kind == "secondary" and { bg = C.selected } or { bg = C.hover },
		on_click = function()
			if not (disabled and disabled()) then
				on_click()
			end
		end,
		K.text(label, 12.5, fg, { weight = 500 }),
	}
	if disabled then
		props.opacity = function()
			return if disabled() then 0.4 else 1
		end
		props.cursor = function()
			return if disabled() then "default" else "pointer"
		end
	end
	return K.button(props, 0.97)
end

-- A switch: ink when on, its knob sliding across. Without `toggle` it only
-- shows, for a row that takes the click itself.
function K.switch(on: () -> boolean, toggle: (() -> ())?, opts: { [string]: any }?)
	opts = opts or {}
	local make = if toggle then function(props)
		return K.button(props, 0.92)
	end else function(props)
		props.hover = nil
		return ui.row(props)
	end
	return make({
		id = opts.id,
		w = 32,
		h = 18,
		p = 2,
		rounded = 9,
		shrink = 0,
		hover = {},
		justify = function()
			return if on() then "end" else "start"
		end,
		bg = function()
			return if on() then C.ink else C.active
		end,
		on_click = toggle,
		ui.div {
			w = 14,
			h = 14,
			rounded = 7,
			layout = "position",
			layout_duration = K.MEDIUM,
			layout_ease = "outCubic",
			bg = function()
				return if on() then C.on_ink else C.tertiary
			end,
		},
	})
end

-- A checkbox, as the browser tiles have it.
function K.check(on: () -> boolean)
	return ui.overlay {
		w = 16,
		h = 16,
		rounded = 4,
		shrink = 0,
		items = "center",
		justify = "center",
		border = 1,
		bg = function()
			return if on() then C.ink else C.clear
		end,
		border_color = function()
			return if on() then C.ink else C.faint
		end,
		ui.presence(function()
			return if on() then "on" else nil
		end, function()
			return K.icon("check", 12, C.on_ink)
		end, {
			enter = { opacity = { 0, 1 }, scale = { 0.6, 1 }, duration = K.BASE, ease = "outBack(2)" },
			exit = { opacity = 0, duration = K.FAST },
		}),
	}
end

-- A radio, filled when on.
function K.radio(on: () -> boolean, size: number?)
	local s = size or 16
	return ui.overlay {
		w = s,
		h = s,
		rounded = s / 2,
		shrink = 0,
		items = "center",
		justify = "center",
		border = 1,
		border_color = function()
			return if on() then C.text else C.line_strong
		end,
		ui.presence(function()
			return if on() then "on" else nil
		end, function()
			return ui.div { w = s / 2, h = s / 2, rounded = s / 4, bg = C.text }
		end, {
			enter = { scale = { 0, 1 }, duration = K.MEDIUM, ease = "outBack(2)" },
			exit = { scale = 0, opacity = 0, duration = K.FAST },
		}),
	}
end

-- The board's segmented control. The selection slides to the option taken.
-- `color` may give an option its own label color (Unguarded's).
function K.segmented(name: string, options: { string }, value: () -> string?, pick: (string) -> (), color: { [string]: string }?)
	local items = {}
	for _, option in options do
		table.insert(
			items,
			K.button({
				id = name .. ":" .. option,
				h = 26,
				px = 10,
				rounded = 5,
				relative = true,
				hover = {},
				on_click = function()
					pick(option)
				end,
				ui.presence(function()
					return if value() == option then "on" else nil
				end, function()
					return ui.div {
						layout_id = "segment:" .. name,
						layout_duration = 260,
						layout_ease = "outQuint",
						h_full = true,
						w_full = true,
						rounded = 5,
						bg = C.selected,
					}
				end, { absolute = true, top = 0, left = 0, right = 0, bottom = 0 }),
				K.text(option, 12, function()
					if color and color[option] then
						return color[option]
					end
					return if value() == option then C.text else C.secondary
				end),
			}, 0.97)
		)
	end
	return ui.row {
		h = 30,
		p = 2,
		gap = 2,
		shrink = 0,
		items = "center",
		bg = C.panel,
		border = 1,
		border_color = C.line,
		rounded = 7,
		items,
	}
end

-- A browser's logo: its own icon, or (Chrome, Arc, Edge) its mark on a
-- white plate.
function K.logo(b: any, size: number)
	if b.id == "file" then
		return ui.overlay {
			w = size,
			h = size,
			shrink = 0,
			items = "center",
			justify = "center",
			K.icon("file-plus", math.floor(size * 0.56), C.tertiary),
		}
	end
	local src = "assets/" .. b.id .. ".png"
	if not b.plate then
		return ui.image { src = src, w = size, h = size, fit = "contain", shrink = 0 }
	end
	local plate = math.floor(size * 0.79)
	return ui.overlay {
		w = size,
		h = size,
		shrink = 0,
		items = "center",
		justify = "center",
		ui.overlay {
			w = plate,
			h = plate,
			rounded = math.floor(plate * 0.24),
			items = "center",
			justify = "center",
			gradient = { angle = 180, stops = { "#FFFFFF", "#ECECEC" } },
			shadow = { x = 0, y = 1, blur = 3, color = "#0000004D" },
			ui.image { src = src, w = math.floor(plate * 0.68), h = math.floor(plate * 0.68), fit = "contain" },
		},
	}
end

-- A section label: Geist Mono, spaced, muted.
function K.eyebrow(label: any, size: number?, props: { [string]: any }?)
	local p = props or {}
	p.letter_spacing = p.letter_spacing or ((size or 11) <= 10 and 1.2 or 0.8)
	return K.mono(label, size or 11, C.muted, p)
end

-- A step's heading: its title, and a paragraph under it when one is given.
function K.heading(title: any, body: any?, opts: { [string]: any }?)
	opts = opts or {}
	return ui.column {
		items = "stretch",
		gap = opts.gap or 8,
		shrink = 0,
		K.text(title, 24, C.text, { weight = 600, letter_spacing = -0.4 }),
		if body then K.text(body, 14, C.secondary, { wrap = true, line_height = opts.line_height or 21, max_w = opts.body_w or 540 }) else nil,
	}
end

return K
