-- Ask Motion Lab / Navbar (experiments/react/src/pages/NavbarPage.tsx + controls/navbar.js). The tree mounts once;
-- state updates retained bindings and nothing animates at rest.
local T = require("./theme")
local C, txt, EASE = T.C, T.txt, T.EASE
local reduced = ui.state("reduce_motion", false)
local slow = ui.state("slow", 1)
local navbar = require("./navbar") { slow = slow, reduced = reduced }
local eased = T.easer(slow, "app")

ui.command { id = "focus_address", title = "Open Location", menu = "View", key = "cmd+l", run = navbar.toggleFocus }

-- tokens.css .linkbtn: secondary text with a quiet underline under its baseline.
local function linkButton(id, label, click)
    local hovered = ui.state(id .. "_hover", false)
    return T.button { id = id, p = 0, bg = C.clear, cursor = "pointer",
        on_hover = function(inside) hovered:set(inside) end, on_click = click,
        ui.overlay {
            T.text { label, text_size = 12, color = function() return hovered:get() and C.text or C.secondary end },
            ui.div { absolute = true, left = 0, right = 0, top = 14.5, h = 1, bg = C.strong } } }
end
-- The page's native controls, as Chrome draws them dark with accent-color: var(--text-secondary).
local ACCENT, NATIVE_FILL, NATIVE_EDGE = C.secondary, "#3B3B3B", "#858585"
local function check(id, label, value, change)
    return T.button { id = id, p = 0, bg = C.clear, cursor = "pointer", direction = "row", gap = 6, items = "center",
        on_click = function() value:set(not value:peek()); if change then change(value:peek()) end end,
        ui.overlay { w = 13, h = 13, rounded = 2.5, items = "center", justify = "center", border = 1,
            border_color = function() return value:get() and ACCENT or NATIVE_EDGE end,
            bg = function() return value:get() and ACCENT or NATIVE_FILL end,
            ui.show(value, T.icon("check", 10, "#10100F")) },
        T.text { label, text_size = 12, color = C.secondary } }
end
-- A horizontal range: `native` is the page's (Chrome's dark range), otherwise the lab bar's .slow-range.
local function range(id, value, min, max, w, native)
    local thumb = native and 15 or 12
    local pressing = false
    local function fraction() return math.clamp((value:get() - min) / (max - min), 0, 1) end
    local function setFrom(x)
        value:set(min + math.clamp((x - thumb / 2) / (w - thumb), 0, 1) * (max - min))
    end
    return ui.overlay { id = id, w = w, h = 16, cursor = "pointer",
        on_press = function(e) pressing = true; setFrom(e.local_x) end,
        on_pointer_move = function(e) if pressing then setFrom(e.local_x) end end,
        on_release = function() pressing = false end,
        native and ui.div { absolute = true, left = 0, right = 0, top = 4, h = 8, rounded = 4, bg = NATIVE_FILL,
            border = 1, border_color = NATIVE_EDGE } or ui.div { absolute = true, left = 0, right = 0, top = 7, h = 2, rounded = 1, bg = C.strong },
        native and ui.div { absolute = true, left = 0, top = 4, h = 8, rounded = 4, bg = ACCENT, border = 1, border_color = NATIVE_EDGE,
            w = function() return thumb / 2 + fraction() * (w - thumb) end } or nil,
        ui.div { absolute = true, top = (16 - thumb) / 2, w = thumb, h = thumb, rounded = thumb / 2,
            bg = native and ACCENT or C.text, shadow = not native and { spread = 3, color = C.window } or nil,
            x = function() return fraction() * (w - thumb) end } }
end
-- Scrub holds the open transition at a fixed progress for inspection; Replay or Esc resumes normal motion.
local function scrubber()
    local value = ui.state("scrub", 1)
    local first = true
    ui.effect(function()
        local v = value:get()
        if first then first = false; return end
        navbar.scrub(v)
    end)
    return ui.row { gap = 6, items = "center",
        T.text { "Scrub", text_size = 12, color = C.secondary },
        range("nb_scrub", value, 0, 1, 90, true) }
end
-- The lab bar's Reduce motion toggle (Preset Toggle).
local function toggle(id, label, value)
    local knob = eased(function() return value:get() and 16 or 2 end, 140, EASE.OUT)
    return T.button { id = id, p = 0, bg = C.clear, cursor = "pointer", direction = "row", gap = 8, items = "center",
        on_click = function() value:set(not value:peek()) end,
        ui.overlay { w = 32, h = 18, rounded = 9, bg = function() return value:get() and C.text or C.active end,
            ui.div { absolute = true, top = 2, w = 14, h = 14, rounded = 7, x = knob,
                bg = function() return value:get() and C.panel or C.muted end } },
        T.text { label, text_size = 12.5, color = C.secondary } }
