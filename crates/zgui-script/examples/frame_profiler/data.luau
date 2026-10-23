-- Sample data, read from the pen design `zgui · Frame profiler` (MvwFD).
local D = {}

-- 120 frames, oldest first: render, paint, layout, update heights in the
-- chart's points (120 points = 16.7 ms).
local BARS = [[
6 1 3 3|8 2 3 6|9 1 3 5|9 2 3 4|9 1 2 4|7 2 3 5|7 1 4 4|7 1 4 5|8 1 3 6|6 2 3 6|
7 2 4 5|9 2 4 4|7 2 4 6|9 2 2 5|9 1 4 4|8 2 3 6|10 2 3 6|7 2 4 4|12 3 2 3|12 3 3 5|
15 3 3 4|14 3 4 6|13 3 3 6|15 3 4 4|14 3 4 4|13 3 4 4|14 3 4 4|11 3 4 4|15 3 3 3|14 3 4 5|
13 3 3 5|14 3 3 5|14 3 3 4|12 3 4 4|10 2 2 4|9 2 2 4|7 1 2 5|8 2 4 4|19 4 17 9|6 2 4 3|
6 2 3 5|7 2 4 6|10 2 3 4|8 1 3 5|6 1 4 5|6 2 4 5|8 2 4 5|9 2 2 4|9 1 3 4|10 2 2 6|
7 1 4 6|9 1 4 5|2 1 0 1|3 1 0 1|3 1 0 1|3 1 0 1|2 1 0 1|2 1 0 1|3 1 0 1|2 1 0 1|
3 1 0 1|3 1 0 1|3 1 0 1|2 1 0 1|9 2 4 4|10 1 3 5|8 2 3 5|9 1 3 5|9 2 3 4|8 1 4 5|
8 1 2 6|50 3 6 6|6 2 4 5|7 1 4 4|9 1 4 3|6 2 3 4|7 1 3 6|7 2 2 5|9 2 3 4|9 2 4 4|
6 1 2 4|9 2 2 3|6 2 3 5|9 2 2 6|7 2 4 6|7 1 3 6|10 2 2 6|7 2 4 6|9 2 2 6|7 1 4 5|
9 1 4 4|6 2 2 6|6 1 3 5|8 2 3 5|7 2 4 5|9 1 2 5|26 5 28 12|9 2 3 5|6 2 4 5|7 1 3 4|
10 2 2 4|6 2 3 4|7 2 4 6|7 2 3 3|9 2 4 5|7 2 4 6|7 2 4 5|6 1 3 4|9 2 4 3|7 1 3 6|
10 1 4 4|9 1 3 4|7 2 3 6|7 1 3 4|7 1 3 5|8 2 2 5|9 1 4 4|8 1 3 5|7 1 4 4|8 2 4 5]]
D.frames = {}
for entry in BARS:gsub("\n", ""):gmatch("[^|]+") do
    local r, p, l, u = entry:match("(%d+) (%d+) (%d+) (%d+)")
    table.insert(D.frames, { r = tonumber(r), p = tonumber(p), l = tonumber(l), u = tonumber(u) })
end
D.first = 48117
D.selected = 97 -- frame 48,213
D.ms_per_point = 16.7 / 120

D.colors = {
    update = "#B9A6F2", layout = "#4A97B8", paint = "#E2C98F", render = "#D9826A",
    present = "#5A5850", input = "#A3A199", actions = "#A3A199",
}
-- A nested span's fill and border, and its fill when selected.
D.dark = {
    update = { "#4D4760", "#B9A6F280", "#6B5F8F" },
    actions = { "#464542", "#A3A19980", "#5E5D58" },
    layout = { "#28424C", "#4A97B880", "#36667A" },
    paint = { "#5B533E", "#E2C98F80", "#7D7051" },
    render = { "#583B32", "#D9826A80", "#8F594A" },
    present = { "#2D2C29", "#5A585080", "#45443F" },
}

-- Frame 48,213's spans: x and width in the trace's points (73.15 per ms),
-- the lane (0-2 main thread, 3 GPU queue), and whether the design labels it.
D.px_per_ms = 1024 / 14
D.phases = {
    { kind = "input", from = 0, to = 14 },
    { kind = "update", from = 15, to = 117 },
    { kind = "layout", from = 118, to = 399 },
    { kind = "paint", from = 401, to = 454 },
    { kind = "render", from = 455, to = 720 },
    { kind = "present", from = 721, to = 834 },
}
local S = {}
local function span(id, lane, kind, name, x, w, label, title)
    table.insert(S, { id = id, lane = lane, kind = kind, name = name, x = x, w = w, label = label, title = title })
