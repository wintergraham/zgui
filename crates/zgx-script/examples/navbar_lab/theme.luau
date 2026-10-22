-- Ask design tokens (tokens.css) and the small helpers every part of the lab shares.
local icons = require("./icons")
local T = {}

T.C = {
    window = "#0A0A09", panel = "#121211", page = "#161615", elevated = "#1A1A18", raised = "#1F1F1D",
    selected = "#262624", active = "#343430", inset = "#0F0F0E",
    subtle = "#222220", default = "#262521", strong = "#33322D", focus = "#5A5850",
    text = "#EDEBE5", secondary = "#A3A199", tertiary = "#858379", muted = "#6E6C65", faint = "#4A4943",
    clear = "#00000000",
}
-- tokens.css easings, as anime eases.
T.EASE = {
    OUT = "cubicBezier(.2,0,0,1)", ENTER = "cubicBezier(.16,1,.3,1)", EXIT = "cubicBezier(.4,0,1,1)",
    MOVE = "cubicBezier(.65,0,.35,1)",
}

function T.lerp(a, b, t) return a + (b - a) * t end
function T.clamp01(v) return math.clamp(v, 0, 1) end
function T.smooth(a, b, x)
    local t = math.clamp((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)
end

local function rgba(hex)
    hex = string.gsub(hex, "#", "")
    local n = #hex
    local r, g, b = tonumber(string.sub(hex, 1, 2), 16), tonumber(string.sub(hex, 3, 4), 16), tonumber(string.sub(hex, 5, 6), 16)
    local a = n == 8 and tonumber(string.sub(hex, 7, 8), 16) or 255
    return r :: number, g :: number, b :: number, a :: number
end
T.rgba = rgba
local function hex(r, g, b, a)
    local function c(v) return math.clamp(math.floor(v + 0.5), 0, 255) end
    return string.format("#%02X%02X%02X%02X", c(r), c(g), c(b), c(a or 255))
end
T.hex = hex
-- CSS color-mix(in srgb, a, b t): premultiplied, so mixing toward transparent fades rather than darkens.
function T.mix(a, b, t)
    t = math.clamp(t, 0, 1)
    if t <= 0 then return a end
    if t >= 1 then return b end
    local ar, ag, ab, aa = rgba(a)
    local br, bg, bb, ba = rgba(b)
    local alpha = aa * (1 - t) + ba * t
    if alpha <= 0 then return "#00000000" end
    local function ch(x, y) return (x * aa * (1 - t) + y * ba * t) / alpha end
    return hex(ch(ar, br), ch(ag, bg), ch(ab, bb), alpha)
end

function T.icon(name, size, color, props)
    local code = icons[name]
    assert(code, "Missing navbar icon: " .. name)
    local p = props or {}
    p.code = code
    p.w = size or 14
    p.h = size or 14
    p.tint = color or T.C.secondary
    return ui.svg(p)
end
-- A filled copy of an outline icon (the bookmark star once saved).
function T.filled(name, size, color)
    local code = string.gsub(icons[name], 'fill="none"', 'fill="currentColor"')
    return ui.svg { code = code, w = size or 14, h = size or 14, tint = color or T.C.secondary }
end

-- A button without zgui's default hover fill and focus ring: the source's buttons (navbar.css `.nb-win button`)
-- have neither unless styled, and browsers ring only keyboard focus.
function T.button(props)
    props.hover = props.hover or {}
    props.focus = props.focus or {}
    return ui.button(props)
end

-- CSS `line-height: normal` for Geist as Chrome lays it out: the font's ascent (1.005em) and descent (.295em),
-- each rounded to whole pixels. zgui's own normal is 1.4em, a little taller.
function T.normal(size)
    return math.floor(size * 1.005 + 0.5) + math.floor(size * 0.295 + 0.5)
end
-- ui.text with the page's line height unless one is given.
function T.text(props)
    if props.line_height == nil then props.line_height = T.normal(props.text_size or 13) end
    return ui.text(props)
end
function T.txt(label, size, color, props)
    local p = props or {}
    p[1] = label
    p.text_size = size or 13
    p.color = color or T.C.text
    return T.text(p)
end

-- navbar.js key(): a keycap of glyphs and words ("⌘ ↵", "Tab").
local KEYG = { ["↑"] = "arrow-up", ["↓"] = "arrow-down", ["↵"] = "corner-down-left", ["⇥"] = "arrow-right-to-line",
    ["⇧"] = "arrow-big-up", ["⌘"] = "command" }
function T.key(keys, mono)
    local parts = {}
    for token in string.gmatch(keys, "%S+") do
        if KEYG[token] then
            table.insert(parts, T.icon(KEYG[token], 11, T.C.tertiary))
        else
            table.insert(parts, T.text { token, text_size = 10, color = T.C.tertiary, font = mono and "Geist Mono" or nil,
                letter_spacing = mono and 0.2 or nil })
        end
    end
    return ui.row { h = 18, px = 6, gap = 2, items = "center", justify = "center", rounded = 4, border = 1,
        border_color = T.C.strong, shrink = 0, parts }
end
-- .nb-hint: a keycap and its label.
function T.hint(keys, label)
    return ui.row { gap = 6, items = "center", shrink = 0, T.key(keys), T.txt(label, 11.5, T.C.muted) }
end

-- A state that eases to what `source` returns whenever that changes (a CSS transition), on the anime engine so
-- it reads like any state and its duration follows the lab's slow-mo. Inside built content it belongs to that
-- content; at the top level it is keyed by order.
function T.easer(slow, prefix)
    local n = 0
    return function(source, ms, ease)
        local target = source()
        local ok, s = pcall(ui.use_state, target)
        if not ok then
            n += 1
            s = ui.state(prefix .. "_eased_" .. n, target)
        end
        ui.effect(function()
            local v = source()
            if v == target then return end
            target = v
            anime.animate(s, { value = v, duration = ms * slow:peek(), ease = ease or T.EASE.OUT })
        end)
        return s
    end
end

return T
