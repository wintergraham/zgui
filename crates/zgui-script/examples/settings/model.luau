-- The Settings model (Settings.tsx `Model`): one keyed state per setting,
-- seeded from what the host saved and written back to `ui.shared("settings")`
-- whenever any of them changes. The host stores that value on disk.
local M = {}

M.FLAGS = {
	["Open assistant on launch"] = true, ["Show orb state in the toolbar"] = true, ["Current tab"] = true,
	["Take over with Esc"] = true, ["Show agent cursor and dither highlight"] = true, ["Sidebar open"] = true,
	["Show frame counter"] = false, ["Collapse while scrolling"] = true, ["Show full URL"] = false,
	["Run extensions in agent tabs"] = false, ["Prefer subscriptions over API billing"] = true,
	["Reuse sign-ins from installed CLIs"] = true, ["Developer mode"] = false, ["Ask where to save each file"] = false,
	["Open safe files after downloading"] = false, ["Show downloads as they start"] = true,
}
-- Rows that need the assistant; they go inert when AI is off.
M.AI_ROWS = { ["Open assistant on launch"] = true, ["Current tab"] = true, ["Show agent cursor and dither highlight"] = true, ["Run extensions in agent tabs"] = true }

M.DEFAULT_DITHER = { master = 1, background = 1, progress = 1, highlight = 1, orbs = 1, animate = true, grain = "default" }

-- Assistant › Confirmations: categories are the native guard's plus forms.
M.BALANCED = { payments = "always", messages = "always", forms = "never", signingIn = "site", sharing = "always", destructive = "always", account = "always", unverified = "always" }
M.DEFAULT_CONFIRM = { mode = "act", rules = M.BALANCED }
M.CATS = {
	{ id = "payments", label = "Purchases and payments", desc = "Checking out, paying, subscribing, tipping", icon = "credit-card", risky = true },
	{ id = "messages", label = "Sending messages", desc = "Email, chat, DMs, replies and calendar invites", icon = "send" },
	{ id = "forms", label = "Submitting forms", desc = "Bookings, applications, sign-ups and surveys", icon = "text-cursor-input" },
	{ id = "signingIn", label = "Signing in", desc = "Filling a saved sign-in from your password manager", icon = "key-round" },
	{ id = "sharing", label = "Publishing and sharing", desc = "Posting publicly, sharing links and files", icon = "share" },
	{ id = "destructive", label = "Deleting things", desc = "Removing files, emails, repos or bookings", icon = "trash-2", risky = true },
	{ id = "account", label = "Account and security changes", desc = "Passwords, emails, 2FA and connected apps", icon = "shield", risky = true },
	{ id = "unverified", label = "Actions it can't verify", desc = "Unlabelled buttons and pages that changed underneath it", icon = "circle-alert" },
}
-- "read", "every", "balanced" (the default rules) or "custom".
function M.confirm_preset(c)
	if c.mode == "read" or c.mode == "every" then
		return c.mode
	end
	for _, k in M.CATS do
		if c.rules[k.id] ~= M.BALANCED[k.id] then
			return "custom"
		end
	end
	return "balanced"
end
-- The categories that pause for approval.
function M.confirm_asks(c)
	local out = {}
	for _, k in M.CATS do
		if c.rules[k.id] ~= "never" then
			table.insert(out, k)
		end
	end
	return out
end

local function loadout(id, name, model, effort, note, extra)
	local l = { id = id, name = name, model = model, effort = effort, length = "Default", fallback = "kimi-k3", tabs = "Inherit", act = "Inherit", memories = "Inherit", instructions = "", keys = { "⌥", tostring(id) }, note = note }
	for k, v in extra or {} do
		l[k] = v
	end
	return l
end
M.loadout = loadout
M.DEFAULT_MODELS = {
	loadouts = {
		loadout(1, "GPT-6 High", "gpt-6", "High", "Reasoning high", { memories = "Off", instructions = "Think it through before answering. For code, show the diff only. Cite sources inline with the page title. Keep answers under 300 words unless I ask for more." }),
		loadout(2, "GPT-6 Low", "gpt-6", "Low", "Reasoning low"),
		loadout(3, "Opus 5.5 · Deep", "opus-5.5", "Max", "Thinking 32K"),
		loadout(4, "Sonnet 5.5 · Agent", "sonnet-5.5", "Medium", "Acts on pages", { act = "On" }),
		loadout(5, "Grok 4.7 Fast", "grok-4.7-fast", "Minimal", "No reasoning"),
		loadout(6, "Kimi K3", "kimi-k3", "Medium", "via OpenRouter"),
	},
	selected = 1, defaultId = 1, delegate = true, tasks = { Fast = "auto", Standard = "auto", Research = "opus-5.5", Deep = "auto", Visual = "auto" },
	hold = true, perSpace = true, answered = false,
}