end
span("input", 0, "input", "Input", 0, 14)
span("update", 0, "update", "Update", 15, 101.8, "name")
span("layout", 0, "layout", "Layout", 118, 281, "dur")
span("paint", 0, "paint", "Paint", 401, 52.8)
span("render", 0, "render", "Render", 455, 264.9, "dur")
span("present", 0, "present", "Present", 721, 112.7, "name")
span("actions", 1, "actions", "actions", 1, 11.1)
span("luau", 1, "update", "luau", 15, 37.4)
span("reactive", 1, "update", "reactive", 54, 39.6)
span("patch", 1, "update", "patch", 95, 22)
span("pass1", 1, "layout", "pass 1", 118, 92.3, "name", "Layout pass 1")
span("sizes", 1, "layout", "sizes", 212, 35.2)
span("pass2", 1, "layout", "pass 2", 249, 86.4, "name", "Layout pass 2")
span("pass3", 1, "layout", "pass 3", 336, 46.9, nil, "Layout pass 3")
span("hover", 1, "layout", "hover", 385, 14.7)
span("flush", 1, "paint", "flush", 401, 45.4)
span("plan", 1, "paint", "plan", 448, 6)
span("prune", 1, "render", "prune", 455, 6)
span("scroll", 1, "render", "scroll", 462, 7.4)
span("layers", 1, "render", "layers", 471, 61.5)
span("blur", 1, "render", "blur", 534, 38.1)
span("render_flat", 1, "render", "render_flat", 573, 146.4, "dur")
span("latch", 1, "present", "latch wait", 721, 74.7)
span("present_call", 1, "present", "present", 797, 29.4)
span("a11y", 1, "present", "a11y", 828, 6)
span("toasts1", 2, "layout", "Toasts", 124, 65.9, "name", "Toasts · measure")
span("toasts2", 2, "layout", "Toasts", 255, 58.6, nil, "Toasts · measure")
span("toasts3", 2, "render", "Toasts", 471, 39.6, nil, "Toasts · layer")
span("orbs", 2, "render", "Orbs", 512, 20.6, nil, "Status orbs")
span("shape", 2, "render", "shape · 31", 573, 77.6, nil, "Shape text · 31 nodes")
span("glyphs", 2, "render", "glyphs", 652, 33.7, nil, "Glyph upload")
span("encode", 2, "render", "encode", 688, 32.3)
span("gpu_layers", 3, "render", "layers", 799, 32.3, nil, "Layer composite")
span("gpu_blur", 3, "render", "blur × 3", 832, 58.6, nil, "Backdrop blur × 3")
span("gpu_main", 3, "render", "main pass", 892, 55.7, nil, "Main pass")
span("gpu_blit", 3, "render", "blit", 949, 16.9, nil, "Blit to drawable")
D.spans = S
D.details = {
    glyphs = { dur = "0.48 ms", rows = { { "glyphs uploaded", "188" }, { "atlas", "1024² → 2048²" }, { "source", "Toasts · 42 words" } } },
    pass1 = { rows = { { "dirty nodes", "46" }, { "measured", "Toasts × 3" }, { "cause", "wrapped text" } } },
    layout = { rows = { { "passes", "3" }, { "nodes laid out", "212" }, { "longest", "pass 1" } } },
    render = { rows = { { "draw calls", "46" }, { "instances", "1,284" }, { "layers", "6" } } },
}

D.areas = {
    { id = "toasts", name = "Toasts", state = "enter · 3 toasts", nodes = { 46, 18 }, bar = { 100, 32, 8 }, mpx = { "0.31", "0.12" } },
    { id = "orbs", name = "Status orbs", state = "animating 120 Hz", nodes = { 12, 12 }, bar = { 2, 21, 21 }, mpx = { "0.04", "0.04" } },
    { id = "palette", name = "Command palette", state = "results list", nodes = { 88, 31 }, bar = { 11, 24, 12 }, mpx = { "0.09", "0.03" } },
    { id = "model", name = "Model menu", state = "open", nodes = { 64, 9 }, bar = { 5, 16, 7 }, mpx = { "0.05", "0.01" } },
    { id = "sliders", name = "Sliders", state = "drag", nodes = { 22, 6 }, bar = { 3, 8, 6 }, mpx = { "0.02", "0.01" } },
    { id = "other", name = "19 other areas", state = "idle", nodes = { "1,280", "1,280" }, bar = { 2, 5, 3 }, mpx = { "0.03", "0.02" }, quiet = true },
}

