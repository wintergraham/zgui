-- The omnibox model and dummy browser data from navbar.js (navbar_model.rs, state/navigation.rs, identity.rs,
-- floating.rs ScrollIntent, LoadProgress). Pure functions over plain tables: the view owns the states.
local M = {}

M.SEARCH = "https://duckduckgo.com/?q="
M.SITES = {
    ["theverge.com"] = { name = "The Verge", fav = "theverge.png", reader = true },
    ["github.com"] = { name = "GitHub", fav = "github-mark.png" },
    ["en.wikipedia.org"] = { name = "Wikipedia", fav = "wikipedia.png", reader = true },
    ["news.ycombinator.com"] = { name = "Hacker News", fav = "hackernews.png" },
    ["linear.app"] = { name = "Linear", fav = "linear.png" },
    ["figma.com"] = { name = "Figma", fav = "figma.png" },
    ["notion.so"] = { name = "Notion", fav = "notion.png" },
    ["youtube.com"] = { name = "YouTube", fav = "youtube.png" },
    ["google.com"] = { name = "Google", fav = "google.png" },
    ["rtings.com"] = { name = "RTINGS", fav = "rtings.png", reader = true },
    ["reddit.com"] = { name = "Reddit", fav = "reddit.png" },
}
M.TITLES = {
    ["https://www.theverge.com/tech/ask-browser-review"] = "Ask review: the browser that reads along | The Verge",
    ["https://github.com/ask-browser/ask/pull/412"] = "Floating navbar motion pass · Pull Request #412 · ask-browser/ask",
    ["https://github.com/ask-browser/ask"] = "ask-browser/ask: A browser with an assistant built in",
    ["https://en.wikipedia.org/wiki/Dither"] = "Dither - Wikipedia",
    ["https://en.wikipedia.org/wiki/Ordered_dithering"] = "Ordered dithering - Wikipedia",
    ["https://linear.app/ask/issue/ASK-218"] = "ASK-218 Navbar collapse hysteresis",
    ["https://news.ycombinator.com/"] = "Hacker News",
    ["https://www.figma.com/files/recent"] = "Recent files – Figma",
    ["https://www.notion.so/ask/Motion-notes"] = "Motion notes",
    ["https://www.youtube.com/watch?v=spring-physics"] = "Spring physics for UI, explained - YouTube",
    ["https://www.rtings.com/headphones/reviews/best/wireless"] = "The 6 Best Wireless Headphones - RTINGS.com",
    ["http://legacy-intranet.example/wiki"] = "Legacy intranet wiki",
    ["ask://newtab"] = "New tab",
    ["ask://settings/privacy"] = "Privacy and security",
    ["ask://password-manager/passwords"] = "Passwords",
    ["ask://extensions"] = "Extensions",
}
M.GLYPH = { copy = "copy", ask = "sparkles", search = "search", site = "search", history = "history",
    bookmark = "star", tab = "panels-top-left" }
M.PERMS = { allow = "Allow", ask = "Ask", block = "Block" }
-- Site permissions in the site menu: name, glyph, choices. Sound and pop-ups have no "ask" state.
M.SITE_PERMS = {
    { "Camera", "video" }, { "Microphone", "mic" }, { "Location", "map-pin" }, { "Notifications", "bell" },
    { "Pop-ups and redirects", "external-link", { "allow", "block" } }, { "Sound", "volume-2", { "allow", "block" } },
}
M.PERM_DEFAULT = { ["Pop-ups and redirects"] = "block", Sound = "allow" }
-- Installed extensions (dummy): "site" reads and changes the current site, "click" waits for the user to grant it,
-- "none" needs no access. A tile is { background, letter, ink } for extensions without a bundled icon.
M.EXTS = {
    { id = "protonpass", name = "Proton Pass", img = "protonpass.png", access = "site", meta = "2 logins" },
    { id = "ublock", name = "uBlock Origin", tile = { "#800000", "u", "#FFFFFF" }, access = "site", meta = "14 blocked" },
    { id = "darkreader", name = "Dark Reader", tile = { "#141E24", "D", "#E8E8E8" }, access = "site" },
    { id = "raindrop", name = "Raindrop.io", tile = { "#2E7FF0", "R", "#FFFFFF" }, access = "click" },
    { id = "kagi", name = "Kagi Search", tile = { "#FFB319", "K", "#1A1A18" }, access = "none" },
}
function M.ext(id)
    for _, x in M.EXTS do if x.id == id then return x end end
    return nil
