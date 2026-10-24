-- Ask Motion Lab / Controls. The tree mounts once; state updates retained bindings.
local icons = require("./icons")
local dither = require("./dither")
local C = {
    window = "#0A0A09", panel = "#121211", elevated = "#1A1A18", raised = "#1F1F1D",
    selected = "#262624", active = "#343430", inset = "#0F0F0E", line = "#262521",
    subtle = "#222220", strong = "#33322D", focus = "#5A5850", text = "#EDEBE5",
    secondary = "#A3A199", tertiary = "#858379", muted = "#6E6C65", error = "#E8A898",
}
local reduced = ui.state("reduce_motion", false)
local slow = ui.state("slow", 1)
local pageOffset = ui.state("page_offset", 0)
local viewport = ui.state("viewport", { w = 1180, h = 900 })
local compact = ui.computed(function() return viewport:get().w <= 820 end)
local function ms(n) return reduced:peek() and 1 or n * slow:peek() end
local function spring() return reduced:peek() and "linear" or anime.createSpring { stiffness = 520, damping = 30 } end
local function tween(source, opts)
    opts = opts or {}
    opts.duration = opts.duration or 160
    return ui.tween(ui.computed(source), opts)
end
local function springBinding(source, opts)
    return ui.spring(ui.computed(source), opts)
end
local function icon(name, size, color)
    local code = icons[name]
    assert(code, "Missing Controls icon: " .. name)
    return ui.svg { code = code:gsub("currentColor", color or C.tertiary), w = size or 14, h = size or 14 }
end
local function orb(size)
    return ui.div { w = size or 14, h = size or 14, rounded = 20, bg = C.raised,
        bg_shader = dither, shader_params = { C.text, { 1, 0, 1, 0 } } }
end
local function press(props, scale)
    props.cursor = "pointer"
    props.p = props.p or 0
    props.hover = props.hover or {}
    props.focus = { border_color = C.secondary }
    local ref = ui.ref()
    props.ref = ref
    props.on_press = function()
        anime.animate(ref, { scale = scale or 0.94, duration = ms(80), ease = "outQuad" })
    end
    props.on_release = function()
        anime.animate(ref, { scale = 1, duration = ms(420), ease = spring() })
    end
    return props
end
local function txt(label, size, color)
    return ui.text { label, text_size = size or 12.5, color = color or C.text }
end
local function caption(label) return txt(label, 12, C.secondary) end
local function row(children, gap)
    return ui.row { w_full = true, items = "center", justify = "center", flex_wrap = true, gap = gap or 12, children }
end
local function button(id, label, variant, load, smooth, click)
    local primary, danger, ghost = variant == "primary", variant == "danger", variant == "ghost"
    local fg = primary and C.panel or danger and C.error or ghost and C.secondary or C.text
    local bg = primary and C.text or danger and "#1C1513" or ghost and "#00000000" or C.raised
    local state = ui.state(id .. "_phase", "rest")
    local ref = ui.ref()
    local wash = ui.ref()
    local function start()
        if state:peek() ~= "rest" then return end
        if click then click() end
        if not load then return end
        state:set("working")
        anime.animate(wash, { shaderTime = { 0, 1.7 }, shaderParam6 = { load == "progress" and 0 or 1, 1 },
            opacity = { 0, 1 }, duration = ms(1700), frameRate = smooth and 60 or 12, ease = "outQuint",
            onComplete = function()
                state:set("done")
                anime.animate(wash, { opacity = 0, duration = ms(smooth and 420 or 640), frameRate = smooth and 60 or 12 })
                anime.createTimer { duration = ms(1100), onComplete = function() state:set("rest") end }
            end,
        })
    end
    local p = press { id = id, h = 32, px = 12, rounded = 8, bg = bg, color = fg,
        border = 1, border_color = danger and "#4A2C25" or primary and "#00000000" or ghost and "#00000000" or C.strong,
        hover = { bg = primary and "#FFFFFF" or danger and "#241917" or ghost and C.raised or C.selected },
        on_click = start,
        ui.overlay { h = 30, items = "center", justify = "center", gap = 7,
            ui.div { ref = wash, absolute = true, top = 0, bottom = 0, left = -12, right = -12,
                opacity = 0, bg_shader = dither,
                shader_params = { primary and "#12121155" or danger and "#E8A89855" or "#EDEBE555", { 2, 1, 1, smooth and 0.2 or 0 } } },
            ui.row { items = "center", gap = 7,
                id == "new" and icon("plus", 14, fg) or nil,
                ui.presence(state, function(phase)
                    return ui.row { ref = ref, items = "center", justify = "center", gap = 5, min_w = load and 54 or 0,
                        phase == "done" and icon("check", 13, fg) or nil,
                        ui.text { phase == "working" and "Working" or phase == "done" and "Saved" or label,
                            id = id .. "_label", text_size = 12.5, weight = 500, color = fg },
                    }
                end, { enter = { y = { 6, 0 }, opacity = { 0, 1 }, duration = 200, ease = "outExpo" },
                    exit = { y = -6, opacity = 0, duration = 140 } }),
                id == "save" and ui.div { h = 18, px = 4, rounded = 4, border = 1, border_color = "#12121133",
                    items = "center", justify = "center", icon("corner-down-left", 11, "#12121199") } or nil,
            },
        },
    }
    return ui.button(p)