-- Renderer counters: this frame, then the 1 s average.
D.counters = {
    { "Draw calls", "46", "22" },
    { "Passes · blur", "7 · 3", "3 · 1" },
    { "Instances", "1,284", "610" },
    { "Shaped nodes", "31", "2", warn = true },
    { "Glyph uploads", "188", "4", warn = true },
    { "Layer repaints · hits", "2 · 9", "1 · 10" },
    { "Damaged", "1.42 Mpx", "0.41" },
}
D.retained = {
    { "Glyph atlas", "16.0 MB" }, { "Layers · 6", "21.4 MB" }, { "Shaped text", "3.1 MB" }, { "Device total", "142 MB" },
}

D.ranges = {
    { label = "1 s", meta = "120 frames · 1 s", ticks = { "−1.0 s", "−0.75 s", "−0.5 s", "−0.25 s", "now" } },
    { label = "5 s", meta = "600 frames · 5 s", ticks = { "−5 s", "−3.75 s", "−2.5 s", "−1.25 s", "now" } },
    { label = "30 s", meta = "3,600 frames · 30 s", ticks = { "−30 s", "−22.5 s", "−15 s", "−7.5 s", "now" } },
}

D.processes = { { "controls_lab", "live" }, { "ask_shell", "48190" }, { "scratchpad", "48302" } }
D.overlays = {
    { "Paint flashing", "tint damaged rects · 1 frame", "#E8A898" },
    { "Layout bounds", "outline every node box", "#4A97B8" },
    { "Layer bounds", "isolated layers · cache hits", "#E2C98F" },
    { "Hit regions", "pointer & drag targets", "#B9A6F2" },
    { "Scroll copies", "copied vs repainted strips", "#C3CEB2" },
    { "Native planes", "under / over web views", "#A9BABF" },
}
D.search = {
    { kind = "spans", swatch = "#D9826A", name = "Glyph upload", meta = "render · frame 48,213", value = "0.48 ms", span = "glyphs" },
    { kind = "spans", swatch = "#4A97B8", name = "glyph_layout", mono = true, meta = "layout · pass 2", value = "0.21 ms", span = "pass2" },
    { kind = "spans", swatch = "#D9826A", name = "Glyph atlas grow", meta = "gpu queue · 1024² → 2048²", value = "0.12 ms", span = "gpu_main" },
    { kind = "areas", icon = "layout", name = "Toasts", meta = "enter · 46 nodes", value = "0.31 Mpx", area = 1,
        keywords = "glyph upload text" },
    { kind = "nodes", icon = "type", name = "GlyphRun#toast-3", mono = true, meta = "Toasts › Body", value = "42 words", area = 1 },
}

-- Frame budgets (pen XFJTT · Budget ▾), time-scale presets, buffer sizes.
D.budgets = { { 8.33, "8.33 ms", "120 Hz" }, { 11.1, "11.1 ms", "90 Hz" }, { 16.7, "16.7 ms", "60 Hz" }, { 33.3, "33.3 ms", "30 Hz" } }
D.scale_presets = { 0.1, 0.25, 0.5, 1, 2 }
D.buffer_counts = { 1200, 3000, 6000, 12000 }
D.cap_hz = { 120, 60, 30, 120 }