end

local function starts(s, prefix) return string.sub(s, 1, #prefix) == prefix end
M.starts = starts
local function trim(s) return (string.gsub(s, "^%s*(.-)%s*$", "%1")) end
M.trim = trim
function M.isEmptyPage(url) return url == "about:blank" or url == "ask://newtab" end
function M.isNative(url) return starts(url, "ask://") or url == "about:blank" end

-- A URL's parts, as `new URL()` reads the dummy browser's addresses.
function M.parse(url)
    local scheme, rest = string.match(url, "^(%a[%w+.-]*):(.*)$")
    if not scheme then return nil end
    scheme = string.lower(scheme)
    if scheme ~= "http" and scheme ~= "https" then
        return { protocol = scheme .. ":", host = "", path = rest, search = "", hash = "" }
    end
    local authority, tail = string.match(rest, "^//([^/?#]*)(.*)$")
    if not authority then return nil end
    local host = string.lower((string.gsub(authority, "^.*@", "")))
    local path, search, hash = string.match(tail, "^([^?#]*)(%??[^#]*)(#?.*)$")
    if path == "" then path = "/" end
    return { protocol = scheme .. ":", host = host, hostname = (string.gsub(host, ":%d+$", "")), path = path,
        search = search, hash = hash }
end
local function decode(s)
    return (string.gsub(s, "%%(%x%x)", function(h) return string.char(tonumber(h, 16) :: number) end))
end
local function encode(s)
    return (string.gsub(s, "[^%w%-%._~!%*'%(%);:@&=%+%$,/%?#]", function(c) return string.format("%%%02X", string.byte(c)) end))
end
M.encodeURI = encode
local function encodeComponent(s)
    return (string.gsub(s, "[^%w%-%._~!%*'%(%)]", function(c) return string.format("%%%02X", string.byte(c)) end))
end
function M.query(url, key)
    local u = M.parse(url)
    if not u then return nil end
    for k, v in string.gmatch(string.sub(u.search, 2), "([^&=]+)=?([^&]*)") do
        if decode((string.gsub(k, "%+", " "))) == key then return decode((string.gsub(v, "%+", " "))) end
    end
    return nil
end

function M.hostOf(url)
    local u = M.parse(url)
    return u and u.hostname and (string.gsub(u.hostname, "^www%.", "")) or ""
end
function M.site(url) return M.SITES[M.hostOf(url)] end
function M.originOf(url)
    local u = M.parse(url)
    if u and u.hostname then return u.protocol .. "//" .. u.host end
    return nil
end
-- identity.rs display_parts
function M.displayParts(url)
    if M.isEmptyPage(url) then return "", {} end
    local parts = {}
    if starts(url, "ask://") then
        for p in string.gmatch(string.sub(url, 7), "[^/]+") do table.insert(parts, p) end
        return "ask", parts
    end
    local u = M.parse(url)
    if not u or not u.hostname then return url, {} end
    for p in string.gmatch(u.path, "[^/]+") do table.insert(parts, decode(p)) end
    return (string.gsub(u.hostname, "^www%.", "")), parts
end
function M.titleFor(url)
    if M.TITLES[url] then return M.TITLES[url] end
    if starts(url, M.SEARCH) then return (M.query(url, "q") or "") .. " at DuckDuckGo" end
    local host, parts = M.displayParts(url)
    local s = M.SITES[host]
    local last = parts[#parts]
    local page = last and (string.gsub((string.gsub(last, "[-_]+", " ")), "^.", string.upper)) or ""
    local name = s and s.name or host
    local out = {}
    if page ~= "" then table.insert(out, page) end
    if name ~= "" then table.insert(out, name) end
    local title = table.concat(out, " · ")
    return title ~= "" and title or url
end
-- browser::normalize_url_with_search (dummy)
function M.normalize(input)
    local v = trim(input)
    if v == "" then return nil end
    local lower = string.lower(v)
    if starts(lower, "http:") or starts(lower, "https:") or starts(lower, "ask:") or starts(lower, "about:") then return v end
    if not string.find(v, "%s") then
        local host = string.match(v, "^([^%s/]+)")
        local isDomain = host and string.match(host, "^[^%s/]+%.%a%a+$") or (host and string.match(host, "^[^%s/]+%.%a%a+:%d+$"))
        if isDomain or string.match(lower, "^localhost") then return "https://" .. v end
    end
    return M.SEARCH .. (string.gsub(encodeComponent(v), "%%20", "+"))
end
-- navbar_model.rs site_search
local SITE_SEARCH = {
    github = { "https://github.com/search", "q" }, ["github.com"] = { "https://github.com/search", "q" },
    youtube = { "https://www.youtube.com/results", "search_query" }, ["youtube.com"] = { "https://www.youtube.com/results", "search_query" },
    wikipedia = { "https://en.wikipedia.org/w/index.php", "search" }, ["wikipedia.org"] = { "https://en.wikipedia.org/w/index.php", "search" },
    ["en.wikipedia.org"] = { "https://en.wikipedia.org/w/index.php", "search" },
}
function M.siteSearch(s, q)
    local hit = SITE_SEARCH[(string.gsub(trim(s), "^www%.", ""))]
    if not hit then return M.normalize("site:" .. s .. " " .. q) end
    -- URLSearchParams: application/x-www-form-urlencoded.
    local form = string.gsub(q, "[^%w%*%-%._ ]", function(c) return string.format("%%%02X", string.byte(c)) end)
    return hit[1] .. "?" .. hit[2] .. "=" .. (string.gsub(form, " ", "+"))
end
-- navbar_model.rs query
function M.queryOf(input, mode)
    local v = trim(input)
    if mode.k == "ask" then return trim(starts(v, "?") and string.sub(v, 2) or v) end
    if mode.k == "tabs" then return trim(starts(v, "@tabs") and string.sub(v, 6) or v) end
    return v
end

-- floating.rs ScrollIntent, with src/app/navbar.rs HYSTERESIS.
M.H = { top = 48, direction = 24, jitter = 8, edge = 12, hover = 120, collapse = 400, addressHover = 150 }
function M.scrollIntent()
    local s = { last = 0, travel = 0, direction = 0 }
    function s.sample(y)
        local H = M.H
        if y <= 0 then s.last = 0; s.travel = 0; return false end
        local delta = y - s.last
        if math.abs(delta) < H.jitter then return nil end
        s.last = y
        local direction = delta > 0 and 1 or -1
        if direction ~= s.direction then s.travel = 0; s.direction = direction end
        s.travel += math.abs(delta)
        if direction > 0 and (y > H.top or s.travel >= H.direction) then s.travel = 0; return true end
        if direction < 0 and s.travel >= H.direction then s.travel = 0; return false end
        return nil
    end
    return s
end

-- Agent bar "Connecting" profile (sidebar_trace.rs connecting_progress): quintic ease-out into an exponential cap.
function M.connecting(t)
    t = math.clamp(t, 0, 1)
    local eased = 1 - (1 - t) ^ 5
    return 0.995 * (1 - math.exp(-3 * eased)) / (1 - math.exp(-3))
end
-- navbar.js FINISH: a finished load runs home on an ease-out cubic whose duration keeps its peak speed under
-- maxSpeed bar widths per second, then the dither fades over six frames of the 12 fps dither clock.
M.FINISH = { ms = 140, maxSpeed = 1.5, frames = 6, frameMs = 1000 / 12 }
M.CONNECT_MS = 8000

-- The suggestion rows for the current input (navbar_model.rs suggestions).
function M.suggestions(nb, tabs, active, saved)
    local mode = nb.mode
    local needle = nb.edited and string.lower(M.queryOf(nb.input, mode)) or ""
    local function matches(title, url)
        return needle == "" or string.find(string.lower(title), needle, 1, true) ~= nil
            or string.find(string.lower(url), needle, 1, true) ~= nil
    end
    local rows = {}
    local function has(value)
        for _, r in rows do if r.value == value then return true end end
        return false
    end
    if mode.k == "ask" then
        local prompts = needle ~= "" and { M.queryOf(nb.input, mode) } or { "Summarize this page", "Compare with my open tabs" }
        for _, prompt in prompts do
            table.insert(rows, { title = prompt, detail = "Ask your assistant", value = prompt, kind = "ask" })
        end
        return rows
    end
    local current = active.entries[active.index]
    local page = not M.isNative(current.url) and { title = current.title, url = current.url } or nil
    if needle == "" and mode.k ~= "tabs" and page then
        table.insert(rows, { title = "Ask about " .. page.title, detail = "This page",
            value = "Summarize this page: " .. page.title .. " (" .. page.url .. ")", kind = "ask" })
        table.insert(rows, { title = "Copy link", detail = page.url, value = page.url, kind = "copy" })
    end
    if nb.edited and needle ~= "" and mode.k ~= "tabs" then
        local q = M.queryOf(nb.input, mode)
        table.insert(rows, { title = q, detail = mode.k == "site" and "Search " .. mode.site or "Search or visit address",
            value = q, kind = mode.k == "site" and "site" or "search" })
        if mode.k == "search" then
            table.insert(rows, { title = q, detail = page and "Ask on " .. page.title or "Ask your assistant", value = q, kind = "ask" })
        end
    end
    for _, t in tabs do
        local e = t.entries[t.index]
        if t.id ~= active.id and matches(e.title, e.url) then
            table.insert(rows, { title = e.title, detail = e.url, value = e.url, kind = "tab", tab = t.id })
            if #rows == 8 then return rows end
        end
    end
    if mode.k == "tabs" then return rows end
    for _, b in saved.bookmarks do
        if matches(b.title, b.url) and not has(b.url) then
            table.insert(rows, { title = b.title, detail = b.url, value = b.url, kind = "bookmark" })
            if #rows == 8 then return rows end
        end
    end
    for i = #saved.history, 1, -1 do
        local h = saved.history[i]
        if matches(h.title, h.url) and not has(h.url) then
            table.insert(rows, { title = h.title, detail = h.url, value = h.url, kind = "history" })
            if #rows == 8 then return rows end
        end
    end
    return rows
end
-- Inline completion: the first bookmark, history or tab row whose address (without its scheme) extends the input.
function M.completion(nb, rows)
    if not nb.edited or nb.mode.k ~= "search" then return nil end
    local prefix = nb.input
    if prefix == "" or string.find(prefix, "%s") then return nil end
    for _, row in rows do
        if row.kind == "bookmark" or row.kind == "history" or row.kind == "tab" then
            local value = string.gsub(row.value, "^https?://", "")
            if #value > #prefix and string.lower(string.sub(value, 1, #prefix)) == string.lower(prefix) then return value end
        end
    end
    return nil
end

-- Identity tokens (navbar.js identityHtml): the host and path parts located in the raw URL; the text between them
-- becomes gaps holding both the identity text (i) and the raw text (r), e.g. "" / "https://www.", "/" / "/".
function M.identity(url)
    local host, parts = M.displayParts(url)
    if host == "" then return nil end
    local tokens, cur, lost = {}, 1, false
    local all = { host }
    for _, p in parts do table.insert(all, p) end
    for n, t in all do
        local at, len = nil, #t
        if not lost then
            at = string.find(url, t, cur, true)
            if not at then
                local e = encode(t)
                at = string.find(url, e, cur, true)
                len = #e
            end
        end
        if not at then lost = true end
        local raw = at and string.sub(url, cur, at - 1) or (n > 1 and "/" or "")
        table.insert(tokens, { kind = n > 1 and "sep" or "prefix", i = n > 1 and "/" or "", r = raw })
        table.insert(tokens, { kind = n > 1 and "part" or "host", text = t })
        if at then cur = at + len end
    end
    table.insert(tokens, { kind = "tail", i = "", r = lost and "" or string.sub(url, cur) })
    return tokens
end

return M
