-- zgui · Frame profiler, from the pen design: the window (MvwFD), its
-- popovers (kZfcb), its states and menus (XFJTT) and the Areas tab
-- (z9i0E5, in areas.luau).
-- The tree mounts once. Selections, hovers and menus update retained
-- bindings; motion runs in Rust. Paused, nothing requests frames.
local K = require("./kit")
local D = require("./data")
local C, icon, text, mono, spacer = K.C, K.icon, K.text, K.mono, K.spacer

local S = {
    menu = ui.state("menu", ""),
    hint = ui.state("hint", ""),
    tab = ui.state("tab", 1),
    range = ui.state("range", 1),
    scope = ui.state("scope", 1),
    frame = ui.state("frame", D.selected),
    hover_frame = ui.state("hover_frame", 0),
    span = ui.state("span", "glyphs"),
    hover_span = ui.state("hover_span", ""),
    passes = ui.state("passes", false),
    area = ui.state("area", 1),
    -- "recording", "paused", "stopped", "countdown" (starts in 3 s), "armed"
    -- (starts on the first input) or "capturing" (a capture of set length).
    rec = ui.state("rec", "recording"),
    clock = ui.state("clock", 42.18),
    capture = ui.state("capture", 0),
    capture_total = ui.state("capture_total", 5),
    countdown = ui.state("countdown", 3),
    -- Capture options: a length (D.lengths) and how it starts (D.starts).
    cap_length = ui.state("cap_length", 4),
    cap_start = ui.state("cap_start", 3),
    logs_unread = ui.state("logs_unread", 3),
    overlays = ui.state("overlays", { true, false, true, false, false, false }),
    time_scale = ui.state("time_scale", 1),
    budget = ui.state("budget", 8.33),
    follow_cap = ui.state("follow_cap", false),
    zoom = ui.state("zoom", { mode = "frame", k = 1, off = 0 }),
    keep_zoom = ui.state("keep_zoom", false),
    ctx = ui.state("ctx", { id = "glyphs", x = 0, y = 0 }),
    query = ui.state("query", "glyph"),
    target = ui.state("target", 1),
    backend = ui.state("backend", 1),
    cap = ui.state("cap", 1),
    buffer_size = ui.state("buffer_size", 3),
    when_full = ui.state("when_full", 1),
    on_top = ui.state("on_top", false),
    switches = {
        blur = ui.state("sw_blur", true), scroll = ui.state("sw_scroll", true),
        damage = ui.state("sw_damage", false), gpu = ui.state("sw_gpu", true),
        trace = ui.state("sw_trace", true), reload = ui.state("sw_reload", false),
    },
    viewport = ui.state("viewport", { w = 1132, h = 1028 }),
    anchors = {},
}
S.BACKENDS = { "wgpu · Metal", "wgpu · Vulkan", "Software" }
S.CAPS = { "120 Hz", "60 Hz", "30 Hz", "Uncapped" }

-- A number without trailing zeros: 2, 0.5, 0.25.
local function num(v)
    if math.abs(v - math.floor(v + 0.5)) < 0.005 then return tostring(math.floor(v + 0.5)) end
    return (string.format("%.2f", v):gsub("0+$", ""):gsub("%.$", ""))
end
local function comma(n)
    local s = tostring(math.floor(n))
    return (s:reverse():gsub("(%d%d%d)", "%1,"):reverse():gsub("^,", ""))
end
S.comma = comma
function S.open(name)
    S.hint:set("")
    S.menu:set(S.menu:peek() == name and "" or name)
end
function S.target_meta()
    return "pid 48211 · " .. S.BACKENDS[S.backend:get()] .. " · " .. S.CAPS[S.cap:get()]
end
function S.flip_overlay(i)
    local l = table.clone(S.overlays:peek())
    l[i] = not l[i]
    S.overlays:set(l)
end
function S.reset_runtime()
    local sw = S.switches
    ui.batch(function()
        S.time_scale:set(1); S.backend:set(1); S.cap:set(1)
        sw.blur:set(true); sw.scroll:set(true); sw.damage:set(false)
        sw.gpu:set(true); sw.trace:set(true); sw.reload:set(false)
    end)
end