-- Dropdown options: a label over a description (pen XFJTT · size / when full).
D.buffer_options = {
    { "1,200 frames", "≈ 10 s at 120 Hz · 4.4 MB" },
    { "3,000 frames", "≈ 25 s at 120 Hz · 11 MB" },
    { "6,000 frames", "≈ 50 s at 120 Hz · 21.9 MB" },
    { "12,000 frames", "≈ 100 s at 120 Hz · 43.8 MB" },
}
D.full_options = {
    { "Drop oldest", "Keep recording, overwrite the earliest frames" },
    { "Stop recording", "Keep the buffer as captured" },
    { "Save and restart", "Export the buffer, then start a new one" },
}
D.backends = {
    { "wgpu · Metal", "The GPU renderer, on Metal" },
    { "wgpu · Vulkan", "The GPU renderer, through MoltenVK" },
    { "Software", "The CPU rasterizer, for checks" },
}
D.caps = {
    { "120 Hz", "The display's refresh" },
    { "60 Hz", "Every other refresh" },
    { "30 Hz", "Every fourth refresh" },
    { "Uncapped", "As fast as frames finish" },
}
-- Capture options: how long a capture runs, and when it starts.
D.lengths = { { "5 seconds", "timer", 5 }, { "15 seconds", "timer", 15 }, { "30 seconds", "timer", 30 }, { "Until stopped", "infinity" } }
D.starts = { { "Immediately", "zap" }, { "After 3 s countdown", "hourglass" }, { "On first interaction", "mouse-pointer-click" } }
D.rec_states = {
    { "recording", "Recording", "" }, { "paused", "Paused", "buffer kept" }, { "stopped", "Stopped", "ready" },
    { "countdown", "Armed · countdown", "starts in 3 s" }, { "armed", "Armed · first input", "waits" },
    { "capturing", "Capturing", "set length" },
}
D.speeds = { { 0.25, "¼×" }, { 0.5, "½×" }, { 1, "1×" }, { 2, "2×" } }

