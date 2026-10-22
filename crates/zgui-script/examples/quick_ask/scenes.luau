-- The four apps the board summons Quick ask over, and what each gives it:
-- the source dock, the capture chips, suggestions and canned answers. Copy
-- is the board's own where it shows it.
local FINDER_FILES = {
	{ name = "Lease agreement.pdf", kind = "PDF", meta = "PDF · 14 pages · 1.1 MB", icon = "file-text", tone = "#E5484D" },
	{ name = "Q3 expenses.xlsx", kind = "XLSX", meta = "XLSX · 3 sheets · 48 KB", icon = "sheet", tone = "#30A46C" },
	{ name = "Floor plan.pdf", kind = "PDF", meta = "PDF · 2 pages · 4.2 MB", icon = "file-text", tone = "#E5484D" },
	{ name = "Move-in notes.md", kind = "MD", meta = "MD · 2 KB", icon = "file", tone = "#8B8D98" },
	{ name = "Photos", kind = "Folder", meta = "Folder · 24 items", icon = "image", tone = "#3E63DD" },
}

-- Files the + button (⌘U) attaches, in turn.
local PICKS = {
	{ name = "Invoice INV-2291.pdf", meta = "PDF · 1 page · 220 KB", icon = "file-text", tone = "#E5484D" },
	{ name = "Screenshot 9.41.png", meta = "PNG · 2.1 MB", icon = "image", tone = "#3E63DD" },
	{ name = "budget-notes.txt", meta = "TXT · 4 KB", icon = "file", tone = "#8B8D98" },
}

local LISBON = "Mild and changeable. Early March averages highs of 17°C and lows of 10°C, with rain on roughly "
	.. "8–10 days of the month. Evenings cool off fast near the river, so a rooftop dinner works best with "
	.. "heaters or a covered fallback."