end
local function sites()
    local list = {
        { "New tab", "ask://newtab" }, { "Article", "https://www.theverge.com/tech/ask-browser-review" },
        { "GitHub", "https://github.com/ask-browser/ask/pull/412" }, { "HTTP", "http://legacy-intranet.example/wiki" },
        { "Ask page", "ask://settings/privacy" },
    }
    local buttons = {}
    for i, s in list do
        local hovered = ui.state("site_hover_" .. i, false)
        table.insert(buttons, T.button { id = "nb_site_" .. i, p = 0, h = 24, px = 10, rounded = 6, bg = C.clear, cursor = "pointer",
            on_hover = function(inside) hovered:set(inside) end, on_click = function() navbar.visit(s[2]) end,
            T.text { s[1], text_size = 12,
                color = eased(function() return hovered:get() and C.text or C.secondary end, 90, EASE.OUT) } })
    end
    return ui.row { p = 3, gap = 2, bg = C.panel, border = 1, border_color = C.default, rounded = 8, self = "center", buttons }
end

local slowLoads, offline = ui.state("slow_loads", false), ui.state("offline", false)
return ui.overlay { id = "navbar_lab", w_full = true, h_full = true, bg = C.window, color = C.text, font = "Geist", text_size = 13,
    ui.column { w_full = true, h_full = true,
        ui.row { w_full = true, h = 48, pl = 88, pr = 24, gap = 18, items = "center", bg = C.window,
            ui.row { grow = 1, h_full = true, gap = 18, items = "center", on_press = ui.action("drag_window"),
                T.text { "ASK", font = "Geist Mono", text_size = 10, color = C.muted, letter_spacing = 1.2 },
                txt("Motion Lab", 13), ui.div { px = 10, py = 4, rounded = 6, bg = C.selected, txt("Navbar", 12) } },
            txt("Slow", 12, C.secondary),
            range("slow_motion", slow, 1, 10, 90),
            T.text { function() return string.format("%.1f×", slow:get()) end, w = 36, text_size = 12, color = C.secondary },
            toggle("lab_reduce", "Reduce motion", reduced),
        },
        ui.div { h = 1, w_full = true, bg = C.subtle },
        ui.scroll { id = "page_scroll", grow = 1, shrink = 1, min_h = 0, w_full = true,
            ui.column { w_full = true, max_w = 1180, self = "center", px = 24, pt = 32, pb = 160,
                ui.column { w_full = true, mt = 8,
                    -- tokens.css .sh: heading, the page switcher and the instructions, wrapping.
                    ui.row { w_full = true, flex_wrap = true, gap = 14, items = "center", mb = 16,
                        T.text { "Floating navbar", text_size = 22, weight = 500, letter_spacing = -0.3, color = C.text },
                        sites(),
                        T.text { "Scroll the page to collapse it to the pill · click the pill to expand · drag the bar or pill to dock it left, centre or right · click the address or ⌘L · ⇥ switches to Ask, @tabs or a site · ↑↓ ↵ · click the site icon or ☆",
                            w_full = true, wrap = true, text_size = 13, line_height = 13 * 1.35, color = C.tertiary } },
                    ui.row { w_full = true, flex_wrap = true, gap_x = 18, gap_y = 8, items = "center", mb = 12,
                        linkButton("nb_replay", "Replay open", navbar.replay),
                        scrubber(),
                        check("nb_slow", " Slow loads", slowLoads, navbar.setSlowLoads),
                        check("nb_perm", " Camera request", navbar.permChecked, navbar.setPermission),
                        check("nb_acting", " Agent acting", navbar.acting),
                        check("nb_offline", " Offline", offline, navbar.setOffline),
                        linkButton("nb_top", "Scroll to top", navbar.scrollTop),
                        T.text { navbar.readout, id = "nb_state", font = "Geist Mono", text_size = 10.5, color = C.muted,
                            ml = "auto", truncate = true, min_w = 0, shrink = 1 } },
                    ui.overlay { id = "nb_win_frame", w_full = true, h = 680, rounded = 12, border = 1, border_color = C.default,
                        bg = "#141413", navbar.window },
                },
            },
        },
    },
}
