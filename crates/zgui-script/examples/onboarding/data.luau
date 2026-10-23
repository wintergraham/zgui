-- What the onboarding knows about: the browsers it found on this Mac, what
-- each holds, the agent's levels and the rules they set. All of it canned:
-- nothing is read from the disk and nothing leaves the window.
local D = {}

export type Browser = {
	id: string,
	name: string,
	meta: string,
	plate: boolean?, -- the logo is a mark on a white plate (Chrome, Arc, Edge)
	chromium: boolean?, -- keeps passwords behind a Keychain "Safe Storage" key
	bookmarks: number,
	pages: number,
	logins: number,
	tabs: number,
	windows: number,
	extensions: number,
	compatible: number,
	size: number, -- how long its import takes, relative to Chrome
}

-- In the board's order: four to a row.
D.browsers = {
	{ id = "chrome", name = "Chrome", meta = "Default · 2 profiles", plate = true, chromium = true, bookmarks = 1204, pages = 12400, logins = 288, tabs = 19, windows = 3, extensions = 11, compatible = 9, size = 1 },
	{ id = "aside", name = "Aside", meta = "Used yesterday", chromium = true, bookmarks = 86, pages = 5600, logins = 24, tabs = 4, windows = 1, extensions = 3, compatible = 3, size = 0.6 },
	{ id = "safari", name = "Safari", meta = "1 profile", bookmarks = 412, pages = 9100, logins = 140, tabs = 12, windows = 2, extensions = 0, compatible = 0, size = 0.8 },
	{ id = "arc", name = "Arc", meta = "3 spaces", plate = true, chromium = true, bookmarks = 238, pages = 7300, logins = 96, tabs = 31, windows = 3, extensions = 6, compatible = 6, size = 0.9 },
	{ id = "dia", name = "Dia", meta = "Used last week", chromium = true, bookmarks = 54, pages = 2100, logins = 18, tabs = 6, windows = 1, extensions = 2, compatible = 2, size = 0.4 },
	{ id = "brave", name = "Brave", meta = "1 profile", chromium = true, bookmarks = 167, pages = 4200, logins = 61, tabs = 9, windows = 2, extensions = 4, compatible = 3, size = 0.6 },
	{ id = "helium", name = "Helium", meta = "1 profile", chromium = true, bookmarks = 22, pages = 800, logins = 7, tabs = 3, windows = 1, extensions = 1, compatible = 1, size = 0.3 },
	{ id = "firefox", name = "Firefox", meta = "2 profiles", bookmarks = 530, pages = 15800, logins = 203, tabs = 14, windows = 2, extensions = 7, compatible = 0, size = 1.1 },
	{ id = "comet", name = "Comet", meta = "1 profile", chromium = true, bookmarks = 31, pages = 1200, logins = 12, tabs = 5, windows = 1, extensions = 2, compatible = 2, size = 0.3 },
	{ id = "vivaldi", name = "Vivaldi", meta = "Not used recently", chromium = true, bookmarks = 95, pages = 600, logins = 40, tabs = 2, windows = 1, extensions = 3, compatible = 2, size = 0.4 },
	{ id = "edge", name = "Edge", meta = "Not used recently", plate = true, chromium = true, bookmarks = 48, pages = 300, logins = 15, tabs = 1, windows = 1, extensions = 2, compatible = 2, size = 0.3 },
} :: { Browser }
D.by_id = {} :: { [string]: Browser }
for _, b in D.browsers do
	D.by_id[b.id] = b
end
-- The tile after the browsers: an exported bookmarks file.
D.file = { id = "file", name = "Bookmarks.html", bookmarks = 214, pages = 0, logins = 0, tabs = 0, windows = 0, extensions = 0, compatible = 0, size = 0.2, meta = "214 bookmarks" } :: Browser

-- What can come over, in the board's order, and how long each part takes.
D.kinds = {
	{ id = "bookmarks", title = "Bookmarks", icon = "bookmark", weight = 1, doing = "Copying bookmarks" },
	{ id = "history", title = "History", icon = "clock", weight = 3, doing = "Copying history" },
	{ id = "passwords", title = "Saved passwords", icon = "key", weight = 1.5, doing = "Reading saved passwords" },
	{ id = "tabs", title = "Open tabs", icon = "layers", weight = 0.5, doing = "Opening tab groups" },
	{ id = "extensions", title = "Extensions", icon = "package", weight = 1, doing = "Installing extensions" },
}