return {
	files = FINDER_FILES,
	picks = PICKS,
	scenes = {
		{
			id = "chrome", app = "Chrome", menu = "Google Chrome", window = "kitchennotebook.com",
			menus = { "File", "Edit", "View", "History", "Window" },
			caps = { "Page", "Selection", "Screen" }, cap_icons = { "file-text", "type", "maximize" },
			on = { true, false, false },
			chips = {
				{ kind = "context", icon = "globe", name = "Reverse-seared ribeye", ctx = "page" },
				{ kind = "context", icon = "type", name = "“a 250°F oven”", ctx = "selection" },
				{ kind = "image", icon = "monitor", name = "Screen", ctx = "screen" },
			},
			placeholder = "Ask about this page, or anything…",
			reading = "Reading kitchennotebook.com",
			group = "SUGGESTED FOR THIS PAGE",
			suggest = {
				{ icon = "align-left", title = "Summarize this page" },
				{ icon = "thermometer", title = "What temperature should I pull it at?" },
				{ icon = "shopping-cart", title = "Turn the ingredients into a shopping list" },
			},
			typed = { { icon = "list-checks", title = "Turn this into a step-by-step timer" } },
			google = "reverse sear ribeye oven temperature",
			sources = { { "kitchennotebook.com", "#A0522D" }, { "seriouseats.com", "#E5484D" }, { "usda.gov", "#3E63DD" } },
			answers = {
				["summarize this page"] = "Start the ribeye in a 250°F oven until it reads 120°F inside, rest it for "
					.. "five minutes, then sear it in a smoking-hot cast-iron pan for about a minute a side. The slow "
					.. "start cooks it evenly edge to edge; the short sear only builds the crust.",
				["what temperature should i pull it at?"] = "Pull it from the oven at 120°F for medium-rare (125°F "
					.. "for medium). The sear and the rest add another 5–8°F, landing it at 128–130°F.",
				["turn the ingredients into a shopping list"] = "Ribeye, bone-in, 1½ in thick · Kosher salt · "
					.. "Black pepper · Neutral oil (avocado or canola) · Butter · 3 cloves garlic · Fresh thyme.",
			},
		},
		{
			id = "code", app = "Code", menu = "Code", window = "parse.ts — billing-service",
			menus = { "File", "Edit", "Selection", "View", "Go" },
			caps = { "File", "Selection", "Screen" }, cap_icons = { "file-text", "type", "maximize" },
			on = { false, true, false },
			chips = {
				{ kind = "context", icon = "file-text", name = "parse.ts", ctx = "file" },
				{ kind = "context", icon = "code", name = "parse.ts · L14–16", ctx = "selection" },
				{ kind = "image", icon = "monitor", name = "Screen", ctx = "screen" },
			},
			placeholder = "Ask about this selection, or anything…",
			draft = "what does this regex match",
			reading = "Reading parse.ts · L14–16",
			group = "SUGGESTED FOR THIS SELECTION",
			suggest = {
				{ icon = "align-left", title = "Explain this selection" },
				{ icon = "check", title = "Write tests for parseDueDate" },
				{ icon = "search", title = "Find other callers" },
			},
			typed = { { icon = "pencil", title = "Rewrite it to also accept 2026/10/17" } },
			google = "javascript regex iso 8601 date",
			sources = { { "parse.ts · L14–16", "#3E63DD" }, { "developer.mozilla.org", "#111111" } },
			answers = {
				["what does this regex match"] = "An ISO date at the start of the string: four digits, two, two "
					.. "(2026-10-17), optionally followed by T and hours:minutes. Groups 1–3 capture year, month and "
					.. "day; 4–5 the time. Seconds and a trailing Z pass but aren't captured, and 2026/10/17 won't match.",
				["explain this selection"] = "It trims the input, matches it against ISO, and throws InvalidDateError "
					.. "when nothing matches, so callers always get either a parsed match or an exception.",
			},
		},
		{
			id = "mail", app = "Mail", menu = "Mail", window = "Re: Offsite — Lisbon in March?",
			menus = { "File", "Edit", "View", "Mailbox", "Message" },
			caps = { "Message", "Selection", "Screen" }, cap_icons = { "mail", "type", "maximize" },
			on = { true, false, false },
			chips = {
				{ kind = "context", icon = "mail", name = "Re: Offsite — Lisbon", ctx = "message" },
				{ kind = "context", icon = "type", name = "“early March”", ctx = "selection" },
				{ kind = "image", icon = "monitor", name = "Screen", ctx = "screen" },
			},
			placeholder = "Ask about this message, or anything…",
			reading = "Reading Re: Offsite — Lisbon",
			group = "SUGGESTED FOR THIS MESSAGE",
			suggest = {
				{ icon = "corner-down-left", title = "Draft a reply to Priya" },
				{ icon = "globe", title = "What's Lisbon like in early March?" },
				{ icon = "calendar", title = "Add the offsite to my calendar" },
			},
			typed = { { icon = "corner-down-left", title = "Draft a reply with this" } },
			google = "lisbon weather march",
			-- The board's third card: the answer is already up.
			start = { q = "what's Lisbon like in early March?", a = LISBON },
			sources = { { "weather-atlas.com", "#3B82F6" }, { "ipma.pt", "#16A34A" }, { "visitlisboa.com", "#E11D48" } },
			answers = {
				["what's lisbon like in early march?"] = LISBON,
				["draft a reply to priya"] = "Hi Priya, early March works for me. Weather looks mild (highs around "
					.. "17°C) but rainy some days, so let's book somewhere with a covered terrace. Happy to look at "
					.. "venues this week.",
			},
		},
		{
			id = "finder", app = "Finder", menu = "Finder", window = "~/Documents/Apartment",
			-- Lower, so the files stay in view above it.
			panel_top = 300,
			menus = { "File", "Edit", "View", "Go", "Window" },
			caps = { "Folder", "Selection", "Screen" }, cap_icons = { "folder", "type", "maximize" },
			on = { false, true, false },
			chips = {
				{ kind = "context", icon = "folder", name = "Apartment", ctx = "folder" },
				{ kind = "files", ctx = "selection" },
				{ kind = "image", icon = "monitor", name = "Screen", ctx = "screen" },
			},
			placeholder = "Ask about these files, or anything…",
			draft = "can I sublet under this lease?",
			reading = "Reading 3 files",
			group = "SUGGESTED FOR THESE FILES",
			suggest = {
				{ icon = "list-checks", title = "Pull every deadline into a checklist" },
				{ icon = "wallet", title = "Does rent fit the Q3 budget?", detail = "— lease + expenses" },
				{ icon = "align-left", title = "Summarize the lease" },
			},
			typed = {
				{ icon = "list-checks", title = "Pull every deadline into a checklist" },
				{ icon = "wallet", title = "Does rent fit the Q3 budget?", detail = "— lease + expenses" },
			},
			google = "can you sublet a rented apartment",
			sources = { { "Lease agreement.pdf · p.9", "#E5484D" }, { "Q3 expenses.xlsx", "#30A46C" } },
			answers = {
				["can i sublet under this lease?"] = "Yes, with conditions. Section 14 allows subletting with the "
					.. "landlord's written consent, which can't be unreasonably withheld. You'd give 30 days' notice "
					.. "and stay liable for rent; the floor plan and expenses don't change that.",
				["pull every deadline into a checklist"] = "☐ 1 Nov: first rent due · ☐ 15 Nov: renter's insurance "
					.. "proof · ☐ 30 days before moving out: written notice · ☐ 31 Oct 2027: lease ends.",
				["does rent fit the q3 budget?"] = "Mostly. Rent is £1,850 a month against £2,000 budgeted for "
					.. "housing in Q3, but utilities push the total £120 over. Trimming dining out by £40 a week covers it.",
			},
		},
	},
}