end
local pinned = ui.state("pinned", false)
local function iconButton(id, name, outlined, click, active)
    return ui.button(press { id = id, w = 28, h = 28, rounded = 7, items = "center", justify = "center",
        border = 1, border_color = outlined and C.strong or "#00000000",
        bg = active and function() return active:get() and C.selected or "#00000000" end or "#00000000",
        hover = { bg = C.raised }, on_click = click, icon(name, 15) })
end

local search = ui.state("search", "")
local name = ui.state("name", "")
local instructions = ui.state("instructions", "")
local function field(id, label, placeholder, value, area)
    local focused = ui.state(id .. "_focused", false)
    local searchIcon = ui.ref()
    if id == "search" then
        ui.effect(function()
            search:get()
            anime.animate(searchIcon, { scale = { 1.12, 1 }, duration = ms(240), ease = "outQuad" })
        end)
    end
    return ui.column { w = area and 280 or 240, grow = area and 1 or 0, min_w = 240, gap = 6,
        caption(label),
        ui.row { w_full = true, h = area and 92 or 32, rounded = area and 10 or 8, px = 10,
            items = "center", gap = 8, bg = function() return focused:get() and "#141413" or C.inset end,
            border = 1, border_color = function() return focused:get() and C.focus or C.line end,
            id == "search" and ui.div { ref = searchIcon, icon("search", 13) } or nil,
            ui.overlay { grow = 1, shrink = 1, min_w = 0, h_full = true,
                ui.text { placeholder, absolute = true, left = 0, top = area and 10 or 7,
                    hidden = function() return value:get() ~= "" end, text_size = 12.5, color = C.muted },
                ui.input { id = id .. "_input", label = placeholder, value = value, multiline = area,
                w_full = true, h_full = true, p = 0, py = area and 10 or 0,
                border = 0, bg = "#00000000", text_size = 12.5, color = C.text,
                on_focus = function() focused:set(true) end, on_blur = function() focused:set(false) end,
                on_key = function(e)
                    if id == "search" and e.key == "Escape" and search:peek() ~= "" then search:set(""); return true end
                end,
                },
            },
            id == "search" and ui.overlay { w = 30, h = 20,
                ui.text { "⌘K", font = "Geist Mono", text_size = 10, color = C.muted, p = 3,
                    border = 1, border_color = C.strong, rounded = 4,
                    opacity = tween(function() return search:get() == "" and 1 or 0 end, { duration = 90 }),
                    scale = springBinding(function() return search:get() == "" and 1 or 0.6 end) },
                ui.button { p = 0, id = "clear_search", absolute = true, right = 0, w = 20, h = 20, rounded = 5,
                    hidden = function() return search:get() == "" end, hover = { bg = C.selected },
                    on_click = function() search:set("") end, items = "center", justify = "center", icon("x", 12) },
            } or nil,
        },
    }
