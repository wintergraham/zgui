-- Floating navbar lab (navbar.js navbarLab), as native zgui nodes. The dummy browser and omnibox model are
-- plain Luau tables; small revision states tell the views which part changed. Motion values are states
-- animated by the anime engine, so nothing runs at rest.
local T = require("./theme")
local model = require("./model")
local dither = require("./dither")
local tints = require("./tints")
local page = require("./page")
local C, EASE, lerp, clamp01, smooth, mix = T.C, T.EASE, T.lerp, T.clamp01, T.smooth, T.mix
local icon, txt, key, hint = T.icon, T.txt, T.key, T.hint
local H = model.H

local SHEET_WIDTH, FULL_WIDTH = 680, 762
local OPEN = { open = { 280, "cubicBezier(.16,1,.3,1)" }, close = { 150, "cubicBezier(.4,0,1,1)" }, stagger = 60 }
local MODE = { ms = 260, ease = "cubicBezier(.2,0,0,1)", slide = 6 }
local URLM = { ms = 220, ease = "cubicBezier(.2,0,0,1)" }
local SHAPE = { ms = 360, ease = "cubicBezier(.32,.72,0,1)" }
-- The dock settle: a damped spring (~6% overshoot), as navbar.js springEase(380, 26, .52).
local function springEase(k, c, seconds)
    local w = math.sqrt(k)
    local z = c / (2 * w)
    local wd = w * math.sqrt(1 - z * z)
    return function(p)
        if p >= 1 then return 1 end
        local t = p * seconds
        return 1 - math.exp(-z * w * t) * (math.cos(wd * t) + z * w / wd * math.sin(wd * t))
    end
end
local DOCK = { margin = 8, threshold = 4, settle = { 520, springEase(380, 26, 0.52) } }
local WASH_NEUTRAL, WASH_TINT, WASH_FADE = { 0x3A, 0x39, 0x33 }, 0.4, 400
-- behavior.js DRIFT: the attachment chip's drifting wash.
local DRIFT = { mode = 3, pixel = 1.5, from = 0, to = 0.5, curve = 1.3, scale = 2.6, soft = 0.18, speed = 0.1 }
local AURA = { mode = 5, pixel = 2, band = 56, from = 0.5, curve = 1.7, scale = 2.4, soft = 0.22, speed = 0.22, seed = 3 }
local DOCKS = { "left", "center", "right" }

