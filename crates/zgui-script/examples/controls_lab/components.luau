-- Additional demos in the current ControlsPage.tsx. All actions are local previews.
return function(k)
    local C, txt, icon, orb, button = k.C, k.txt, k.icon, k.orb, k.button
    local function stack(children, gap) return ui.column { w_full = true, gap = gap or 12, children } end
    local function pair(children) return ui.row { gap = 8, items = "center", flex_wrap = true, children } end
    local function keycap(keys)
        return ui.div { h = 20, px = 5, rounded = 4, border = 1, border_color = C.strong, bg = C.raised,
            items = "center", justify = "center", ui.text { keys, font = "Geist Mono", text_size = 10, color = C.secondary } }
    end
    local function progress(label, value, detail)
        return stack({ ui.row { w_full = true, txt(label, 12), ui.div { grow = 1 }, txt(detail or math.floor(value * 100) .. "%", 11, C.muted) },
            ui.overlay { w_full = true, h = 6, rounded = 3, bg = C.raised,
                ui.div { w = value * 100 .. "%", h = 6, bg_shader = k.dither, shader_params = { C.text, { 1.5, 5, 1, 0 } } } },
        }, 8)
    end
    local function slider(id, label, initial, min, max, suffix)
        local value = ui.state(id .. "_value", initial)
        local width = ui.state(id .. "_width", 300)
        local function fraction() return (value:get() - min) / (max - min) end
        return stack({ ui.row { w_full = true, txt(label, 12), ui.div { grow = 1 },
                ui.text { function() return math.floor(value:get() + 0.5) .. suffix end, id = id .. "_readout", text_size = 11, color = C.muted } },
            ui.overlay { w_full = true, h = 20, on_resize = function(size) width:set(size.w) end,
                ui.div { absolute = true, top = 7, left = 0, right = 0, h = 6, rounded = 3,
                    opacity = 0.18, bg_shader = k.dither, shader_params = { C.secondary, { 1.5, 5, 1, 0 } } },
                ui.div { absolute = true, top = 7, left = 0, h = 6, w = function() return fraction() * 100 .. "%" end,
                    bg_shader = k.dither, shader_params = { C.secondary, { 1.5, 5, 1, 0 } } },
                ui.div { absolute = true, top = 3, left = function() return fraction() * (width:get() - 12) end,
                    w = 12, h = 14, rounded = 3, bg = C.text },
                -- Keep the engine's slider input, keyboard handling and accessibility under the authored track.
                ui.slider { id = id, value = value, min = min, max = max, h = 20, w_full = true, p = 0,
                    opacity = 0.001, bg = "#00000000", border = 0 },
            },
        }, 4)
    end
    local function valueSelect(id, options, initial, multi)
        local open = ui.state(id .. "_open", false)
        local value = ui.state(id .. "_value", initial)
        local counts = ui.state(id .. "_choices", { true, true, false, false })
        local selected = ui.computed(function()
            if not multi then return value:get() end
            local n = 0; for _, on in counts:get() do if on then n += 1 end end
            return n > 0 and n .. " selected" or "Nothing shared"
        end)
        local items = {}
        for i, option in options do
            items[i] = ui.button { p = 0, id = id .. "_option_" .. i, h = 30, w_full = true, px = 8, rounded = 6,
                direction = "row", items = "center", bg = "#00000000", hover = { bg = C.selected },
                on_click = function()
                    if multi then local next = table.clone(counts:peek()); next[i] = not next[i]; counts:set(next)
                    else value:set(option); open:set(false) end
                end,
                txt(option), ui.div { grow = 1 }, ui.text { function()
                    return (multi and counts:get()[i] or not multi and value:get() == option) and "✓" or ""
                end, color = C.secondary },
            }
        end
        return ui.popover { open = open, w = 190, p = 5, bg = C.elevated, rounded = 10, border = 1, border_color = C.strong,
            anchor = ui.button { p = 0, id = id, px = 10, h = 30, rounded = 8, bg = C.raised, border = 1, border_color = C.strong,
                direction = "row", gap = 9, items = "center", on_click = function() open:toggle() end,
                ui.text { function() return selected:get() end, id = id .. "_label", text_size = 12.5 }, icon("chevrons-up-down", 12) },
            items,
        }
    end
    local recording = ui.state("shortcut_recording", false)
    local keys = ui.state("shortcut_keys", "⌘ ⇧ L")
    local shortcut = ui.row { gap = 10, items = "center", px = 10, py = 8, rounded = 8, bg = C.inset,
        ui.text { function() return recording:get() and "Press keys…" or keys:get() end, id = "shortcut_keys", font = "Geist Mono", text_size = 11 },
        ui.button { p = 0, id = "record_shortcut", px = 9, h = 26, bg = C.raised, rounded = 6, focusable = true,
            on_click = function() recording:toggle() end,
            on_key = function(e)
                if not recording:peek() then return end
                if e.key == "Escape" then recording:set(false); return true end
                if table.find({ "Alt", "Meta", "Control", "Shift" }, e.key) then return true end
                keys:set((e.meta and "⌘ " or "") .. (e.ctrl and "⌃ " or "") .. (e.alt and "⌥ " or "") .. (e.shift and "⇧ " or "") .. string.upper(e.key))
                recording:set(false); return true
            end,
            ui.text { function() return recording:get() and "Cancel" or "Record" end, text_size = 12 },
        },
    }
    local tint = ui.state("tint", 1)
    local swatches = {}
    -- shortcut: sampled tint colors; use the source OKLCH painter for theme editing.
    local colors = { "#858379", "#73859E", "#6D8C73", "#938060", "#9B7775", "#8A7CA5" }
    for i, color in colors do
        swatches[i] = ui.button { p = 0, id = "tint_" .. i, w = 38, h = 38, p = 5, rounded = 10, bg = "#00000000",
            border = 1, border_color = function() return tint:get() == i and C.secondary or "#00000000" end,
            on_click = function() tint:set(i) end, ui.div { w = 26, h = 26, rounded = 13, bg = color, border = 4, border_color = C.strong } }
    end
    local statuses = {}
    local activeTop = ui.state("active_orb_top", 4000)
    for i, label in { "Ready", "AI off", "Working", "Paused" } do
        statuses[i] = ui.column { gap = 8, items = "center",
            ui.div { id = i == 3 and "status_working" or nil, w = 28, h = 28, rounded = 14,
                on_mount = i == 3 and function(el)
                    ui.effect(function()
                        k.viewport:get()
                        activeTop:set((el.y or 0) + k.pageOffset:peek())
                    end)
                end or nil,
                bg = i == 3 and "#18221A" or i == 4 and C.elevated or C.raised,
                bg_shader = k.dither, shader_params = { i == 3 and "#9EE07E" or i == 4 and C.focus or C.text,
                    { 1.5, 0, i % 2 == 0 and 0.12 or 1, 0 } } }, txt(label, 12, C.tertiary) }
    end
    local activeVisible = ui.state("active_orb_visible", false)
    local activeOrb = anime.animate("#status_working", { shaderTime = { 0, 6 }, duration = 6000, loop = true,
        frameRate = 12, ease = "linear", autoplay = false })
    -- The active demo's 12 fps clock stops when its card scrolls out of view.
    ui.effect(function()
        local y = activeTop:get() - k.pageOffset:get()
        activeVisible:set(y + 28 > 49 and y < k.viewport:get().h)
        if activeVisible:get() and not k.reduced:get() then activeOrb:resume() else activeOrb:pause() end
    end)
    local function link(id, label)
        return ui.button { p = 0, id = id, bg = "#00000000", hover = { color = C.text }, color = C.secondary, text_size = 12.5, label }
    end
    local badges = {}
    for i, label in { "Neutral", "Beta", "Connected", "3 waiting", "Failed" } do
        local color = ({ C.secondary, "#9FB8D9", "#A6D4AE", "#E2C98F", C.error })[i]
        badges[i] = ui.div { h = 20, px = 8, rounded = 6, bg = color .. "1F", items = "center", txt(label, 11, color) }
    end
    local avatars = {}
    for _, initial in { "R", "S", "A" } do
        table.insert(avatars, ui.div { w = 28, h = 28, rounded = 14, bg = C.active, items = "center", justify = "center", txt(initial, 12) })
    end
    local function tooltip(label, keys)
        return ui.row { h = 26, px = 8, gap = 8, rounded = 6, bg = C.text, items = "center",
            txt(label, 11.5, C.panel), keys and txt(keys, 10, "#12121199") or nil }
    end
    local function setting(label, description, child)
        return ui.row { w_full = true, py = 14, gap = 16, items = "center",
            border = 1, border_sides = { 0, 0, 1, 0 }, border_color = C.line,
            ui.column { grow = 1, shrink = 1, gap = 4, txt(label, 13), ui.text { description, text_size = 12, color = C.tertiary, wrap = true } }, child }
    end
    local function notice(id, title, text, action, error)
        return ui.overlay { w_full = true, p = 14, rounded = 12, bg = C.elevated, border = 1, border_color = C.strong,
            ui.div { absolute = true, top = 0, bottom = 0, left = 0, right = 0, rounded = 12, bg_shader = k.dither,
                shader_params = { error and "#4A2A22" or "#3A3933", { 1.5, 2, 1, 0.2 } } },
            ui.row { w_full = true, gap = 10, items = "center",
                error and icon("circle-alert", 16, C.error) or orb(16),
                ui.column { grow = 1, shrink = 1, gap = 4, txt(title, 12.5), ui.text { text, text_size = 12, color = C.tertiary, wrap = true } },
                action and button(id, action, "secondary") or nil,
            },
        }
    end
    local modalName = ui.state("engine_name", "")
    local modalUrl = ui.state("engine_url", "")
    local modal = ui.column { w_full = true, max_w = 420, p = 22, rounded = 14, bg = C.elevated,
        border = 1, border_color = C.strong, gap = 16,
        txt("Add search engine", 18), txt("Any site with a search results page works.", 12, C.tertiary),
        stack({ txt("Name", 12, C.secondary), ui.input { id = "engine_name", label = "Wikipedia", value = modalName, h = 32, w_full = true, bg = C.inset, rounded = 6 } }, 6),
        stack({ txt("URL with %s in place of the query", 12, C.secondary), ui.input { id = "engine_url", label = "https://en.wikipedia.org/w/index.php?search=%s", value = modalUrl, h = 32, w_full = true, bg = C.inset, rounded = 6 } }, 6),
        ui.row { w_full = true, justify = "end", gap = 8, button("engine_cancel", "Cancel", "secondary"), button("engine_add", "Add", "primary") },
    }
    local dialogCheck = ui.state("delete_links", false)
    local dialog = ui.column { w_full = true, max_w = 420, p = 20, rounded = 14, bg = C.elevated, border = 1, border_color = C.strong, gap = 18,
        txt("Delete all chats?", 15), ui.text { "This removes 214 chats and their attachments from this device and your account.", text_size = 12.5, color = C.secondary, wrap = true },
        ui.checkbox { id = "delete_links", label = "Also delete shared links", value = dialogCheck, text_size = 12 },
        ui.row { w_full = true, justify = "end", gap = 8, button("dialog_cancel", "Cancel", "secondary"), button("dialog_delete", "Delete all", "danger") },
    }
    local paletteOpen = ui.state("palette_open", false)
    local query = ui.state("palette_query", "")
    local scope = ui.state("palette_scope", 1)
    local selected = ui.state("palette_selected", 1)
    local paletteOffset = ui.state("palette_offset", 0)
    local scopes = { "All", "Tabs", "Chats", "History", "Commands" }
    local paletteData = {
        { "The Verge", "Tab", 2, "panel-top" }, { "GitHub", "Tab", 2, "panel-top" }, { "Motion review", "Chat", 3, "message-square" },
        { "Lisbon restaurants – OpenTable", "opentable.com/lisbon", 4, "history" }, { "Bayer matrix – Wikipedia", "en.wikipedia.org/wiki/Bayer_matrix", 4, "history" },
        { "Spring – Motion", "motion.dev/docs/spring", 4, "history" }, { "Cycle 42 · Linear", "linear.app/ask/cycle/42", 4, "history" },
        { "Vendor pricing comparison", "notion.so/ask/vendors", 4, "history" },
        { "New chat", "Command", 5, "message-square" }, { "New tab", "Command", 5, "plus" },
        { "Close duplicate tabs", "Command", 5, "copy" }, { "Clear browsing data…", "Command", 5, "trash-2" },
        { "Settings", "Command", 5, "settings" }, { "Components", "Command", 5, "component" },
    }
    local results = ui.computed(function()
        local q = string.lower(query:get()):match("^%s*(.-)%s*$")
        local onlyCommands = q:sub(1, 1) == ">" or scope:get() == 5
        q = q:gsub("^>%s*", "")
        local out = {}
        for _, item in paletteData do
            if (onlyCommands and item[3] == 5 or not onlyCommands and (scope:get() == 1 or item[3] == scope:get()))
                and (q == "" or string.find(string.lower(item[1] .. " " .. item[2]), q, 1, true)) then table.insert(out, item) end
        end
        return out
    end)
    ui.effect(function() results:get(); selected:set(1); paletteOffset:set(0) end)
    local scopeButtons = {}
    for i, label in scopes do scopeButtons[i] = ui.button { p = 0, id = "palette_scope_" .. i, px = 12, h = 34,
        bg = function() return scope:get() == i and C.selected or "#00000000" end, rounded = 6,
        on_click = function() scope:set(i) end, txt(label, 12) } end
    local function openPalette() query:set(""); scope:set(1); selected:set(1); paletteOpen:set(true) end
    local palette = ui.column { w_full = true,
        ui.div { h = 440, w_full = true, items = "center", justify = "center", button("open_palette", "Search anything  ⌘K", "secondary", nil, nil, openPalette) },
        ui.modal { id = "palette", label = "Search anything", open = paletteOpen, w = 680, h = 440, p = 0, gap = 0,
            rounded = 14, bg = C.elevated, border = 1, border_color = C.strong,
            on_key = function(e)
                if e.key == "Tab" then scope:set((scope:peek() - 1 + (e.shift and -1 or 1)) % 5 + 1); return true end
                if (e.meta or e.ctrl) and tonumber(e.key) and tonumber(e.key) <= 5 then scope:set(tonumber(e.key)); return true end
                local d = ({ ArrowDown = 1, ArrowUp = -1 })[e.key]
                if d and #results:peek() > 0 then
                    local i = (selected:peek() - 1 + d) % #results:peek() + 1
                    selected:set(i); paletteOffset:set(math.max(0, (i - 5) * 44)); return true
                end
                if e.key == "Enter" then paletteOpen:set(false); return true end
            end,
            ui.row { w_full = true, p = 16, gap = 12, items = "center", icon("search", 16),
                ui.input { id = "palette_query", label = "Search anything…", value = query, grow = 1, h = 32, bg = "#00000000", border = 0 },
                ui.button { p = 0, on_click = function() paletteOpen:set(false) end, bg = "#00000000", txt("Esc  Close", 11, C.muted) } },
            ui.row { w_full = true, px = 12, gap = 4, scopeButtons },
            ui.scroll { w_full = true, grow = 1, shrink = 1, min_h = 0, offset = paletteOffset,
                ui.column { w_full = true, px = 8,
                    ui.each(results, function(item, i)
                        return ui.button { p = 0, id = "palette_result_" .. i, w_full = true, h = 44, px = 12, rounded = 7,
                            direction = "row", items = "center", gap = 12,
                            bg = function() return selected:get() == i and C.selected or "#00000000" end,
                            on_hover = function(inside) if inside then selected:set(i) end end,
                            on_click = function() paletteOpen:set(false) end, icon(item[4]),
                            ui.text { item[1], grow = 1, text_size = 12.5 }, txt(item[2], 11, C.tertiary),
                        }
                    end),
                    ui.show(function() return #results:get() == 0 end, ui.column { p = 28, gap = 8,
                        txt("No matches", 14), txt("Enter to open or search the web · Shift Enter to ask", 12, C.tertiary) }),
                },
            },
            ui.div { px = 16, py = 12, w_full = true, txt("↑↓ Select    ⇥ Category    ↵ Open    ⇧↵ Ask", 11, C.muted) },
        },
    }
    return {
        on_key = function(e)
            if (e.meta or e.ctrl) and string.lower(e.key) == "k" then
                if paletteOpen:peek() then paletteOpen:set(false) else openPalette() end
                return true
            end
        end,
        sliders = ui.column { w_full = true, max_w = 300, gap = 20, slider("volume", "Volume", 64, 0, 100, "%"), slider("text_size", "Text size", 15, 11, 24, "px") },
        progress = ui.column { w_full = true, max_w = 300, gap = 20, progress("ask-0.1.1-arm64.dmg", 0.63, "214 of 338 MB"), progress("Indexing history", 0.28), progress("Sync complete", 1) },
        palette = palette,
        selects = pair { valueSelect("engine_select", { "Google", "DuckDuckGo", "Kagi", "Bing" }, "Kagi"), valueSelect("sharing_select", { "Tabs", "History", "Bookmarks", "Downloads" }, "", true) },
        shortcut = stack { pair { keycap("⌘J"), keycap("⌥  Tab"), keycap("↑↓"), keycap("⏎") }, shortcut },
        tints = pair(swatches), status = pair(statuses), modal = modal,
        links = pair { link("reset_link", "Reset"), link("learn_link", "Learn more"), link("show_all_link", "Show all 214") },
        badges = stack { pair(badges), pair { keycap("⌘J"), keycap("⌥1"), keycap("⇧⌘T"), pair(avatars) } },
        tooltips = stack { tooltip("Toggle sidebar", "⌘S"), tooltip("New tab") },
        settings = ui.column { w_full = true, max_w = 520,
            setting("Current tab", "Shared automatically when the panel is open.", k.toggle("current_tab", "", true)),
            setting("Open assistant", "Toggles the side panel from any page.", keycap("⌘J")),
            setting("Developer mode", "Shows extension IDs and Load unpacked.", k.toggle("developer_mode", "", false)),
        },
        menu = ui.column { w = 248, p = 5, gap = 1, bg = C.elevated, rounded = 10, border = 1, border_color = C.strong,
            ui.button { p = 0, h = 30, px = 8, direction = "row", gap = 9, items = "center", bg = "#00000000", hover = { bg = C.selected }, icon("plus"), txt("New tab to the right"), ui.div { grow = 1 }, txt("⌘T", 10, C.muted) },
            ui.button { p = 0, h = 30, px = 8, direction = "row", gap = 9, items = "center", bg = "#00000000", hover = { bg = C.selected }, icon("copy"), txt("Duplicate") },
            ui.button { p = 0, h = 30, px = 8, direction = "row", gap = 9, items = "center", bg = "#00000000", hover = { bg = C.selected }, icon("panel-left"), txt("Move to space"), ui.div { grow = 1 }, icon("chevron-right", 12) },
            ui.button { p = 0, h = 30, px = 8, direction = "row", gap = 9, items = "center", bg = "#00000000", hover = { bg = C.selected }, icon("x", 14, C.error), txt("Close tab", 12.5, C.error), ui.div { grow = 1 }, txt("⌘W", 10, C.muted) },
        },
        alerts = stack { notice("alert_run", "Changes apply after the current run", "The automation picks up new instructions on its next scheduled run.", "Run now"),
            notice("alert_retry", "Couldn't reach opentable.com", "The page didn't load in time. Ask can try again or open it in a tab.", "Retry", true) },
        preview = ui.column { w_full = true, max_w = 520, p = 16, gap = 10, bg = C.elevated, rounded = 10, border = 1, border_color = C.strong,
            ui.row { gap = 6, items = "center", orb(12), txt("9 hours ago", 11, C.muted) },
            txt("Work network access explained", 15), ui.text { "A ChatGPT Work setting that controls whether code and shell commands can reach the internet.", text_size = 12.5, color = C.secondary, wrap = true } },
        dialog = dialog,
        toasts = pair { notice("preview_undo", "Saved to Reading list", "3 tabs from Lisbon trip", "Undo"),
            notice("preview_retry", "Couldn't send to Linear", "The connection expired. Sign in again.", "Retry", true) },
    }
end