end

local function segmented(id, labels, initial, width)
    local selected = ui.state(id, initial)
    local children = {}
    for i, label in labels do
        children[i] = ui.button(press { id = id .. "_" .. i, w = width, h = 24, items = "center", justify = "center",
            rounded = 7, border = 1, border_color = "#00000000", bg = "#00000000",
            color = function() return selected:get() == i and C.text or C.tertiary end,
            on_click = function() selected:set(i) end,
            on_key = function(e)
                local d = ({ ArrowRight = 1, ArrowDown = 1, ArrowLeft = -1, ArrowUp = -1 })[e.key]
                if d then selected:set((selected:peek() - 1 + d) % #labels + 1); return true end
            end,
            txt(label, 12.5),
        })
    end
    return ui.overlay { w = #labels * (width + 2) + 4, h = 32, p = 3, bg = C.inset, rounded = 10,
        border = 1, border_color = C.subtle,
        ui.div { absolute = true, left = 3, top = 3, w = width, h = 24, rounded = 7, bg = C.selected,
            x = tween(function() return (selected:get() - 1) * (width + 2) end, { duration = 160, ease = "outCubic" }) },
        ui.row { gap = 2, children },
    }, selected
end
local mode = segmented("mode", { "Search", "Ask", "Agent" }, 2, 79)
local range = segmented("range", { "Day", "Week", "Month", "Year" }, 2, 56)

local function toggle(id, label, initial, shared)
    local value = shared or ui.state(id, initial)
    local pressed = ui.state(id .. "_pressed", false)
    local target = function()
        local x = value:get() and 23 or 9
        if pressed:get() then x += value:get() and -4 or 4 end
        return x
    end
    local lead = ui.state(id .. "_lead", initial and 23 or 9)
    local tail = ui.state(id .. "_tail", initial and 23 or 9)
    ui.effect(function()
        local goal = target()
        local home = value:get() and 23 or 9
        anime.animate(lead, { value = goal, ease = reduced:peek() and "linear" or anime.createSpring { stiffness = 900, damping = 42 }, duration = ms(400) })
        anime.animate(tail, { value = home, ease = reduced:peek() and "linear" or anime.createSpring { stiffness = 300, damping = 22 }, duration = ms(500) })
    end)
    return ui.row { h = 20, gap = 10, items = "center",
        ui.button { p = 0, id = id, w = 32, h = 18, rounded = 9, border = 1, border_color = "#00000000",
            hover = {}, bg = function() return value:get() and C.text or C.active end, focus = { border_color = C.secondary },
            cursor = "pointer", on_press = function() pressed:set(true) end,
            on_release = function() pressed:set(false) end, on_click = function() value:toggle() end,
            ui.canvas { w = 32, h = 18, draw = function(c)
                -- Both ends have separate springs, matching the source's stretched knob.
                local l = reduced:get() and (value:get() and 23 or 9) or lead:get()
                local t = reduced:get() and l or tail:get()
                local gap = math.abs(l - t)
                local rl, rt = 7 - math.min(1, gap * 0.07), 7 - math.min(3.2, gap * 0.32)
                local fill = value:get() and C.panel or C.muted
                local left, right = math.min(l, t), math.max(l, t)
                c:rect(left, 9 - math.min(rl, rt), right - left, math.min(rl, rt) * 2, fill)
                c:circle(l, 9, rl, fill); c:circle(t, 9, rt, fill)
            end },
        }, txt(label),
    }
end

local sites = { ui.state("amazon", true), ui.state("argos", false), ui.state("johnlewis", true) }
local all = ui.computed(function()
    local n = 0
    for _, site in sites do if site:get() then n += 1 end end
    return n == 3 and "true" or n == 0 and "false" or "mixed"
end)
local function checkbox(id, label, value, parent)
    local checked = function() return parent and all:get() ~= "false" or not parent and value:get() end
    return ui.row { gap = 10, h = 18, pl = parent and 0 or 26, items = "center",
        ui.button(press({ id = id, w = 16, h = 16, rounded = 4, p = 1, items = "center", justify = "center",
            border = 1, border_color = C.focus, bg = function() return checked() and C.text or "#00000000" end,
            on_click = function()
                if parent then local on = all:peek() ~= "true"; ui.batch(function() for _, s in sites do s:set(on) end end)
                else value:toggle() end
            end,
            ui.div { opacity = tween(function() return checked() and 1 or 0 end, { duration = 60 }),
                scale = springBinding(function() return checked() and 1 or 0.5 end),
                ui.show(function() return parent and all:get() == "mixed" end, icon("minus", 12, C.panel), icon("check", 12, C.panel)) },
        }, 0.86)), txt(label),
    }
end

local nav = ui.state("nav", 3)
local navHover = ui.state("nav_hover", 0)
local navItems = {}
local navLabels = { "General", "Appearance", "Assistant", "Automations", "Privacy", "Shortcuts" }
local navIcons = { "sliders-horizontal", "droplet", "orb", "git-branch", "lock", "command" }
for i, label in navLabels do
    navItems[i] = ui.button(press({ id = "nav_" .. i, h = 32, w = 240, px = 10, rounded = 8, gap = 10,
        border = 1, border_color = "#00000000", bg = "#00000000", direction = "row", items = "center",
        on_click = function() nav:set(i) end,
        on_hover = function(inside) navHover:set(inside and i or 0) end,
        on_key = function(e)
            local d = ({ ArrowDown = 1, ArrowUp = -1 })[e.key]
            if d then nav:set((nav:peek() - 1 + d) % 6 + 1); return true end
        end,
        navIcons[i] == "orb" and orb() or icon(navIcons[i], 15),
        ui.text { label, text_size = 13, grow = 1, color = function() return nav:get() == i and C.text or C.secondary end,
            weight = function() return nav:get() == i and 500 or 400 end },
        i == 4 and ui.text { "7", font = "Geist Mono", text_size = 10, color = C.muted } or nil,
    }, 0.97))
end
local navigation = ui.overlay { w = 240, h = 202,
    ui.div { absolute = true, w = 240, h = 32, rounded = 8, bg = C.raised,
        opacity = tween(function() return navHover:get() > 0 and 1 or 0 end, { duration = 90 }),
        y = tween(function() return math.max(0, navHover:get() - 1) * 34 end, { duration = 160 }) },
    ui.div { id = "nav_pill", absolute = true, w = 240, h = 32, rounded = 8, bg = C.selected,
        y = tween(function() return (nav:get() - 1) * 34 end, { duration = 160, ease = "outCubic" }) },
    ui.column { gap = 2, navItems },
}

local toastStyle, toastStyleState = segmented("toast_style", { "Corner", "Timer edge", "Halo" }, 1, 80)
local toasts = ui.state("toasts", {})
local expanded = ui.state("toasts_expanded", false)
local timers = {}
local serial = 0
local TOASTS = {
    { title = "Morning brief is ready", text = "6 items · opened in a new tab", action = "View" },
    { title = "Price dropped to £274", text = "Sony XM6 · John Lewis", action = "Open" },
    { title = "Couldn't reach argos.co.uk", text = "Retrying in 30 seconds", action = "Retry", error = true },
    { title = "Automation saved", text = "Runs every weekday at 8:00" },
}
local function removeToast(id)
    if timers[id] then timers[id]:cancel(); timers[id] = nil end
    local next = {}
    for _, t in toasts:peek() do if t.id ~= id then table.insert(next, t) end end
    toasts:set(next)
end
local function showToast(spec)
    serial += 1
    local next = { { id = serial, spec = spec } }
    local old = toasts:peek()
    if #old == 3 then removeToast(old[3].id) end
    for _, t in toasts:peek() do table.insert(next, t) end
    toasts:set(next)
end
local function toastView(t)
    local id = "toast_" .. t.id
    local ref = ui.ref()
    local life = ui.use_state(1)
    local leaving = false
    local function dismiss(swipe)
        if leaving then return end
        leaving = true
        if timers[t.id] then timers[t.id]:cancel() end
        anime.animate(ref, { x = swipe and 380 or 0, opacity = 0, scale = 0.94, duration = ms(200), ease = "inQuad",
            onComplete = function() removeToast(t.id) end })
    end
    local function index()
        for i, v in toasts:get() do if v.id == t.id then return i - 1 end end
        return 0
    end
    local spec = t.spec
    local ink = spec.error and "#4A2A22" or "#3A3933"
    return ui.div { id = id .. "_position", absolute = true, bottom = 0, right = 0, w = 340, h = 76,
        y = springBinding(function() return -index() * (expanded:get() and 84 or 9) end),
        scale = springBinding(function() return expanded:get() and 1 or 1 - index() * 0.05 end),
        ui.overlay { id = id, ref = ref, w_full = true, h_full = true, px = 14, py = 12, rounded = 12,
            bg = C.elevated, border = 1, border_color = C.strong, shadow = { y = 16, blur = 40, color = "#00000080" },
            on_hover = function(inside) expanded:set(inside) end,
            on_mount = function(el)
                anime.animate(el, { opacity = { 0, 1 }, y = { 16, 0 }, duration = ms(420), ease = spring() })
                local timer = anime.animate(life, { value = { 1, 0 }, duration = 5000, ease = "linear", frameRate = 10,
                    onComplete = function() dismiss(false) end })
                timers[t.id] = timer
                if expanded:peek() then timer:pause() end
                anime.createDraggable(el, { y = false, releaseStiffness = 520, releaseDamping = 30,
                    onGrab = function() timer:pause() end,
                    onRelease = function(d)
                        if d.x > 90 or d.velocity > 0.6 then dismiss(true)
                        else d:setX(0); if not expanded:peek() then timer:resume() end end
                    end,
                })
            end,
            ui.div { absolute = true, top = 0, bottom = 0, left = 0, right = 0, rounded = 12,
                opacity = function() return (expanded:get() or index() == 0) and 1 or 0 end,
                bg_shader = dither, shader_params = { ink, { 1.5, 2, 1, 0.2 } },
                on_mount = function(el)
                    ui.effect(function()
                        anime.utils.set(el, { shaderParam5 = toastStyleState:get() + 1, shaderParam6 = life:get() })
                    end)
                end },
            ui.row { w_full = true, h_full = true, gap = 10, items = "center",
                opacity = tween(function() return (expanded:get() or index() == 0) and 1 or 0 end, { duration = 140 }),
                spec.error and icon("circle-alert", 16, C.error) or orb(16),
                ui.column { grow = 1, shrink = 1, gap = 2,
                    ui.text { spec.title, text_size = 12.5, weight = 500 }, txt(spec.text, 11.5, C.tertiary) },
                spec.action and ui.button { p = 0, id = id .. "_action", h = 26, px = 9, rounded = 8, bg = C.raised,
                    border = 1, border_color = C.strong, hover = { bg = C.selected }, on_click = function() dismiss(false) end,
                    txt(spec.action, 11.5) } or nil,
            },
            ui.button { p = 0, id = id .. "_dismiss", absolute = true, right = 3, top = 3, w = 15, h = 15, rounded = 4,
                bg = C.raised, on_click = function() dismiss(false) end, items = "center", justify = "center", icon("x", 11) },
        },
    }
end
ui.effect(function()
    local hover = expanded:get()
    for _, timer in timers do if hover then timer:pause() else timer:resume() end end
end)

local models = { "Grok 4.7 High", "Claude Opus 5.5", "Claude Sonnet 5.5", "Fable 5.1", "Local · Qwen 4" }
local model = ui.state("model", models[1])
local modelOpen = ui.state("model_open", false)
local contextOpen = ui.state("context_open", false)
local groupOpen = ui.state("group_open", false)
local function popup(id, open, anchor, labels, pick, selected)
    local highlight = ui.state(id .. "_highlight", 1)
    local items = {}
    local function choose(i)
        if pick then pick(i) end
        open:set(false)
    end
    for i, label in labels do
        if label == "Move to group" then
            -- shortcut: below-anchor submenu until Luau supports authored side-opening triggers.
            items[i] = popup("groups", groupOpen,
                ui.button { p = 0, id = "move_group", w_full = true, h = 30, px = 8, direction = "row", items = "center", gap = 9,
                    bg = "#00000000", rounded = 6, hover = { bg = C.selected }, on_click = function() groupOpen:toggle() end,
                    icon("folder-input"), txt(label), ui.div { grow = 1 }, icon("chevron-right", 12) },
                { "Research", "Travel", "Work", "New group…" }, function() contextOpen:set(false) end)
        else
            items[i] = ui.button { p = 0, id = id .. "_item_" .. i, h = 30, w_full = true, px = 8, rounded = 6,
                bg = "#00000000", border = 1, border_color = "#00000000", focus = { border_color = C.focus },
                direction = "row", items = "center", gap = 9, on_click = function() choose(i) end,
                on_hover = function(inside) if inside then highlight:set(i) end end,
                id == "models" and orb() or icon(({ "message-circle", "plus", "copy", "pin", "folder-input", "x" })[i] or "plus"),
                ui.text { label, text_size = 12.5, grow = 1, color = label == "Close tab" and C.error or C.text },
                selected and ui.div { absolute = true, left = -3, top = 8, w = 2, h = 14, rounded = 1, bg = C.text,
                    opacity = function() return selected:get() == label and 1 or 0 end } or nil,
                id == "context" and txt(({ "⌘J", "⌘T", "", "", "", "⌘W" })[i], 10, C.muted) or nil,
            }
        end
    end
    ui.effect(function()
        local isOpen = open:get()
        if isOpen then anime.animate("#" .. id .. "_content", { opacity = { 0, 1 }, scale = { 0.97, 1 }, duration = ms(200), ease = "outExpo" }) end
    end)
    return ui.popover { id = id .. "_popup", open = open, anchor = anchor, w = id == "groups" and 180 or 248,
        p = 5, bg = C.elevated, border = 1, border_color = C.strong, rounded = 10, overflow = "visible",
        shadow = { y = 16, blur = 40, color = "#00000080" },
        on_key = function(e)
            local d = ({ ArrowDown = 1, ArrowUp = -1 })[e.key]
            if d then highlight:set((highlight:peek() - 1 + d) % #labels + 1); return true end
            if e.key == "Enter" or e.key == " " then
                if labels[highlight:peek()] == "Move to group" then groupOpen:set(true) else choose(highlight:peek()) end
                return true
            end
            if e.key == "ArrowRight" and labels[highlight:peek()] == "Move to group" then groupOpen:set(true); return true end
        end,
        ui.overlay { id = id .. "_content", w_full = true,
            ui.div { absolute = true, top = 0, left = 0, right = 0, h = 30, rounded = 6, bg = C.selected,
                y = tween(function() return (highlight:get() - 1) * 31 end, { duration = 160 }) },
            ui.column { w_full = true, gap = 1, items },
        },
    }
end
local modelPopup = popup("models", modelOpen,
    ui.button(press({ id = "model_select", w = 240, h = 32, px = 10, rounded = 8, bg = C.raised,
        border = 1, border_color = C.strong, direction = "row", items = "center", gap = 8,
        hover = { bg = C.selected }, on_click = function() contextOpen:set(false); modelOpen:toggle() end,
        orb(12), ui.text { function() return model:get() end, id = "model_value", grow = 1, text_size = 12.5 },
        icon("chevrons-up-down", 13),
    }, 0.98)), models, function(i) model:set(models[i]) end, model)
local contextPopup = popup("context", contextOpen,
    iconButton("context_menu", "ellipsis", true, function() modelOpen:set(false); contextOpen:toggle() end),
    { "Ask about this tab", "New tab to the right", "Duplicate", "Pin tab", "Move to group", "Close tab" })

local extra = require("./components") { C = C, txt = txt, icon = icon, orb = orb, button = button, toggle = toggle,
    dither = dither, reduced = reduced, pageOffset = pageOffset, viewport = viewport }
local columns = ui.state("columns", 2)
local function card(id, title, master, demo, wide)
    return ui.column { id = "card_" .. id, min_h = 300, w_full = true, col_span_full = wide or false,
        border = 1, border_color = "#FFFFFF12", rounded = 12, hover = { border_color = "#FFFFFF21" },
        ui.column { w_full = true, grow = 1, px = 24, py = 56, items = "center", justify = "center", gap = 16, demo },
        ui.row { w_full = true, px = 16, py = 12, gap = 12, border = 1, border_sides = { 1, 0, 0, 0 }, border_color = "#FFFFFF0C",
            txt(title, 13, C.tertiary), ui.div { grow = 1 }, ui.text { master, font = "Geist Mono", text_size = 10.5, color = "#4A4943" } },
    }
end
local cards = {
    card("buttons", "Button · Icon button", "COIRv · oof6h", row {
        button("save", "Save", "primary", "progress"), button("run", "Run now", "secondary", "indeterminate"),
        button("cancel", "Cancel", "secondary"), button("new", "New", "ghost"),
        button("delete", "Delete", "danger", "indeterminate"), button("delete_smooth", "Delete", "danger", "indeterminate", true),
        iconButton("more", "ellipsis", false, function() contextOpen:toggle() end),
        iconButton("pin", "pin", false, function() pinned:toggle() end, pinned),
    }, true),
    card("fields", "Text field", "rE9DK", ui.column { w_full = true, max_w = 620,
        row({ field("search", "Search", "Find a tab", search), field("name", "Name", "e.g. Morning brief", name),
            field("instructions", "Instructions", "What should Ask do?", instructions, true) }, 20) }, true),
    card("segmented", "Segmented control", "U4wtrd", ui.column { gap = 12, mode, range }),
    card("selection", "Toggle · Checkbox", "Ik0TY · yVQOU", ui.row { flex_wrap = true, gap = 40, items = "start",
        ui.column { gap = 10, toggle("sync", "Sync tabs", true), toggle("trackers", "Block trackers", false), toggle("reduce", "Reduce motion", false, reduced) },
        ui.column { gap = 10, checkbox("all_sites", "All sites", nil, true), checkbox("site_1", "amazon.co.uk", sites[1]),
            checkbox("site_2", "argos.co.uk", sites[2]), checkbox("site_3", "johnlewis.com", sites[3]) },
    }),
    card("sliders", "Slider", "Not in catalogue", extra.sliders),
    card("progress", "Progress bar", "Not in catalogue", extra.progress),
    card("palette", "Command palette", "Not in catalogue", extra.palette, true),
    card("selects", "Value select · Multi-select", "Not in catalogue", extra.selects),
    card("shortcut", "Kbd · Shortcut recorder", "tu9v6 · recorder not in catalogue", extra.shortcut),
    card("tints", "Tint swatches", "Not in catalogue", extra.tints),
    card("orbs", "Orb", "Not in catalogue", extra.status),
    card("modal", "Modal", "Not in catalogue", extra.modal),
    card("links", "Link button", "Not in catalogue", extra.links),
    card("badges", "Badge · Kbd · Avatar", "QZ1Of · tu9v6 · mN3q6", extra.badges),
    card("tooltips", "Tooltip", "A7pCv0", extra.tooltips),
    card("settings", "Setting row", "DlbRR", extra.settings, true),
    card("dropdowns", "Model select · Menu button", "rE9DK · oof6h", ui.row { gap = 24, items = "start", flex_wrap = true,
        ui.column { gap = 6, caption("Model"), modelPopup }, ui.column { gap = 6, caption("Menu"), contextPopup } }),
    card("menu", "Menu item", "G1BOL", extra.menu),
    card("alerts", "Alert", "MD9kw", extra.alerts),
    card("preview", "Preview card", "W7RNzi", extra.preview),
    card("nav", "Nav item", "R7YY9L", navigation),
    card("dialog", "Dialog", "y8cca", extra.dialog),
    card("toasts", "Toast", "wJIe1", {
        extra.toasts,
        row { toastStyle, button("show_toast", "Show toast", "secondary", nil, nil, function() showToast(TOASTS[serial % 2 + 1]) end),
            button("show_error", "Show error", "secondary", nil, nil, function() showToast(TOASTS[3]) end),
            button("show_three", "Show three", "ghost", nil, nil, function() showToast(TOASTS[1]); showToast(TOASTS[4]); showToast(TOASTS[2]) end) },
        txt("Hover to expand · swipe right or × to dismiss · 5s timer pauses on hover", 12, C.muted),
    }, true),
}
return ui.overlay { id = "controls_lab", w_full = true, h_full = true, bg = C.window, color = C.text, font = "Geist", text_size = 13,
    on_resize = function(size) columns:set(size.w <= 820 and 1 or 2); viewport:set(size) end,
    on_key = extra.on_key,
    ui.column { w_full = true, h_full = true,
        ui.row { id = "lab_header", w_full = true, h = 48, pl = 88, pr = 24,
            gap = function() return compact:get() and 10 or 18 end, items = "center", bg = C.window,
            ui.row { grow = 1, h_full = true, gap = 18, items = "center", on_press = ui.action("drag_window"),
                ui.text { "ASK", hidden = compact, font = "Geist Mono", text_size = 10, color = C.muted, letter_spacing = 1.2 },
                ui.text { "Motion Lab", hidden = compact, text_size = 13, color = C.text },
                ui.div { px = 10, py = 4, rounded = 6, bg = C.selected, txt("Components", 12) } },
            ui.text { "Slow", hidden = compact, text_size = 12, color = C.secondary },
            ui.slider { id = "slow_motion", value = slow, min = 1, max = 10, w = 90, h = 20 },
            ui.text { function() return string.format("%.1f×", slow:get()) end, hidden = compact, w = 36, text_size = 12, color = C.secondary },
            ui.checkbox { id = "lab_reduce", label = "Reduce motion", value = reduced, text_size = 12 },
        },
        ui.div { h = 1, w_full = true, bg = C.subtle },
        ui.scroll { id = "page_scroll", offset = pageOffset, grow = 1, shrink = 1, min_h = 0, w_full = true,
            ui.column { w_full = true, max_w = 1180, self = "center", px = 24, pb = 160,
                ui.column { w_full = true, max_w = 680, self = "center", pt = 72, pb = 56, gap = 8,
                    ui.text { "Components", text_size = 28, weight = 400, letter_spacing = -0.4 },
                    txt("Ask's standard controls. Every card draws the catalogue master named underneath it.", 13, C.tertiary) },
                ui.grid { id = "component_grid", w_full = true, cols = columns, gap = 12, cards },
            },
        },
    },
    ui.div { id = "toast_stack", absolute = true, right = 24, bottom = 24, w = 340,
        h = function() return #toasts:get() == 0 and 0 or expanded:get() and #toasts:get() * 84 or 94 end,
        on_hover = function(inside) expanded:set(inside) end,
        ui.each(ui.computed(function()
            local ordered = {}
            for i = #toasts:get(), 1, -1 do table.insert(ordered, toasts:get()[i]) end
            return ordered
        end), toastView, { key = function(t) return t.id end }),
    },
}
