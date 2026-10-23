-- Settings search (SettingsSearch.tsx): an index of every setting across all
-- pages, so the box finds page content and not just page names. Results
-- group by page; opening one goes to its page and flashes the setting.
return function(K)
	local C = K.C
	local S = {}

	local function e(section, label, desc, keys, target)
		return { section = section, label = label, desc = desc, keys = keys or "", target = target }
	end
	S.INDEX = {
		e("General", "Account", "Robin Hale · activity, devices, export data and sign out", "profile email sign out log out", "Account"),
		e("General", "Search engine", "Used when you type in the address bar", "google duckduckgo bing brave ecosia kagi startpage perplexity custom add omnibox default"),
		e("General", "AI features", "The assistant, agent actions and page reading. Turn off to use Ask as a plain browser", "disable ai turn off assistant agent llm model"),
		e("General", "Open assistant on launch", "Restore the assistant panel", "startup side panel"),
		e("Appearance", "Theme", "Dark, light or follow the system", "dark mode light mode system appearance colour scheme"),
		e("Appearance", "Tint", "Stone, Slate, Sage, Sand, Rose or Iris", "colour color hue accent palette"),
		e("Appearance", "Reduce motion", "Keep dither patterns static and shorten transitions", "animation accessibility"),
		e("Appearance", "Dither intensity", "Disable dither, Subtle, Default or Rich", "dithering pattern noise grain off", "Dither"),
		e("Appearance", "Window background", "The glow in the corner and the drops along the top", "dither amount", "Dither"),
		e("Appearance", "Loading & progress", "Tab loads, saves and the connecting bar", "dither amount progress bar", "Dither"),
		e("Appearance", "Agent highlight", "The focus drawn around what the agent is about to use", "dither amount halo", "Dither"),
		e("Appearance", "Orbs", "The assistant's shaded sphere wherever it appears", "dither amount orb", "Dither"),
		e("Appearance", "Grain", "Fine, Default or Coarse dither pixels", "pixel size", "Dither"),
		e("Appearance", "Animate dither", "Animate at 12 fps", "fps motion", "Dither"),
		e("Tabs & navigation", "Tab layout", "Vertical tabs in the sidebar or horizontal tabs across the top", "vertical horizontal tab strip"),
		e("Tabs & navigation", "Show sidebar", "Narrow to a rail of favicons, or hide it", "left sidebar rail collapse"),
		e("Tabs & navigation", "Position", "Floating over the page or inset above it", "address bar navbar url bar omnibox floating inset", "Position"),
		e("Tabs & navigation", "Collapse while scrolling", "Tuck the floating bar into a pill as you scroll", "navbar pill hide"),
		e("Tabs & navigation", "Show full URL", "Always show the whole address instead of just the site", "address domain https"),
		e("Providers", "Codex", "Run on your ChatGPT plan", "openai chatgpt gpt sign in subscription provider connect", "Codex"),
		e("Providers", "Claude Code", "Run on your Claude Pro or Max plan", "anthropic claude opus sonnet sign in subscription provider connect", "Claude Code"),
		e("Providers", "OpenRouter", "300+ models on one bill, OAuth or API key", "gateway api key models provider connect", "OpenRouter"),
		e("Providers", "OpenCode", "Open-source agent with OpenCode Zen", "zen open source api key provider connect", "OpenCode"),
		e("Providers", "Prefer subscriptions over API billing", "Use plan limits first, then fall back to a key", "billing cost"),
		e("Assistant", "Default model", "The loadout new chats start on", "model gpt claude grok loadout"),
		e("Assistant", "Open assistant", "Toggles the side panel from any page · ⌘J", "shortcut keyboard cmd j"),
		e("Assistant", "Show orb state in the toolbar", "Thinking, reading, acting or waiting", "status indicator"),
		e("Assistant", "Current tab", "Shared automatically when the panel is open", "privacy context page reading"),
		e("Assistant", "Other tabs", "Only when you @‑mention them or the task needs them", "mention context"),
		e("Assistant", "History, Mail and Calendar", "Connected sources it can search", "gmail calendar history sources connectors"),
		e("Assistant", "Password manager", "Fills sign-ins and cards without the agent seeing them", "1password proton pass bitwarden dashlane apple passwords keychain lastpass nordpass keeper autofill credentials"),
		e("Assistant", "Take over with Esc", "Hands control back to you immediately", "escape stop agent"),
		e("Models", "Providers", "Codex, Anthropic, Grok and OpenRouter · usage and limits", "api key add provider openai claude xai connect usage limit credit", "Providers"),
		e("Models", "Loadouts", "A model saved with its settings and a keybind", "favourite favorite new loadout reasoning effort fallback instructions keybind ⌥1", "Loadouts"),
		e("Models", "Reasoning effort", "How long it thinks before answering", "minimal low medium high max thinking", "Loadouts"),
		e("Models", "Instructions", "Added before every message you send with a loadout", "system prompt custom instructions", "Loadouts"),
		e("Models", "Default for new chats", "Each new chat starts on this loadout", "default model"),
		e("Models", "Hold ⌥ to show the loadout switcher", "Press a number to switch", "option alt shortcut"),
		e("Models", "Cycle loadouts", "Steps through loadouts from the composer · ⌥Tab", "shortcut"),
		e("Models", "Remember loadout per space", "Each space starts on its own loadout", "work personal"),
		e("Models", "Show which loadout answered", "Loadout name and response time under each reply", "label"),
		e("Models", "Delegate sub-tasks automatically", "Agents send side work to task models", "subagent delegation"),
		e("Models", "Task models", "Fast, Standard, Research, Deep and Visual", "subagent auto", "Fast"),
		e("Confirmations", "Approach", "Read only, ask for risky actions, or ask every step", "autonomy permission approve agent confirm"),
		e("Confirmations", "Purchases and payments", "Checking out, paying, subscribing, tipping", "buy money card confirm"),
		e("Confirmations", "Sending messages", "Email, chat, DMs, replies and calendar invites", "send confirm"),
		e("Confirmations", "Submitting forms", "Bookings, applications, sign-ups and surveys", "form confirm"),
		e("Confirmations", "Signing in", "Filling a saved sign-in from your password manager", "login log in password confirm"),
		e("Confirmations", "Publishing and sharing", "Posting publicly, sharing links and files", "post share confirm"),
		e("Confirmations", "Deleting things", "Removing files, emails, repos or bookings", "delete destructive remove confirm"),
		e("Confirmations", "Account and security changes", "Passwords, emails, 2FA and connected apps", "security 2fa confirm"),
		e("Confirmations", "Actions it can't verify", "Unlabelled buttons and pages that changed", "unverified confirm"),
		e("Confirmations", "Show agent cursor and dither highlight", "The assistant's cursor and focus on the element it's about to use", "cursor halo"),
		e("Confirmations", "Site permissions", "Allowed, read only and blocked sites", "per site block allow"),
		e("Passwords", "Passwords", "Open Chromium's saved passwords", "chromium saved logins"),
		e("Extensions", "Installed extensions", "Turn extensions on or off and pin them to the address bar", "chrome web store add-on addon plugin ublock 1password pin toolbar", "Installed extensions"),
		e("Extensions", "Run extensions in agent tabs", "Content scripts also run on pages the assistant drives", "agent content script"),
		e("Extensions", "Developer mode", "Show extension IDs and load unpacked extensions", "unpacked developer id"),
		e("Downloads", "Location", "~/Downloads", "save folder directory path download location"),
		e("Downloads", "Ask where to save each file", "Choose a folder and name every time", "save as prompt"),
		e("Downloads", "Open safe files after downloading", "PDFs, images and text", "auto open"),
		e("Downloads", "Show downloads as they start", "A bubble under the address bar with progress", "bubble shelf progress"),
		e("Downloads", "Files the assistant downloads", "Ask every time, allow from the current site, or never", "agent download permission confirm"),
		e("Downloads", "Recent downloads", "Files saved recently · show in Finder or clear", "history finder clear", "Recent downloads"),
		e("Spaces & profiles", "Spaces & profiles", "Work · Personal", "profile workspace"),
		e("About Ask", "Version", "Ask 0.1.0 · Chromium 154 · CEF · Lumine · Rust", "update check for updates build", "Versions"),
		e("About Ask", "Check for updates", "Ask is up to date", "update", "Ask is up to date"),
		e("About Ask", "Open source licences", "726 Rust packages and third-party notices", "license licence mit apache bsd notices acknowledgements", "Open source"),
		e("Developer", "Show frame counter", "Native UI FPS and presented frames", "fps debug performance"),
	}

	local function norm(s)
		return string.lower(s)
	end
	local function words_of(q)
		local out = {}
		for w in string.gmatch(norm(q), "%S+") do
			table.insert(out, w)
		end
		return out
	end
	local function has(s, w)
		return string.find(s, w, 1, true) ~= nil
	end
	-- Every word of the query must appear somewhere; label hits rank above
	-- description and keyword hits.
	function S.search(q)
		local words = words_of(q)
		if #words == 0 then
			return {}
		end
		local hits = {}
		for i, x in S.INDEX do
			local label, rest = norm(x.label), norm(x.desc .. " " .. x.keys .. " " .. x.section)
			local ok, score = true, 0
			for _, w in words do
				if not (has(label, w) or has(rest, w)) then
					ok = false
					break
				end
				score += label:sub(1, #w) == w and 4 or has(label, w) and 3 or has(norm(x.section), w) and 2 or 1
			end
			if ok then
				table.insert(hits, { x = x, score = score, i = i })
			end
		end
		table.sort(hits, function(a, b)
			if a.score ~= b.score then
				return a.score > b.score
			end
			return a.i < b.i
		end)
		local out = {}
		for i, h in hits do
			out[i] = h.x
		end
		return out
	end

	-- The text with each query word's occurrences highlighted (<mark>).
	local function marked(text, q, size, color)
		local words = words_of(q)
		local lower = norm(text)
		local hit = {}
		for _, w in words do
			local i = string.find(lower, w, 1, true)
			while i do
				for k = i, i + #w - 1 do
					hit[k] = true
				end
				i = string.find(lower, w, i + #w, true)
			end
		end
		local spans, i = {}, 1
		while i <= #text do
			local j = i
			while j <= #text and (hit[j] or false) == (hit[i] or false) do
				j += 1
			end
			local piece = text:sub(i, j - 1)
			table.insert(spans, hit[i] and ui.span { piece, color = C.text, bg = K.P("EDEBE5", 0.16) } or ui.span { piece })
			i = j
		end
		-- Spans go straight in the table: rich does not flatten nested lists.
		local node = { font = K.SANS, text_size = size, color = color, wrap = true }
		for _, span in spans do
			table.insert(node, span)
		end
		return ui.rich(node)
	end

	-- o: q (string), results, active (state), open(x), suggest(q), icon(section).
	function S.Results(o)
		local groups, order = {}, {}
		for _, x in o.results do
			if not groups[x.section] then
				groups[x.section] = {}
				table.insert(order, x.section)
			end
			table.insert(groups[x.section], x)
		end
		local n = #o.results
		local desc = n > 0 and (n .. (n == 1 and " setting matches " or " settings match ") .. "“" .. o.q .. "”")
			or ("Nothing matches “" .. o.q .. "”. Try a page name or what the setting does.")
		local children = { K.Header("Search", desc) }
		local index = 0
		for _, section in order do
			local items = { ui.row { pt = 14, pb = 6, px = 2, gap = 7, items = "center", o.icon(section), K.label(string.upper(section)) } }
			for _, x in groups[section] do
				index += 1
				local i = index
				local function on()
					return o.active:get() == i
				end
				table.insert(items, K.button({
					id = "result_" .. i, w_full = true, px = 15, py = 12, gap = 16, rounded = 10, border = 1,
					bg = function()
						return on() and C.page or C.clear
					end,
					border_color = function()
						return on() and C.ctl_line or C.clear
					end,
					hover = { bg = C.page, border_color = C.ctl_line },
					on_hover = function(inside)
						if inside then
							o.active:set(i)
						end
					end,
					on_click = function()
						o.open(x)
					end,
					ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
						marked(x.label, o.q, 13.5, C.text),
						marked(x.desc, o.q, 12, C.tertiary),
					},
					K.icon("corner-down-left", 13, C.muted, { opacity = K.tween(function()
						return on() and 1 or 0
					end, K.FAST) }),
				}, false))
			end
			table.insert(children, ui.column { w_full = true, mb = 10, items })
		end
		if n == 0 then
			local chips = {}
			for _, s in { "Theme", "Password manager", "Dither", "Licences", "Payments" } do
				table.insert(chips, K.btn { small = true, label = s, id = "suggest_" .. string.lower(s:gsub(" ", "_")), on_click = function()
					o.suggest(s)
				end })
			end
			table.insert(children, ui.row { gap = 6, flex_wrap = true, chips })
		end
		return children
	end

	return S
end