-- The agent's levels, left to right.
D.levels = {
	{ id = "navigate", name = "Navigate", hint = "Reads and clicks", meter = 1 },
	{ id = "assist", name = "Assist", hint = "Recommended", meter = 2 },
	{ id = "delegate", name = "Delegate", hint = "Pays and deletes", meter = 3 },
	{ id = "unguarded", name = "Unguarded", hint = "Never asks", meter = 4 },
}

-- The actions that can pause for approval: how "Choose per action" lists
-- them, and how the level breakdown says them either way.
D.rules = {
	{ id = "purchases", label = "Purchases and payments", does = "Pays at checkout with your saved cards", asks = "Purchases and payments" },
	{ id = "messages", label = "Sending messages", does = "Sends messages and replies", asks = "Sending messages" },
	{ id = "forms", label = "Submitting forms", does = "Submits forms, bookings and sign-ups", asks = "Submitting forms" },
	{ id = "signin", label = "Signing in", does = "Signs in with your saved passwords", asks = "Signing in" },
	{ id = "publishing", label = "Publishing and sharing", does = "Posts and shares links and files", asks = "Publishing and sharing" },
	{ id = "deleting", label = "Deleting things", does = "Deletes files, emails and bookings", asks = "Deleting things" },
	{ id = "account", label = "Account and security changes", does = "Changes passwords and account settings", asks = "Password and account changes" },
	{ id = "unverifiable", label = "Actions it can’t verify", does = "Clicks through pages it can't verify", asks = "Pages it can't verify" },
}
-- The order each column of the breakdown lists them in.
D.does_order = { "purchases", "deleting", "signin", "forms", "messages", "publishing", "account", "unverifiable" }
D.asks_order = { "purchases", "deleting", "account", "unverifiable", "publishing", "messages", "forms", "signin" }

D.FREQUENCIES = { "Always ask", "Once per site", "Don’t ask" }
local A, N = "Always ask", "Don’t ask"
-- What each level asks before; "Choose per action" starts from one of these.
D.presets = {
	navigate = { purchases = A, messages = A, forms = A, signin = A, publishing = A, deleting = A, account = A, unverifiable = A },
	assist = { purchases = A, messages = N, forms = N, signin = N, publishing = A, deleting = A, account = A, unverifiable = A },
	delegate = { purchases = N, messages = N, forms = N, signin = N, publishing = N, deleting = N, account = A, unverifiable = A },
	unguarded = { purchases = N, messages = N, forms = N, signin = N, publishing = N, deleting = N, account = N, unverifiable = N },
}

-- The tints, as the Appearance step names them.
D.tints = {
	{ id = "stone", name = "Stone", color = "#858379" },
	{ id = "slate", name = "Slate", color = "#7A8295" },
	{ id = "sage", name = "Sage", color = "#7B867A" },
	{ id = "sand", name = "Sand", color = "#8F8170" },
	{ id = "rose", name = "Rose", color = "#937E85" },
	{ id = "iris", name = "Iris", color = "#897D91" },
}

-- The steps, and the five the progress strip and footer count.
D.steps = {
	splash = { n = 0 },
	welcome = { n = 1, label = "Welcome" },
	browsers = { n = 2, label = "Import" },
	data = { n = 2, label = "Import" },
	importing = { n = 2, label = "Import" },
	appearance = { n = 3, label = "Appearance" },
	agent = { n = 4, label = "Agent" },
	per_action = { n = 4, label = "Agent" },
	ready = { n = 5, label = "Ready" },
}
-- Where each segment of the strip goes, once it has been reached.
D.segments = { "welcome", "browsers", "appearance", "agent", "ready" }

-- 1,204
function D.count(n: number): string
	local s = tostring(math.floor(n))
	local out = s:reverse():gsub("(%d%d%d)", "%1,"):reverse()
	return (out:gsub("^,", ""))
end

-- "Chrome", "Chrome and Aside", "Chrome, Aside and Arc"
function D.join(names: { string }): string
	if #names <= 1 then
		return names[1] or ""
	end
	return table.concat(names, ", ", 1, #names - 1) .. " and " .. names[#names]
end

return D
