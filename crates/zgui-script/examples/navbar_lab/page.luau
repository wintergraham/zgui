-- The dummy page under the bar (navbar.js docHtml): new tab, offline, internal Ask pages, search results and
-- articles, with reader mode.
local T = require("./theme")
local model = require("./model")
local dither = require("./dither")
local C, txt = T.C, T.txt

local PARA = {
    "The address bar is the one control every page shares, so it carries more state than anything else on screen: where you are, whether the connection is safe, how far a load has got and what you can do next.",
    "When a page scrolls, the navigator steps out of the way. It shrinks to a small identity pill, and comes back when you scroll up, click it or press the shortcut.",
    "Suggestions stay local. Open tabs, bookmarks and history are matched as you type, and the first match completes inline so a couple of letters is usually enough.",
    "Tab switches the field into Ask. The same sheet becomes a composer with the current page already attached as context.",
    "Site controls live behind the page icon. Reading and acting on a page are separate permissions, and each origin keeps its own camera, microphone and notification choices.",
}
local SETTINGS = { "Save browsing history", "Let Ask read open tabs", "Clear cookies on quit", "Saved passwords",
    "Site permissions", "Secure DNS" }
local RESULT_HOSTS = { "en.wikipedia.org", "github.com", "theverge.com", "news.ycombinator.com", "rtings.com" }

local function orb(size)
    return ui.div { w = size, h = size, bg_shader = dither.fill, shader_params = dither.params(C.text, dither.ORB) }
end
local function eyebrow(children)
    return ui.row { gap = 8, items = "center", children }
end
local function mono(label)
    return T.text { string.upper(label), font = "Geist Mono", text_size = 11, letter_spacing = 1, color = C.muted }
end
local function h1(label)
    return T.text { label, text_size = 30, weight = 500, letter_spacing = -0.5, line_height = 36, wrap = true,
        w_full = true, mt = 10, mb = 8, color = C.text }
end
local function article(children, reader)
    return ui.column { w_full = true, max_w = reader and 560 or 640, self = "center", pt = 96, pb = 160, px = 32, children }
end
local function para(text, reader)
    return T.text { text, wrap = true, w_full = true, mb = 18,
        font = reader and "Georgia" or nil, text_size = reader and 18 or 15,
        line_height = reader and 18 * 1.7 or 15 * 1.65, color = reader and C.text or C.secondary }
end
local function figure()
    -- margin: 6px 0 24px, its top collapsing into the paragraph's 18px above (CSS margin collapsing).
    return ui.div { w_full = true, h = 220, rounded = 10, mb = 24, border = 1, border_color = C.subtle,
        gradient = { angle = 135, stops = { "#1F1F1D", "#262624" } } }
end

-- `doc` is { url, title, reader, offline }.
return function(doc)
    local url, title = doc.url, doc.title
    if model.isEmptyPage(url) then
        return ui.column { w_full = true, grow = 1, gap = 10, items = "center", justify = "center",
            orb(28), txt("New tab", 13, C.muted) }
    end
    if doc.offline and not model.isNative(url) then
        return ui.column { w_full = true, grow = 1, gap = 10, items = "center", justify = "center",
            T.icon("wifi-off", 22, C.muted),
            T.text { "You're offline", text_size = 18, weight = 500, color = C.text, mt = 6 },
            txt("Check your connection, then reload the page.", 13, C.muted) }
    end
    if model.starts(url, "ask://") then
        local _, parts = model.displayParts(url)
        local rows = { eyebrow { mono("ASK · " .. (parts[1] or "")) }, h1(title) }
        for i = 0, 11 do
            table.insert(rows, ui.row { w_full = true, pt = 14, pb = 15, items = "center", justify = "between",
                border = 1, border_sides = { 0, 0, 1, 0 }, border_color = C.subtle,
                ui.column { gap = 3, T.text { SETTINGS[i % 6 + 1], text_size = 13, weight = 500, color = C.text },
                    txt("Dummy internal page", 12, C.muted) },
                ui.div { w = 32, h = 18, rounded = 9, bg = i % 3 == 0 and C.text or C.strong } })
        end
        return article(rows)
    end
    if model.starts(url, model.SEARCH) then
        local q = model.query(url, "q") or ""
        local rows = { eyebrow { mono("DUCKDUCKGO") }, h1(q) }
        for i = 0, 9 do
            table.insert(rows, ui.column { w_full = true, pt = 14, pb = 15, gap = 4, border = 1, border_sides = { 0, 0, 1, 0 },
                border_color = C.subtle,
                txt(RESULT_HOSTS[i % 5 + 1], 12, C.muted),
                T.text { q .. " — result " .. (i + 1), text_size = 15, weight = 500, color = C.text },
                T.text { PARA[i % #PARA + 1], text_size = 13, line_height = 13 * 1.65, wrap = true, w_full = true,
                    color = C.secondary } })
        end
        return article(rows)
    end
    local s = model.site(url)
    local rows = {
        eyebrow { s and ui.image { src = "assets/" .. s.fav, w = 14, h = 14, rounded = 3 } or nil,
            mono(s and s.name or model.hostOf(url)) },
        h1(title),
        T.text { "Dummy page · " .. url, text_size = 12, color = C.muted, mb = 28, truncate = true, w_full = true },
    }
    for i = 0, 15 do
        if i % 4 == 2 then
            if not doc.reader then table.insert(rows, figure()) end
        else
            table.insert(rows, para(PARA[i % #PARA + 1], doc.reader))
        end
    end
    return article(rows, doc.reader)
end
