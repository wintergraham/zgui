-- What the sidebar holds. Work is the board's master frame, row for row;
-- Personal and Research are the other two spaces its switcher shows.
local D = {}

-- Brand washes for an active pin (dither-fog-sweep): three colours, the
-- fill under the dither and the pin's border. Gmail and Figma are the
-- board's; the rest follow the same recipe from each brand's colours.
D.brands = {
	gmail = { "#EA4335", "#FBBC04", "#4285F4", fill = "#3E1D19", line = "#B03B30" },
	figma = { "#F24E1E", "#A259FF", "#1ABCFE", fill = "#302141", line = "#7E4BBE" },
	gcal = { "#4285F4", "#34A853", "#FBBC04", fill = "#17243A", line = "#3B6FC4" },
	linear = { "#5E6AD2", "#A78BFA", "#5E6AD2", fill = "#1D1F3A", line = "#5E6AD2" },
	spotify = { "#1DB954", "#1DB9A0", "#1ED760", fill = "#10281A", line = "#1A8F45" },
	notion = { "#FFFFFF", "#A3A199", "#6E6C65", fill = "#24231F", line = "#8F8D85" },
	["github-mark"] = { "#8B949E", "#F0F6FC", "#6E7681", fill = "#1E2024", line = "#6E7681" },
	chatgpt = { "#10A37F", "#74AA9C", "#19C37D", fill = "#11261F", line = "#13896B" },
	youtube = { "#FF0000", "#FF4E45", "#CC0000", fill = "#331414", line = "#B3261E" },
	reddit = { "#FF4500", "#FF8717", "#FF4500", fill = "#331A0F", line = "#C2410C" },
	wikipedia = { "#FFFFFF", "#A3A199", "#6E6C65", fill = "#24231F", line = "#8F8D85" },
	amazon = { "#FF9900", "#146EB4", "#FF9900", fill = "#2E2210", line = "#B36B00" },
	gmaps = { "#34A853", "#4285F4", "#EA4335", fill = "#13281B", line = "#2F8A48" },
	stripe = { "#635BFF", "#A960EE", "#00D4FF", fill = "#1E1C3A", line = "#5851DB" },
}