-- Areas tab (pen `zgui · Frame profiler · Areas tab`, z9i0E5) ------------------
--
-- SAMPLE DATA. Every value the Areas tab shows lives in this table; live
-- area measurements will replace it, and areas.luau only formats and lays
-- them out. Rows the design draws collapsed have made-up children (marked
-- below) so that expanding them shows something.
--
-- A row's `v` holds, per scope (Frame, 1 s avg, Capture), "layout paint
-- composite updates damage": dirty nodes per frame by flag, updates a
-- second, damaged Mpx. A child given only its 1 s avg scales with its
-- parent in the other two. `last` is the Last 1 s sparkline: 20 bars,
-- heights in points (14 = full). `damage` (Mpx, 60 buckets of 2 frames) is
-- the detail's chart; rows without one get a seeded stand-in below.
local function rep(s, n) return string.rep(s .. " ", n) end
local A = {
    scopes = { { label = "Frame", window = "frame 48,213" }, { label = "1 s avg", window = "1 s" },
        { label = "Capture", window = "capture" } },
    selected = "toasts",
    open = { toasts = true, toast_3 = true },
    rows = {
        { id = "toasts", tag = "toast_*", name = "Toasts", issue = "3× layout",
            v = { "46.1 12.0 3.0 120 0.92", "38.2 12.0 3.0 120 0.31", "14.6 4.4 1.2 38 0.12" },
            last = "10 9 11 7 8 8 9 9 7 7 10 12 8 7 13 14 14 14 14 14",
            detail = { kind = "Group", nodes = 46, note = "portal over window", chip = "isolated layer",
                why = { { "layout", "text wrap feeds content_sizes", "3 / frame" },
                    { "paint", "opacity animation", "120 / s" }, { "composite", "slide-in transform", "120 / s" } },
                trigger = "started by onClick · app.luau:286 · push toast",
                layer = { prepare = "0.28 ms", texture = "640×480 @2×", repaints = "120 · 0",
                    note = "Never reused: the layer repaints every frame, so isolating it only adds cost." },
                damage = rep("0.04", 46) .. "0.30 0.92 0.60 0.34 0.28 0.32 0.24 0.30 0.34 0.28 0.26 0.26 0.24 0.32" },
            children = {
                { id = "toast_3", tag = "toast_3", name = "Couldn't reach argos.co.uk", v = "14.0 4.0 1.0 120 0.11",
                    last = "11 10 8 10 11 8 10 9 12 10 9 11 9 7 10 14 14 14 14 14",
                    detail = { kind = "Toast", nodes = 18, note = "error · entering",
                        why = { { "layout", "wrapped text re-measured", "3 / frame" },
                            { "paint", "opacity animation", "120 / s" }, { "composite", "slide-in transform", "120 / s" } },
                        trigger = "started by onClick · app.luau:286 · push toast" },
                    children = {
                        { id = "retry", tag = "button_retry", name = "Retry button", v = "6.0 2.0 1.0 120 0.05",
                            last = "6 7 8 8 10 9 10 10 9 8 11 7 8 9 10 14 14 14 14 14",
                            detail = { kind = "Button", nodes = 4, note = "inside toast_3",
                                why = { { "layout", "parent re-measure", "3 / frame" }, { "paint", "parent opacity", "120 / s" } } } },
                        { id = "msg_3", tag = "text_msg", name = "Message", v = "5.0 2.0 1.0 120 0.04",
                            last = "6 7 8 8 10 9 10 10 9 8 11 7 8 9 10 14 14 14 14 14",
                            detail = { kind = "Text", nodes = 3, note = "42 words · wraps",
                                why = { { "layout", "text wrap feeds content_sizes", "3 / frame" },
                                    { "paint", "parent opacity", "120 / s" } } } },
                    } },
                { id = "toast_2", tag = "toast_2", name = "Price dropped to £274", v = "12.0 4.0 1.0 120 0.10",
                    last = "10 9 8 9 6 8 7 10 6 7 6 6 10 10 9 14 14 14 14 14",
                    detail = { kind = "Toast", nodes = 14, note = "settling",
                        why = { { "layout", "sibling insert shifts it", "1 / frame" }, { "paint", "opacity animation", "120 / s" },
                            { "composite", "stack transform", "120 / s" } } },
                    -- Made up: the design draws toast_2 collapsed.
                    children = {
                        { id = "open_2", tag = "button_open", name = "Open deal", v = "4.0 1.5 0.5 120 0.03",
                            last = "8 7 7 8 6 7 6 8 6 6 6 6 8 8 7 12 12 12 12 12",
                            detail = { kind = "Button", nodes = 4, note = "inside toast_2",
                                why = { { "paint", "parent opacity", "120 / s" } } } },
                        { id = "msg_2", tag = "text_msg", name = "Message", v = "5.0 1.5 0.5 120 0.04",
                            last = "9 8 7 8 6 7 6 9 6 6 6 6 9 9 8 13 13 13 13 13",
                            detail = { kind = "Text", nodes = 3, note = "12 words",
                                why = { { "layout", "sibling insert shifts it", "1 / frame" } } } },
                    } },
                { id = "toast_1", tag = "toast_1", name = "Morning brief is ready", v = "12.0 4.0 1.0 120 0.10",
                    last = "6 7 8 8 10 9 10 10 9 8 11 7 8 9 10 14 14 14 14 14",
                    detail = { kind = "Toast", nodes = 14, note = "settling",
                        why = { { "layout", "sibling insert shifts it", "1 / frame" }, { "paint", "opacity animation", "120 / s" },
                            { "composite", "stack transform", "120 / s" } } } },
            } },
        { id = "orbs", tag = "card_orbs", name = "Status orbs", issue = "idle repaint", idle = true,
            v = { "0 8.0 8.0 120 0.04", "0 8.0 8.0 120 0.04", "0 8.0 8.0 120 0.04" },
            last = "8 8 6 8 8 6 7 6 5 7 5 7 5 6 8 5 6 7 5 6",
            detail = { kind = "Card", nodes = 12, note = "idle",
                why = { { "paint", "pulse with no visible change", "120 / s" }, { "composite", "opacity on 8 nodes", "120 / s" } },
                trigger = "started by mount · orbs.luau:42 · loop = true" },
            -- Made up, as are all the children below: collapsed in the design.
            children = {
                { id = "orb_thinking", tag = "orb_thinking", name = "Thinking orb", v = "0 4.0 4.0 120 0.02",
                    last = "8 8 6 8 8 6 7 6 5 7 5 7 5 6 8 5 6 7 5 6",
                    detail = { kind = "Orb", nodes = 4, note = "loop", why = { { "paint", "pulse animation", "120 / s" } } } },
                { id = "orb_reading", tag = "orb_reading", name = "Reading orb", v = "0 4.0 4.0 120 0.02",
                    last = "7 8 6 7 8 6 7 6 5 6 5 7 5 6 7 5 6 7 5 6",
                    detail = { kind = "Orb", nodes = 4, note = "loop", why = { { "paint", "pulse animation", "120 / s" } } } },
            } },
        { id = "palette", tag = "card_palette", name = "Command palette",
            v = { "0 0 0 6 0", "4.0 9.0 0 6 0.09", "3.1 7.0 0 5 0.07" },
            last = "3 3 5 4 4 4 3 4 4 5 5 5 3 5 5 4 4 4 5 4",
            detail = { kind = "Card", nodes = 31, note = "results list",
                why = { { "layout", "results list grows", "6 / s" }, { "paint", "query text changes", "6 / s" } },
                trigger = "started by onInput · palette.luau:58 · filter" },
            children = {
                { id = "query", tag = "input_query", name = "Query field", v = "1.0 3.0 0 6 0.02",
                    last = "2 2 3 3 3 3 2 3 3 3 3 3 2 3 3 3 3 3 3 3",
                    detail = { kind = "Input", nodes = 6, note = "focused", why = { { "paint", "query text changes", "6 / s" } } } },
                { id = "results", tag = "list_results", name = "Results list", v = "3.0 6.0 0 6 0.07",
                    last = "3 3 4 4 4 4 3 4 4 4 4 4 3 4 4 4 4 4 4 4",
                    detail = { kind = "List", nodes = 24, note = "8 results",
                        why = { { "layout", "results list grows", "6 / s" }, { "paint", "rows replaced", "6 / s" } } } },
            } },
        { id = "model", tag = "card_dropdowns", name = "Model menu",
            v = { "0 0 0 4 0", "2.0 6.1 0 4 0.05", "1.4 4.3 0 3 0.04" },
            last = "3 3 4 4 3 3 2 2 4 4 3 4 3 3 3 3 4 3 3 2",
            detail = { kind = "Card", nodes = 9, note = "open",
                why = { { "layout", "menu panel measures", "4 / s" }, { "paint", "hover highlight", "4 / s" } },
                trigger = "started by onHover · menus.luau:112" },
            children = {
                { id = "model_button", tag = "button_model", name = "Trigger", v = "0.5 1.6 0 4 0.01",
                    last = "2 2 2 2 2 2 1 1 2 2 2 2 2 2 2 2 2 2 2 1",
                    detail = { kind = "Button", nodes = 3, note = "pressed", why = { { "paint", "hover highlight", "4 / s" } } } },
                { id = "model_menu", tag = "menu_model", name = "Menu panel", v = "1.5 4.5 0 4 0.04",
                    last = "3 3 3 3 3 3 2 2 3 3 3 3 3 3 3 3 3 3 3 2",
                    detail = { kind = "Menu", nodes = 6, note = "6 items", why = { { "layout", "menu panel measures", "4 / s" } } } },
            } },
        { id = "sliders", tag = "card_sliders", name = "Sliders",
            v = { "0 0 0 2 0", "1.0 3.2 0 2 0.02", "0.6 1.9 0 1 0.01" },
            last = "3 2 2 2 2 2 3 3 2 2 2 3 2 2 3 3 3 3 3 2",
            detail = { kind = "Card", nodes = 6, note = "drag",
                why = { { "paint", "thumb position", "2 / s" }, { "layout", "value label width", "2 / s" } } },
            children = {
                { id = "volume", tag = "slider_volume", name = "Volume", v = "0.5 1.6 0 2 0.01",
                    last = "2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2",
                    detail = { kind = "Slider", nodes = 3, note = "dragging", why = { { "paint", "thumb position", "2 / s" } } } },
                { id = "speed", tag = "slider_speed", name = "Speed", v = "0.5 1.6 0 1 0.01",
                    last = "1 1 1 1 1 1 2 2 1 1 1 2 1 1 2 2 2 2 2 1",
                    detail = { kind = "Slider", nodes = 3, note = "idle", why = { { "layout", "value label width", "1 / s" } } } },
            } },
        { id = "progress", tag = "card_progress", name = "Progress",
            v = { "0 1.0 1.0 30 0.01", "0 1.0 1.0 30 0.01", "0 0.6 0.6 18 0.01" },
            last = "2 3 2 2 3 2 2 2 2 2 2 2 3 2 3 3 1 1 2 2",
            detail = { kind = "Card", nodes = 5, note = "determinate",
                why = { { "paint", "fill width", "30 / s" }, { "composite", "fill transform", "30 / s" } },
                trigger = "started by onProgress · upload.luau:19" },
            children = {
                { id = "upload", tag = "progress_upload", name = "Upload bar", v = "0 1.0 1.0 30 0.01",
                    last = "2 3 2 2 3 2 2 2 2 2 2 2 3 2 3 3 1 1 2 2",
                    detail = { kind = "Progress", nodes = 3, note = "64 %", why = { { "paint", "fill width", "30 / s" } } } },
            } },
        { id = "header", tag = "lab_header", name = "Header",
            v = { "0 0 0 1 0", "0.3 0.3 0 1 0.01", "0.3 0.3 0 1 0.01" },
            last = rep("1", 20),
            detail = { kind = "Card", nodes = 4, note = "static",
                why = { { "layout", "title text set", "1 / s" }, { "paint", "title text set", "1 / s" } } },
            children = {
                { id = "title", tag = "text_title", name = "Title", v = "0.3 0.3 0 1 0.01", last = rep("1", 20),
                    detail = { kind = "Text", nodes = 2, note = "1 line", why = { { "paint", "title text set", "1 / s" } } } },
            } },
        { id = "other", name = "16 other areas", quiet = true,
            v = { "0.4 0.3 0 3 0.01", "0.4 0.3 0 3 0.01", "0.4 0.3 0 3 0.01" },
            last = rep("1", 20),
            detail = { kind = "Areas", nodes = 58, note = "idle", why = { { "paint", "hover and focus", "3 / s" } } } },
    },
    -- All areas: the frame-wide sums (the design's 1 s avg, 62 / 28 / 10 %).
    total = { count = "26 areas · 214 nodes",
        v = { "46.5 21.3 12.0 120 0.98", "60.5 27.3 9.8 120 0.54", "20.4 26.8 9.8 120 0.21" },
        last = "11 10 12 8 9 9 10 10 8 8 11 12 9 8 13 14 14 14 14 14" },
    -- Findings: the icon, the text, the link and the row it selects.
    findings = {
        { "alert", "warn", "Toasts lay out 3× per frame: wrapped text feeds its size back into layout.",
            "Show passes", "toasts" },
        { "alert", "warn", "Toasts' isolated layer repaints 120/s and never hits cache, 0.28 ms per frame for nothing.",
            "Show layer", "toasts" },
        { "info", "info", "Status orbs repaint at 120 Hz while idle: 8 paint nodes every frame with no visible change.",
            "Highlight in app", "orbs" },
    },
    footnote = "Work = dirty nodes per frame from the LAYOUT / PAINT / COMPOSITE flags; damage is clipped "
        .. "to each subtree. Only isolated layers are timed.",
}

-- Parse the strings above into numbers.
local function nums(s)
    local out = {}
    for x in s:gmatch("%S+") do table.insert(out, tonumber(x)) end
    return out
end
local function scoped(s)
    local n = nums(s)
    return { l = n[1], p = n[2], c = n[3], u = n[4], d = n[5] }
end
-- A stand-in damage history: `d` Mpx a bucket, on as many buckets as the
-- row updates (Park–Miller, seeded per row, so it stays put).
local seed = 48213
local function rand()
    seed = (seed * 16807) % 2147483647
    return seed / 2147483647
end
local function standIn(avg)
    local out, chance = {}, math.min(1, avg.u / 60)
    for i = 1, 60 do
        out[i] = rand() < chance and avg.d * (0.6 + 0.8 * rand()) or 0
    end
    return out
end
local order = 0
local function prepare(list, parent)
    for _, r in list do
        order += 1
        r.order = order
        if type(r.v) == "table" then
            r.scopes = { scoped(r.v[1]), scoped(r.v[2]), scoped(r.v[3]) }
        else
            local own = scoped(r.v)
            r.scopes = {}
            for i = 1, 3 do
                local s = {}
                for f, x in own do
                    local base = parent.scopes[2][f]
                    s[f] = base > 0 and x * parent.scopes[i][f] / base or x
                end
                r.scopes[i] = s
            end
            r.scopes[2] = own
        end
        r.last = nums(r.last)
        r.detail.damage = r.detail.damage and nums(r.detail.damage) or standIn(r.scopes[2])
        if r.children then prepare(r.children, r) end
    end
end
prepare(A.rows)
A.total.scopes = { scoped(A.total.v[1]), scoped(A.total.v[2]), scoped(A.total.v[3]) }
A.total.last = nums(A.total.last)
D.areas_tab = A

return D
