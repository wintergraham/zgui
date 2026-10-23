-- The Areas tab (pen `zgui · Frame profiler · Areas tab`, z9i0E5): which
-- parts of the UI do work each frame. A tree of areas with their dirty
-- nodes per flag, updates and damage, the selected area's detail, and the
-- findings over them. Every value is D.areas_tab's (sample data until the
-- engine measures areas); this file only formats and lays it out.
return function(S, K, D)
    local C, icon, text, mono, spacer = K.C, K.icon, K.text, K.mono, K.spacer
    local A = D.areas_tab
    local FLAGS = { { "l", "Layout", D.colors.layout }, { "p", "Paint", D.colors.paint },
        { "c", "Composite", D.colors.render } }
    local FLAG_COLOR = { layout = D.colors.layout, paint = D.colors.paint, composite = D.colors.render }

    local T = {
        scope = ui.state("areas_scope", 2),
        query = ui.state("areas_query", ""),
        selected = ui.state("areas_selected", A.selected),
        open = ui.state("areas_open", A.open),
        sort = ui.state("areas_sort", { col = "work", desc = true }),
        highlight = ui.state("areas_highlight", false),
    }

    -- Each row's place in the tree, by id.
    local byId = {}
    local function index(list, depth, parent)
        for _, r in list do
            byId[r.id] = { row = r, depth = depth, parent = parent }
            if r.children then index(r.children, depth + 1, r) end
        end
    end
    index(A.rows, 0, nil)

    local function work(v) return v.l + v.p + v.c end
    local function fmtWork(w)
        if w <= 0 then return "—" end
        return w >= 10 and tostring(math.floor(w + 0.5)) or string.format("%.1f", w)
    end
    local function fmtRate(u) return string.format("%d/s", math.floor(u + 0.5)) end

    -- Rows ----------------------------------------------------------------------

    -- Sorting keeps the tree: siblings sort among themselves, and the quiet
    -- "other areas" row stays last.
    local KEYS = {
        area = function(r) return string.lower(r.name) end,
        work = function(r, s) return work(r.scopes[s]) end,
        updates = function(r, s) return r.scopes[s].u end,
        damage = function(r, s) return r.scopes[s].d end,
    }
    local function sorted(list, s, sort)
        local out, key = table.clone(list), KEYS[sort.col]
        table.sort(out, function(a, b)
            if (a.quiet or false) ~= (b.quiet or false) then return not a.quiet end
            local x, y = key(a, s), key(b, s)
            if x == y then return a.order < b.order end
            if sort.desc then return x > y end
            return x < y
        end)
        return out
    end
    -- The filter matches names and ids; a match keeps its ancestors, opened.
    local function matches(r, q)
        if q == "" or string.find(string.lower(r.name), q, 1, true)
            or (r.tag and string.find(string.lower(r.tag), q, 1, true)) then
            return true
        end
        for _, c in r.children or {} do
            if matches(c, q) then return true end
        end
        return false
    end
    local visible = ui.computed(function()
        local q, s, sort, open = string.lower(T.query:get()), T.scope:get(), T.sort:get(), T.open:get()
        local out = {}
        local function walk(list)
            for _, r in sorted(list, s, sort) do
                if matches(r, q) then
                    table.insert(out, r.id)
                    if r.children and (open[r.id] or q ~= "") then walk(r.children) end
                end
            end
        end
        walk(A.rows)
        return out
    end)
    local function isOpen(id)
        return T.open:get()[id] == true or T.query:get() ~= ""
    end
    local function toggle(id)
        local open = table.clone(T.open:peek())
        open[id] = not open[id] or nil
        T.open:set(open)
    end
    -- Select a row, opening its ancestors (a finding's link).
    local function reveal(id)
        local open, p = table.clone(T.open:peek()), byId[id].parent
        while p do
            open[p.id] = true
            p = byId[p.id].parent
        end
        ui.batch(function()
            T.query:set("")
            T.open:set(open)
            T.selected:set(id)
        end)
    end

    -- Work bars share one scale: the busiest top-level area fills 120 points.
    local barScale = ui.computed(function()
        local s, most = T.scope:get(), 0
        for _, r in A.rows do most = math.max(most, work(r.scopes[s])) end
        return most > 0 and 120 / most or 0
    end)

    local function sparkline(values, color)
        return ui.canvas { w = 64, h = 14, shrink = 0, draw = function(c, w, h)
            local bw = (w - (#values - 1)) / #values
            for i, v in values do
                if v > 0 then c:rect((i - 1) * (bw + 1), h - v, bw, v, color) end
            end
        end }
    end

    local function rowView(id)
        local e = byId[id]
        local r, depth = e.row, e.depth
        local v = ui.computed(function() return r.scopes[T.scope:get()] end)
        local selected = function() return T.selected:get() == id end
        local segments = {}
        for k, f in FLAGS do
            segments[k] = ui.div { h = 8, bg = f[3], shrink = 0,
                hidden = function() return v:get()[f[1]] <= 0 end,
                w = ui.spring(ui.computed(function()
                    local x = v:get()[f[1]]
                    return x > 0 and math.max(2, x * barScale:get()) or 0
                end), { stiffness = 300, damping = 30 }) }
        end
        local disclosure = ui.div { w = 12, h = 12, shrink = 0 }
        if r.children then
            disclosure = K.button({ id = "areas_toggle_" .. id, w = 12, h = 12, justify = "center", hover = { bg = C.clear },
                on_click = function() toggle(id) end,
                icon("chevron-right", 12, C.muted, {
                    rotate = ui.spring(ui.computed(function() return isOpen(id) and 90 or 0 end), { stiffness = 520, damping = 34 }) }),
            }, 0.85)
        end
        return ui.row {
            id = "areas_row_" .. id, w_full = true, h = 30, px = 8, gap = 12, items = "center", rounded = 7,
            cursor = "pointer", hover = { bg = "#FFFFFF06" },
            on_click = function() T.selected:set(id) end,
            on_mount = function(el) anime.animate(el, { opacity = { 0, 1 }, duration = 160, ease = "outQuad" }) end,
            ui.row { grow = 1, shrink = 1, min_w = 0, gap = 7, pl = depth * 19, items = "center",
                disclosure,
                text(r.name, 12.5, function()
                    if selected() or (depth == 0 and not r.quiet) then return C.text end
                    return r.quiet and C.tertiary or C.secondary
                end, { weight = function() return selected() and 500 or 400 end, truncate = true, shrink = 1, min_w = 0 }),
                r.tag and mono(r.tag, 10, C.muted, { shrink = 0 }) or nil,
                r.issue and ui.row { h = 18, px = 6, gap = 4, rounded = 4, bg = "#E2C98F1F", items = "center", shrink = 0,
                    icon("alert", 10, C.gold), mono(r.issue, 9.5, C.gold) } or nil,
            },
            ui.row { w = 160, gap = 8, items = "center",
                ui.row { w = 120, items = "center",
                    ui.row { h = 8, gap = 1, rounded = 2, overflow = "hidden", segments } },
                mono(function() return fmtWork(work(v:get())) end, 11, function()
                    if work(v:get()) <= 0 then return C.faint end
                    return selected() and C.text or C.secondary
                end, { class = "areas_scoped" }) },
            ui.row { w = 64, justify = "end",
                mono(function() return fmtRate(v:get().u) end, 11, function()
                    if r.idle then return C.gold end
                    return v:get().u >= 100 and C.secondary or C.tertiary
                end, { class = "areas_scoped" }) },
            ui.row { w = 70, justify = "end",
                mono(function() return string.format("%.2f", v:get().d) end, 11, C.secondary, { class = "areas_scoped" }) },
            sparkline(r.last, "#D9826A80"),
        }
    end

    -- Table ----------------------------------------------------------------------

    local function sortBy(col)
        local s = T.sort:peek()
        if s.col == col then
            T.sort:set({ col = col, desc = not s.desc })
        else
            T.sort:set({ col = col, desc = col ~= "area" })
        end
    end
    local function head(col, label, w, right, unit)
        local active = function() return T.sort:get().col == col end
        return K.button({ id = "areas_sort_" .. col, w = w, grow = w and 0 or 1, shrink = w and 0 or 1, min_w = 0,
            h = 20, gap = 4, justify = right and "end" or "start", hover = { brightness = 1.3 },
            on_click = function() sortBy(col) end,
            text(label, 11, function() return active() and C.secondary or C.muted end),
            unit and mono(unit, 9.5, C.faint) or nil,
            icon("arrow-down", 11, C.secondary, {
                hidden = function() return not active() end,
                rotate = ui.spring(ui.computed(function() return T.sort:get().desc and 0 or 180 end), { stiffness = 520, damping = 34 }) }),
        }, 0.97)
    end
    local selectedIndex = ui.computed(function()
        local id = T.selected:get()
        for i, v in visible:get() do
            if v == id then return i end
        end
        return 0
    end)
    local total = ui.computed(function() return A.total.scopes[T.scope:get()] end)
    local flagged = 0
    for _, e in byId do
        if e.row.issue then flagged += 1 end
    end
    local function key(k)
        local f = FLAGS[k]
        return ui.row { gap = 6, items = "center",
            ui.div { w = 8, h = 8, rounded = 2, bg = f[3] }, text(f[2], 11, C.secondary),
            mono(function()
                local t = total:get()
                return string.format("%d%%", math.floor(t[f[1]] / math.max(work(t), 0.001) * 100 + 0.5))
            end, 10, C.muted, { class = "areas_scoped" }) }
    end
    local tableView = ui.column { grow = 1, shrink = 1, min_w = 0,
        ui.row { w_full = true, h = 28, px = 8, gap = 12, items = "center",
            head("area", "Area"),
            ui.row { w = 160, head("work", "Work / frame", 160) },
            head("updates", "Updates", 64, true),
            head("damage", "Damage", 70, true, "Mpx"),
            ui.row { w = 64, justify = "end", text("Last 1 s", 11, C.muted) } },
        ui.overlay { w_full = true,
            ui.div { absolute = true, left = 0, right = 0, top = 0, h = 30, rounded = 7, bg = C.selected,
                y = ui.spring(ui.computed(function() return (math.max(selectedIndex:get(), 1) - 1) * 30 end),
                    { stiffness = 520, damping = 38 }),
                opacity = ui.tween(ui.computed(function() return selectedIndex:get() > 0 and 1 or 0 end), { duration = 0.12 }) },
            ui.column { w_full = true,
                ui.each(visible, rowView, { key = function(id) return id end }),
                ui.show(function() return #visible:get() == 0 end,
                    ui.row { w_full = true, h = 30, px = 27, items = "center",
                        text(function() return "No areas match “" .. T.query:get() .. "”" end, 12, C.muted) }) },
        },
        ui.row { id = "areas_total", w_full = true, h = 32, mt = 8, px = 8, gap = 12, items = "center",
            border = 1, border_sides = { 1, 0, 0, 0 }, border_color = C.subtle,
            ui.row { grow = 1, shrink = 1, min_w = 0, gap = 7, items = "center",
                ui.div { w = 12 }, text("All areas", 12.5, C.text, { weight = 500 }), mono(A.total.count, 10, C.muted) },
            ui.row { w = 160, gap = 8, items = "center", ui.div { w = 120 },
                mono(function() return string.format("%.1f", work(total:get())) end, 11, C.text, { class = "areas_scoped" }) },
            ui.row { w = 64, justify = "end", mono(function() return fmtRate(total:get().u) end, 11, C.text) },
            ui.row { w = 70, justify = "end",
                mono(function() return string.format("%.2f", total:get().d) end, 11, C.text, { class = "areas_scoped" }) },
            sparkline(A.total.last, C.tertiary),
        },
        ui.row { w_full = true, pt = 6, pb = 2, px = 8, gap = 16, items = "center",
            key(1), key(2), key(3), spacer(),
            mono(flagged .. " areas flagged", 10, C.gold) },
    }

    -- Selected area ---------------------------------------------------------------

    local chartGrow = ui.state("areas_chart_grow", 1)
    local function chip(label, fg, bg)
        return ui.row { h = 20, px = 7, rounded = 5, bg = bg, items = "center", shrink = 0, mono(label, 10, fg) }
    end
    local function headerButton(id, glyph, on_click, active)
        return K.button { id = id, w = 26, h = 26, rounded = 7, justify = "center", on_click = on_click,
            bg = active and function() return active() and C.selected or C.clear end or nil,
            icon(glyph, 14, C.secondary) }
    end
    local function stat(r, k)
        local f = FLAGS[k]
        return ui.column { grow = 1, basis = 0, min_w = 0, gap = 4,
            pl = k == 1 and 0 or 12, border = 1, border_sides = { 0, 0, 0, k == 1 and 0 or 1 }, border_color = C.line,
            ui.row { gap = 5, items = "center", ui.div { w = 7, h = 7, rounded = 2, bg = f[3] }, text(f[2], 11, C.tertiary) },
            ui.row { gap = 3, items = "end",
                text(function() return string.format("%.1f", r.scopes[T.scope:get()][f[1]]) end, 17, C.text,
                    { weight = 500, letter_spacing = -0.3, class = "areas_scoped" }),
                text("/ frame", 10.5, C.muted, { pb = 3 }) } }
    end
    local function why(w)
        return ui.row { w_full = true, h = 30, gap = 9, items = "center",
            border = 1, border_sides = { 0, 0, 1, 0 }, border_color = C.subtle,
            ui.row { w = 80, h = 18, px = 6, gap = 5, rounded = 4, bg = C.selected, items = "center", shrink = 0,
                ui.div { w = 6, h = 6, rounded = 1.5, bg = FLAG_COLOR[w[1]] }, mono(string.upper(w[1]), 9.5, C.secondary) },
            text(w[2], 12, C.body_text, { truncate = true, shrink = 1, min_w = 0 }),
            spacer(),
            mono(w[3], 10.5, C.secondary, { shrink = 0 }) }
    end
    local function layerStat(label, value)
        return ui.column { grow = 1, basis = 0, min_w = 0, gap = 3, mono(label, 10, C.muted), mono(value, 11.5, C.text) }
    end
    local function detailView(id)
        local e = byId[id]
        if not e then return ui.div {} end
        local r, d = e.row, e.row.detail
        local meta = { d.kind, d.nodes .. " nodes" }
        if r.children then table.insert(meta, #r.children .. " children") end
        if d.note then table.insert(meta, d.note) end
        local reasons = {}
        for i, w in d.why do reasons[i] = why(w) end
        local peak, at = 0, 0
        for i, x in d.damage do
            if x > peak then peak, at = x, i end
        end
        local layer = d.layer
        return ui.column { w_full = true, gap = 14,
            ui.row { w_full = true, h = 26, gap = 8, items = "center",
                text(r.name, 16, C.text, { id = "areas_detail_title", weight = 500, truncate = true, shrink = 1, min_w = 0 }),
                r.tag and chip(r.tag, C.secondary, C.selected) or nil,
                d.chip and chip(d.chip, C.gold, "#E2C98F26") or nil,
                spacer(),
                headerButton("areas_highlight", "crosshair", function() T.highlight:toggle() end, function() return T.highlight:get() end),
                headerButton("areas_trace", "activity", function() S.tab:set(1) end) },
            mono(table.concat(meta, " · "), 10.5, C.muted),
            ui.row { w_full = true, stat(r, 1), stat(r, 2), stat(r, 3) },
            ui.column { w_full = true, gap = 2,
                mono(function() return "WHY IT UPDATES · " .. A.scopes[T.scope:get()].window end, 9.5, C.muted,
                    { letter_spacing = 0.7 }),
                reasons,
                d.trigger and ui.row { w_full = true, pt = 6, gap = 6, items = "center",
                    icon("corner-down-right", 12, C.muted), mono(d.trigger, 10, C.tertiary) } or nil },
            layer and ui.column { id = "areas_layer", w_full = true, gap = 10, py = 10, px = 12, rounded = 8, bg = C.window,
                ui.row { w_full = true, gap = 16,
                    layerStat("prepare_layer", layer.prepare), layerStat("texture", layer.texture),
                    layerStat("repaints · hits", layer.repaints) },
                layer.note and ui.row { w_full = true, pt = 9, gap = 7, items = "start",
                    border = 1, border_sides = { 1, 0, 0, 0 }, border_color = C.subtle,
                    icon("alert", 12, C.gold, { mt = 2 }),
                    text(layer.note, 11.5, C.body_text, { wrap = true, line_height = 16.1, grow = 1, shrink = 1, min_w = 0 }) } or nil,
            } or nil,
            ui.column { w_full = true, gap = 6,
                ui.row { w_full = true, justify = "between", items = "center",
                    text("Damaged px in subtree · last 1 s", 11, C.tertiary),
                    mono(peak > 0 and string.format("peak %.2f Mpx · #%s", peak, S.comma(D.first + 2 * at)) or "no damage",
                        10, C.muted) },
                -- 60 buckets of two frames; the peak's in full colour. They
                -- rise in as the selection changes.
                ui.canvas { w_full = true, h = 44,
                    on_mount = function() anime.animate(chartGrow, { value = { 0, 1 }, duration = 420, ease = "outCubic" }) end,
                    draw = function(c, w, h)
                        local g, n = chartGrow:get(), #d.damage
                        local bw = (w - (n - 1)) / n
                        for i, x in d.damage do
                            local hh = math.max(2, peak > 0 and x / peak * (h - 1) * g or 0)
                            c:rect((i - 1) * (bw + 1), h - hh, bw, hh, i == at and D.colors.render or "#D9826A80")
                        end
                    end } },
        }
    end
    local detail = ui.column { id = "areas_detail", w = 384, shrink = 0, pl = 20, pt = 2,
        border = 1, border_sides = { 0, 0, 0, 1 }, border_color = C.subtle,
        ui.presence(T.selected, detailView, { enter = { opacity = { 0, 1 }, duration = 160, ease = "outQuad" },
            exit = { opacity = 0, duration = 80 } }) }

    -- Header and findings ------------------------------------------------------------

    local filter = ui.row { id = "areas_filter", w = 200, h = 24, px = 8, gap = 6, rounded = 7, bg = C.inset, items = "center",
        icon("search", 12, C.muted),
        ui.overlay { grow = 1, shrink = 1, min_w = 0, h = 16, items = "center",
            text("Filter areas or ids", 11.5, C.muted, { absolute = true, left = 0, top = 1, pointer_events = "none",
                hidden = function() return T.query:get() ~= "" end }),
            ui.input { id = "areas_filter_input", label = "Filter areas", value = T.query, w_full = true, h = 16, p = 0, border = 0,
                bg = C.clear, text_size = 11.5, color = C.text, focus = { border_color = C.clear },
                -- While it has focus, the window's keys (space pauses, ←/→
                -- step frames) are left to it.
                on_focus = function() S.typing = true end,
                on_blur = function() S.typing = false end,
                on_key = function(e)
                    if e.key == "Escape" then T.query:set(""); return true end
                    return false
                end } },
        K.button { id = "areas_filter_clear", w = 16, h = 16, rounded = 4, justify = "center",
            hidden = function() return T.query:get() == "" end,
            on_click = function() T.query:set("") end,
            icon("x", 11, C.muted) },
    }
    local function finding(i, f)
        return ui.row { id = "areas_finding_" .. i, grow = 1, basis = 0, min_w = 0, gap = 10, py = 10, px = 12, rounded = 9,
            bg = C.window,
            icon(f[1], 14, f[2] == "warn" and C.warn or C.info),
            ui.column { grow = 1, shrink = 1, min_w = 0, gap = 6,
                text(f[3], 12, C.body_text, { wrap = true, line_height = 17.4 }),
                ui.button { id = "areas_finding_" .. i .. "_link", p = 0, bg = C.clear, border = 0, cursor = "pointer",
                    hover = { color = C.text }, focus = {}, color = C.secondary,
                    on_click = function() reveal(f[5]) end, ui.text { f[4], text_size = 12 } } },
        }
    end
    local findings = {}
    for i, f in A.findings do findings[i] = finding(i, f) end

    -- A scope change flashes the values it changed, as the Timeline's does.
    local lastScope = T.scope:peek()
    ui.effect(function()
        local s = T.scope:get()
        if s ~= lastScope then
            lastScope = s
            anime.animate(".areas_scoped", { opacity = { 0.2, 1 }, duration = 260, ease = "outQuad" })
        end
    end)
    local lastTab = S.tab:peek()
    ui.effect(function()
        local t = S.tab:get()
        if t ~= lastTab and t == 2 then
            anime.animate("#areas", { opacity = { 0, 1 }, y = { 6, 0 }, duration = 260, ease = "outExpo" })
        end
        lastTab = t
    end)

    return ui.column {
        id = "areas", w_full = true, gap = 18,
        hidden = function() return S.tab:get() ~= 2 end,
        ui.row { w_full = true, h = 28, gap = 8, items = "center",
            text("Areas", 13, C.text, { weight = 500 }),
            mono("which parts of the UI do work each frame", 10.5, C.muted),
            spacer(),
            filter,
            K.segmented("areas_scope", T.scope, #A.scopes, function(i, active)
                return text(A.scopes[i].label, 11.5, function() return active() and C.text or C.tertiary end)
            end, { h = 20, px = 8, bg = C.inset }) },
        ui.row { w_full = true, gap = 10, items = "stretch", findings },
        ui.row { w_full = true, gap = 24, items = "start", tableView, detail },
        text(A.footnote, 11.5, C.muted, { wrap = true }),
    }
end