-- Every page, by id: title, favicon (a file in favicons/, or nil for a
-- globe) and address.
D.pages = {
	gmail = { "Inbox (3) – Gmail", "gmail", "mail.google.com" },
	gcal = { "Calendar – Week of 21 Oct", "gcal", "calendar.google.com" },
	linear = { "My issues · Linear", "linear", "linear.app" },
	spotify = { "Spotify – Daily Mix 1", "spotify", "open.spotify.com" },
	notion = { "Work overview", "notion", "notion.so" },
	figma = { "Sidebar v4 – Figma", "figma", "figma.com" },
	github = { "ask · Pull requests", "github-mark", "github.com" },
	chatgpt = { "ChatGPT", "chatgpt", "chatgpt.com" },
	stripe = { "Payments – Stripe", "stripe", "dashboard.stripe.com" },
	hn = { "Hacker News", "hackernews", "news.ycombinator.com" },
	data = { "Data controls", "chatgpt", "chatgpt.com/settings" },
	overview = { "Work overview", "notion", "notion.so/work" },
	google = { "Google", "google", "google.com" },
	inbox = { "Inbox · Linear", "linear", "linear.app/inbox" },
	spotify_tab = { "Spotify", "spotify", "open.spotify.com" },
	prs = { "Pull requests · ask", "github-mark", "github.com/ask/pulls" },
	sidebar = { "Sidebar v4 – Figma", "figma", "figma.com/design/sidebar-v4" },
	lofi = { "Lo-fi beats to design to", "youtube", "youtube.com/watch" },
	dither = { "Dithering – Wikipedia", "wikipedia", "en.wikipedia.org/wiki/Dither" },
	rl_verge = { "The Verge", "theverge", "theverge.com" },
	rl_hn = { "Show HN: a dithering shader", "hackernews", "news.ycombinator.com/item" },
	rl_wiki = { "Ordered dithering – Wikipedia", "wikipedia", "en.wikipedia.org/wiki/Ordered_dithering" },
	-- Personal
	p_gmail = { "Inbox – Gmail", "gmail", "mail.google.com" },
	p_youtube = { "YouTube", "youtube", "youtube.com" },
	p_spotify = { "Spotify – Liked songs", "spotify", "open.spotify.com" },
	p_reddit = { "r/MechanicalKeyboards", "reddit", "reddit.com" },
	p_maps = { "Google Maps", "gmaps", "maps.google.com" },
	p_amazon = { "Amazon.co.uk", "amazon", "amazon.co.uk" },
	p_cal = { "Calendar", "gcal", "calendar.google.com" },
	p_wiki = { "Wikipedia", "wikipedia", "wikipedia.org" },
	p_food = { "Shakshuka – BBC Good Food", "bbcgoodfood", "bbcgoodfood.com" },
	p_amazon_cart = { "Basket – Amazon", "amazon", "amazon.co.uk/cart" },
	p_rtings = { "Best headphones – RTINGS", "rtings", "rtings.com/headphones" },
	p_which = { "Headphones reviews – Which?", "which", "which.co.uk" },
	p_senn = { "Momentum 4 – Sennheiser", "sennheiser", "sennheiser.com" },
	p_jl = { "Headphones – John Lewis", "johnlewis", "johnlewis.com" },
	p_verge = { "The Verge", "theverge", "theverge.com" },
	-- Research
	r_wiki = { "Wikipedia", "wikipedia", "wikipedia.org" },
	r_github = { "GitHub", "github-mark", "github.com" },
	r_chatgpt = { "ChatGPT", "chatgpt", "chatgpt.com" },
	r_notion = { "Research notes", "notion", "notion.so/research" },
	r_figma = { "Moodboard – Figma", "figma", "figma.com" },
	r_linear = { "Research · Linear", "linear", "linear.app/research" },
	r_youtube = { "YouTube", "youtube", "youtube.com" },
	r_gcal = { "Calendar", "gcal", "calendar.google.com" },
	r_bayer = { "Ordered dithering – Wikipedia", "wikipedia", "en.wikipedia.org/wiki/Ordered_dithering" },
	r_hn = { "Ask HN: favourite shaders?", "hackernews", "news.ycombinator.com" },
	r_shader = { "bayer-dither.glsl · GitHub", "github-mark", "github.com/ask/designs" },
	r_expensify = { "Conference expenses", "expensify", "expensify.com" },
}