return function(lab)
    local slow, reduced = lab.slow, lab.reduced
    local function S(ms) return ms * slow:peek() end
    local function R(ms) return reduced:peek() and 0 or ms end
    local function later(ms, fn)
        return anime.createTimer { duration = math.max(1, S(ms)), onComplete = function() fn() end }
    end
    local function state(name, initial) return ui.state("nb_" .. name, initial) end
    local eased = T.easer(slow, "nb")

    -- Motion (design_system::interaction::Motion): a value eased toward targets, scaled by the lab's slow-mo.
    local function to(m, v, ms, ease, done)
        anime.utils.remove(m)
        if ms <= 0 then
            m:set(v)
            if done then done() end
            return
        end
        anime.animate(m, { value = v, duration = S(ms), ease = ease, onComplete = done and function() done() end or nil })
    end
    local morph, hoverM, openM, askM = state("morph", 0), state("hover_m", 0), state("open_m", 0), state("ask_m", 0)
    local offX, offY = state("off_x", 0), state("off_y", 0)
    local popM, idM, swapM, bounce, heightM = state("pop_m", 0), state("id_m", 1), state("swap_m", 0), state("bounce", 1), state("height_m", 0)
    local washA = state("wash_a", 0)
    local morphing = false

    -- Revisions: what changed in the plain model.
    local pageRev, docRev, sugRev, savedRev = state("page_rev", 0), state("doc_rev", 0), state("sug_rev", 0), state("saved_rev", 0)
    local function bump(s) s:set(s:peek() + 1) end

    ---------- Dummy browser ----------
    local labState = { slowLoads = false, offline = false, toast = nil }
    local dock = state("dock", "center")
    local acting = state("acting", false)
    local offline = state("offline", false)
    local permChecked = state("perm_checked", false)
    local saved = {
        bookmarks = {
            { url = "https://github.com/ask-browser/ask", title = "ask-browser/ask", folder = "Bookmarks" },
            { url = "https://www.figma.com/files/recent", title = "Recent files – Figma", folder = "Bookmarks" },
            { url = "https://www.notion.so/ask/Motion-notes", title = "Motion notes", folder = "Reading list" },
        },
        history = {
            { url = "https://www.rtings.com/headphones/reviews/best/wireless", title = model.TITLES["https://www.rtings.com/headphones/reviews/best/wireless"] },
            { url = "https://en.wikipedia.org/wiki/Ordered_dithering", title = "Ordered dithering - Wikipedia" },
            { url = "https://www.youtube.com/watch?v=spring-physics", title = model.TITLES["https://www.youtube.com/watch?v=spring-physics"] },
            { url = "https://www.google.com/search?q=cubic+bezier+easing", title = "cubic bezier easing - Google Search" },
            { url = "http://legacy-intranet.example/wiki", title = "Legacy intranet wiki" },
        },
        permissions = {},
        pinned = { "protonpass" },
    }
    local tabSeq, permSeq = 0, 0
    local function makeTab(url)
        tabSeq += 1
        return { id = tabSeq, entries = { { url = url, title = model.titleFor(url), scroll = 0 } }, index = 1,
            loading = false, navigation = 0, reader = false, iconResolved = true,
            siteData = { origin = "", loading = false, entries = {} }, timers = {} }
    end
    local tabs = {}
    for _, url in { "https://www.theverge.com/tech/ask-browser-review", "https://github.com/ask-browser/ask/pull/412",
        "https://en.wikipedia.org/wiki/Dither", "https://linear.app/ask/issue/ASK-218", "https://news.ycombinator.com/" } do
        table.insert(tabs, makeTab(url))
    end
    local active = tabs[1].id
    local function activeTab()
        for _, t in tabs do if t.id == active then return t end end
        return tabs[1]
    end
    local function entry(t) t = t or activeTab(); return t.entries[t.index] end

    ---------- Navbar model (navbar_model.rs Navbar) ----------
    local nb = { input = "", edited = false, mode = { k = "search" }, hasSelection = false, suggestions = {} }
    local focused, modeK = state("focused", false), state("mode", "search")
    local selected, popover, collapsed, hover = state("selected", 0), state("popover", ""), state("collapsed", false), state("hover", false)
    local inputValue, promptValue = state("input", ""), state("prompt", "")
    local P = { url = "", title = "", security = "internal", loading = false, canGoBack = false, canGoForward = false,
        online = true, reader = false, iconResolved = true }
    local progress, settle = state("progress", 0), state("settle", 0)
    local docScroll = state("doc_scroll", 0)
    local W, Hh, pillW = state("w", 1130), state("h", 678), state("pill_w", 139)
    local sheetH, cmpH = state("sheet_h", 0), state("cmp_h", 0)
    local inputEl, promptEl, docEl
    local function setMode(m) nb.mode = m; modeK:set(m.k) end

    local function siteFav(url)
        local s = model.site(url)
        return s and "assets/" .. s.fav or nil
    end
    -- navbar_model.rs LoadProgress, driven by anime rather than sampled each frame.
    local lp = { key = nil, started = false, shown = false, finished = false }
    local function progressSync()
        local t = activeTab()
        local k = t.id .. ":" .. t.navigation
        local function start()
            lp.started, lp.shown, lp.finished = true, false, false
            anime.animate(progress, { value = { 0, 1 }, delay = S(150), duration = S(model.CONNECT_MS),
                ease = model.connecting, onBegin = function() lp.shown = true end })
        end
        if lp.key ~= k then
            lp.key = k
            anime.utils.remove(progress); anime.utils.remove(settle)
            lp.started, lp.shown, lp.finished = false, false, false
            progress:set(0); settle:set(0)
            if t.loading then start() end
            return
        end
        if t.loading and not lp.started then start() end
        if not t.loading and lp.started and not lp.finished then
            lp.finished = true
            anime.utils.remove(progress)
            if not lp.shown then progress:set(0); lp.started = false; return end
            local from = progress:peek()
            -- Ease-out cubic starts at 3x its average speed: 3 * distance / maxSpeed keeps the peak at maxSpeed.
            local finishMs = math.max(model.FINISH.ms, 3 * (1 - from) / model.FINISH.maxSpeed * 1000)
            anime.animate(progress, { value = 1, duration = S(finishMs), ease = "outCubic", onComplete = function()
                anime.animate(settle, { value = { 0, model.FINISH.frames }, duration = S(model.FINISH.frames * model.FINISH.frameMs),
                    ease = "linear", modifier = math.floor, onComplete = function()
                        progress:set(0); settle:set(0); lp.started = false
                    end })
            end })
        end
    end
    local function syncPage()
        local t = activeTab()
        local e = entry(t)
        local url = e.url
        local s = model.site(url)
        P.url, P.title = url, e.title
        P.security = model.isNative(url) and "internal" or model.starts(url, "https://") and "secure"
            or model.starts(url, "http://") and "insecure" or "unknown"
        P.loading, P.canGoBack, P.canGoForward = t.loading, t.index > 1, t.index < #t.entries
        P.online = not labState.offline
        P.permission, P.siteData = t.permission, t.siteData
        P.favicon = s and "assets/" .. s.fav or nil
        P.reader = (s and s.reader or false) and not t.loading
        P.tabReader = t.reader
        P.iconResolved = t.iconResolved ~= false
        bump(pageRev)
        progressSync()
    end

    ---------- Document ----------
    local docKey, docFade = "", false
    local doc = { url = "", title = "", reader = false, offline = false }
    local scroll = model.scrollIntent()
    local function renderDoc(fade)
        local t = activeTab()
        local e = entry(t)
        local k = table.concat({ t.id, t.index, e.url, e.title, tostring(t.reader), tostring(labState.offline) }, "|")
        if k == docKey then return end
        docKey = k
        doc = { url = e.url, title = e.title, reader = t.reader, offline = labState.offline }
        docFade = fade and not reduced:peek()
        bump(docRev)
        docScroll:set(e.scroll or 0)
        scroll.sample(e.scroll or 0)
    end

    local refresh
    local function changed() syncPage(); if refresh then refresh() end end
    local function startLoad(t, url)
        for _, timer in t.timers do timer:cancel() end
        t.timers = {}
        t.loading = true
        t.navigation += 1
        local total = labState.slowLoads and 4200 or 900 + math.random() * 500
        local function at(f, fn) table.insert(t.timers, later(total * f, fn)) end
        if t.iconFor ~= url then t.iconResolved = false end
        at(0.35, function()
            local e = entry(t)
            e.title = model.titleFor(url); e.scroll = 0
            t.iconResolved = true; t.iconFor = url
            if t.id == active then renderDoc(true) end
            changed()
        end)
        at(1, function()
            t.loading = false; t.timers = {}
            if not model.isNative(url) then table.insert(saved.history, { url = url, title = entry(t).title }) end
            changed()
        end)
        changed()
    end
    local function navigateTab(url)
        local t = activeTab()
        while #t.entries > t.index do table.remove(t.entries) end
        table.insert(t.entries, { url = url, title = url, scroll = 0 })
        t.index += 1
        t.permission = nil; t.reader = false
        startLoad(t, url)
    end
    local function command(c)
        local t = activeTab()
        if c == "back" and t.index > 1 then entry(t).scroll = docScroll:peek(); t.index -= 1; startLoad(t, entry(t).url) end
        if c == "forward" and t.index < #t.entries then entry(t).scroll = docScroll:peek(); t.index += 1; startLoad(t, entry(t).url) end
        if c == "reload" then
            if t.loading then
                for _, timer in t.timers do timer:cancel() end
                t.timers = {}; t.loading = false
                changed()
            else
                startLoad(t, entry(t).url)
            end
        end
    end
    local function activate(id)
        entry().scroll = docScroll:peek()
        active = id
        renderDoc(false)
        changed()
    end

    -- Toast
    local toastText, toastOn = state("toast", ""), state("toast_on", false)
    local toastTimer
    local function notice(text)
        toastText:set(text); toastOn:set(true)
        if toastTimer then toastTimer:cancel() end
        toastTimer = later(1600, function() toastOn:set(false) end)
    end
    local function copyLink() notice("Link copied") end

    local function sitePermission(name)
        local o = model.originOf(entry().url)
        return (o and saved.permissions[o] and saved.permissions[o][name]) or model.PERM_DEFAULT[name] or "ask"
    end
    local function setSitePermission(name, value)
        local o = model.originOf(entry().url)
        if not o then return end
        saved.permissions[o] = saved.permissions[o] or {}
        saved.permissions[o][name] = value
        bump(savedRev)
    end
    local function permsChanged()
        local o = model.originOf(entry().url)
        return o ~= nil and saved.permissions[o] ~= nil and next(saved.permissions[o]) ~= nil
    end
    local popBuild = state("pop_build", 0)
    local function rebuildPop() bump(popBuild) end
    local function siteData()
        local t = activeTab()
        local origin = model.originOf(entry().url) or entry().url
        t.siteData = { origin = origin, loading = true, entries = {} }
        popover:set("data"); syncPage(); rebuildPop()
        later(600, function()
            local host = model.hostOf(origin)
            if host == "" then host = "site" end
            local names = { "session", "_ga", "consent", "prefs", "theme", "csrftoken", "_gid", "lang", "ab_bucket", "seen_banner", "tz", "cart" }
            local list = {}
            for i = 1, 3 + #host % 10 do table.insert(list, { name = names[i], domain = "." .. host }) end
            t.siteData = { origin = origin, loading = false, cookies = #list, entries = list }
            syncPage(); rebuildPop()
        end)
    end
    local function clearCookies()
        local t = activeTab()
        t.siteData = { origin = t.siteData.origin, loading = false, cookies = 0, entries = {} }
        syncPage(); rebuildPop()
        notice("Cookies cleared")
    end
    local function requestPermission(on)
        local t = activeTab()
        local origin = model.originOf(entry().url)
        if on and origin then
            permSeq += 1
            t.permission = { id = permSeq, origin = origin, kinds = { "camera", "microphone" } }
        else
            t.permission = nil
        end
        if not t.permission and popover:peek() == "permission" then popover:set("") end
        syncPage()
    end
    local function respondPermission(allow)
        local t = activeTab()
        if not t.permission then return end
        notice((allow and "Allowed" or "Blocked") .. " camera and microphone")
        t.permission = nil
        permChecked:set(false)
        syncPage()
    end

    local function bookmarked()
        for _, b in saved.bookmarks do if b.url == P.url then return b end end
        return nil
    end
    function refresh()
        local rows = model.suggestions(nb, tabs, activeTab(), saved)
        if selected:peek() >= #rows then selected:set(0) end
        nb.suggestions = rows
        bump(sugRev)
    end
    local function toggleBookmark()
        if P.url == "" or model.isNative(P.url) then return end
        local b = bookmarked()
        if b then
            table.remove(saved.bookmarks, table.find(saved.bookmarks, b))
        else
            table.insert(saved.bookmarks, { url = P.url, title = P.title, folder = "Bookmarks" })
        end
        bump(savedRev)
        refresh()
    end

    local prevValue, completionState = "", nil
    local function syncInput()
        if inputValue:peek() ~= nb.input then inputValue:set(nb.input) end
        prevValue = nb.input
        completionState = nil
    end
    local function focusNav(url)
        nb.input = model.isEmptyPage(url) and "" or url
        setMode({ k = "search" })
        nb.edited, nb.hasSelection = false, false
        selected:set(0); popover:set("")
        focused:set(true)
    end
    local ask = { initialized = false, prompt = "" }
    local attachments = state("attachments", {})
    local focusComposer = false
    local function dismiss()
        focused:set(false)
        nb.edited = false
        setMode({ k = "search" })
        nb.input = P.url
        popover:set("")
        syncInput()
        if inputEl then inputEl:blur() end
        if promptEl then promptEl:blur() end
    end
    local function step(delta)
        local len = #nb.suggestions
        if len == 0 then return end
        if not nb.hasSelection then
            selected:set(delta < 0 and len - 1 or 0)
        else
            selected:set(((selected:peek() + delta) % len + len) % len)
        end
        nb.hasSelection = true
    end
    -- state/navigation.rs navbar_changed
    local function navbarChanged(input)
        nb.input = input
        nb.edited, nb.hasSelection = true, true
        selected:set(0)
        if model.starts(input, "?") then setMode({ k = "ask" })
        elseif input == "@tabs" or model.starts(input, "@tabs ") then setMode({ k = "tabs" }) end
        refresh()
    end

    ---------- Ask composer (ask_composer.rs, reduced to the dummy contract) ----------
    local function enterAsk()
        if not ask.initialized then
            ask.initialized = true
            local e = entry()
            if not model.isEmptyPage(e.url) then
                attachments:set({ { id = activeTab().id, title = e.title, url = e.url, fav = model.site(e.url) and model.site(e.url).fav or nil } })
            end
        end
        if ask.prompt == "" and model.starts(nb.input, "?") then
            ask.prompt = string.gsub(nb.input, "^%?%s*", "")
            promptValue:set(ask.prompt)
        end
        focusComposer = true
    end
    local function focusInputSoon()
        if inputEl then inputEl:focus() end
    end
    local function askBack()
        setMode({ k = "search" })
        nb.input = ""
        refresh(); syncInput()
        focusInputSoon()
    end
    local function askSend(background)
        local prompt = model.trim(promptValue:peek())
        if prompt == "" then return end
        notice((background and "Running in background: " or "Opened in sidebar: ") .. prompt)
        ask.prompt = ""; promptValue:set("")
        ask.initialized = false; attachments:set({})
        dismiss()
    end

    -- state/navigation.rs submit_address
    local function submitAddress()
        local sel = (nb.edited or nb.hasSelection) and nb.suggestions[selected:peek() + 1] or nil
        if sel and sel.tab then activate(sel.tab); dismiss(); return end
        if sel and sel.kind == "copy" then copyLink(); dismiss(); return end
        local value = (not sel or sel.kind == "search" or sel.kind == "site") and model.queryOf(nb.input, nb.mode) or sel.value
        if model.trim(value) == "" then return end
        if nb.mode.k == "ask" or (sel and sel.kind == "ask") then
            setMode({ k = "ask" }); focused:set(true); enterAsk()
            ask.prompt = value; promptValue:set(value)
            return
        end
        value = nb.mode.k == "site" and model.siteSearch(nb.mode.site, value) or model.normalize(value)
        nb.input = value
        focused:set(false); popover:set("")
        if inputEl then inputEl:blur() end
        navigateTab(value)
    end
    local function accept(index)
        selected:set(index); nb.hasSelection = true
        submitAddress()
    end
    -- Command::FocusAddress
    local function focusAddress()
        focusNav(entry().url); refresh(); syncInput()
        if inputEl then inputEl:focus(); inputEl:select_all() end
    end

    ---------- Effects (floating.rs, address.rs, popovers.rs, identity.rs, navigation.rs) ----------
    local hoverGen, addrSerial, addrHover = 0, 0, false
    local pointer = nil
    local fx = { focused = false, ask = false, compact = false, popover = "", url = nil, oldUrl = "", loading = false,
        askTarget = 0, justDragged = false, staggerUntil = 0 }
    local oldUrl, newUrl = state("old_url", ""), state("new_url", "")
    local popVisible = state("pop_visible", "")
    local popSerial = 0
    local drag = nil
    local dragActive = state("drag_active", false)
    local dragX, dragY = state("drag_x", 0), state("drag_y", 0)
    local staggerOpen = false
    local staggerTimer

    local function syncAddrHover(animate, rect)
        local inside = pointer ~= nil and not focused:peek() and pointer[1] >= rect.x and pointer[1] <= rect.x + rect.w
            and pointer[2] >= rect.y and pointer[2] <= rect.y + rect.h
        addrSerial += 1
        if inside == addrHover then return end
        addrHover = inside
        if animate then to(hoverM, inside and 1 or 0, R(URLM.ms), URLM.ease) else to(hoverM, inside and 1 or 0, 0) end
    end

    -- Geometry. W, Hh are the window's inner size (navbar.js windowSize).
    local function anchorX(d, w)
        local width = W:get()
        return d == "left" and DOCK.margin or d == "right" and width - w - DOCK.margin or (width - w) / 2
    end
    local function finalWidth() return math.min(W:get() * 0.92, FULL_WIDTH) end
    -- The bar's surface: under the pointer while dragging (keeping the grab point), otherwise at its dock.
    local function navBox()
        local m = morph:get()
        local w, h = lerp(finalWidth(), pillW:get(), m), lerp(40, 30, m)
        if dragActive:get() then
            local width, height = W:get(), Hh:get()
            return { x = math.clamp(dragX:get() - drag.fx * w, 0, math.max(0, width - w)),
                y = math.clamp(dragY:get() - drag.fy * h, 0, math.max(0, height - h)), w = w, h = h }
        end
        return { x = anchorX(dock:get(), w) + offX:get(), y = DOCK.margin + offY:get(), w = w, h = h }
    end
    local function nearestDock(box)
        local c, pw, width = box.x + box.w / 2, pillW:get(), W:get()
        local best, dist = "center", math.huge
        for _, d in { { "left", DOCK.margin + pw / 2 }, { "center", width / 2 }, { "right", width - DOCK.margin - pw / 2 } } do
            if math.abs(d[2] - c) < dist then best, dist = d[1], math.abs(d[2] - c) end
        end
        return best
    end
    -- The full row's layout (navbar.css .nb-full): 6pt padding, 6pt gaps, the 82pt history group, the address
    -- slot (flex 1 1 560, 120 to 600) and the extensions group.
    local pinnedCount = state("pinned_count", #saved.pinned)
    local function rowLayout()
        local fw = finalWidth()
        local n = pinnedCount:get()
        local right = n > 0 and 28 * n + 26 or 28
        local slot = math.clamp(fw - 12 - 82 - right - 12, 120, 600)
        return { fw = fw, slot = slot, right = right, extRight = 6 + 82 + 6 + slot + 6 + right }
    end
    -- Where the full controls sit inside the surface (anchored to the dock side, so they never slide).
    local function align() local d = dock:get(); return d == "left" and 0 or d == "right" and 1 or 0.5 end
    -- The address slot's rect in the window, as getBoundingClientRect reads it: under the bar's shrink and lift.
    local function fieldBounds()
        local box, L = navBox(), rowLayout()
        local m, k = morph:get(), clamp01(openM:get())
        local left, top = box.x + (box.w - L.fw) * align(), box.y + (box.h - 40) / 2 - 4 * k
        local s = m > 0 and 1 - 0.03 * m or 1
        local cx, cy = left + L.fw / 2, top + 20
        local x, y = left + 94, top + 6
        return { x = cx + (x - cx) * s, y = cy + (y - cy) * s, w = L.slot * s, h = 28 * s }
    end
    local function expandedFieldRect()
        local L = rowLayout()
        return { x = anchorX(dock:peek(), L.fw) + 94, y = DOCK.margin + 6, w = L.slot, h = 28 }
    end
    -- The open omnibox: centred like the bar, sharing its top edge.
    local function openRect()
        local width = math.min(SHEET_WIDTH, math.max(0, W:get() - 16))
        return { x = (W:get() - width) / 2, y = 8, w = width, h = math.max(0, Hh:get() - 8 - 16) }
    end

    local resultsEl
    local function staggerRows()
        staggerOpen = true
        if staggerTimer then staggerTimer:cancel() end
        staggerTimer = later(700, function() staggerOpen = false end)
        if not resultsEl or reduced:peek() then return end
        anime.animate(".nb-res", { opacity = { 0, 1 }, y = { 4, 0 }, duration = S(220), ease = EASE.OUT,
            delay = anime.stagger(S(16), { start = S(OPEN.stagger) }) })
    end

    -- address.rs entering Ask, and the open transition (OPEN).
    ui.effect(function()
        local open, m = focused:get(), modeK:get()
        local askNow = open and m == "ask"
        if askNow and not fx.ask then enterAsk() end
        fx.ask = askNow
        if open ~= fx.focused then
            fx.focused = open
            hoverGen += 1
            local token = hoverGen
            if open then
                collapsed:set(false)
                fx.askTarget = m == "ask" and 1 or 0
                to(askM, fx.askTarget, 0)
                staggerRows()
            else
                later(H.collapse, function()
                    if hoverGen == token and not focused:peek() and not hover:peek() and docScroll:peek() > H.top then collapsed:set(true) end
                end)
            end
            local spec = OPEN[open and "open" or "close"]
            to(openM, open and 1 or 0, R(spec[1]), spec[2])
        end
        -- Search <-> Ask, only while open; closing keeps whichever side is showing.
        local target = m == "ask" and 1 or 0
        if open and target ~= fx.askTarget then fx.askTarget = target; to(askM, target, R(MODE.ms), MODE.ease) end
    end)
    -- floating.rs: pill <-> full navigator. Hover never expands the pill; a click or scrolling back up does.
    ui.effect(function()
        local compact = dragActive:get() or (collapsed:get() and not focused:get())
        if compact == fx.compact then return end
        fx.compact = compact
        morphing = true
        to(morph, compact and 1 or 0, R(SHAPE.ms), SHAPE.ease, function()
            morphing = false
            if compact then addrSerial += 1; addrHover = false; to(hoverM, 0, 0)
            else syncAddrHover(true, fieldBounds()) end
        end)
    end)
    -- popovers.rs
    ui.effect(function()
        local p = popover:get()
        if p == fx.popover then return end
        fx.popover = p
        popSerial += 1
        local serial = popSerial
        if p ~= "" then
            popVisible:set(p)
            to(popM, 0, 0)
            to(popM, 1, reduced:peek() and 100 or 200, EASE.ENTER)
        else
            to(popM, 0, 100, EASE.EXIT)
            later(100, function() if popSerial == serial then popVisible:set("") end end)
        end
    end)
    -- identity.rs crossfade, navigation.rs reload/stop swap.
    ui.effect(function()
        pageRev:get()
        if P.url == "" then return end
        if fx.url == nil then fx.url = P.url; oldUrl:set(P.url); newUrl:set(P.url); fx.loading = P.loading end
        if P.url ~= fx.url then
            oldUrl:set(fx.url); fx.url = P.url; newUrl:set(P.url)
            to(idM, 0, 0); to(idM, 1, R(140), EASE.OUT)
        end
        if P.loading ~= fx.loading then fx.loading = P.loading; to(swapM, fx.loading and 1 or 0, R(140), EASE.OUT) end
    end)
    -- The sheet's height eases to its content while it shows; hidden, it snaps.
    ui.effect(function()
        local h = sheetH:get()
        if h <= 0 then return end
        if focused:peek() or openM:peek() > 0.001 then to(heightM, h, R(140), EASE.OUT) else to(heightM, h, 0) end
    end)
    -- Scroll intent: the bar tucks into the pill scrolling down and comes back scrolling up.
    ui.effect(function()
        local y = docScroll:get()
        local collapse = scroll.sample(y)
        if collapse ~= nil and not focused:peek() and not (collapse and hover:peek()) then collapsed:set(collapse) end
    end)

    -- floating.rs floating_hover: shared by the navigator and the field.
    local function hoverIntent(entering)
        if entering and drag then return end
        hoverGen += 1
        local token = hoverGen
        later(entering and H.hover or H.collapse, function()
            if hoverGen ~= token then return end
            hover:set(entering)
            if not entering and docScroll:peek() > H.top and not focused:peek() then collapsed:set(true) end
        end)
    end

    local derivedOpen = function() return clamp01(openM:get()) end
    local nav, dockLayer, extIcon, field, searchOp, hintButton, sheet, panel, place, composer, popHost, aura, agentBar, toast, backdrop, scrim
    ---------- Views ----------
    do

        -- Buttons that act inside the bar: pressing one must not start a drag.
        local buttonPressed = false
        local function tb(props)
            props.p = 0
            props.w = props.w or 26
            props.h = props.h or 26
            props.rounded = props.rounded or 6
            props.items = "center"
            props.justify = "center"
            props.bg = props.bg or C.clear
            props.cursor = "pointer"
            local press = props.on_press
            props.on_press = function() buttonPressed = true; if press then press() end end
            return T.button(props)
        end

        -- Reload <-> stop: the glyphs rotate a quarter turn past each other.
        local reloadButton = tb { id = "nb_reload", hover = { bg = C.selected },
            on_click = function() command("reload") end,
            ui.overlay { w = 15, h = 15,
                ui.div { absolute = true, left = 0, top = 0, opacity = function() return 1 - swapM:get() end,
                    rotate = function() return swapM:get() * 90 end, icon("rotate-cw", 15, C.secondary) },
                ui.div { absolute = true, left = 0, top = 0, opacity = function() return swapM:get() end,
                    rotate = function() return (swapM:get() - 1) * 90 end, icon("square", 15, C.secondary) } },
        }
        local function historyButton(id, glyph, can, c)
            return tb { id = id, hover = { bg = C.selected },
                opacity = function() pageRev:get(); return P[can] and 1 or 0.35 end,
                disabled = function() pageRev:get(); return not P[can] end,
                on_click = function() command(c) end, icon(glyph, 15, C.secondary) }
        end
        function extIcon(x, size)
            if x.img then return ui.image { src = "assets/" .. x.img, w = size, h = size, rounded = 3 } end
            return ui.overlay { w = size, h = size, rounded = 3, bg = x.tile[1], items = "center", justify = "center",
                border = 0.5, border_color = "#FFFFFF2E",
                T.text { x.tile[2], text_size = math.floor(size * 0.64 + 0.5), weight = 600, color = x.tile[3], line_height = size } }
        end
        local pinnedView = ui.presence(ui.computed(function() savedRev:get(); return table.concat(saved.pinned, ",") end), function()
            local items = {}
            for _, id in saved.pinned do
                local x = model.ext(id)
                if x then
                    table.insert(items, tb { id = "nb_ext_tb_" .. id, hover = { bg = C.selected },
                        on_click = function() notice(x.name) end, extIcon(x, 16) })
                end
            end
            return ui.row { gap = 2, items = "center", shrink = 0, items }
        end)
        local extButton = tb { id = "nb_ext", hover = { bg = C.selected },
            bg = function() return popover:get() == "extensions" and C.selected or C.clear end,
            on_click = function()
                focused:set(false); if inputEl then inputEl:blur() end
                popover:set(popover:peek() == "extensions" and "" or "extensions"); rebuildPop()
            end,
            icon("puzzle", 15, C.secondary) }

        local full = ui.row { id = "nb_full", absolute = true, h = 40, px = 6, gap = 6, items = "center",
            w = function() return rowLayout().fw end,
            left = function() local b = navBox(); return (b.w - rowLayout().fw) * align() end,
            top = function() return (navBox().h - 40) / 2 end,
            opacity = function() return 1 - smooth(0, 0.55, morph:get()) end,
            scale = function() return 1 - 0.03 * morph:get() end,
            blur = function() local m = morph:get(); return (m > 0 and m < 1) and 2 * m or 0 end,
            invisible = function() return 1 - smooth(0, 0.55, morph:get()) <= 0 end,
            ui.row { gap = 2, items = "center", shrink = 0,
                historyButton("nb_back", "arrow-left", "canGoBack", "back"),
                historyButton("nb_fwd", "arrow-right", "canGoForward", "forward"),
                reloadButton },
            ui.div { id = "nb_slot", h = 28, w = function() return rowLayout().slot end, shrink = 0 },
            ui.row { gap = 2, items = "center", shrink = 0, pinnedView, extButton },
        }

        local pillContent = ui.presence(ui.computed(function() pageRev:get(); return P.url .. "|" .. (P.favicon or "") end), function()
            local host = model.displayParts(P.url)
            return ui.row { gap = 8, items = "center",
                P.favicon and ui.image { src = P.favicon, w = 13, h = 13, rounded = 3 } or icon("globe", 13, C.text),
                T.text { host ~= "" and host or "Ask", text_size = 12, weight = 500, color = C.text } }
        end)
        local function pillOp() return smooth(0.4, 1, morph:get()) end
        local pill = ui.row { id = "nb_pill", h = 30, px = 10, rounded = 10, items = "center",
            on_resize = function(size) if size.w > 0 then pillW:set(size.w) end end,
            scale = function() return 0.94 + 0.06 * pillOp() end,
            blur = function() local o = pillOp(); return (o > 0 and o < 1) and 2 * (1 - o) or 0 end,
            pillContent }
        local pillRow = ui.row { absolute = true, left = 0, top = 0, w_full = true, items = "center",
            justify = function() local a = align(); return a == 0 and "start" or a == 1 and "end" or "center" end,
            h = function() return navBox().h end, opacity = pillOp, invisible = function() return pillOp() <= 0 end,
            pill }

        -- Drag to move (DOCK): any empty part of the full bar, or the pill.
        local function navPress(e)
            if buttonPressed then buttonPressed = false; return end
            if focused:peek() then return end
            local box = navBox()
            local px, py = box.x + e.local_x, box.y + e.local_y
            -- The pill is part of the bar's surface here so a drag that starts on it keeps the pointer.
            local pw = pillW:peek()
            local pillX = box.x + (box.w - pw) * align()
            local onPill = pillOp() > 0.5 and px >= pillX and px <= pillX + pw
            drag = { sx = px, sy = py, active = false, onPill = onPill, fx = 0, fy = 0 }
            pointer = { px, py }
        end
        local function navMove(e)
            local box = navBox()
            local px, py = box.x + e.local_x, box.y + e.local_y
            pointer = { px, py }
            if not drag then return end
            dragX:set(px); dragY:set(py)
            if not drag.active and math.sqrt((px - drag.sx) ^ 2 + (py - drag.sy) ^ 2) > DOCK.threshold then
                -- Grab point as a fraction of the bar, so the bar shrinks toward the pointer.
                local box = navBox()
                drag.fx = clamp01((drag.sx - box.x) / box.w)
                drag.fy = clamp01((drag.sy - box.y) / box.h)
                drag.active = true
                hoverGen += 1
                to(offX, 0, 0); to(offY, 0, 0)
                dragActive:set(true)
            end
        end
        local function navRelease()
            if not drag then return end
            if drag.active then
                local box = navBox()
                local d = nearestDock(box)
                dragActive:set(false)
                dock:set(d)
                -- Re-anchor to the new dock and spring the remaining offset away.
                to(offX, box.x - anchorX(d, box.w), 0); to(offY, box.y - DOCK.margin, 0)
                to(offX, 0, R(DOCK.settle[1]), DOCK.settle[2]); to(offY, 0, R(DOCK.settle[1]), DOCK.settle[2])
                -- Stays the pill until clicked or the page scrolls back up.
                hoverGen += 1; hover:set(false); collapsed:set(true)
            elseif drag.onPill then
                syncAddrHover(false, expandedFieldRect())
                collapsed:set(false)
            elseif not collapsed:peek() then
                -- A click on the expanded bar's background tucks it into the pill, like a drop does.
                hoverGen += 1; hover:set(false); collapsed:set(true)
            end
            drag = nil
        end

        nav = ui.overlay { id = "nb_nav", absolute = true, overflow = "hidden", backdrop_blur = 24, pointer_events = true,
            bg = "#1C1C1AEB", border = 1,
            border_color = function() return dragActive:get() and "#3A3934" or C.strong end,
            shadow = function()
                return dragActive:get() and { y = 16, blur = 32, color = "#00000080" } or { y = 10, blur = 28, color = "#00000073" }
            end,
            cursor = function() return dragActive:get() and "grabbing" or "grab" end,
            left = function() return navBox().x end, top = function() return navBox().y end,
            w = function() return navBox().w end, h = function() return navBox().h end,
            rounded = function() return lerp(12, 10, morph:get()) end,
            opacity = function() local k = derivedOpen(); return k > 0 and 1 - k or 1 end,
            y = function() return -4 * derivedOpen() end,
            on_hover = hoverIntent,
            on_press = navPress, on_pointer_move = navMove, on_release = navRelease,
            full, pillRow,
        }
        local docks = {}
        for _, d in DOCKS do
            table.insert(docks, ui.div { absolute = true, top = 8, h = 30, rounded = 10, border = 1, border_dash = { 3, 3 },
                w = function() return pillW:get() end, left = function() return anchorX(d, pillW:get()) end,
                border_color = function()
                    return dragActive:get() and nearestDock(navBox()) == d and "#5A5850" or "#3A3934"
                end,
                bg = function() return dragActive:get() and nearestDock(navBox()) == d and "#1C1C1A99" or "#1C1C1A40" end })
        end
        dockLayer = ui.overlay { absolute = true, left = 0, top = 0, w_full = true, h_full = true,
            opacity = eased(function() return dragActive:get() and 1 or 0 end, 140, EASE.OUT),
            docks }

    end
    ---------- Address field ----------
    do
        local function mixHover(h) return mix(C.panel, C.elevated, h) end
        -- The field's rect: at rest it is the bar's slot; open, the omnibox header.
        local function fieldRect()
            if focused:get() and derivedOpen() > 0 then
                local o = openRect()
                return { x = o.x, y = o.y, w = o.w, h = 40 }
            end
            return fieldBounds()
        end
        local function fullOp() return 1 - smooth(0, 0.55, morph:get()) end
        function searchOp() return clamp01(1 - askM:get() / 0.6) end
        local function fieldOpacity()
            local k, t = derivedOpen(), openM:get()
            local op = 1
            if k > 0 then
                if focused:get() then op = clamp01(t * 1.6) * searchOp() else op = 1 - k end
            end
            if not focused:get() then op *= fullOp() end
            return op
        end
        local function fieldHidden()
            local f, a, k = focused:get(), askM:get(), derivedOpen()
            return (f and modeK:get() == "ask" and a >= 0.999) or (not f and k <= 0.001 and fullOp() <= 0.001)
        end

        -- Identity (identity.rs): host and path tokens; on address hover the raw URL grows out of the gaps.
        local function identity(url, slot)
            local tokens = model.identity(url)
            if not tokens then
                return ui.row { w_full = true, h = 22, items = "center",
                    T.text { "Search or type a URL", text_size = 12, color = C.muted, truncate = true } }
            end
            local h = function() return model.isEmptyPage(P.url) and 0 or hoverM:get() end
            local children = {}
            for n, t in tokens do
                if t.kind == "host" then
                    table.insert(children, T.text { t.text, text_size = 12.5, line_height = 16, color = C.text, shrink = 0,
                        weight = function() return math.floor(lerp(500, 400, h()) + 0.5) end })
                elseif t.kind == "part" then
                    table.insert(children, T.text { t.text, text_size = 12.5, line_height = 16, truncate = true, min_w = 0, shrink = 1,
                        color = function() local v = h(); return v > 0 and mix(C.secondary, C.text, v) or C.secondary end })
                else
                    local widths = ui.state("nb_gap_" .. slot .. "_" .. n, { 0, 0 })
                    local right = t.kind == "prefix"
                    local function textNode(label, color, index, opacity)
                        return T.text { label, text_size = 12.5, line_height = 16, color = color, absolute = true, top = 0,
                            left = not right and 0 or nil, right = right and 0 or nil, opacity = opacity,
                            on_resize = function(size)
                                local wv = widths:peek()
                                if wv[index] ~= size.w then
                                    local nextW = { wv[1], wv[2] }
                                    nextW[index] = size.w
                                    widths:set(nextW)
                                end
                            end }
                    end
                    table.insert(children, ui.overlay { h = 16, shrink = 0, overflow = "hidden",
                        w = function() local wv = widths:get(); return lerp(wv[1], wv[2], h()) end,
                        mx = function() return t.kind == "sep" and 6 * (1 - h()) or 0 end,
                        textNode(t.i, C.faint, 1, function() return 1 - h() end),
                        textNode(t.r, C.text, 2, h) })
                end
            end
            return ui.row { w_full = true, h = 22, items = "center", overflow = "hidden", children }
        end
        local identityView = ui.overlay { id = "nb_identity", absolute = true, left = 0, top = 0, w_full = true, h_full = true,
            cursor = "text", keep_focus = true,
            opacity = function() return focused:get() and 0 or 1 end,
            on_press = function()
                focusNav(entry().url); refresh(); syncInput()
                if inputEl then inputEl:focus(); inputEl:select_all() end
            end,
            ui.overlay { absolute = true, left = 0, top = 0, w_full = true, h_full = true,
                opacity = function() return 1 - idM:get() end, invisible = function() return idM:get() >= 1 end,
                ui.presence(oldUrl, function(url) return identity(url, "old") end) },
            ui.overlay { absolute = true, left = 0, top = 0, w_full = true, h_full = true,
                opacity = function() return idM:get() end,
                ui.presence(newUrl, function(url) return identity(url, "new") end) },
        }

        -- The editor. Its events run before its own key handling, so returning true keeps the caret still.
        local input = ui.input { id = "nb_input", label = "Address and search bar", value = inputValue,
            absolute = true, left = 0, top = 0, w_full = true, h_full = true, p = 0, border = 0, bg = C.clear,
            focus = { border = 0 }, text_size = 13.5, line_height = 17.55, color = C.text, selection_color = "#393933",
            invisible = function() return not focused:get() end,
            on_mount = function(el) inputEl = el end,
            on_focus = function() if not focused:peek() then focusNav(P.url); refresh(); syncInput() end end,
            on_key = function(e)
                local k = e.key
                if k == "Enter" then submitAddress(); return true end
                if k == "ArrowDown" then step(1); return true end
                if k == "ArrowUp" then step(-1); return true end
                if k == "Escape" then
                    if nb.mode.k ~= "search" then setMode({ k = "search" }); nb.input = ""; syncInput(); navbarChanged("")
                    else dismiss() end
                    return true
                end
                if k == "Tab" and not e.shift and nb.mode.k == "search" then
                    local v = model.trim(inputValue:peek())
                    if v == "@tabs" then setMode({ k = "tabs" })
                    elseif v == "github" or v == "youtube" or v == "wikipedia" or (string.find(v, ".", 1, true) and not string.find(v, "/", 1, true)) then
                        setMode({ k = "site", site = v })
                    else setMode({ k = "ask" }) end
                    if nb.mode.k == "ask" then enterAsk(); fx.ask = true end
                    nb.input = ""; syncInput(); nb.edited = true; refresh()
                    return true
                end
                if k == "Backspace" and inputValue:peek() == "" and nb.mode.k ~= "search" then
                    setMode({ k = "search" }); refresh(); return true
                end
                if e.meta and string.lower(k) == "l" then dismiss(); return true end
                return nil
            end,
        }
        -- address.rs: the completion is inserted and selected in the same batch as typing.
        ui.effect(function()
            local value = inputValue:get()
            if value == prevValue or not focused:peek() then return end
            if not (completionState and completionState.full == value) then
                local grew = #value > #prevValue or not model.starts(prevValue, value)
                    or (completionState ~= nil and #value > #completionState.prefix)
                completionState = nil
                navbarChanged(value)
                local full = grew and model.completion(nb, nb.suggestions)
                if full then
                    completionState = { prefix = value, full = full }
                    nb.input = full
                    inputValue:set(full)
                    if inputEl then inputEl:set_selection_bytes(#value, #full) end
                    prevValue = full
                    return
                end
            end
            prevValue = value
        end)

        -- navbar.css .nb-fb: 22pt squares whose fill and ink brighten together over 90ms on hover (and stay lit
        -- while .on), pressed at .94.
        local function fb(id, glyph, props)
            local hovered = ui.state(id .. "_hover", false)
            local on = props.on or function() return false end
            local t = eased(function() return (hovered:get() or on()) and 1 or 0 end, 90, EASE.OUT)
            -- The glyph in tertiary ink, cross-fading to primary.
            local function ink(name)
                return ui.overlay { w = 14, h = 14,
                    ui.div { absolute = true, opacity = function() return 1 - t:get() end, icon(name, 14, C.tertiary) },
                    ui.div { absolute = true, opacity = t, icon(name, 14, C.text) } }
            end
            local content = props.content and props.content(ink) or ink(glyph)
            props.id = id
            props.p = 0
            props.w, props.h, props.rounded = 22, 22, 6
            props.items, props.justify = "center", "center"
            props.cursor = "pointer"
            props.keep_focus = true
            props.bg = C.clear
            props.active = { scale = 0.94 }
            props.on = nil
            props.content = nil
            props.on_hover = function(inside) hovered:set(inside) end
            table.insert(props, ui.div { absolute = true, left = 0, top = 0, w = 22, h = 22, rounded = 6, bg = C.selected, opacity = t })
            table.insert(props, content)
            return T.button(props)
        end
        local permButton = fb("nb_perm_btn", "video", {
            hidden = function() pageRev:get(); return focused:get() or not P.permission end,
            on_click = function() popover:set("permission"); focused:set(false); if inputEl then inputEl:blur() end; rebuildPop() end })
        local readerButton = fb("nb_reader", "book-open-text", {
            on = function() pageRev:get(); return P.tabReader == true end,
            hidden = function() pageRev:get(); return focused:get() or not P.reader end,
            on_click = function() activeTab().reader = not activeTab().reader; renderDoc(true); syncPage() end })
        local starButton = fb("nb_star", "star", {
            hidden = function() pageRev:get(); return focused:get() or model.isEmptyPage(P.url) end,
            on_click = function()
                local newly = bookmarked() == nil
                if newly then toggleBookmark() end
                focused:set(false); if inputEl then inputEl:blur() end
                popover:set("bookmark")
                if newly and not reduced:peek() then
                    to(bounce, 1.15, 100, EASE.OUT, function() to(bounce, 1, 100, EASE.OUT) end)
                end
            end,
            -- Saved, the star fills in its own colour; otherwise it takes the button's ink.
            content = function(ink)
                return ui.overlay { w = 14, h = 14, scale = bounce,
                    ui.presence(ui.computed(function() savedRev:get(); pageRev:get(); return bookmarked() ~= nil end), function(marked)
                        return marked and T.filled("star", 14, "#C9C2A8") or ink("star")
                    end) }
            end })

        -- loading.rs wash: neutral dots, the leading edge shading toward the logo's colour once its icon resolves.
        local washEl
        local washInk = { color = nil, target = 0 }
        local function washTint()
            local fav = P.favicon and string.match(P.favicon, "[^/]+$")
            local tint = P.iconResolved and fav and tints[fav] or nil
            if not tint then return nil end
            local r, g, b = T.rgba(tint)
            local c = { r, g, b }
            for i = 1, 3 do c[i] = math.floor(WASH_NEUTRAL[i] + (c[i] - WASH_NEUTRAL[i]) * WASH_TINT + 0.5) end
            return c
        end
        ui.effect(function()
            pageRev:get()
            local p = progress:peek()
            local tint = washTint()
            if tint then washInk.color = tint end
            local target = tint and 1 or 0
            if not (p > 0) then
                to(washA, target, 0)
            elseif washInk.target ~= target then
                to(washA, target, R(WASH_FADE), "outCubic")
            end
            washInk.target = target
        end)
        local washTicker
        ui.effect(function()
            local p, s, a, base = progress:get(), settle:get(), washA:get(), fieldBounds()
            if not washEl then return end
            local fade = math.max(0, 1 - s / 6)
            local c = washInk.color or WASH_NEUTRAL
            anime.utils.set(washEl, {
                shaderParam4 = c[1] / 255, shaderParam5 = c[2] / 255, shaderParam6 = c[3] / 255, shaderParam7 = washInk.color and a or 0,
                shaderParam8 = 0.4 * fade, shaderParam9 = 0.16 * fade, shaderParam11 = base.w * p,
            })
            -- The dots re-seed on the 12 fps dither clock while the bar shows.
            local on = p > 0 and not reduced:peek()
            if on and not washTicker then
                washTicker = anime.animate(washEl, { shaderTime = { 0, 3600 }, duration = 3600000, ease = "linear", frameRate = 12, loop = true })
            elseif not on and washTicker then
                washTicker:cancel(); washTicker = nil
            end
        end)
        local wash = ui.div { id = "nb_wash", absolute = true, left = 0, top = 0, h_full = true, rounded = 8, pointer_events = false,
            w = function() return fieldBounds().w end,
            hidden = function() return not (progress:get() > 0) or focused:get() end,
            clip_inset = function() local b = fieldBounds(); return { 0, b.w * (1 - progress:get()), 0, 0 } end,
            bg_shader = dither.wash,
            shader_params = { "#3A3933FF", { 0, 0, 0, 0 }, { 0.4, 0.16, 7, 0 }, { 0, 0, 0, 0 } },
            on_mount = function(el) washEl = el end }

        -- The site icon: favicon, orb, skeleton while loading, or a glyph.
        local skeletonParams = dither.params(C.text, { mode = 0, pixel = 1, angle = 90, from = 0.4, to = 0.15, curve = 0.8,
            noise = 0.25, scale = 3, seed = 7, speed = 12 })
        local function logoKey()
            pageRev:get()
            return table.concat({ tostring(focused:get()), P.security, tostring(P.online), P.favicon or "", tostring(P.loading),
                tostring(model.isEmptyPage(P.url)) }, "|")
        end
        local logo = ui.presence(ui.computed(logoKey), function()
            local f, empty = focused:peek(), model.isEmptyPage(P.url)
            if empty and not f then
                return ui.overlay { w = 22, h = 22, items = "center", justify = "center", icon("search", 12, C.muted) }
            end
            local glyph = f and "search" or not P.online and "wifi-off" or P.security == "insecure" and "triangle-alert" or "globe"
            local inner
            if not f and P.security == "internal" then
                inner = ui.div { w = 14, h = 14, bg_shader = dither.fill, shader_params = dither.params(C.text, dither.ORB) }
            elseif not f and P.loading and P.online and P.security ~= "insecure" then
                inner = ui.div { w = 14, h = 14, rounded = 3, bg_shader = dither.fill, shader_params = skeletonParams,
                    on_mount = function(el)
                        -- The skeleton eases in rather than popping over the favicon, then steps at 12 fps.
                        anime.animate(el, { opacity = { 0, 1 }, duration = S(180), ease = EASE.OUT })
                        if not reduced:peek() then
                            anime.animate(el, { shaderTime = { 0, 3600 }, duration = 3600000, ease = "linear", frameRate = 12, loop = true })
                        end
                    end }
            elseif not f and P.online and P.security ~= "insecure" and P.favicon then
                inner = ui.image { src = P.favicon, w = 14, h = 14, rounded = 3 }
            else
                inner = icon(glyph, 14, P.security == "insecure" and not f and "#C9A78B" or C.text)
            end
            return T.button { id = "nb_logo", p = 0, w = 22, h = 22, rounded = 6, items = "center", justify = "center",
                bg = C.clear, hover = { bg = C.selected }, cursor = "pointer",
                on_click = function()
                    focused:set(false); if inputEl then inputEl:blur() end
                    popover:set(popover:peek() == "site" and "" or "site"); rebuildPop()
                end,
                inner }
        end)
        local chip = ui.presence(ui.computed(function()
            local m = modeK:get()
            return (focused:get() and m ~= "search") and (m == "ask" and "Ask" or m == "tabs" and "@tabs" or nb.mode.site or "") or ""
        end), function(label)
            if label == "" then return ui.div {} end
            return ui.row { h = 20, px = 6, rounded = 5, bg = C.strong, items = "center", shrink = 0,
                T.text { label, text_size = 12, color = C.text } }
        end)
        function hintButton(id, kind, keys, label, bold)
            local hovered = ui.state(id .. "_hover", false)
            local ink = eased(function() return hovered:get() and 1 or 0 end, 90, EASE.OUT)
            return T.button { id = id, p = 0, h = 24, gap = 6, items = "center", bg = C.clear, cursor = "pointer", keep_focus = true,
                direction = "row",
                on_hover = function(inside) hovered:set(inside) end,
                on_click = function()
                    if kind == "focus" then focusAddress(); return end
                    local toAsk = kind == "ask"
                    setMode(toAsk and { k = "ask" } or { k = "search" })
                    nb.input = ""; syncInput(); refresh()
                    if toAsk then enterAsk() else focusInputSoon() end
                end,
                key(keys, true),
                label and ui.overlay {
                    ui.row { gap = 3, items = "center", opacity = function() return 1 - ink:get() end,
                        txt(label, 11.5, C.muted), bold and T.text { bold, text_size = 11.5, weight = 500, color = C.secondary } or nil },
                    ui.row { absolute = true, left = 0, top = 0, gap = 3, items = "center", opacity = ink,
                        txt(label, 11.5, C.text), bold and T.text { bold, text_size = 11.5, weight = 500, color = C.text } or nil },
                } or nil }
        end
        local hintSlot = ui.presence(ui.computed(function()
            pageRev:get()
            local f, m = focused:get(), modeK:get()
            if not f then return model.isEmptyPage(P.url) and "focus" or "" end
            return m ~= "search" and "search" or "ask"
        end), function(kind)
            if kind == "" then return ui.div {} end
            if kind == "focus" then return ui.row { mr = 2, hintButton("nb_hint_focus", "focus", "⌘ L") } end
            if kind == "search" then return hintButton("nb_hint_search", "search", "Esc", "Back to", "Search") end
            return hintButton("nb_hint_ask", "ask", "Tab", "to", "Ask")
        end)

        field = ui.overlay { id = "nb_field", absolute = true, overflow = "hidden",
            pointer_events = function() return not (focused:get() and askM:get() > 0.5) end,
            left = function() return fieldRect().x end, top = function() return fieldRect().y end,
            w = function() return fieldRect().w end, h = function() return fieldRect().h end,
            hidden = fieldHidden,
            opacity = fieldOpacity,
            origin = { 0.5, 0 },
            y = function()
                if not focused:get() or derivedOpen() <= 0 then return 0 end
                return -8 * (1 - openM:get()) - MODE.slide * askM:get()
            end,
            scale = function()
                if not focused:get() or derivedOpen() <= 0 then return 1 end
                return 0.96 + 0.04 * openM:get()
            end,
            corners = function()
                local k = derivedOpen()
                local top, bottom = lerp(8, 12, k), lerp(8, 0, k)
                return { top, top, bottom, bottom }
            end,
            bg = function()
                local k, rest = derivedOpen(), mixHover(hoverM:get())
                return k > 0 and mix(rest, C.elevated, k) or rest
            end,
            border = 1,
            border_sides = function() return derivedOpen() > 0 and { 1, 1, 0, 1 } or { 1, 1, 1, 1 } end,
            border_color = function() return mix(C.default, C.strong, derivedOpen()) end,
            -- At rest the field lives inside the bar: clipped to the bar so it never shows outside it mid-morph.
            clip_inset = function()
                if focused:get() then return { 0, 0, 0, 0 } end
                local b, r = navBox(), fieldRect()
                local k = derivedOpen()
                local by = b.y - 4 * k
                return { math.max(0, by - r.y), math.max(0, r.x + r.w - b.x - b.w), math.max(0, r.y + r.h - by - b.h), math.max(0, b.x - r.x) }
            end,
            on_hover = function(inside)
                hoverIntent(inside)
                -- While the bar morphs the field slides under a still pointer; the hover settles once it ends.
                if morphing then return end
                addrSerial += 1
                local serial = addrSerial
                if inside then
                    later(H.addressHover, function()
                        if addrSerial == serial and not focused:peek() then addrHover = true; to(hoverM, 1, R(URLM.ms), URLM.ease) end
                    end)
                else
                    addrHover = false
                    to(hoverM, 0, R(URLM.ms), URLM.ease)
                end
            end,
            on_pointer_move = function(e) local r = fieldRect(); pointer = { r.x + e.local_x, r.y + e.local_y } end,
            ui.div { absolute = true, left = 0, right = 0, bottom = 0, h = 1, bg = C.default,
                hidden = function() return derivedOpen() <= 0 end, opacity = function() return 1 - derivedOpen() end },
            ui.row { id = "nb_fc", absolute = true, left = 1, top = 1, gap = 6, items = "center",
                w = function() return (focused:get() and openRect().w or fieldBounds().w) - 2 end,
                h = function() return (focused:get() and 40 or fieldBounds().h) - 2 end,
                pl = function() return focused:get() and 12 or 4 end,
                pr = function() return focused:get() and 16 or 2 end,
                wash,
                ui.row { mr = function() return focused:get() and 0 or -2 end, shrink = 0, logo },
                ui.row { shrink = 0, hidden = function() return not focused:get() or modeK:get() == "search" end, chip },
                ui.overlay { grow = 1, shrink = 1, basis = 0, min_w = 0, h = 22, identityView, input },
                ui.row { gap = 2, items = "center", shrink = 0,
                    hidden = function()
                        pageRev:get()
                        return focused:get() or not (P.permission or P.reader or not model.isEmptyPage(P.url))
                    end,
                    permButton, readerButton, starButton },
                ui.row { shrink = 0, hidden = function() pageRev:get(); return not focused:get() and not model.isEmptyPage(P.url) end,
                    hintSlot },
            },
        }

    end
    ---------- Suggestion sheet ----------
    do
        local hoveredRow = state("hovered_row", -1)
        local function group(title)
            return ui.row { class = "nb-res", h = 25, px = 10, items = "center", shrink = 0,
                T.text { title, font = "Geist Mono", text_size = 10, letter_spacing = 0.6, color = C.muted } }
        end
        local function resultsKey()
            sugRev:get(); pageRev:get()
            local parts = { nb.mode.k, nb.mode.site or "", tostring(P.online), tostring(nb.edited) }
            for _, r in nb.suggestions do table.insert(parts, r.kind .. ":" .. r.title .. ":" .. r.detail) end
            return table.concat(parts, "\n")
        end
        -- Ask mode freezes the results, so the outgoing list does not swap to Ask rows while it fades.
        local frozenKey = ""
        local resultsState = ui.computed(function()
            local k = resultsKey()
            if modeK:get() ~= "ask" then frozenKey = k end
            return frozenKey
        end)
        local results = ui.presence(resultsState, function()
            local rows, mode = nb.suggestions, nb.mode
            local children = {}
            if #rows == 0 then
                local empty = mode.k == "tabs" and "No matching open tabs" or mode.k == "ask" and "Ask about this page" or "No local matches"
                table.insert(children, ui.column { class = "nb-res", h = 110, gap = 8, items = "center", justify = "center",
                    icon("search", 20, C.muted), T.text { empty, text_size = 13, color = C.secondary },
                    txt(mode.k == "tabs" and "Try a different title or address" or "Type a query to search the web", 11, C.muted) })
            else
                local previous = nil
                for i, row in rows do
                    local index = i - 1
                    local title = (mode.k == "search" and not nb.edited) and ((row.kind == "ask" or row.kind == "copy") and "THIS PAGE" or "JUMP BACK IN")
                        or row.kind == "tab" and "OPEN TABS" or (row.kind == "history" or row.kind == "bookmark") and "HISTORY" or ""
                    if previous ~= title and title ~= "" then table.insert(children, group(title)) end
                    previous = title
                    table.insert(children, T.button { id = "nb_row_" .. index, class = "nb-res", p = 0, w_full = true, h = 34, px = 10,
                        gap = 10, rounded = 7, direction = "row", items = "center", shrink = 0, keep_focus = true, cursor = "pointer",
                        bg = function()
                            return selected:get() == index and C.selected or hoveredRow:get() == index and C.subtle or C.clear
                        end,
                        on_hover = function(inside)
                            if inside then hoveredRow:set(index) elseif hoveredRow:peek() == index then hoveredRow:set(-1) end
                        end,
                        on_click = function() accept(index) end,
                        ui.overlay { w = 20, h = 20, items = "center", justify = "center", shrink = 0, icon(model.GLYPH[row.kind], 14, C.secondary) },
                        T.text { row.title, text_size = 13, color = C.text, truncate = true, min_w = 0, shrink = 1 },
                        T.text { row.detail, text_size = 12.5, color = C.muted, truncate = true, min_w = 0, shrink = 1 },
                        ui.div { grow = 1, shrink = 1, basis = 0, min_w = 0 },
                        row.kind == "tab" and T.text { "Switch to tab", text_size = 10.5, color = C.tertiary, shrink = 0 } or key("↵"),
                    })
                end
            end
            if not P.online then
                table.insert(children, ui.row { class = "nb-res", h = 34, px = 10, gap = 10, items = "center", opacity = 0.55,
                    icon("wifi-off", 14, C.secondary), txt("Offline · web suggestions paused", 12, C.secondary) })
            end
            return ui.column { w_full = true, p = 6, gap = 1,
                on_mount = function(el)
                    resultsEl = el
                    -- Rows built during the open stagger window enter like the first ones.
                    if staggerOpen and not reduced:peek() then
                        anime.animate(".nb-res", { opacity = { 0, 1 }, y = { 4, 0 }, duration = S(220), ease = EASE.OUT,
                            delay = anime.stagger(S(16), { start = S(OPEN.stagger) }) })
                    end
                end,
                children }
        end)
        function panel(props)
            props.bg = C.elevated
            props.border = 1
            props.border_color = C.strong
            props.rounded = 12
            props.shadow = { { y = 16, blur = 20, color = "#00000080" }, { y = 2, blur = 3, color = "#0000004D" } }
            return ui.overlay(props)
        end
        local function surfaceH()
            local o = openRect()
            local bodyH = math.max(40, math.min(heightM:get(), o.h))
            return lerp(bodyH, math.min(cmpH:get(), o.h), askM:get())
        end
        function place(props)
            props.left = function() return openRect().x end
            props.top = function() return openRect().y end
            props.w = function() return openRect().w end
            props.h = surfaceH
            props.origin = { 0.5, 0 }
            props.y = function() return -8 * (1 - openM:get()) end
            props.scale = function() return 0.96 + 0.04 * openM:get() end
            props.absolute = true
            props.overflow = "hidden"
            return props
        end
        sheet = panel(place { id = "nb_sheet", pointer_events = true,
            invisible = function() return not (focused:get() or derivedOpen() > 0.001) end,
            opacity = function() return derivedOpen() > 0 and clamp01(openM:get() * 1.6) or 0 end,
            ui.column { id = "nb_sheet_content", absolute = true, left = 1, top = 1, pt = 39, w = function() return openRect().w - 2 end,
                on_resize = function(size) if size.h > 0 then sheetH:set(size.h) end end,
                opacity = searchOp,
                y = function() return -MODE.slide * askM:get() end,
                ui.div { h = 1, w_full = true, bg = C.default, shrink = 0 },
                results,
                ui.row { w_full = true, h = 34, px = 14, pt = 1, gap = 16, items = "center", shrink = 0, border = 1,
                    border_sides = { 1, 0, 0, 0 }, border_color = C.default,
                    hint("↑ ↓", "Select"), hint("↵", "Open"), hint("Tab", "Ask"), ui.div { grow = 1, basis = 0 },
                    txt("Search with DuckDuckGo", 10.5, C.muted) } },
        })

    end
    ---------- Ask composer ----------
    do
        local function seedOf(s)
            local a = 7
            for i = 1, #s do a = (a * 31 + string.byte(s, i)) % 997 end
            return a / 7.3
        end
        local function attachmentView(a)
            local host = (function()
                local h, parts = model.displayParts(a.url)
                return parts[1] and h .. "/" .. parts[1] or h
            end)()
            local tint = a.fav and tints[a.fav]
            local hovered = ui.state("att_hover_" .. a.id, false)
            local xOp = eased(function() return hovered:get() and 1 or 0 end, 90, EASE.OUT)
            local u = table.clone(DRIFT)
            u.seed = seedOf(a.url)
            if reduced:peek() then u.speed = 0 end
            return ui.overlay { h = 40, max_w = 280, rounded = 10, bg = "#1A1A18", border = 1, border_color = "#262521",
                on_hover = function(inside) hovered:set(inside) end,
                ui.div { absolute = true, left = 0, top = 0, w_full = true, h_full = true, rounded = 10,
                    bg_shader = dither.fill, shader_params = dither.params((tint or "#8A877E") .. "59", u),
                    on_mount = function(el)
                        if not reduced:peek() then
                            anime.animate(el, { shaderTime = { 0, 3600 }, duration = S(3600000), ease = "linear", frameRate = 30, loop = true })
                        end
                    end },
                ui.row { h_full = true, pl = 5, pr = 12, gap = 9, items = "center",
                    ui.overlay { w = 30, h = 30, rounded = 7, bg = "#121211", items = "center", justify = "center", shrink = 0,
                        a.fav and ui.image { src = "assets/" .. a.fav, w = 16, h = 16, rounded = 3 } or icon("app-window", 16, C.text) },
                    ui.column { gap = 2, min_w = 0, shrink = 1,
                        T.text { a.title, text_size = 12, weight = 500, color = C.text, truncate = true },
                        ui.row { gap = 5, items = "center",
                            T.text { "TAB", font = "Geist Mono", text_size = 9.5, weight = 500, letter_spacing = 0.4, color = C.secondary },
                            T.text { host, font = "Geist Mono", text_size = 10, color = C.muted, truncate = true } } } },
                T.button { p = 0, absolute = true, top = -6, right = -6, w = 18, h = 18, rounded = 9, bg = "#2A2926",
                    items = "center", justify = "center", cursor = "pointer", keep_focus = true,
                    shadow = { spread = 2, color = C.elevated }, hover = { bg = "#33322E" },
                    opacity = xOp, scale = function() return 0.85 + 0.15 * xOp:get() end,
                    on_click = function()
                        local list = {}
                        for _, x in attachments:peek() do if x.id ~= a.id then table.insert(list, x) end end
                        attachments:set(list)
                    end,
                    icon("x", 10, C.secondary) } }
        end
        local promptLines = ui.computed(function()
            local _, n = string.gsub(promptValue:get(), "\n", "")
            return math.min(6, n + 1)
        end)
        ui.effect(function()
            -- Read both first: a short-circuited read would leave the effect subscribed to nothing.
        local open, m = focused:get(), modeK:get()
        if focusComposer and open and m == "ask" and promptEl then
                focusComposer = false
                promptEl:focus()
                promptEl:set_selection_bytes(#promptValue:peek())
            end
        end)
        local toolButton = function(children)
            return T.button { p = 0, h = 28, min_w = 28, px = 6, gap = 3, rounded = 7, items = "center", justify = "center",
                direction = "row", bg = C.clear, hover = { bg = "#2A2926" }, cursor = "pointer", keep_focus = true, children }
        end
        local composerIn = ui.column { id = "nb_cmp_in", absolute = true, left = 0, top = 0, w_full = true,
            on_resize = function(size) if size.h > 0 then cmpH:set(size.h) end end,
            y = function() return MODE.slide * (1 - askM:get()) end,
            ui.show(function() return #attachments:get() > 0 end, ui.row { flex_wrap = true, gap = 8, pt = 10, px = 10,
                ui.each(attachments, attachmentView, { key = function(a) return a.id end }) }),
            ui.row { w_full = true, min_h = 40, gap = 10, items = "start", pt = 10, pr = 17, pb = 6, pl = 10,
                T.button { id = "nb_scope", p = 0, w = 54, h = 24, gap = 6, pl = 7, pr = 9, rounded = 6, border = 1,
                    border_color = C.strong, bg = "#1F1F1C", items = "center", direction = "row", cursor = "pointer", shrink = 0,
                    on_click = askBack,
                    ui.div { w = 12, h = 12, bg_shader = dither.fill, shader_params = dither.params(C.text, dither.ORB) },
                    T.text { "Ask", text_size = 12, weight = 500, color = C.text } },
                ui.overlay { grow = 1, shrink = 1, basis = 0, min_w = 0,
                    h = function() return promptLines:get() * 20 + 2 end,
                    T.text { "Ask about this page or attach context…", absolute = true, left = 0, top = 2, text_size = 13.5,
                        line_height = 20, color = C.muted, hidden = function() return promptValue:get() ~= "" end },
                    ui.input { id = "nb_prompt", label = "Ask", value = promptValue, multiline = true, absolute = true, left = 0,
                        top = 0, w_full = true, h_full = true, p = 0, pt = 2, border = 0, bg = C.clear, focus = { border = 0 },
                        text_size = 13.5, line_height = 20, color = C.text, selection_color = "#393933",
                        on_mount = function(el) promptEl = el end,
                        on_key = function(e)
                            if e.key == "Escape" then askBack(); return true end
                            if e.key == "Enter" and not e.shift then askSend(e.meta or e.ctrl); return true end
                            return nil
                        end } } },
            ui.row { w_full = true, gap = 6, items = "center", pt = 4, px = 10, pb = 10,
                toolButton({ icon("plus", 14, C.secondary) }),
                toolButton({ icon("briefcase-business", 14, C.secondary), icon("chevron-down", 11, C.secondary) }),
                ui.div { grow = 1, basis = 0 },
                T.button { id = "nb_send", p = 0, w = 28, h = 28, rounded = 8, bg = C.text, items = "center", justify = "center",
                    cursor = "pointer", on_click = function() askSend(false) end, icon("arrow-up", 14, C.panel) } },
            ui.row { w_full = true, h = 34, px = 14, pt = 1, gap = 16, items = "center", border = 1, border_sides = { 1, 0, 0, 0 },
                border_color = C.default,
                hint("↵", "Open in sidebar"), hint("⌘ ↵", "Run in background"), hint("⇧ ↵", "New line"), ui.div { grow = 1, basis = 0 },
                hintButton("nb_cmp_back", "search", "Esc", "Back to", "Search") },
        }
        local function askOp() return clamp01((askM:get() - 0.4) / 0.6) end
        composer = ui.overlay(place { id = "nb_composer",
            pointer_events = function() return focused:get() and modeK:get() == "ask" end,
            invisible = function()
                return not ((focused:get() or derivedOpen() > 0.001) and (modeK:get() == "ask" or askM:get() > 0.001))
            end,
            opacity = function() return math.max(0.001, clamp01(openM:get() * 1.6) * askOp()) end,
            composerIn,
        })

    end
    ---------- Popovers (popovers.rs, site.rs, bookmarks.rs) ----------
    do
        local subMenu = state("sub_menu", "")
        local folderOpen, folder, bmName = state("folder_open", false), state("folder", "Bookmarks"), state("bm_name", "")
        local function permissionLabel(name)
            local v = sitePermission(name)
            return name == "Act on pages" and v == "ask" and "Ask first" or model.PERMS[v]
        end
        -- The shared context-menu surface (pages.css .ctx): 30pt rows, Geist Mono section heads and one highlight per
        -- menu that slides to the hovered row (behavior.js ctxHighlight). Over a head, separator or padding it stays on
        -- the last row; over another menu's rows it goes off.
        local activeMenu = state("active_menu", "")
        local function ctxMenu(id, width, build, extra)
            local hlY = state(id .. "_hl_y", 0)
            local marker
            -- An element's offset from the menu's own origin for absolute children (navbar.js offsetTop).
            local function offset(el) return (marker and el and el.y and marker.y) and el.y - marker.y or 0 end
            local function head(label)
                return T.text { label, font = "Geist Mono", text_size = 10, color = C.muted, pt = 6, px = 8, pb = 4, truncate = true,
                    shrink = 0 }
            end
            local function sep() return ui.div { h = 1, bg = C.default, mx = 4, my = 4, shrink = 0 } end
            local function item(props)
                local disabled = props.disabled
                props.p = 0
                props.h = 30
                props.w_full = true
                props.px = 8
                props.gap = 9
                props.rounded = 6
                props.items = "center"
                props.direction = "row"
                props.bg = C.clear
                props.cursor = "pointer"
                props.shrink = 0
                if disabled then props.opacity = function() return disabled() and 0.4 or 1 end end
                local itemEl
                local mount = props.on_mount
                props.on_mount = function(el) itemEl = el; if mount then mount(el) end end
                props.on_hover = function(inside)
                    if not inside or not itemEl or (disabled and disabled()) then return end
                    local first = activeMenu:peek() ~= id
                    activeMenu:set(id)
                    to(hlY, offset(itemEl), (first or reduced:peek()) and 0 or 160, EASE.OUT)
                end
                return T.button(props)
            end
            local menu = build(head, sep, item, offset)
            local props = extra or {}
            props.id = id
            props.w = width
            props.p = 5
            props.bg = "#1A1A18"
            props.border = 1
            props.border_color = C.strong
            props.rounded = 10
            props.shadow = { { y = 12, blur = 28, color = "#00000066" }, { y = 2, blur = 6, color = "#00000040" } }
            props.on_hover = function(inside) if not inside and activeMenu:peek() == id then activeMenu:set("") end end
            table.insert(props, ui.div { absolute = true, left = 0, top = 0, w = 1, h = 1, on_mount = function(el) marker = el end })
            table.insert(props, ui.div { absolute = true, left = 5, right = 5, top = 0, h = 30, rounded = 6, bg = C.selected,
                y = hlY, opacity = eased(function() return activeMenu:get() == id and 1 or 0 end, 90, EASE.OUT) })
            table.insert(props, ui.column { w_full = true, gap = 1, items = "stretch", menu.rows })
            if menu.overlay then table.insert(props, menu.overlay) end
            return ui.column(props)
        end
        local function lb(label, color, props)
            local p = props or {}
            p[1] = label
            p.text_size = 12.5
            p.color = color or C.text
            p.truncate = true
            return T.text(p)
        end
        local function sp() return ui.div { grow = 1, basis = 0 } end
        local function siteMenu()
            local host = model.displayParts(P.url)
            local secure = P.security == "secure"
            local line = ({ secure = "Connection is secure", internal = "Internal Ask page", insecure = "Connection is not secure",
                unknown = "Connection not verified" })[P.security]
            local rowEls = {}
            local subs = {}
            local rowsRev = state("site_rows_rev", 0)
            return ctxMenu("nb_site", math.min(300, W:peek() - 16), function(head, sep, item, offset)
                -- A value row's choices: a small menu under the row (navbar.js choiceMenu).
                local function choiceMenu(name, values)
                    values = values or { "allow", "ask", "block" }
                    return ctxMenu("nb_sub_" .. name, 140, function(_, _, subItem)
                        local opts = {}
                        for _, v in values do
                            local sel = function() savedRev:get(); return sitePermission(name) == v end
                            table.insert(opts, subItem { id = "nb_set_" .. name .. "_" .. v,
                                on_click = function() setSitePermission(name, v); subMenu:set("") end,
                                -- The selected value: a short ink bar at the row's edge and a stronger label.
                                ui.div { absolute = true, left = -3, top = 8, w = 2, h = 14, rounded = 1, bg = C.text,
                                    opacity = function() return sel() and 1 or 0 end },
                                lb(name == "Act on pages" and v == "ask" and "Ask first" or model.PERMS[v], C.text,
                                    { weight = function() return sel() and 500 or 400 end }),
                                sp() })
                        end
                        return { rows = opts }
                    end, { absolute = true, right = 5,
                        hidden = function() return subMenu:get() ~= name end,
                        top = function() rowsRev:get(); subMenu:get(); return rowEls[name] and offset(rowEls[name]) + 32 or 0 end })
                end
                local function choiceRow(name, glyph, values)
                    table.insert(subs, choiceMenu(name, values))
                    return item { id = "nb_choice_" .. name,
                        on_click = function() subMenu:set(subMenu:peek() == name and "" or name) end,
                        on_mount = function(el) rowEls[name] = el; bump(rowsRev) end,
                        icon(glyph, 14, C.tertiary), lb(name), sp(),
                        T.text { function() savedRev:get(); return permissionLabel(name) end, text_size = 12, color = C.tertiary, shrink = 0 },
                        icon("chevron-down", 12, C.muted) }
                end
                local toggleOn = ui.computed(function() savedRev:get(); return sitePermission("Read pages") == "allow" end)
                local rows = {
                    ui.row { p = 8, gap = 10, items = "center", shrink = 0,
                        ui.overlay { w = 30, h = 30, rounded = 7, border = 1, border_color = C.strong, items = "center", justify = "center", shrink = 0,
                            P.favicon and ui.image { src = P.favicon, w = 16, h = 16, rounded = 3 } or icon("globe", 16, C.text) },
                        ui.column { gap = 2, min_w = 0,
                            T.text { host, text_size = 12.5, weight = 500, color = C.text },
                            ui.row { gap = 4, items = "center",
                                icon(secure and "lock" or "triangle-alert", 10, secure and "#A3B59A" or "#C9A78B"),
                                T.text { line, text_size = 11, color = C.tertiary } } } },
                    sep(), head("Ask on this site"),
                    item { id = "nb_read_pages", on_click = function()
                            setSitePermission("Read pages", sitePermission("Read pages") == "allow" and "block" or "allow")
                        end,
                        icon("eye", 14, C.tertiary), lb("Read pages"), sp(),
                        ui.overlay { w = 26, h = 16, rounded = 8, shrink = 0,
                            bg = eased(function() return toggleOn:get() and C.text or C.strong end, 140, EASE.OUT),
                            ui.div { absolute = true, left = 2, top = 2, w = 12, h = 12, rounded = 6, bg = C.elevated,
                                x = eased(function() return toggleOn:get() and 10 or 0 end, 140, EASE.OUT) } } },
                    choiceRow("Act on pages", "sparkles"),
                    sep(), head("Permissions"),
                }
                for _, perm in model.SITE_PERMS do table.insert(rows, choiceRow(perm[1], perm[2], perm[3])) end
                table.insert(rows, sep())
                table.insert(rows, item { id = "nb_site_data", on_click = siteData,
                    icon("cookie", 14, C.tertiary), lb("Cookies and site data"), sp(),
                    T.text { function()
                        pageRev:get()
                        local d = P.siteData
                        return d and d.loading and "Reading…" or (d and d.cookies ~= nil) and (d.cookies .. " cookies") or ""
                    end, text_size = 12, color = C.tertiary, shrink = 0 },
                    icon("chevron-right", 12, C.muted) })
                table.insert(rows, item { id = "nb_site_settings", on_click = function() popover:set(""); navigateTab("ask://settings/privacy") end,
                    icon("settings-2", 14, C.tertiary), lb("Site settings"), sp(), icon("arrow-up-right", 12, C.muted) })
                table.insert(rows, item { id = "nb_reset_perms",
                    disabled = function() savedRev:get(); pageRev:get(); return not permsChanged() end,
                    on_click = function() local o = model.originOf(entry().url); if o then saved.permissions[o] = nil end; bump(savedRev) end,
                    icon("rotate-ccw", 14, C.tertiary), lb("Reset permissions") })
                return { rows = rows, overlay = ui.overlay { absolute = true, left = 0, top = 0, w_full = true, subs } }
            end)
        end
        local function extensionsMenu()
            local host = model.displayParts(P.url)
            if host == "" then host = "this site" end
            return ctxMenu("nb_extmenu", math.min(280, W:peek() - 16), function(head, sep, item)
                local rows = {}
                local function section(title, access)
                    local list = {}
                    for _, x in model.EXTS do if x.access == access then table.insert(list, x) end end
                    if #list == 0 then return end
                    table.insert(rows, head(title))
                    for _, x in list do
                        local pinned = function() savedRev:get(); return table.find(saved.pinned, x.id) ~= nil end
                        local hovered = ui.state("ext_hover_" .. x.id, false)
                        table.insert(rows, item { id = "nb_extrow_" .. x.id,
                            on_click = function()
                                if x.access == "click" then x.access = "site"; rebuildPop(); notice(x.name .. " can now read and change this site")
                                else popover:set(""); notice(x.name) end
                            end,
                            extIcon(x, 14), lb(x.name),
                            x.access == "site" and x.meta and T.text { x.meta, text_size = 12, color = C.tertiary } or nil, sp(),
                            x.access == "click" and T.text { "Allow", text_size = 12, color = C.secondary } or nil,
                            T.button { id = "nb_pin_" .. x.id, p = 0, w = 22, h = 22, mr = -4, rounded = 5, items = "center", justify = "center",
                                shrink = 0, bg = C.clear, hover = { bg = C.active }, cursor = "pointer",
                                on_hover = function(inside) hovered:set(inside) end,
                                opacity = eased(function() return pinned() and 1 or 0 end, 90, EASE.OUT),
                                on_click = function()
                                    local i = table.find(saved.pinned, x.id)
                                    if i then table.remove(saved.pinned, i) else table.insert(saved.pinned, x.id) end
                                    pinnedCount:set(#saved.pinned)
                                    bump(savedRev)
                                end,
                                ui.presence(ui.computed(function() return pinned() end), function(on)
                                    return ui.row { w = 22, h = 22, items = "center", justify = "center",
                                    on and T.filled("pin", 12, C.text) or icon("pin", 12, "#6E6C65") }
                                end) } })
                    end
                end
                section("Can read and change " .. host, "site")
                section("Requesting access", "click")
                section("No access needed", "none")
                table.insert(rows, sep())
                table.insert(rows, item { id = "nb_manage_ext", on_click = function() popover:set(""); navigateTab("ask://extensions") end,
                    icon("puzzle", 14, C.tertiary), lb("Manage extensions"), sp(), icon("arrow-up-right", 12, C.muted) })
                return { rows = rows }
            end)
        end
        local function btn(id, label, kind, click)
            local primary, danger = kind == "primary", kind == "danger"
            return T.button { id = id, p = 0, h = 28, px = 12, rounded = 6, items = "center", justify = "center", cursor = "pointer",
                bg = primary and C.text or C.selected, on_click = click,
                T.text { label, text_size = 12, color = primary and "#10100F" or danger and "#D8A899" or C.text } }
        end
        local function padded(id, children)
            return panel { id = id, w = math.min(360, W:peek() - 16), p = 15, gap = 12, direction = "column",
                shadow = { y = 16, blur = 20, color = "#00000080" }, children }
        end
        local function permissionPopover()
            local r = P.permission
            if not r then return ui.div {} end
            return padded("nb_permission", {
                T.text { r.origin, text_size = 13, weight = 500, color = C.text },
                T.text { "Allow " .. table.concat(r.kinds, " and ") .. "?", text_size = 12, color = C.secondary },
                ui.row { w_full = true, justify = "end", gap = 8,
                    btn("nb_perm_block", "Block", nil, function() respondPermission(false); popover:set("") end),
                    btn("nb_perm_allow", "Allow", "primary", function() respondPermission(true); popover:set("") end) } })
        end
        local function dataPopover()
            local d = P.siteData
            local body = {}
            if d.loading then
                table.insert(body, T.text { "Reading cookies…", text_size = 12, color = C.secondary })
            else
                table.insert(body, T.text { (d.cookies or 0) .. " cookies", text_size = 12, color = C.text })
                for i = 1, math.min(10, #d.entries) do
                    local c = d.entries[i]
                    table.insert(body, ui.row { w_full = true, gap = 8, items = "center",
                        T.text { c.name, text_size = 11.5, color = C.text, grow = 1, basis = 0, truncate = true },
                        T.text { c.domain, text_size = 10.5, color = C.muted } })
                end
                if #d.entries > 10 then table.insert(body, T.text { "and " .. (#d.entries - 10) .. " more", text_size = 10.5, color = C.muted }) end
            end
            return padded("nb_data", {
                T.text { "Cookies and site data", text_size = 13.5, weight = 500, color = C.text },
                T.text { d.origin, text_size = 11, color = C.tertiary, truncate = true },
                body,
                T.text { "Cookie storage for this site", text_size = 10.5, color = C.muted },
                ui.row { w_full = true, justify = "end", gap = 8,
                    btn("nb_data_back", "Back", nil, function() popover:set("site"); rebuildPop() end),
                    btn("nb_data_clear", "Clear cookies", "danger", clearCookies) } })
        end
        local function bookmarkPopover()
            local b = bookmarked()
            bmName:set(b and b.title or P.title)
            folder:set(b and b.folder or "Bookmarks")
            folderOpen:set(false)
            local folders = { "Bookmarks", "Reading list" }
            for _, x in saved.bookmarks do if not table.find(folders, x.folder) then table.insert(folders, x.folder) end end
            local options = {}
            for _, f in folders do
                table.insert(options, T.button { p = 0, h = 28, w_full = true, px = 8, rounded = 5, items = "center", direction = "row",
                    bg = C.clear, hover = { bg = C.selected }, cursor = "pointer",
                    on_click = function() folder:set(f); folderOpen:set(false) end,
                    T.text { f, text_size = 11.5, color = C.text } })
            end
            local function label(s) return T.text { s, font = "Geist Mono", text_size = 10.5, color = C.tertiary } end
            return panel { id = "nb_bookmark", w = math.min(320, W:peek() - 16), p = 1, direction = "column",
                shadow = { y = 16, blur = 20, color = "#00000080" },
                ui.row { h = 42, px = 14, gap = 8, items = "center", icon("star", 13, C.text), txt("Bookmark saved", 13) },
                ui.column { w_full = true, h = 128, px = 14, pb = 14, gap = 7,
                    label("Name"),
                    ui.overlay { h = 32, w_full = true, px = 11, rounded = 10, bg = "#10100F", border = 1, border_color = "#58564E",
                        ui.input { id = "nb_bm_name", label = "Name", value = bmName, w_full = true, h_full = true, p = 0, border = 0,
                            bg = C.clear, focus = { border = 0 }, text_size = 12, color = C.text, selection_color = "#393933",
                            on_focus = function() end } },
                    label("Folder"),
                    ui.overlay { h = 30, w_full = true,
                        T.button { id = "nb_folder_btn", p = 0, w_full = true, h = 30, px = 11, gap = 8, rounded = 7, bg = "#10100F",
                            border = 1, border_color = C.default, items = "center", direction = "row", cursor = "pointer",
                            on_click = function() folderOpen:toggle() end,
                            icon("folder", 12, C.secondary),
                            T.text { function() return folder:get() end, text_size = 12, color = C.text, grow = 1, basis = 0 },
                            icon("chevron-down", 11, C.secondary) },
                        panel { absolute = true, left = 0, top = 32, w_full = true, p = 5, direction = "column",
                            hidden = function() return not folderOpen:get() end, options } } },
                ui.row { w_full = true, h = 50, px = 14, pt = 1, gap = 8, items = "center", border = 1, border_sides = { 1, 0, 0, 0 }, border_color = C.default,
                    T.button { id = "nb_bm_remove", p = 0, bg = C.clear, cursor = "pointer",
                        on_click = function() if bookmarked() then toggleBookmark() end; popover:set("") end,
                        T.text { "Remove", text_size = 12, color = "#D8A899" } },
                    ui.div { grow = 1, basis = 0 },
                    btn("nb_bm_done", "Done", "primary", function()
                        local bm = bookmarked()
                        if bm then bm.title = bmName:peek(); bm.folder = folder:peek() end
                        popover:set(""); refresh()
                    end) } }
        end
        -- Where the open popover sits: under the field, the bookmark editor right-aligned to it, the extension menu under
        -- its button; kept 8pt inside the window.
        local function popWidth(kind)
            return math.min(kind == "bookmark" and 320 or kind == "extensions" and 280 or kind == "site" and 300 or 360, math.max(0, W:get() - 16))
        end
        local popKey = ui.computed(function()
            local v = popVisible:get()
            return v == "" and "" or v .. "|" .. (v == "bookmark" and "" or tostring(popBuild:get()))
        end)
        -- A rebuilt popover starts with its submenus closed and no highlight (fresh markup in the source).
        ui.effect(function() popKey:get(); subMenu:set(""); activeMenu:set("") end)
        popHost = ui.overlay { id = "nb_pop", absolute = true, pointer_events = true,
            left = function()
                local v = popVisible:get()
                local b, w = fieldBounds(), popWidth(v)
                local box, L = navBox(), rowLayout()
                local extRight = box.x + (box.w - L.fw) * align() + L.extRight
                local left = v == "bookmark" and b.x + b.w - w or v == "extensions" and extRight - w or b.x
                return math.max(8, math.min(left, math.max(W:get() - w - 8, 8)))
            end,
            top = function() local b = fieldBounds(); return b.y + b.h + 8 end,
            opacity = popM,
            y = function() return reduced:get() and 0 or 4 * (1 - popM:get()) end,
            ui.presence(popKey, function(k)
                local v = string.match(k, "^[^|]*")
                if v == "" then return ui.div {} end
                if v == "bookmark" then return bookmarkPopover() end
                if v == "permission" then return permissionPopover() end
                if v == "data" then return dataPopover() end
                if v == "extensions" then return extensionsMenu() end
                return siteMenu()
            end) }

    end
    ---------- Agent acting, toast, backdrop ----------
    do
        local auraParams = dither.params("#9DB2FFB3", AURA)
        local auraTicker
        local auraOp = eased(function() return acting:get() and 1 or 0 end, 320, EASE.OUT)
        aura = ui.div { id = "nb_aura", absolute = true, left = 0, top = 0, w_full = true, h_full = true, pointer_events = false,
            bg_shader = dither.fill, shader_params = auraParams,
            opacity = auraOp,
            invisible = function() return not acting:get() and auraOp:get() <= 0 end,
            on_mount = function(el)
                ui.effect(function()
                    local on, rd = acting:get(), reduced:get()
                    if auraTicker then auraTicker:cancel(); auraTicker = nil end
                    anime.utils.set(el, { shaderParam15 = rd and 0 or AURA.speed, shaderTime = 0 })
                    if on and not rd then
                        -- 15 fps, as the source repaints its aura canvas; time is seconds of slow-mo scaled clock.
                        auraTicker = anime.animate(el, { shaderTime = { 0, 3600 }, duration = S(3600000), ease = "linear",
                            frameRate = 15, loop = true })
                    end
                end)
            end }
        local agentOn = eased(function() return acting:get() and 1 or 0 end, 200, EASE.OUT)
        local agentY = eased(function() return acting:get() and 0 or 8 end, 260, EASE.ENTER)
        local function handBack(stopped)
            acting:set(false)
            notice(stopped and "Stopped Ask" or "You have control of the page")
        end
        agentBar = ui.row { id = "nb_agentbar", absolute = true, bottom = 20, h = 52, pl = 15, pr = 11, gap = 11, items = "center",
            w = function() return math.min(460, W:get() - 32) end,
            left = function() return (W:get() - math.min(460, W:get() - 32)) / 2 end,
            rounded = 12, bg = C.elevated, border = 1, border_color = C.strong,
            shadow = { { y = 16, blur = 32, color = "#00000080" }, { y = 2, blur = 6, color = "#00000040" } },
            opacity = agentOn, y = agentY, pointer_events = function() return acting:get() end,
            invisible = function() return not acting:get() and agentOn:get() <= 0 end,
            ui.div { w = 16, h = 16, shrink = 0, bg_shader = dither.fill, shader_params = dither.params(C.text, dither.ORB) },
            ui.column { gap = 2, min_w = 0, grow = 1, basis = 0,
                T.text { "Ask is using this page", text_size = 12.5, weight = 500, color = C.text },
                txt("Clicking, typing and filling forms", 11, C.muted) },
            T.button { id = "nb_agent_stop", p = 0, h = 30, px = 12, rounded = 8, bg = C.clear, hover = { bg = C.selected },
                cursor = "pointer", shrink = 0, on_click = function() handBack(true) end,
                txt("Stop", 12.5, C.secondary) },
            T.button { id = "nb_agent_take", p = 0, h = 30, px = 12, rounded = 8, bg = C.text, hover = { bg = "#FFFFFF" },
                cursor = "pointer", shrink = 0, on_click = function() handBack(false) end,
                T.text { "Take control", text_size = 12.5, weight = 500, color = "#10100F" } },
        }
        local toastOp = eased(function() return toastOn:get() and 1 or 0 end, 140, EASE.OUT)
        local toastW = state("toast_w", 0)
        toast = ui.row { id = "nb_toast", absolute = true, h = 34, px = 13, rounded = 8, items = "center", pointer_events = false,
            bg = C.elevated, border = 1, border_color = C.strong,
            max_w = function() return W:get() * 0.7 end,
            bottom = function() return acting:get() and 84 or 16 end,
            left = function() return (W:get() - toastW:get()) / 2 end,
            on_resize = function(size) toastW:set(size.w) end,
            opacity = toastOp, y = function() return 6 * (1 - toastOp:get()) end,
            T.text { function() return toastText:get() end, text_size = 12, color = C.text, truncate = true } }
        backdrop = ui.div { id = "nb_backdrop", absolute = true, left = 0, top = 0, w_full = true, h_full = true, pointer_events = true,
            hidden = function() return not (focused:get() or popover:get() ~= "") end,
            on_press = function() dismiss() end }
        scrim = ui.div { absolute = true, left = 0, top = 0, w_full = true, h_full = true, bg = "#0A0A0980", backdrop_blur = 2,
            hidden = function() return derivedOpen() <= 0 end, opacity = derivedOpen }

    end
    ---------- Document view ----------
    local docView = ui.scroll { id = "nb_doc", absolute = true, left = 0, top = 0, w_full = true, h_full = true,
        offset = docScroll, bg = C.page,
        on_mount = function(el) docEl = el end,
        on_pointer_move = function(e) pointer = { e.local_x, e.local_y } end,
        ui.presence(docRev, function()
            local d = doc
            local fade = docFade
            docFade = false
            return ui.column { w_full = true, min_h = function() return Hh:get() end,
                on_mount = function(el)
                    if fade then anime.animate(el, { opacity = { 0, 1 }, y = { 4, 0 }, duration = S(180), ease = EASE.OUT }) end
                end,
                page(d) }
        end) }

    local window = ui.overlay { id = "nb_win", w_full = true, h_full = true, rounded = 11, overflow = "hidden", bg = "#141413",
        on_resize = function(size) W:set(size.w); Hh:set(size.h) end,
        docView,
        aura,
        -- navbar.css: the float and overlay layers let the pointer through; their surfaces take it.
        ui.overlay { absolute = true, left = 0, top = 0, w_full = true, h_full = true, pointer_events = false, dockLayer, nav },
        agentBar,
        ui.overlay { absolute = true, left = 0, top = 0, w_full = true, h_full = true, pointer_events = false,
            scrim, backdrop, sheet, field, composer, popHost },
        toast,
    }

    renderDoc(false)
    changed()
    syncInput()

    -- Lab readout.
    local readout = function()
        pageRev:get()
        local m = modeK:get()
        return string.format("focused:%s · mode:%s%s · collapsed:%s · hover:%s · popover:%s · scroll:%d · progress:%.2f",
            tostring(focused:get()), m, nb.mode.site and m == "site" and "(" .. nb.mode.site .. ")" or "",
            tostring(collapsed:get()), tostring(hover:get()), popover:get() ~= "" and popover:get() or "–",
            math.floor(docScroll:get() + 0.5), progress:get())
    end

    return {
        window = window,
        readout = readout,
        focusAddress = focusAddress,
        toggleFocus = function() if focused:peek() then dismiss() else focusAddress() end end,
        visit = function(url) dismiss(); navigateTab(url) end,
        setSlowLoads = function(on) labState.slowLoads = on end,
        setOffline = function(on) labState.offline = on; offline:set(on); renderDoc(true); syncPage(); refresh() end,
        acting = acting,
        permChecked = permChecked,
        setPermission = requestPermission,
        replay = function()
            if focused:peek() then dismiss() end
            later(OPEN.close[1] + 250, focusAddress)
        end,
        -- Holds the open transition at a fixed progress for inspection; Replay or Esc resumes normal motion.
        scrub = function(v)
            if not focused:peek() then focusAddress() end
            to(openM, v, 0)
        end,
        scrollTop = function()
            if reduced:peek() then docScroll:set(0) else anime.animate(docScroll, { value = 0, duration = 420, ease = "outCubic" }) end
        end,
    }
end