-- Speed: the target's time scale, from the status bar.
function S.step_scale(d)
    local s, best = S.time_scale:peek(), 1
    for i, p in D.speeds do
        if math.abs(p[1] - s) < math.abs(D.speeds[best][1] - s) then best = i end
    end
    S.time_scale:set(D.speeds[math.clamp(best + d, 1, #D.speeds)][1])
end

-- Recording (pen XFJTT · Recording control): the clock runs at 20 fps
-- (the engine asks for no more). Started from stopped, the capture options
-- decide when it begins (now, after a 3 s countdown, or on the first input)
-- and how long it runs (a length fills the pill's underline, then pauses).
local clockBase = S.clock:peek()
local ticker = anime.createTimer { duration = 1e9, frameRate = 20, autoplay = false,
    onUpdate = function(t) S.clock:set(clockBase + t.currentTime / 1000) end }
local pending
-- The capture's progress line scales across the control on the compositor,
-- every display frame, in one animation (its width at the clock's 20 fps
-- stepped visibly).
local progressRef = ui.ref()
local progress
local function setRec(r)
    if pending then pending:cancel(); pending = nil end
    if progress then progress:cancel(); progress = nil end
    S.rec:set(r)
end
local function begin(length)
    length = length or D.lengths[S.cap_length:peek()][3]
    if not length then setRec("recording") return end
    setRec("capturing")
    S.capture_total:set(length)
    progress = anime.animate(progressRef, { scaleX = { 0, 1 }, duration = length * 1000, ease = "linear" })
    pending = anime.animate(S.capture, { value = { 0, length }, duration = length * 1000, ease = "linear", frameRate = 20,
        onComplete = function() pending = nil; S.rec:set("paused") end })
end
local function countdown()
    setRec("countdown")
    pending = anime.animate(S.countdown, { value = { 3, 0 }, duration = 3000, ease = "linear", frameRate = 20,
        onComplete = function() pending = nil; begin() end })
end
function S.start()
    local start = S.cap_start:peek()
    if start == 2 then countdown()
    elseif start == 3 then setRec("armed")
    else begin() end
end
-- Any of the six states at once, from the record control's context menu.
function S.preview(r)
    if r == "countdown" then countdown()
    elseif r == "capturing" then begin(D.lengths[S.cap_length:peek()][3] or 5)
    else setRec(r) end
end
-- Armed, the first click, key or scroll starts it.
function S.input()
    if S.rec:peek() == "armed" then begin() end
end
function S.toggle_rec()
    local r = S.rec:peek()
    if r == "recording" then setRec("paused")
    elseif r == "paused" then setRec("recording")
    elseif r == "stopped" then S.start()
    elseif r == "capturing" then setRec("paused")
    else setRec("stopped") end
end
function S.clear()
    setRec("stopped")
    clockBase = 0
    S.clock:set(0)
end
ui.effect(function()
    local r = S.rec:get()
    if r == "recording" or r == "capturing" then
        clockBase = S.clock:peek()
        ticker:restart()
    else
        ticker:pause()
    end
end)

-- Capture buffer: 120 frames a second of clock, in a ring.
local function bufferFrames()
    return math.min(D.buffer_counts[S.buffer_size:get()], math.floor(S.clock:get() * 120))
end
function S.buffer_fill() return bufferFrames() / D.buffer_counts[S.buffer_size:get()] end
function S.buffer_frames_text() return comma(bufferFrames()) .. " / " .. comma(D.buffer_counts[S.buffer_size:get()]) .. " frames" end
function S.buffer_bytes() return string.format("%.1f MB", bufferFrames() * 0.00365) end
local function untilFull() return math.max(0, (D.buffer_counts[S.buffer_size:get()] - bufferFrames()) / 120) end
function S.buffer_eta()
    local full = untilFull()
    return string.format("≈ %d s at 120 Hz · ", bufferFrames() // 120) .. (full > 0 and string.format("full in %d s", math.ceil(full))
        or (S.when_full:get() == 1 and "dropping oldest" or "stopped"))
end
function S.buffer_drop()
    local full = untilFull()
    return full > 0 and string.format("oldest frames drop in %d s", math.ceil(full)) or "dropping oldest frames"
end

-- Frames ----------------------------------------------------------------

-- Live frames from an app run with ZGUI_PROFILE=1 (live.rs publishes them),
-- or false for the sample data. Paused or stopped, the last ones stay. They
-- are kept as plain tables: a signal's `get` copies its whole value, and the
-- chart reads every frame many times. `liveVersion` is what readers track.
local liveFeed = ui.shared("live", false)
local liveVersion = ui.state("live_version", 0)
local liveData, liveBars = false, false
-- Chart points (120 = 16.7 ms): input counts with update, present with render.
local function livePoints(list)
    local k, out, pad = D.ms_per_point, {}, 120 - #list
    for i = 1, 120 do
        local f = list[i - pad]
        out[i] = (f and not f.empty)
            and { u = (f.input + f.update) / k, l = f.layout / k, p = f.paint / k, r = (f.render + f.present) / k, live = f }
            or { u = 0, l = 0, p = 0, r = 0 }
    end
    return out
end
ui.effect(function()
    local v = liveFeed:get()
    local rec = S.rec:get()
    if rec == "recording" or rec == "capturing" then
        liveData = v
        liveBars = v and { livePoints(v.frames), livePoints(v.frames5), livePoints(v.frames30) } or false
        liveVersion:set(liveVersion:peek() + 1)
    end
end)
local live = { get = function() liveVersion:get(); return liveData end }
local liveRanges = { get = function() liveVersion:get(); return liveBars end }
S.live = live

local STEP = { 1, 5, 30 }
local ranged = {}
local function rangeFrames(r)
    local l = liveRanges.get()
    if l then return l[r] end
    if r == 1 then return D.frames end
    if ranged[r] then return ranged[r] end
    -- Longer windows: each bar is the worst of its frames, so spikes stack up.
    local random = anime.utils.createSeededRandom(r * 97)
    local out = {}
    for i = 1, 120 do
        local f = D.frames[(i * (r == 2 and 7 or 13)) % 120 + 1]
        local k = 1 + random(0, 45) / 100
        out[i] = { u = math.floor(f.u * k + 0.5), l = math.floor(f.l * k + 0.5),
            p = math.max(1, math.floor(f.p * k + 0.5)), r = math.floor(f.r * k + 0.5) }
    end
    ranged[r] = out
    return out
end
local budget = ui.computed(function()
    return S.follow_cap:get() and math.floor(10000 / D.cap_hz[S.cap:get()] + 0.5) / 10 or S.budget:get()
end)
S.effective_budget = budget
local function liveInfo(f)
    local x = f.live
    if not x then
        return { n = 0, f = f, ms = { u = 0, l = 0, p = 0, r = 0, present = 0 }, cpu = 0, gpu = 0,
            over = -budget:get(), passes = 1, empty = true }
    end
    local ms = { u = x.input + x.update, l = x.layout, p = x.paint, r = x.render, present = x.present }
    local cpu = ms.u + ms.l + ms.p + ms.r + ms.present
    return { n = x.n, f = f, ms = ms, cpu = cpu, gpu = x.gpu or 0, over = cpu - budget:get(), passes = 1, live = x }
end
local function info(i, r)
    local f = rangeFrames(r)[i]
    if liveRanges.get() then return liveInfo(f) end
    local k = D.ms_per_point
    local design = r == 1 and i == D.selected
    local ms = design and { u = 0.62, l = 3.86, p = 0.74, r = 3.64, present = 1 }
        or { u = f.u * k, l = f.l * k, p = f.p * k, r = f.r * k, present = 1 }
    local cpu = design and 9.86 or ms.u + ms.l + ms.p + ms.r + ms.present
    return {
        n = 48236 - (120 - i) * STEP[r], f = f, ms = ms, cpu = cpu, gpu = design and 2.31 or cpu * 0.234,
        over = cpu - budget:get(), passes = f.l >= 12 and 3 or f.l >= 5 and 2 or 1, design = design,
    }
end
local current = ui.computed(function() return info(S.frame:get(), S.range:get()) end)
S.current = current
function S.step(d) S.frame:set(math.clamp(S.frame:peek() + d, 1, 120)) end

-- The selected frame's spans: the design's, or its phases stretched to
-- another frame's update, layout, paint and render times.
local PHASE = { input = "input", actions = "input", update = "update", layout = "layout", paint = "paint",
    render = "render", present = "present" }
-- A live frame's phases in order on the main lane, the refresh wait after
-- them, and the GPU's run from the end of render.
local LIVE_PHASES = { { "input", "Input" }, { "update", "Update" }, { "layout", "Layout" }, { "paint", "Paint" },
    { "render", "Render" }, { "present", "Present" } }
local function liveTrace(f)
    local k, out, at, submit = D.px_per_ms, {}, 0, 0
    local function add(id, lane, kind, name, x, ms, title)
        local w = ms * k
        if w >= 0.5 then
            table.insert(out, { id = id, lane = lane, kind = kind, name = name, x = x, w = math.max(w, 2),
                label = w > 90 and "dur" or w > 40 and "name" or nil, title = title })
        end
        return w
    end
    for _, ph in LIVE_PHASES do
        local w = add(ph[1], 0, ph[1], ph[2], at, f[ph[1]])
        at += w > 0.5 and w + 1 or 0
        if ph[1] == "render" then submit = at end
    end
    add("wait", 1, "present", "refresh wait", at, f.wait, "Waiting for the refresh")
    if f.gpu then add("gpu", 3, "render", "GPU", submit, f.gpu, "GPU · whole frame") end
    return out
end
local function traceFor(i, r)
    local x = info(i, r)
    if x.empty then return {} end
    if x.live then return liveTrace(x.live) end
    if x.design then return D.spans end
    local base = D.frames[D.selected]
    local ratio = { input = 1, present = 1, update = x.f.u / base.u, layout = math.max(x.f.l, 1) / base.l,
        paint = x.f.p / base.p, render = x.f.r / base.r }
    local map, cursor = {}, 0
    for _, ph in D.phases do
        local w = math.max(2, (ph.to - ph.from) * ratio[ph.kind])
        map[ph.kind] = { from = ph.from, to = cursor, k = ratio[ph.kind], w = w }
        cursor += w + 1
    end
    local shift = map.render.to + map.render.w - 720
    local out = {}
    for _, s in D.spans do
        local m = map[PHASE[s.kind]]
        local copy = table.clone(s)
        if s.lane == 3 then
            copy.x = s.x + shift
            copy.w = math.max(3, s.w * math.sqrt(map.render.k))
        elseif s.lane == 0 then
            copy.x, copy.w = m.to, m.w
        else
            copy.x = m.to + (s.x - m.from) * m.k
            copy.w = math.max(3, s.w * m.k)
        end
        table.insert(out, copy)
    end
    return out
end
local trace = ui.computed(function() return traceFor(S.frame:get(), S.range:get()) end)
function S.span_by_id(id)
    for _, s in trace:peek() do if s.id == id then return s end end
end

-- Trace zoom: `k` times the frame's 14 ms across 1024 points, scrolled `off`.
local TRACK = 1024
local function setZoom(mode, k, center)
    k = math.clamp(k, 1, 12)
    local off = math.clamp(center * k - TRACK / 2, 0, TRACK * k - TRACK)
    S.zoom:set({ mode = mode, k = k, off = off })
end
function S.zoom_to(mode)
    if mode == "vsync" then
        setZoom(mode, 14 / 8.33, 0)
    elseif mode == "selection" then
        local s = S.span_by_id(S.span:peek())
        if not s then return S.zoom_to("frame") end
        setZoom(mode, TRACK * 0.6 / s.w, s.x + s.w / 2)
    else
        setZoom("frame", 1, 0)
    end
end
function S.zoom_by(f)
    local z = S.zoom:peek()
    setZoom("custom", z.k * f, (TRACK / 2 + z.off) / z.k)
end
local lastFrame = S.frame:peek()
ui.effect(function()
    local f = S.frame:get()
    if f ~= lastFrame then
        lastFrame = f
        if not S.keep_zoom:peek() and S.zoom:peek().mode ~= "frame" then S.zoom_to("frame") end
    end
end)

-- Hover hints: the menus module provides S.hint_hover.
local function hint(name, props, stays)
    local hover = props.on_hover
    props.on_hover = function(inside)
        if hover then hover(inside) end
        S.hint_hover(name, inside)
    end
    if not stays then
        local press = props.on_press
        props.on_press = function()
            S.hint:set("")
            if press then press() end
        end
    end
    return props
end
-- Anchors: the elements menus and tooltips open from. A resize bumps
-- `anchor_tick`, so a tooltip follows a trigger that changes width.
S.anchor_tick = ui.state("anchor_tick", 0)
local function anchor(name)
    return function(_, el)
        S.anchors[name] = el
        S.anchor_tick:set(S.anchor_tick:peek() + 1)
    end
end
local function menuActive(name)
    return function() return S.menu:get() == name and C.selected or C.clear end
end

-- Title bar ---------------------------------------------------------------

local TABS = { { "activity", "Timeline" }, { "layout", "Areas" }, { "cpu", "GPU" }, { "database", "Memory" }, { "list", "Events", "1.2k" } }
local tabs = K.segmented("tabs", S.tab, #TABS, function(i, active)
    local t = TABS[i]
    return {
        icon(t[1], 13, function() return active() and C.text or C.muted end),
        text(t[2], 12.5, function() return active() and C.text or C.tertiary end,
            { weight = function() return active() and 500 or 400 end }),
        t[3] and mono(t[3], 10, C.muted) or nil,
    }
end, { h = 28, px = 10, r = 7, p = 0, outer = 7 })

local swatches = {}
for i, o in D.overlays do
    local on = ui.computed(function() return S.overlays:get()[i] end)
    swatches[i] = ui.div { h = 7, rounded = 2, bg = o[3],
        w = ui.spring(ui.computed(function() return on:get() and 7 or 0 end), { stiffness = 600, damping = 30 }),
        ml = ui.spring(ui.computed(function() return on:get() and 3 or 0 end), { stiffness = 600, damping = 30 }),
        opacity = ui.tween(ui.computed(function() return on:get() and 1 or 0 end), { duration = 0.12 }) }
end

-- The record control's six states (pen XFJTT · Recording control). One
-- button whose parts stay put and morph: its fill, border, label and rule
-- ease to the state's colours, the label's slot springs to the new text's
-- width (so the button resizes rather than swapping), and the leading
-- glyph and the action icon change in place. Right-click previews any state.
-- Only the stopped (ready) control has an outline, as in the design.
local REC = {
    recording = { fill = "#E8A89824", border = "#4A2C2500", fg = C.coral, rule = "#E8A89840", glyph = "dot", action = "pause" },
    capturing = { fill = "#E8A89824", border = "#4A2C2500", fg = C.coral, rule = "#E8A89840", glyph = "dot", action = "x" },
    countdown = { fill = "#E8A89814", border = "#4A2C2500", fg = C.coral, rule = "#E8A89840", glyph = "arc", action = "x" },
    armed = { fill = "#E8A89814", border = "#4A2C2500", fg = C.coral, rule = "#E8A89840", glyph = "ring", action = "x" },
    paused = { fill = "#E2C98F1F", border = "#E2C98F00", fg = C.gold, rule = "#E2C98F40", glyph = "pause", action = "play" },
    stopped = { fill = C.selected, border = C.line, fg = C.text, rule = C.line, glyph = "dot", action = "chevron-down" },
}
local function recLabel()
    local r = S.rec:get()
    if r == "stopped" then return "Record" end
    if r == "countdown" then return "starts in " .. math.max(1, math.ceil(S.countdown:get())) end
    if r == "armed" then return "on first input" end
    if r == "capturing" then return string.format("%.1f / %d s", S.capture:get(), S.capture_total:get()) end
    local t = S.clock:get()
    return string.format("%02d:%05.2f", math.floor(t / 60), t % 60)
end
local pillRef, labelRef, ruleRef, dot, ring = ui.ref(), ui.ref(), ui.ref(), ui.ref(), ui.ref()
local labelW = ui.state("rec_label_w", 51)
local lastRec = S.rec:peek()
ui.effect(function()
    local r = S.rec:get()
    if r == lastRec then return end
    lastRec = r
    local t = REC[r]
    local ease = { duration = 240, ease = "outQuad" }
    anime.animate(pillRef, { bg = t.fill, borderColor = t.border, duration = ease.duration, ease = ease.ease })
    anime.animate(labelRef, { color = t.fg, duration = ease.duration, ease = ease.ease })
    anime.animate(ruleRef, { bg = t.rule, duration = ease.duration, ease = ease.ease })
end)
local pulse
ui.effect(function()
    local r = S.rec:get()
    if pulse then pulse:cancel(); pulse = nil end
    if r == "recording" or r == "capturing" or r == "armed" then
        pulse = anime.animate({ dot, ring }, { opacity = { 1, 0.3 }, duration = 900, loop = true, alternate = true,
            ease = "inOutSine", frameRate = 20 })
    else
        anime.animate({ dot, ring }, { opacity = 1, duration = 160 })
    end
end)
-- A part shown in some states: it fades and scales in place, centred in
-- its slot (a row: an overlay stacks its children at the top left).
local function showIn(test, node)
    local on = ui.computed(function() return test(S.rec:get()) end)
    return ui.row { absolute = true, top = 0, left = 0, right = 0, bottom = 0, items = "center", justify = "center",
        pointer_events = "none",
        opacity = ui.tween(ui.computed(function() return on:get() and 1 or 0 end), { duration = 0.16 }),
        scale = ui.spring(ui.computed(function() return on:get() and 1 or 0.6 end), { stiffness = 600, damping = 32 }),
        node }
end
local function glyphIs(kind) return function(r) return REC[r].glyph == kind end end
local function actionIs(kind) return function(r) return REC[r].action == kind end end
local stopped = function() return S.rec:get() == "stopped" end
local spring = { stiffness = 460, damping = 38 }
local record = ui.row(hint("record", {
    id = "record", ref = pillRef, h = 26, rounded = 7, border = 1, pl = 9, pr = 3, gap = 7, items = "center",
    bg = REC[S.rec:peek()].fill, border_color = REC[S.rec:peek()].border,
    on_resize = anchor("record"),
    context_menu = function() S.open("rec_states"); return nil end,
    -- The leading glyph, the label and (stopped) ⌘R: pressing them records.
    ui.button { id = "record_start", p = 0, bg = C.clear, border = 0, hover = {}, focus = {}, direction = "row", gap = 7,
        items = "center",
        cursor = function() return stopped() and "pointer" or "default" end,
        on_click = function() if S.rec:peek() == "stopped" then S.start() end end,
        ui.overlay { w = 10, h = 10,
            showIn(glyphIs("dot"), ui.div { ref = dot, rounded = 4, bg = C.coral,
                w = ui.spring(ui.computed(function() return stopped() and 8 or 7 end), spring),
                h = ui.spring(ui.computed(function() return stopped() and 8 or 7 end), spring) }),
            showIn(glyphIs("pause"), icon("pause", 10, C.gold)),
            showIn(glyphIs("ring"), ui.div { ref = ring, w = 8, h = 8, rounded = 4, border = 1.5, border_color = C.coral }),
            showIn(glyphIs("arc"), ui.canvas { w = 10, h = 10, draw = function(c)
                local left = S.countdown:get() % 1
                left = left == 0 and 1 or left
                c:stroke(ui.path():arc(5, 5, 4, -90, 360 * left), C.coral, 1.5, { cap = "round" })
            end }),
        },
        -- The label's slot springs to the text's width; the text itself
        -- is laid out unconstrained so it reports its own.
        ui.overlay { h = 16, overflow = "hidden", w = ui.spring(labelW, spring),
            ui.text { recLabel, ref = labelRef, absolute = true, left = 0,
                top = function() return stopped() and 0 or 1 end,
                font = function() return stopped() and K.SANS or K.MONO end,
                text_size = function() return stopped() and 12 or 10.5 end,
                color = REC[S.rec:peek()].fg,
                on_resize = function(size) labelW:set(math.ceil(size.w)) end } },
        ui.overlay { h = 14, overflow = "hidden", items = "center",
            w = ui.spring(ui.computed(function() return stopped() and 21 or 0 end), spring),
            ml = ui.spring(ui.computed(function() return stopped() and 0 or -7 end), spring),
            opacity = ui.tween(ui.computed(function() return stopped() and 1 or 0 end), { duration = 0.14 }),
            K.keys({ "command", "R" }) },
    },
    ui.div { ref = ruleRef, w = 1, h = 12, bg = REC[S.rec:peek()].rule },
    ui.div { on_resize = anchor("record_action"),
        K.button({ id = "record_toggle", w = 20, h = 20, rounded = 5, justify = "center",
            hover = { bg = "#FFFFFF0F" },
            on_click = function()
                if S.rec:peek() == "stopped" then S.open("capture") else S.toggle_rec() end
            end,
            ui.overlay { w = 12, h = 12,
                showIn(actionIs("pause"), icon("pause", 12, C.coral)),
                showIn(actionIs("play"), icon("play", 12, C.gold)),
                showIn(actionIs("x"), icon("x", 12, C.coral)),
                showIn(actionIs("chevron-down"), icon("chevron-down", 12, C.tertiary)) } }, 0.88) },
    -- A capture's progress: square-ended, along the bottom edge from the
    -- left (as the design draws it, not cut by the corner).
    ui.div { ref = progressRef, absolute = true, left = -1, right = -1, bottom = -1, h = 2, bg = C.coral,
        origin = { 0, 0.5 }, scale_x = 0, pointer_events = "none",
        opacity = function() return S.rec:get() == "capturing" and 1 or 0 end },
}, true))

local function action(id, glyph, size, name)
    return ui.div { on_resize = anchor(name),
        K.button(hint(name, { id = id, w = 28, h = 28, rounded = 7, justify = "center",
            bg = menuActive(name), on_click = function() S.open(name) end,
            icon(glyph, size, function() return S.menu:get() == name and C.text or C.secondary end) })) }
end
local titleBar = ui.row {
    id = "title_bar", w_full = true, h = 44, pl = 84, pr = 4, gap = 10, items = "center",
    tabs,
    ui.div { grow = 1, h_full = true, on_press = ui.action("drag_window") },
    record,
    K.button(hint("overlays", {
        id = "overlays_button", h = 28, rounded = 7, pl = 9, pr = 8, gap = 7,
        bg = menuActive("overlays"), on_click = function() S.open("overlays") end, on_resize = anchor("overlays"),
        icon("layers", 14, C.secondary), text("Overlays", 12, C.secondary),
        ui.row { items = "center", ml = -3, swatches },
        icon("chevron-down", 12, C.muted),
    })),
    ui.row { gap = 2, items = "center",
        action("search_button", "search", 14, "search"),
        action("more_button", "more", 15, "more"),
    },
}

-- Summary -------------------------------------------------------------------

local function metric(first, label, value, unit, sub, tone, warn)
    return ui.column {
        grow = 1, basis = 0, min_w = 0, gap = 5, py = 2, pl = first and 0 or 20, pr = 20,
        border = first and 0 or 1, border_sides = { 0, 0, 0, 1 }, border_color = C.line,
        ui.row { gap = 6, items = "center", warn and icon("alert", 12, C.coral) or nil, text(label, 11.5, C.tertiary) },
        ui.row { gap = 5, items = "end",
            text(value, 24, tone or C.text, { weight = 500, letter_spacing = -0.4 }),
            text(unit, 12, C.muted, { pb = 4 }) },
        mono(sub, 10.5, C.muted),
    }
end
local overCount = ui.computed(function()
    local n, b = 0, budget:get()
    for i = 1, 120 do if info(i, S.range:get()).cpu > b then n += 1 end end
    return n
end)
-- Live: percentiles over the last 120 frames, the rate over the last second.
local liveStats = ui.computed(function()
    local v = live.get()
    if not v or #v.frames == 0 then return false end
    local cpu, gpu, worst = {}, {}, nil
    for _, f in v.frames do
        local c = f.input + f.update + f.layout + f.paint + f.render + f.present
        table.insert(cpu, c)
        if f.gpu then table.insert(gpu, f.gpu) end
        if not worst or c > worst.cpu then worst = { cpu = c, n = f.n } end
    end
    table.sort(cpu); table.sort(gpu)
    local function pct(list, p) return #list > 0 and list[math.clamp(math.ceil(#list * p), 1, #list)] or 0 end
    return { fps = v.avg and v.avg.frames or 0, cpu = { pct(cpu, 0.5), pct(cpu, 0.95), pct(cpu, 0.99) },
        gpu = #gpu > 0 and { pct(gpu, 0.5), pct(gpu, 0.95), pct(gpu, 0.99) } or false, worst = worst }
end)
local function stat(f, sample)
    return function() local x = liveStats:get(); return x and f(x) or sample end
end
local function pcts(list) return string.format("p95 %.2f · p99 %.2f", list[2], list[3]) end
local summary = ui.row { w_full = true,
    metric(true, "Frame rate", stat(function(x) return tostring(x.fps) end, "119.4"), "fps",
        stat(function(x) return "last second · " .. comma(live.get().count) .. " kept" end, "target 120 · 3 dropped")),
    metric(false, "CPU frame · p50", stat(function(x) return string.format("%.2f", x.cpu[1]) end, "3.12"), "ms",
        stat(function(x) return pcts(x.cpu) end, "p95 5.84 · p99 9.71")),
    metric(false, "GPU frame · p50", stat(function(x) return x.gpu and string.format("%.2f", x.gpu[1]) or "—" end, "1.18"), "ms",
        stat(function(x) return x.gpu and pcts(x.gpu) or "no GPU timing" end, "p95 1.64 · p99 2.31")),
    metric(false, "Over budget", function() return tostring(overCount:get()) end, "/ 120",
        stat(function(x)
            return string.format("worst %+.2f ms · #%s", x.worst.cpu - budget:get(), comma(x.worst.n))
        end, "worst +1.53 ms · #48,213"), C.coral, true),
}

-- Frames chart ---------------------------------------------------------------

local grow = ui.state("chart_grow", 0)
local chartMax = ui.state("chart_max", 16.7)
local plotW = ui.state("plot_w", 1040)
local function barW() return (plotW:get() - 119) / 120 end
local function yOf(ms) return 126 - ms / chartMax:get() * 120 end
local SEGMENTS = { { "u", D.colors.update }, { "l", D.colors.layout }, { "p", D.colors.paint }, { "r", D.colors.render } }
local function drawBars(c, w, h)
    local frames = rangeFrames(S.range:get())
    local g = grow:get()
    local scale = 16.7 / chartMax:get()
    local bw = (w - 119) / 120
    c:rect(0, 6, w, 1, C.subtle)
    for i, f in frames do
        -- Each bar rises a little after the one before it.
        local t = math.clamp(g * 1.5 - (i - 1) / 120 * 0.5, 0, 1)
        t = (1 - (1 - t) ^ 3) * scale
        if t > 0 then
            local x, y = (i - 1) * (bw + 1), 126
            local top
            for _, seg in SEGMENTS do
                local v = f[seg[1]]
                if v > 0 then
                    local hh = v * t
                    y -= hh
                    c:rect(x, y, bw, hh, seg[2])
                    top = { y, hh, seg[2] }
                    y -= 1
                end
            end
            if top and top[2] > 2 then
                c:rect(x, top[1], bw, top[2], C.body)
                c:rect(x, top[1], bw, top[2], top[3], 2)
                c:rect(x, top[1] + 2, bw, top[2] - 2, top[3])
            end
        end
    end
    c:rect(0, 126, w, 1, C.strong)
    c:rect(0, math.floor(yOf(budget:get())), w, 1, "#E8A898B0")
end
local function growBars()
    anime.animate(grow, { value = { 0, 1 }, duration = 760, ease = "outQuart" })
end
local lastRange = S.range:peek()
ui.effect(function()
    local r = S.range:get()
    if r ~= lastRange then lastRange = r; growBars() end
end)
-- A budget past the chart rescales it.
ui.effect(function()
    local top = math.max(16.7, budget:get() * 1.25)
    if math.abs(top - chartMax:peek()) > 0.01 then
        anime.animate(chartMax, { value = top, duration = 420, ease = "outCubic" })
    end
end)

local function column(state, color, stiff)
    return ui.div {
        absolute = true, top = 6, left = 0, h = 120, rounded = 2, bg = color, pointer_events = "none",
        w = function() return math.max(1, barW()) end,
        x = ui.spring(ui.computed(function() return (math.max(state:get(), 1) - 1) * (barW() + 1) end),
            { stiffness = stiff, damping = 40 }),
        opacity = ui.tween(ui.computed(function() return state:get() > 0 and 1 or 0 end), { duration = 0.1 }),
    }
end
local plotEl, bodyEl
local plot = ui.overlay {
    id = "plot", grow = 1, shrink = 1, min_w = 0, h = 132, cursor = "pointer",
    on_resize = function(size, el) plotW:set(size.w); plotEl = el end,
    on_pointer_move = function(e)
        S.hover_frame:set(math.clamp(math.floor(e.local_x / (barW() + 1)) + 1, 1, 120))
    end,
    on_hover = function(inside) if not inside then S.hover_frame:set(0) end end,
    on_click = function()
        if S.hover_frame:peek() > 0 then S.frame:set(S.hover_frame:peek()) end
    end,
    column(S.frame, "#FFFFFF14", 520),
    column(S.hover_frame, "#FFFFFF0A", 1400),
    ui.canvas { w_full = true, h = 132, draw = drawBars, on_mount = growBars, pointer_events = "none" },
}

local function legend(color, label)
    return ui.row { gap = 6, items = "center", ui.div { w = 8, h = 8, rounded = 2, bg = color }, text(label, 11.5, C.secondary) }
end
local function fmtMs(v) return (string.format("%.2f", v):gsub("0$", "")) end
local framesSection = ui.column { w_full = true, gap = 12,
    ui.row { w_full = true, h = 24, gap = 10, items = "center",
        text("Frames", 13, C.text, { weight = 500 }),
        mono(function() return D.ranges[S.range:get()].meta end, 10.5, C.muted),
        spacer(),
        ui.row { gap = 14, pr = 10, items = "center",
            legend(D.colors.update, "Update"), legend(D.colors.layout, "Layout"),
            legend(D.colors.paint, "Paint"), legend(D.colors.render, "Render"),
            ui.div { on_resize = anchor("budget"),
                K.button { id = "budget", gap = 6, h = 22, px = 5, rounded = 6, bg = menuActive("budget"),
                    on_click = function() S.open("budget") end,
                    ui.div { w = 12, h = 2, bg = "#E8A898B0" },
                    text(function() return "Budget " .. fmtMs(budget:get()) .. " ms" end, 11.5, C.secondary),
                    icon("chevron-down", 10, C.muted) } },
        },
        K.segmented("range", S.range, 3, function(i, active)
            return mono(D.ranges[i].label, 10.5, function() return active() and C.text or C.tertiary end)
        end, { h = 20, px = 8, bg = C.inset }),
    },
    ui.row { w_full = true, gap = 8,
        ui.overlay { w = 40, h = 132,
            mono(function() return fmtMs(chartMax:get()) end, 10, C.muted, { absolute = true, right = 0, top = 0 }),
            mono(function() return fmtMs(budget:get()) end, 10, C.coral, { absolute = true, right = 0,
                top = function() return math.clamp(yOf(budget:get()) - 6, 12, 108) end }),
            mono("0 ms", 10, C.muted, { absolute = true, right = 0, top = 120 }) },
        plot,
    },
    ui.row { w_full = true, pl = 48, justify = "between",
        mono(function() return D.ranges[S.range:get()].ticks[1] end, 10, C.muted),
        mono(function() return D.ranges[S.range:get()].ticks[2] end, 10, C.muted),
        mono(function() return D.ranges[S.range:get()].ticks[3] end, 10, C.muted),
        mono(function() return D.ranges[S.range:get()].ticks[4] end, 10, C.muted),
        mono("now", 10, C.muted) },
}

-- The hovered bar's card (pen XFJTT · Frame hover).
local cardRef = ui.ref()
local cardShown = false
local hovered = ui.computed(function()
    local h = S.hover_frame:get()
    return h > 0 and info(h, S.range:get()) or nil
end)
local PHASES = { { "u", "update", D.colors.update }, { "l", "layout", D.colors.layout }, { "p", "paint", D.colors.paint },
    { "r", "render", D.colors.render }, { "present", "present", "#8C8A82" } }
local function hv(f, fallback)
    return function() local x = hovered:get(); return x and f(x) or fallback end
end
local phaseBar, phaseRows = {}, {}
for i, ph in PHASES do
    phaseBar[i] = ui.div { h = 6, bg = ph[3], shrink = 0,
        corners = i == 1 and { 2, 0, 0, 2 } or i == #PHASES and { 0, 2, 2, 0 } or nil,
        w = hv(function(x) return math.max(1, (212 - 4) * x.ms[ph[1]] / math.max(x.cpu, 0.001)) end, 1) }
    phaseRows[i] = ui.row { w_full = true, gap = 6, items = "center",
        ui.div { w = 6, h = 6, rounded = 1.5, bg = ph[3] }, text(ph[2], 11, C.tertiary), spacer(),
        mono(hv(function(x) return string.format("%.2f ms", x.ms[ph[1]]) end, ""), 10.5,
            hv(function(x) return (ph[1] == "l" and x.passes >= 3) and C.gold or C.text end, C.text)) }
end
local frameCard = ui.column {
    id = "frame_card", ref = cardRef, absolute = true, left = 0, top = 0, w = 236, gap = 8, py = 10, px = 12,
    bg = C.tip, border = 1, border_color = C.strong, rounded = 10, pointer_events = "none",
    shadow = { y = 8, blur = 24, color = "#00000080" },
    opacity = ui.tween(ui.computed(function() return hovered:get() and 1 or 0 end), { duration = 0.1 }),
    ui.row { w_full = true, gap = 6, items = "center",
        text(hv(function(x) return "Frame " .. comma(x.n) end, ""), 12, C.text, { weight = 500 }), spacer(),
        mono(hv(function(x) return string.format("%.2f ms", x.cpu) end, ""), 10.5,
            hv(function(x) return x.over > 0 and C.coral or C.secondary end, C.secondary)) },
    mono(hv(function(x)
        local b = x.over > 0 and string.format("over budget +%.2f ms", x.over) or string.format("under budget −%.2f ms", -x.over)
        return b .. string.format(" · GPU %.2f ms", x.gpu)
    end, ""), 10, C.muted),
    ui.row { w_full = true, h = 6, gap = 1, phaseBar },
    ui.column { w_full = true, gap = 4, phaseRows },
    mono("click opens it in the trace", 10, C.faint),
}
ui.effect(function()
    local h = S.hover_frame:get()
    if h == 0 or not plotEl or not bodyEl then return end
    local bx = plotEl.x - bodyEl.x + (h - 1) * (barW() + 1)
    local x = bx + barW() + 10
    if x + 236 > bodyEl.width - 16 then x = bx - 10 - 236 end
    local y = plotEl.y - bodyEl.y - 14
    if cardShown then
        anime.animate(cardRef, { x = x, y = y, duration = 260, ease = K.spring(700, 40) })
    else
        anime.utils.set(cardRef, { x = x, y = y })
    end
    cardShown = true
end)
ui.effect(function() if S.hover_frame:get() == 0 then cardShown = false end end)

-- Frame trace -------------------------------------------------------------------

local LANE_Y = { [0] = 0, 26, 52, 90 }
local LANES = { [0] = "Main · UI thread", "Main · UI thread", "Main · UI thread", "GPU queue" }
local function badge(id, content, fg, bg, border, hidden)
    return ui.row { id = id, h = 22, px = 8, rounded = 6, items = "center", bg = bg,
        hidden = hidden, mono(content, 10.5, fg) }
end
local function overTone(field)
    return function()
        local over = current:get().over > 0
        if field == "fg" then return over and C.coral or C.live end
        if field == "bg" then return over and "#E8A89829" or "#C3CEB21C" end
        return over and "#4A2C25" or "#3A4232"
    end
end
local function zx(x) local z = S.zoom:get(); return x * z.k - z.off end

local function spanView(s)
    local top = s.lane == 0
    local dark = D.dark[s.kind]
    local selected = function() return S.span:get() == s.id end
    local flagged = function()
        return s.id == "pass1" or (S.passes:get() and (s.id == "pass2" or s.id == "pass3"))
    end
    local need = s.label and #s.name * 6.2 + 12 + (s.label == "dur" and 30 or 0)
    local fits = function() return need and s.w * S.zoom:get().k >= need end
    local fg = (top and s.kind ~= "present") and C.window or C.text
    local spring = { stiffness = 420, damping = 40 }
    return ui.row {
        id = "span_" .. s.id, absolute = true, top = LANE_Y[s.lane], h = 22, rounded = 4,
        left = ui.spring(ui.computed(function() return zx(s.x) end), spring),
        w = ui.spring(ui.computed(function() return s.w * S.zoom:get().k end), spring),
        px = 6, gap = 6, items = "center", overflow = "hidden", cursor = "pointer",
        bg = top and D.colors[s.kind] or function() return (selected() or flagged()) and dark[3] or dark[1] end,
        hover = { brightness = 1.18 },
        on_hover = function(inside)
            if inside then S.hover_span:set(s.id) elseif S.hover_span:peek() == s.id then S.hover_span:set("") end
        end,
        on_click = function() S.span:set(S.span:peek() == s.id and "" or s.id) end,
        context_menu = function(e)
            S.hover_span:set("")
            S.ctx:set({ id = s.id, x = e.x, y = e.y })
            S.menu:set("")
            S.menu:set("span")
            return nil
        end,
        s.label and text(s.name, 11, fg, { weight = top and 500 or 400, hidden = function() return not fits() end }) or nil,
        s.label == "dur" and mono(string.format("%.2f", s.w / D.px_per_ms), 10, top and "#121211A6" or C.tertiary,
            { hidden = function() return not fits() end }) or nil,
    }
end

local spansEl
local tipRef = ui.ref()
local tip = ui.state("tip", { id = "" })
local tipPlaced = false
local function placeTip()
    local id = S.hover_span:peek() ~= "" and S.hover_span:peek() or S.span:peek()
    local s = id ~= "" and S.span_by_id(id)
    if not s or not spansEl or not bodyEl or not spansEl.mounted then
        tip:set({ id = "" })
        return
    end
    local z = S.zoom:peek()
    local right = math.clamp((s.x + s.w) * z.k - z.off, 228, TRACK)
    local x = spansEl.x - bodyEl.x + right - 228
    local y = spansEl.y - bodyEl.y + LANE_Y[s.lane] + 28
    local detail = current:peek().design and D.details[s.id] or nil
    local rows = detail and detail.rows or {
        { "lane", LANES[s.lane] }, { "starts at", string.format("%.2f ms", s.x / D.px_per_ms) },
        { "frame", "#" .. comma(current:peek().n) } }
    tip:set({ id = id, color = D.colors[s.kind], title = s.title or (s.name:sub(1, 1):upper() .. s.name:sub(2)),
        dur = detail and detail.dur or string.format("%.2f ms", s.w / D.px_per_ms), rows = rows })
    if tipPlaced then
        anime.animate(tipRef, { x = x, y = y, duration = 360, ease = K.spring(520, 36) })
    else
        tipPlaced = true
        anime.utils.set(tipRef, { x = x, y = y })
    end
end
ui.effect(function()
    S.hover_span:get(); S.span:get(); trace:get(); S.tab:get(); S.zoom:get()
    placeTip()
end)

local function tipRow(i)
    return ui.row { w_full = true, justify = "between",
        text(function() local r = tip:get().rows; return r and r[i] and r[i][1] or "" end, 11, C.tertiary),
        mono(function() local r = tip:get().rows; return r and r[i] and r[i][2] or "" end, 10.5, C.text) }
end
local tooltip = ui.column {
    id = "span_tooltip", ref = tipRef, absolute = true, left = 0, top = 0, w = 228, gap = 8, py = 10, px = 12,
    bg = C.tip, border = 1, border_color = C.strong, rounded = 10, pointer_events = "none",
    shadow = { y = 8, blur = 24, color = "#00000080" },
    opacity = ui.tween(ui.computed(function()
        return (tip:get().id ~= "" and S.tab:get() == 1 and S.hover_frame:get() == 0) and 1 or 0
    end), { duration = 0.12 }),
    ui.row { w_full = true, gap = 6, items = "center",
        ui.div { w = 8, h = 8, rounded = 2, bg = function() return tip:get().color or D.colors.render end },
        text(function() return tip:get().title or "" end, 12, C.text, { weight = 500 }),
        spacer(),
        mono(function() return tip:get().dur or "" end, 10.5, C.secondary) },
    ui.column { w_full = true, gap = 4, tipRow(1), tipRow(2), tipRow(3) },
}

-- Ruler and grid for the zoom: ticks every 2 ms at 1×, finer as it zooms.
local function gridFor(z)
    local ppm = D.px_per_ms * z.k
    local step = ({ 2, 1, 0.5, 0.25 })[math.clamp(math.floor(math.log(z.k * 1.2, 2)) + 1, 1, 4)]
    local lines, ticks = {}, {}
    local first = math.floor(z.off / ppm / step) * step
    local ms = first
    while ms * ppm - z.off <= TRACK do
        local x = math.floor(ms * ppm - z.off)
        if x >= 0 then
            table.insert(lines, ui.div { absolute = true, left = x, top = 0, w = 1, h = 116, bg = C.subtle })
            local near = math.abs(ms - 8.33) * ppm < 40
            if not near then
                table.insert(ticks, ui.text { ms == 0 and "0" or (num(ms) .. " ms"), absolute = true, left = x + 3, top = 0,
                    font = K.MONO, text_size = 10, color = C.muted })
            end
        end
        ms += step
    end
    return lines, ticks
end
local zoomKey = function() local z = S.zoom:get(); return string.format("%.3f:%.1f", z.k, z.off) end
local vsyncX = ui.spring(ui.computed(function() return math.floor(zx(8.33 * D.px_per_ms)) end), { stiffness = 420, damping = 40 })

local zoomLabel = function()
    local z = S.zoom:get()
    if z.mode == "frame" then return "Fit frame" end
    if z.mode == "vsync" then return "Fit vsync" end
    if z.mode == "selection" then return "Fit selection" end
    return string.format("%d%%", math.floor(z.k * 100 + 0.5))
end
local traceSection = ui.column {
    id = "frame_trace", w_full = true, gap = 12, pt = 20, border = 1, border_sides = { 1, 0, 0, 0 }, border_color = C.subtle,
    ui.row { w_full = true, h = 26, gap = 8, items = "center",
        text(function() return "Frame " .. comma(current:get().n) end, 13, C.text, { weight = 500, id = "frame_title" }),
        ui.div { w = 4 },
        badge("badge_budget", function()
            local x = current:get()
            return x.over > 0 and string.format("over budget +%.2f ms", x.over) or string.format("under budget −%.2f ms", -x.over)
        end, overTone("fg"), overTone("bg"), overTone("border")),
        badge("badge_cpu", function() return string.format("CPU %.2f ms", current:get().cpu) end, C.secondary, C.selected, C.strong),
        badge("badge_gpu", function() return string.format("GPU %.2f ms", current:get().gpu) end, C.secondary, C.selected, C.strong),
        badge("badge_passes", function() return "layout × " .. current:get().passes .. " passes" end,
            C.gold, "#E2C98F24", "#4D4130", function() return current:get().passes < 2 end),
        spacer(),
        ui.div { on_resize = anchor("zoom"),
            K.button { id = "fit_frame", h = 26, rounded = 6, pl = 9, pr = 8, gap = 6, border = 1, border_color = C.line,
                bg = function() return S.menu:get() == "zoom" and "#2C2C29" or C.selected end, hover = { bg = "#2C2C29" },
                on_click = function() S.open("zoom") end,
                icon("maximize", 12, C.muted), text(zoomLabel, 12, C.secondary), icon("chevron-down", 12, C.muted) } },
        K.button { id = "prev_frame", w = 26, h = 26, rounded = 7, justify = "center", on_click = function() S.step(-1) end,
            icon("chevron-left", 14, C.secondary) },
        K.button { id = "next_frame", w = 26, h = 26, rounded = 7, justify = "center", on_click = function() S.step(1) end,
            icon("chevron-right", 14, C.secondary) },
    },
    ui.row { w_full = true, gap = 8, pb = 14,
        ui.overlay { w = 56, h = 146, shrink = 0,
            text("Main", 11.5, C.secondary, { absolute = true, top = 24, left = 0 }),
            mono("UI thread", 10, C.muted, { absolute = true, top = 40, left = 0 }),
            text("GPU", 11.5, C.secondary, { absolute = true, top = 114, left = 0 }),
            mono("queue", 10, C.muted, { absolute = true, top = 130, left = 0 }) },
        ui.column { w = TRACK, shrink = 1, min_w = 0, gap = 6, overflow = "hidden",
            ui.overlay { w = TRACK, h = 14, overflow = "hidden",
                ui.presence(zoomKey, function()
                    local _, ticks = gridFor(S.zoom:peek())
                    return ui.overlay { w = TRACK, h = 14, ticks }
                end, { enter = { opacity = { 0, 1 }, duration = 160 }, exit = { opacity = 0, duration = 100 } }),
                ui.text { "8.33 vsync", absolute = true, top = 0, left = 0, x = vsyncX, ml = 4, font = K.MONO,
                    text_size = 10, color = C.coral } },
            ui.overlay {
                id = "spans", w = TRACK, h = 116, overflow = "hidden",
                on_resize = function(_, el) spansEl = el; placeTip() end,
                ui.presence(zoomKey, function()
                    local lines = gridFor(S.zoom:peek())
                    return ui.overlay { w = TRACK, h = 116, lines }
                end, { enter = { opacity = { 0, 1 }, duration = 160 }, exit = { opacity = 0, duration = 100 } }),
                ui.div { absolute = true, left = 0, top = 86, w = TRACK, h = 30, bg = "#FFFFFF05" },
                -- A live frame rebuilds when its bar shows another frame.
                ui.presence(function()
                    local x = current:get()
                    return x.live and -x.n or S.range:get() * 1000 + S.frame:get()
                end, function()
                    local nodes = {}
                    for _, s in trace:peek() do table.insert(nodes, spanView(s)) end
                    return ui.overlay { w = TRACK, h = 116, nodes }
                end, { enter = { opacity = { 0, 1 }, duration = 180, ease = "outQuad" },
                    exit = { opacity = 0, duration = 110, ease = "inQuad" } }),
                ui.div { absolute = true, left = 0, x = vsyncX, top = 0, w = 1, h = 116, bg = "#E8A898B0", pointer_events = "none" },
            },
        },
    },
}

-- Work by area and renderer counters -----------------------------------------------

local hoverArea = ui.state("hover_area", 0)
local lastScope = S.scope:peek()
ui.effect(function()
    local s = S.scope:get()
    if s ~= lastScope then
        lastScope = s
        anime.animate(".scoped", { opacity = { 0.2, 1 }, duration = 260, ease = "outQuad" })
    end
end)
local function areaRow(i, a)
    local selected = function() return S.area:get() == i end
    local segments = {}
    local colors = { D.colors.layout, D.colors.paint, D.colors.render }
    for k = 1, 3 do
        segments[k] = ui.div { h = 10, bg = colors[k], shrink = 0,
            corners = k == 1 and { 2, 0, 0, 2 } or k == 3 and { 0, 2, 2, 0 } or nil,
            w = ui.spring(ui.computed(function() return a.bar[k] * (S.scope:get() == 1 and 1 or 0.42) end),
                { stiffness = 300, damping = 30 }) }
    end
    return ui.row {
        id = "area_" .. a.id, w_full = true, h = 32, px = 8, gap = 12, items = "center", rounded = 7,
        cursor = "pointer", hover = { bg = "#FFFFFF06" },
        on_click = function() S.area:set(i) end,
        on_hover = function(inside)
            if inside then hoverArea:set(i) elseif hoverArea:peek() == i then hoverArea:set(0) end
        end,
        ui.row { grow = 1, shrink = 1, min_w = 0, gap = 8, items = "center",
            text(a.name, 12.5, a.quiet and C.tertiary or C.text, { weight = function() return selected() and 500 or 400 end }),
            mono(a.state, 10, C.muted) },
        ui.row { w = 44, justify = "end",
            mono(function() return tostring(a.nodes[S.scope:get()]) end, 11, C.secondary, { class = "scoped" }) },
        ui.row { w = 150, h = 10, gap = 1, items = "center", segments },
        ui.row { w = 48, justify = "end",
            mono(function() return a.mpx[S.scope:get()] end, 11.5, function() return selected() and C.text or C.secondary end,
                { class = "scoped" }) },
        ui.row { w = 24, justify = "end",
            opacity = ui.tween(ui.computed(function() return (selected() or hoverArea:get() == i) and 1 or 0 end), { duration = 0.12 }),
            K.button { id = "highlight_" .. a.id, w = 22, h = 22, rounded = 7, justify = "center", icon("crosshair", 13, C.secondary) } },
    }
end
local rows = {}
for i, a in D.areas do rows[i] = areaRow(i, a) end
local function header(label, w, right)
    return ui.row { w = w, grow = w and 0 or 1, justify = right and "end" or "start", text(label, 11, C.muted) }
end
local function finding(id, glyph, color, body, link, act)
    return ui.row { id = id, grow = 1, basis = 0, min_w = 0, gap = 10, py = 10, px = 12, rounded = 9,
        bg = C.window,
        icon(glyph, 14, color),
        ui.column { grow = 1, shrink = 1, min_w = 0, gap = 6,
            text(body, 12, C.body_text, { wrap = true, line_height = 17.4 }),
            ui.button { id = id .. "_link", p = 0, bg = C.clear, border = 0, cursor = "pointer", hover = { color = C.text }, focus = {},
                color = C.secondary, on_click = act, ui.text { link, text_size = 12 } } },
    }
end
local work = ui.column { grow = 1, shrink = 1, min_w = 0, gap = 10,
    ui.row { w_full = true, h = 24, gap = 8, items = "center",
        text("Work by area", 13, C.text, { weight = 500 }),
        mono(function() return "dirty nodes · frame " .. comma(current:get().n) end, 10.5, C.muted),
        spacer(),
        K.segmented("scope", S.scope, 2, function(i, active)
            return text(i == 1 and "Frame" or "1 s avg", 11.5, function() return active() and C.text or C.tertiary end)
        end, { h = 20, px = 8, bg = C.inset }),
    },
    ui.column { w_full = true,
        ui.row { w_full = true, h = 28, px = 8, gap = 12, items = "center",
            header("Area"), header("Nodes", 44, true), header("Layout · Paint · Comp.", 150), header("Mpx", 48, true),
            ui.div { w = 24 } },
        ui.overlay { w_full = true,
            ui.div { absolute = true, left = 0, right = 0, top = 0, h = 32, rounded = 7, bg = C.selected,
                y = ui.spring(ui.computed(function() return (S.area:get() - 1) * 32 end), { stiffness = 520, damping = 38 }) },
            ui.column { w_full = true, rows },
        },
    },
    ui.column { w_full = true, gap = 8, pt = 6,
        mono("FINDINGS", 10, C.muted, { letter_spacing = 0.8 }),
        ui.row { w_full = true, gap = 10, items = "stretch",
            finding("finding_passes", "alert", C.warn,
                "Toasts measured 3×: wrapped text fed its size back into layout, +2.24 ms.", "Show passes",
                function() S.range:set(1); S.frame:set(D.selected); S.passes:toggle(); S.span:set("pass1") end),
            finding("finding_span", "info", C.info,
                "188 glyph uploads grew the atlas to 2048² on first-shaped toast text.", "Show span",
                function() S.range:set(1); S.frame:set(D.selected); S.span:set("glyphs") end),
        },
    },
}
local function counter(row, avg)
    local warn = row.warn
    return ui.row { w_full = true, h = 24, gap = 8, items = "center",
        text(row[1], 12, C.tertiary), spacer(),
        mono(row[2], 11.5, type(warn) == "function" and function() return warn() and C.warn or C.text end
            or warn and C.warn or C.text),
        avg and ui.row { w = 44, justify = "end", mono(row[3], 10.5, C.muted) } or nil }
end
-- The counters: the selected live frame's and the last second's mean, or the
-- sample's. Shaped nodes and glyph uploads warn when any happen.
local function round(n) return math.floor(n + 0.5) end
local COUNTERS = {
    function(f) return comma(round(f.draws)) end,
    function(f) return string.format("%d · %d", round(f.passes), round(f.blur)) end,
    function(f) return comma(round(f.instances)) end,
    function(f) return comma(round(f.shaped)) end,
    function(f) return comma(round(f.glyphs)) end,
    function(f) return string.format("%d · %d", round(f.repaints), round(f.hits)) end,
    function(f) return string.format("%.2f Mpx", f.damage / 1e6) end,
}
local liveCounters = {}
for i, row in D.counters do
    local show = COUNTERS[i]
    liveCounters[i] = {
        row[1],
        function() local x = current:get(); return x.live and show(x.live) or (live.get() and "—" or row[2]) end,
        function() local v = live.get(); return v and v.avg and show(v.avg) or (v and "—" or row[3]) end,
        warn = row.warn and function()
            local x = current:get()
            if not x.live then return not live.get() end
            return (i == 4 and x.live.shaped or x.live.glyphs) > 0
        end,
    }
end
local function group(label, avg, list)
    local items = { ui.row { w_full = true, h = 26, items = "center", justify = "between",
        border = 1, border_sides = { 0, 0, 1, 0 }, border_color = C.subtle,
        text(label, 11, C.muted), avg and mono("1 s avg", 10, C.faint) or nil } }
    for _, row in list do table.insert(items, counter(row, avg)) end
    return ui.column { w_full = true, items }
end
local renderer = ui.column { w = 248, shrink = 0, gap = 10,
    ui.row { w_full = true, h = 24, gap = 8, items = "center",
        text("Renderer", 13, C.text, { weight = 500 }), mono("GpuStats", 10.5, C.muted) },
    group("This frame", true, liveCounters),
    group("Retained", false, D.retained),
}
local breakdown = ui.row { w_full = true, gap = 24, pt = 20, border = 1, border_sides = { 1, 0, 0, 0 }, border_color = C.subtle,
    items = "start", work, renderer }

-- Body ------------------------------------------------------------------------------

local timeline = ui.column {
    id = "timeline", w_full = true, gap = 18,
    hidden = function() return S.tab:get() ~= 1 end,
    summary, framesSection, traceSection, breakdown,
}
-- The Areas tab (pen z9i0E5) is its own module; GPU, Memory and Events
-- are not imported yet.
local areas = require("./areas")(S, K, D)
local placeholder = ui.column {
    id = "tab_placeholder", w_full = true, h = 640, items = "center", justify = "center", gap = 10,
    hidden = function() return S.tab:get() <= 2 end,
    ui.presence(function() return S.tab:get() > 2 and S.tab:get() or nil end, function(t)
        return ui.column { items = "center", gap = 10,
            ui.div { w = 40, h = 40, rounded = 10, bg = C.selected, items = "center", justify = "center",
                icon(TABS[t][1], 18, C.secondary) },
            text(TABS[t][2], 14, C.text, { weight = 500 }),
            mono("not part of this import · Timeline and Areas only", 10.5, C.muted) }
    end, { enter = { opacity = { 0, 1 }, y = { 8, 0 }, duration = 260, ease = "outExpo" }, exit = { opacity = 0, duration = 90 } }),
}
local lastTab = S.tab:peek()
ui.effect(function()
    local t = S.tab:get()
    if t ~= lastTab and t == 1 then
        anime.animate("#timeline", { opacity = { 0, 1 }, y = { 6, 0 }, duration = 260, ease = "outExpo" })
    end
    lastTab = t
end)

local body = ui.scroll { id = "body", grow = 1, shrink = 1, min_h = 0, w_full = true, bg = C.body,
    rounded = 10, overflow = "hidden",
    ui.overlay { w_full = true,
        on_resize = function(_, el) bodyEl = el; placeTip() end,
        on_press = function() S.input() end,
        ui.column { w_full = true, pt = 16, px = 16, pb = 14, timeline, areas, placeholder },
        tooltip,
        frameCard,
    },
}

-- Status bar ------------------------------------------------------------------------

-- The status bar's chips sit on the body's colour and brighten on hover;
-- its end chips round their outer bottom corners with the window's.
local function chipStyle(name, props)
    props.bg = function() return S.menu:get() == name and C.selected or C.body end
    props.hover = { bg = C.selected }
    props.on_click = function() S.open(name) end
    return props
end
local speedIndex = {
    get = function()
        for i, sp in D.speeds do if math.abs(sp[1] - S.time_scale:get()) < 0.001 then return i end end
        return 0
    end,
    set = function(_, i) S.time_scale:set(D.speeds[i][1]) end,
}
ui.effect(function() if S.menu:get() == "logs" then S.logs_unread:set(0) end end)
local statusBar = ui.row { id = "status_bar", w_full = true, h = 34, gap = 6, items = "center",
    ui.div { on_resize = anchor("target"),
        K.button(chipStyle("target", { id = "target_button", h = 22, pl = 7, pr = 8, gap = 7, corners = { 6, 6, 6, 10 },
            ui.div { w = 6, h = 6, rounded = 3, bg = function()
                if liveFeed:get() then return C.live end
                return (not live.get() and S.target:get() == 1) and C.live or C.muted
            end },
            text(function()
                local v = live.get()
                return v and v.name or D.processes[S.target:get()][1]
            end, 11.5, C.text, { weight = 500 }),
            mono(function() return S.CAPS[S.cap:get()] end, 10.5, C.muted),
            icon("chevrons-up-down", 11, C.muted) }), 0.96) },
    K.segmented("speed", speedIndex, #D.speeds, function(i, active)
        return mono(D.speeds[i][2], 10.5, function() return active() and C.text or C.tertiary end)
    end, { h = 18, px = 7, r = 4, p = 2, outer = 6, spacing = 1, bg = C.inset, plain = true }),
    ui.div { grow = 1, h_full = true, on_press = ui.action("drag_window") },
    ui.div { on_resize = anchor("buffer"),
        K.button(chipStyle("buffer", hint("buffer", { id = "status_buffer", h = 20, rounded = 5, px = 6, gap = 5,
            ui.div { w = 36, h = 4, rounded = 2, bg = C.selected,
                ui.div { h = 4, rounded = 2, bg = C.tertiary, w = function() return 36 * S.buffer_fill() end } },
            mono(function() return S.buffer_frames_text() .. " · " .. S.buffer_bytes() end, 10.5, C.tertiary) })), 0.97) },
    ui.div { on_resize = anchor("logs"),
        K.button(chipStyle("logs", { id = "status_logs", h = 22, px = 8, gap = 6, corners = { 6, 6, 10, 6 },
            icon("scroll-text", 12, C.secondary),
            text("Logs", 11.5, C.secondary, { weight = 500 }),
            ui.presence(function() return S.logs_unread:get() > 0 and S.logs_unread:get() or nil end, function(n)
                return ui.row { h = 14, px = 4, rounded = 4, bg = "#1F1B14", items = "center", mono(tostring(n), 9.5, C.gold) }
            end, { layout = "width", enter = { opacity = { 0, 1 }, scale = { 0.6, 1 }, duration = 180, ease = "outExpo" },
                exit = { opacity = 0, scale = 0.6, duration = 120 } }) }), 0.96) },
}

-- Keys: ⌘F searches, ⌘R records, ←/→ step frames, space pauses, ⌥1–6
-- flip overlays, ⌥[ / ⌥] step the speed. Armed, any key starts recording.
local menus = require("./menus")(S, K, D)
local function onKey(e)
    if menus.on_key(e) then return true end
    S.input()
    if e.meta and (e.key == "f" or e.key == "F") then S.open("search"); return true end
    if e.meta and (e.key == "r" or e.key == "R") then S.toggle_rec(); return true end
    -- Keys reach this handler first; while a field is being typed in
    -- (the Areas filter) they are its own.
    if S.menu:peek() ~= "" or S.typing then return false end
    local r = S.rec:peek()
    if e.key == "Escape" and (r == "capturing" or r == "countdown" or r == "armed") then S.toggle_rec(); return true end
    if e.key == "ArrowRight" then S.step(1); return true end
    if e.key == "ArrowLeft" then S.step(-1); return true end
    if e.key == " " then S.toggle_rec(); return true end
    if e.alt and (e.key == "[" or e.key == "]") then S.step_scale(e.key == "]" and 1 or -1); return true end
    local n = e.alt and tonumber(e.key)
    if n and n >= 0 and n <= 6 then
        if n == 0 then S.overlays:set({ false, false, false, false, false, false }) else S.flip_overlay(n) end
        return true
    end
    return false
end

return ui.overlay {
    id = "profiler", w_full = true, h_full = true, bg = C.window, color = C.text, font = K.SANS, text_size = 12,
    on_resize = function(size)
        local v = S.viewport:peek()
        if v.w ~= size.w or v.h ~= size.h then
            S.viewport:set(size)
            if S.menu:peek() ~= "" then S.menu:set("") end
        end
    end,
    on_key = onKey,
    ui.column { w_full = true, h_full = true, px = 6, titleBar, body, statusBar },
    menus.layer,
}