-- Each space, top to bottom as the sidebar shows it. Tabs carry their
-- state: `asleep` (unloaded from memory), `unread`, `audio`, `reading`
-- (an agent is on the page) and `loading` (resolving its favicon).
D.spaces = {
	{
		id = "work", name = "Work", icon = "briefcase", key = "⌃1",
		mark = { ink = "#EDEBE5", angle = 225, cy = 0.55, radius = 0.42 },
		pins = { { "gmail", "gcal", "linear", "spotify" }, { "notion", "figma", "github", "chatgpt" } },
		chats = { { "c_network", "Work network access", "9:26" }, { "c_vendors", "Compare vendors", "Tue" }, { "c_q3", "Q3 planning notes", "Mon" } },
		top = { "stripe", "hn" },
		groups = {
			{ id = "g_ask", name = "Project · Ask", open = true, tabs = { "data", "overview" } },
			{ id = "g_daily", name = "Daily", open = true, tabs = { "google", "inbox", "spotify_tab" } },
			{ id = "g_reading", name = "Reading list", open = false, tabs = { "rl_verge", "rl_hn", "rl_wiki" } },
		},
		bottom = { "prs", "sidebar", "lofi", "dither" },
		selected = "data", chat = "c_network",
		flags = {
			data = { reading = true }, overview = { asleep = true }, google = { loading = true },
			inbox = { unread = true }, spotify_tab = { audio = true },
		},
	},
	{
		id = "personal", name = "Personal", icon = "home", key = "⌃2",
		mark = { ink = "#E2C98F", angle = 300, cy = 0.5, radius = 0.42 },
		pins = { { "p_gmail", "p_youtube", "p_spotify", "p_reddit" }, { "p_maps", "p_amazon", "p_cal", "p_wiki" } },
		chats = { { "c_lisbon", "Weekend in Lisbon", "Sat" }, { "c_phones", "Headphones under £300", "Thu" } },
		top = { "p_food", "p_amazon_cart" },
		groups = {
			{ id = "g_phones", name = "Headphones", open = true, tabs = { "p_rtings", "p_which", "p_senn", "p_jl" } },
		},
		bottom = { "p_verge" },
		selected = "p_rtings", chat = "c_lisbon",
		flags = { p_which = { unread = true }, p_jl = { asleep = true } },
	},
	{
		id = "research", name = "Research", icon = "book-open", key = "⌃3",
		mark = { ink = "#A9BABF", angle = 160, cy = 0.5, radius = 0.42 },
		pins = { { "r_wiki", "r_github", "r_chatgpt", "r_notion" }, { "r_figma", "r_linear", "r_youtube", "r_gcal" } },
		chats = { { "c_dither", "Ordered dithering history", "Wed" } },
		top = { "r_bayer" },
		groups = {
			{ id = "g_shaders", name = "Shaders", open = true, tabs = { "r_shader", "r_hn" } },
			{ id = "g_conf", name = "Conference", open = false, tabs = { "r_expensify" } },
		},
		bottom = {},
		selected = "r_bayer", chat = "c_dither",
		flags = { r_hn = { unread = true } },
	},
}

-- Agents, front first. `steps` are what the PiP's footer reads; `target`
-- is where the cursor works on the page miniature (points, in its frame).
D.agents = {
	{
		id = "dinner", task = "Book dinner for 4", domain = "opentable.com", state = "working", step = 4,
		page = "opentable", accent = "#C9634F",
		tabs = { { "OpenTable", "opentable" }, { "Maps", "gmaps" }, { "Calendar", "gcal" } },
		steps = {
			{ "Opening OpenTable", 15, 13 }, { "Picking a date", 39, 44 }, { "Picking a time", 102, 44 },
			{ "Typing party size", 165, 44 }, { "Choosing a table", 80, 60 }, { "Confirming booking", 41, 85 },
		},
		finished = "Booked · Fri 19:30",
	},
	{
		id = "passport", task = "Renew passport", domain = "gov.uk", state = "waiting", step = 3,
		page = "gov.uk", accent = "#8FA3C9", tabs = { { "GOV.UK", nil } },
		steps = { { "Reading guidance", 39, 30 }, { "Filling details", 102, 44 }, { "Confirm your photo", 165, 44 }, { "Paying the fee", 41, 85 } },
		ask = "needs you", finished = "Application sent",
	},
	{
		id = "flights", task = "Flights to Lisbon", domain = "5 tabs", state = "reading", step = 2,
		page = "google.com/travel", accent = "#6FA88F",
		tabs = { { "Flights", "google" }, { "Maps", "gmaps" }, { "Calendar", "gcal" }, { "Mail", "gmail" }, { "Notes", "notion" } },
		steps = { { "Reading fares", 39, 44 }, { "Comparing times", 102, 44 }, { "Checking bags", 165, 44 }, { "Holding seats", 80, 60 }, { "Summarising", 41, 85 } },
		finished = "3 options found",
	},
}

-- Where an address typed in the search field lands: a known favicon by
-- name, else a globe.
D.known = {
	google = "google", gmail = "gmail", github = "github-mark", figma = "figma", notion = "notion",
	linear = "linear", spotify = "spotify", youtube = "youtube", wikipedia = "wikipedia", reddit = "reddit",
	amazon = "amazon", stripe = "stripe", openai = "chatgpt", chatgpt = "chatgpt", ycombinator = "hackernews",
	opentable = "opentable", theverge = "theverge", maps = "gmaps",
}

return D