M.DEFAULT_EXTENSIONS = {
	{ id = "ddkjiahejlhfcafbddmgiahcphecmpfh", name = "uBlock Origin Lite", version = "2026.9.28", desc = "Blocks ads and trackers with declarative rules", hue = 25, on = true, pinned = true },
	{ id = "aeblfdkhhhdcdjpifhhbdiojplfjncoa", name = "1Password", version = "8.12.4", desc = "Fills sign-ins and cards from your vault", hue = 250, on = true, pinned = true },
	{ id = "hlepfoohegkhhmjieoechaddaejaokhf", name = "Refined GitHub", version = "26.10.2", desc = "Simplifies the GitHub interface and adds shortcuts", hue = 300, on = true, pinned = false },
	{ id = "eimadpbcbfnmbkopoojfekhnkhdbieeh", name = "Dark Reader", version = "4.9.110", desc = "Dark theme for every website", hue = 200, on = false, pinned = false },
	{ id = "fmkadmapgofadopljbjfkapdkoienihi", name = "React Developer Tools", version = "6.1.5", desc = "Inspect React components and profile renders", hue = 195, on = true, pinned = false },
}

M.NAV = {
	{ "General", "general", "sliders-horizontal" }, { "Appearance", "appearance", "palette" }, { "Tabs & navigation", "tabs", "panel-left" },
	{ "Assistant", "assistant", "orb" }, { "Providers", "providers", "link", "Assistant" }, { "Models", "models", "layers", "Assistant" },
	{ "Confirmations", "confirmations", "list-checks", "Assistant" },
	{ "Passwords", "passwords", "key-round" }, { "Extensions", "extensions", "puzzle" }, { "Downloads", "downloads", "download" },
	{ "Spaces & profiles", "spaces", "users" }, { "About Ask", "about", "info" }, { "Developer", "developer", "code" },
}

local function merged(base, over)
	local out = table.clone(base)
	if type(over) == "table" then
		for k, v in over do
			out[k] = v
		end
	end
	return out
end
local saved = ui.shared("settings", nil):peek()
local function from(key, default)
	if type(saved) ~= "table" or saved[key] == nil then
		return default
	end
	return saved[key]
end

local section = from("section", "General")
local known = false
for _, n in M.NAV do
	known = known or n[1] == section
end
M.section = ui.state("section", known and section or "General")
M.filter = ui.state("filter", "")
M.ai = ui.state("ai", from("ai", true))
M.flags = ui.state("flags", merged(M.FLAGS, from("flags", nil)))
M.otherTabs = ui.state("other_tabs", from("otherTabs", "When mentioned"))
M.sources = ui.state("sources", merged({ History = true, Mail = true, Calendar = true }, from("sources", nil)))
local confirm = from("confirm", M.DEFAULT_CONFIRM)
M.confirm = ui.state("confirm", { mode = confirm.mode or "act", rules = merged(M.BALANCED, confirm.rules) })
M.manager = ui.state("manager", from("manager", "1password"))
M.engine = ui.state("engine", from("engine", "google"))
M.engines = ui.state("engines", from("engines", {}))
M.providers = ui.state("providers", merged({ codex = true, ["claude-code"] = false, openrouter = false, opencode = false }, from("providers", nil)))
M.extensions = ui.state("extensions", from("extensions", M.DEFAULT_EXTENSIONS))
M.agentDownloads = ui.state("agent_downloads", from("agentDownloads", "Ask every time"))
M.tabs = ui.state("tabs", from("tabs", "vertical"))
M.navbar = ui.state("navbar", from("navbar", "floating"))
M.models = ui.state("models", merged(M.DEFAULT_MODELS, from("models", nil)))
M.dither = ui.state("dither", merged(M.DEFAULT_DITHER, from("dither", nil)))
M.theme = ui.state("theme", from("theme", "dark"))
M.tint = ui.state("tint", from("tint", "stone"))
M.reduced = ui.state("reduced", from("reduced", false))

-- Partial updates of the table-valued settings.
function M.patch(state, changes)
	local next = table.clone(state:peek())
	for k, v in changes do
		next[k] = v
	end
	state:set(next)
end
function M.flag(name)
	return M.flags:get()[name] == true
end
function M.set_flag(name, on)
	M.patch(M.flags, { [name] = on })
end

local store = ui.shared("settings", nil)
ui.effect(function()
	store:set {
		section = M.section:get(), ai = M.ai:get(), flags = M.flags:get(), otherTabs = M.otherTabs:get(),
		sources = M.sources:get(), confirm = M.confirm:get(), manager = M.manager:get(), engine = M.engine:get(),
		engines = M.engines:get(), providers = M.providers:get(), extensions = M.extensions:get(),
		agentDownloads = M.agentDownloads:get(), tabs = M.tabs:get(), navbar = M.navbar:get(), models = M.models:get(),
		dither = M.dither:get(), theme = M.theme:get(), tint = M.tint:get(), reduced = M.reduced:get(),
	}
end)

return M
