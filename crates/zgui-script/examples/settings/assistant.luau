-- Assistant (what it can see and do, the AI switch that slides in when it is
-- off) and Assistant › Providers (Settings.tsx Assistant, ProvidersPage;
-- Providers.tsx; SettingsSections.tsx PasswordManager; SettingsConfirm.tsx
-- ConfirmCard).
return function(K)
	local model = K.model
	local logos = require("./logos")
	local C = K.C

	---------------------------------------------------------------------------
	-- Password manager
	---------------------------------------------------------------------------
	local MANAGERS = {
		{ "builtin", "Ask Passwords", "Built in · syncs with your Ask account" }, { "1password", "1Password", "1password.com" },
		{ "proton-pass", "Proton Pass", "proton.me/pass" }, { "bitwarden", "Bitwarden", "bitwarden.com" }, { "dashlane", "Dashlane", "dashlane.com" },
		{ "apple-passwords", "Apple Passwords", "iCloud Keychain" }, { "lastpass", "LastPass", "lastpass.com" }, { "nordpass", "NordPass", "nordpass.com" },
		{ "keeper", "Keeper", "keepersecurity.com" },
	}
	K.MANAGERS = MANAGERS
	local function ManagerIcon(id, size)
		size = size or 16
		if id == "builtin" then
			return ui.row { w = size, h = size, rounded = 5, bg = C.ink, items = "center", justify = "center", shrink = 0, K.icon("key-round", size * 0.62, C.on_ink) }
		end
		return K.image("password-managers/" .. id, size, { rounded = 4 })
	end
	K.ManagerIcon = ManagerIcon
	local function current_manager()
		for _, m in MANAGERS do
			if m[1] == model.manager:get() then
				return m
			end
		end
		return MANAGERS[1]
	end
	local function PasswordManager()
		local a = K.anchor()
		local function build()
			local items = {}
			local cur = current_manager()
			for i, m in MANAGERS do
				items[i] = ui.div { w_full = true, mt = i == 2 and 5 or 0,
					K.menu_item {
						id = "manager_" .. m[1], label = m[2], desc = m[3], lead = ManagerIcon(m[1], 20), checked = m[1] == cur[1], center = true,
						on_click = function()
							model.manager:set(m[1])
							K.close_menu()
						end,
					},
				}
			end
			return K.menu_panel({ min_w = 260 }, items)
		end
		return K.Row {
			label = "Password manager", desc = "The agent asks it to fill sign-ins and cards, and never sees the passwords themselves.",
			child = K.button {
				id = "manager", h = 30, pl = 8, pr = 10, gap = 6, rounded = 7, shrink = 0,
				bg = function()
					return K.menu_open("manager") and C.hover or C.ctl
				end, border = 1, border_color = function()
					return K.menu_open("manager") and C.hover_line or C.ctl_line
				end, hover = { bg = C.hover, border_color = C.hover_line },
				on_mount = a.mount,
				on_click = function()
					K.toggle_menu("manager", a.el, build)
				end,
				K.fixed(ui.presence(function()
					return current_manager()[1]
				end, function()
					local m = current_manager()
					return ui.row { gap = 6, items = "center", ManagerIcon(m[1]), K.text(m[2], 12.5, C.soft, { id = "manager_value" }) }
				end)),
				K.icon("chevrons-up-down", 12, C.muted),
			},
		}
	end

	---------------------------------------------------------------------------
	-- Confirmations summary (the Assistant page's card)
	---------------------------------------------------------------------------
	local PRESETS = { read = "Read only", balanced = "Ask for risky actions", every = "Ask every step", custom = "Custom" }
	local function ConfirmCard()
		local hovered = ui.use_state(false)
		return K.button({
			id = "confirm_summary", w_full = true, gap = 14, p = 15, mb = 8, rounded = 12, bg = C.card, border = 1, border_color = C.line,
			hover = { bg = C.elevated, border_color = C.hover_line },
			on_hover = function(inside)
				hovered:set(inside)
			end,
			on_click = function()
				K.go("Confirmations")
			end,
			ui.row { w = 34, h = 34, rounded = 10, items = "center", justify = "center", shrink = 0, border = 1, border_color = C.ctl_line,
				gradient = { angle = 180, stops = { C.hover, C.page } }, K.icon("shield", 15, C.text) },
			ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
				ui.text { function()
					return PRESETS[model.confirm_preset(model.confirm:get())]
				end, id = "confirm_summary_label", font = K.SANS, text_size = 13.5, color = C.text, align = "left" },
				ui.text { function()
					local c = model.confirm:get()
					if c.mode == "read" then
						return "The agent can read pages but never acts on them"
					end
					return #model.confirm_asks(c) .. " of " .. #model.CATS .. " kinds of action ask first"
				end, font = K.SANS, text_size = 12, color = C.tertiary, align = "left" },
			},
			K.fixed(ui.presence(function()
				local c = model.confirm:get()
				if c.mode == "read" then
					return ""
				end
				local ids = {}
				for _, k in model.confirm_asks(c) do
					table.insert(ids, k.id)
				end
				return table.concat(ids, ",")
			end, function(key)
				if key == "" then
					return ui.div {}
				end
				local asks = model.confirm_asks(model.confirm:peek())
				local chips = {}
				for i = 1, math.min(5, #asks) do
					chips[i] = ui.row { w = 24, h = 24, rounded = 7, items = "center", justify = "center", bg = C.hover, border = 1, border_color = C.ctl_line, K.icon(asks[i].icon, 12, C.secondary) }
				end
				return ui.row { gap = 4, items = "center", chips, #asks > 5 and K.mono("+" .. #asks - 5, 10.5, C.muted, { ml = 2 }) or nil }
			end)),
			K.icon("chevron-right", 14, C.muted, { x = K.tween(function()
				return hovered:get() and 2 or 0
			end, K.BASE, K.EASE_OUT, true) }),
		}, false)
	end

	---------------------------------------------------------------------------
	-- Assistant
	---------------------------------------------------------------------------
	local OTHER_TABS = { "Never", "When mentioned", "Always" }
	local Assistant = ui.component(function()
		local ai = model.ai
		local function default_loadout()
			local ms = model.models:get()
			for _, l in ms.loadouts do
				if l.id == ms.defaultId then
					return l
				end
			end
			return ms.loadouts[1]
		end
		local off = function()
			return not ai:get()
		end
		return ui.column {
			w_full = true,
			K.Header("Assistant", "What the assistant can see, what it can do on pages, and when it stops to ask you.", K.Orb { size = 44, lit = ai, active = true }),
			-- When AI is off the switch slides in above the settings it governs.
			ui.div {
				w_full = true, layout = "height", layout_clip = true,
				opacity = K.tween(function()
					return off() and 1 or 0
				end, K.MEDIUM),
				ui.show(off, ui.div { w_full = true, pb = 8, K.AiSwitch("ai_toggle_top") }),
			},
			ui.column {
				id = "ai_body", w_full = true,
				opacity = K.tween(function()
					return off() and 0.38 or 1
				end, K.MEDIUM),
				saturate = K.tween(function()
					return off() and 0 or 1
				end, K.MEDIUM),
				disabled = off,
				K.Section("BASICS", true),
				K.Row {
					label = "Default model", desc = "The loadout new chats start on. You can switch per message from the composer.",
					child = ui.row { gap = 14, items = "center", shrink = 0,
						K.link("Models", function()
							K.go("Models")
						end, { id = "go_models" }),
						K.Select {
							id = "default_model", orb = true,
							value = function()
								return default_loadout().name
							end,
							options = function()
								local names = {}
								for i, l in model.models:get().loadouts do
									names[i] = l.name
								end
								return names
							end,
							pick = function(name)
								local ms = model.models:peek()
								for _, l in ms.loadouts do
									if l.name == name then
										model.patch(model.models, { defaultId = l.id })
									end
								end
							end,
						},
					},
				},
				K.Row { label = "Open assistant", desc = "Toggles the side panel from any page.", child = K.keycap("⌘J") },
				K.flag("Show orb state in the toolbar", "The toolbar pill mirrors what the assistant is doing: thinking, reading, acting or waiting."),
				K.Section("WHAT IT CAN SEE"),
				K.flag("Current tab", "Shared automatically when the panel is open. Turn off per site from the address bar.", nil, "current_tab"),
				K.Row {
					label = "Other tabs", desc = "Only when you @‑mention them or the task needs them.",
					child = K.Select { id = "other_tabs", options = OTHER_TABS, value = function()
						return model.otherTabs:get()
					end, pick = function(v)
						model.otherTabs:set(v)
					end },
				},
				K.Row {
					label = "History, Mail and Calendar", desc = "Connected sources it can search.",
					child = K.Select {
						id = "sources", options = { "Calendar", "History", "Mail" },
						value = function()
							return ""
						end,
						label = function()
							local n = 0
							for _, on in model.sources:get() do
								if on then
									n += 1
								end
							end
							return n > 0 and n .. " connected" or "None"
						end,
						multi = function()
							return model.sources:get()
						end,
						pick = function(v)
							model.patch(model.sources, { [v] = not model.sources:peek()[v] })
						end,
					},
				},
				K.Section("SIGN-INS AND CARDS"),
				PasswordManager(),
				K.Section("ACTING ON PAGES"),
				ConfirmCard(),
				K.Row {
					label = "Take over with Esc", desc = "Pressing Esc on a page the assistant is driving hands control back to you immediately.", accent = true,
					child = K.Toggle { id = "take_over", label = "Take over with Esc", on = function()
						return model.flag("Take over with Esc")
					end, set = function(v)
						model.set_flag("Take over with Esc", v)
					end },
				},
			},
			ui.show(ai, ui.div { w_full = true, mt = 32, K.AiSwitch("ai_toggle") }),
		}
	end)

	---------------------------------------------------------------------------
	-- Providers: the agent subscriptions and model gateways the assistant can
	-- run on. Sign-in is faked: connecting completes after 2.4 s.
	---------------------------------------------------------------------------
	local KINDS = { subscription = { "Subscription", "#A9BEE6" }, gateway = { "Gateway", "#BBB0F2" }, ["open-source"] = { "Open source", "#A6D4AE" } }
	local PROVIDERS = {
		{ id = "codex", name = "Codex", kind = "subscription", tile = "#FFFFFF", inset = false, acceptsKey = false,
			blurb = "Runs on your ChatGPT plan. GPT-6 and the Codex models, no API credit.", signIn = "Sign in with ChatGPT",
			models = { "GPT-6", "GPT-6 mini", "codex-2" }, account = "robin@hale.studio · ChatGPT Pro" },
		{ id = "claude-code", name = "Claude Code", kind = "subscription", tile = "#1F1E1B", inset = true, acceptsKey = false,
			blurb = "Runs on your Claude Pro or Max plan. Opus, Sonnet and Haiku.", signIn = "Sign in with Claude",
			models = { "Opus 5.5", "Sonnet 5.5", "Haiku 5.5" }, account = "robin@hale.studio · Claude Max", detected = "Signed in on this Mac · ~/.claude" },
		{ id = "openrouter", name = "OpenRouter", kind = "gateway", tile = "#6566F1", inset = false, acceptsKey = true,
			blurb = "300+ models from every major lab on one bill, with automatic fallbacks.", signIn = "Sign in with OpenRouter",
			models = { "Kimi K3", "DeepSeek V4", "Llama 5" }, account = "Personal · $20.00 credit" },
		{ id = "opencode", name = "OpenCode", kind = "open-source", tile = "#0E0E0D", inset = true, acceptsKey = true,
			blurb = "Open-source agent with OpenCode Zen, a curated set of coding models.", signIn = "Sign in with OpenCode Zen",
			models = { "Zen models", "Bring your own" }, account = "OpenCode Zen · robin@hale.studio", detected = "opencode 1.4 found in PATH" },
	}
	local pending = ui.state("providers_pending", {})
	local timers = {}
	local function settle(id, on)
		if timers[id] then
			timers[id]:cancel()
			timers[id] = nil
		end
		model.patch(pending, { [id] = false })
		model.patch(model.providers, { [id] = on })
	end
	local function act(kind, id)
		if kind == "connect" then
			model.patch(pending, { [id] = true })
			timers[id] = K.after(2400, function()
				timers[id] = nil
				settle(id, true)
			end)
		else
			settle(id, kind ~= "cancel" and kind ~= "disconnect")
		end
	end
	local function status(p)
		if pending:get()[p.id] then
			return "connecting"
		end
		return model.providers:get()[p.id] and "connected" or "available"
	end
	local function Logo(p)
		local px = p.id == "codex" and 34 or 20
		local code = (logos.marks[p.id] or ""):gsub('width="24"', 'width="' .. px .. '"'):gsub('height="24"', 'height="' .. px .. '"')
		return ui.row {
			w = 34, h = 34, rounded = 9, overflow = "hidden", bg = p.tile, items = "center", justify = "center", shrink = 0,
			border = p.inset and 1 or 0, border_color = "#FFFFFF1F", border_over = true,
			ui.svg { code = code, w = px, h = px },
		}
	end
	local function pv_btn(id, label, variant, on_click, enabled)
		local primary, quiet = variant == "primary", variant == "quiet"
		return K.button {
			id = id, h = 28, px = 11, rounded = 7, shrink = 0,
			bg = primary and C.ink or quiet and C.clear or C.raised,
			border = 1, border_color = primary and C.ink or quiet and C.line or C.strong,
			opacity = enabled and function()
				return enabled() and 1 or 0.4
			end or nil,
			on_click = function()
				if not enabled or enabled() then
					on_click()
				end
			end,
			K.text(label, 12, primary and C.on_ink or quiet and C.tertiary or C.text, { weight = primary and 500 or nil }),
		}
	end
	local function pv_link(id, label, on_click)
		return K.button({ id = id, hover = { color = C.text }, color = C.secondary, on_click = on_click, K.text(label, 11.5, false) }, false)
	end
	local function glyph(name, color)
		return K.icon(name, 13, color)
	end
	local ProviderCard = ui.component(function(props)
		local p = props.p
		local key_open, draft = ui.use_state(false), ui.use_state("")
		local function close_key()
			key_open:set(false)
			draft:set("")
		end
		local function save_key()
			if draft:peek():match("%S") then
				act("saveKey", p.id)
				close_key()
			end
		end
		local function st()
			return status(p)
		end
		local kind = KINDS[p.kind]
		local bar = ui.ref()
		ui.effect(function()
			local s = st()
			if s == "connecting" then
				anime.animate(bar, { w = { "0%", "100%" }, opacity = { 1, 1 }, duration = 8000, ease = K.EASE_OUT })
			elseif s == "connected" then
				anime.animate(bar, { w = "100%", duration = K.FAST, ease = K.EASE_OUT })
				anime.animate(bar, { opacity = 0, duration = K.MEDIUM, ease = K.EASE_OUT })
			else
				anime.animate(bar, { w = "0%", opacity = 0, duration = K.EXIT, ease = K.EASE_EXIT })
			end
		end)
		local actions = ui.presence(st, function(s)
			if s == "connected" then
				return pv_btn("provider_" .. p.id .. "_disconnect", "Disconnect", "quiet", function()
					act("disconnect", p.id)
				end)
			elseif s == "connecting" then
				return pv_btn("provider_" .. p.id .. "_cancel", "Cancel", nil, function()
					act("cancel", p.id)
				end)
			elseif p.detected then
				return pv_btn("provider_" .. p.id .. "_import", "Use this sign-in", "primary", function()
					act("import", p.id)
				end)
			end
			return pv_btn("provider_" .. p.id .. "_connect", "Connect", "primary", function()
				act("connect", p.id)
			end)
		end)
		local meta = ui.presence(function()
			return st() .. (key_open:get() and "/key" or "")
		end, function(key)
			local s = st()
			local lead
			if s == "connected" then
				lead = K.text(p.account, 11.5, C.muted)
			elseif s == "connecting" then
				lead = ui.row { gap = 6, items = "center", glyph("arrow-up-right", C.secondary), K.text("Finish signing in in your browser…", 11.5, C.secondary) }
			elseif p.detected then
				lead = ui.row { gap = 6, items = "center", glyph("circle-check", "#A6D4AE"), K.text(p.detected, 11.5, "#A6D4AE") }
			else
				lead = K.text(p.signIn, 11.5, C.muted)
			end
			local chips = {}
			if s == "connected" then
				for i, m in p.models do
					chips[i] = ui.div { px = 6, py = 2, rounded = 4, bg = C.raised, border = 1, border_color = C.subtle, K.mono(m, 10.5, C.tertiary) }
				end
			end
			return ui.row { w_full = true, pl = 46, gap = 10, items = "center",
				lead, K.spacer(),
				(s == "available" and p.detected) and pv_link("provider_" .. p.id .. "_other", "Use another account", function()
					act("connect", p.id)
				end) or nil,
				(s == "available" and p.acceptsKey and not key_open:peek()) and pv_link("provider_" .. p.id .. "_key", "Paste an API key", function()
					key_open:set(true)
					draft:set("")
				end) or nil,
				#chips > 0 and ui.row { gap = 4, shrink = 0, chips } or nil,
			}
		end)
		local key_row = ui.show(key_open, ui.row { w_full = true, pl = 46, gap = 6, items = "center",
			ui.row { grow = 1, shrink = 1, min_w = 0, h = 28, px = 10, rounded = 7, bg = C.inset, border = 1, border_color = C.strong, items = "center",
				K.input {
					id = "provider_" .. p.id .. "_key_input", value = draft, mono = true, label = p.name .. " API key",
					placeholder = p.id == "openrouter" and "sk-or-…" or "API key", placeholder_color = C.faint,
					on_mount = function(el)
						K.after(30, function()
							el:focus()
						end)
					end,
					on_key = function(e)
						if e.key == "Enter" then
							save_key()
							return true
						elseif e.key == "Escape" then
							close_key()
							return true
						end
						return false
					end,
				},
			},
			pv_btn("provider_" .. p.id .. "_key_cancel", "Cancel", "quiet", close_key),
			pv_btn("provider_" .. p.id .. "_key_save", "Save key", "primary", save_key, function()
				return draft:get():match("%S") ~= nil
			end),
		})
		return ui.overlay(K.target(p.name, {
			id = "provider_" .. p.id, w_full = true,
			K.flash_ring(p.name),
			ui.column { w_full = true, p = 15, gap = 10, rounded = 10, overflow = "hidden", bg = C.elevated, border = 1, border_color = C.line,
				ui.row { w_full = true, gap = 12, items = "center",
					Logo(p),
					ui.column { grow = 1, shrink = 1, min_w = 0, gap = 3,
						ui.row { gap = 8, items = "center",
							K.text(p.name, 13.5, C.text, { weight = 500 }),
							ui.div { px = 6, py = 2, rounded = 4, bg = kind[2] .. "14", K.mono(kind[1], 10, kind[2]) },
						},
						K.text(p.blurb, 12, C.tertiary, { wrap = true }),
					},
					K.fixed(ui.show(function()
						return st() == "connected"
					end, K.text("Connected", 11.5, "#A6D4AE", { shrink = 0 }))),
					K.fixed(actions),
				},
				meta,
				key_row,
				ui.div { w_full = true, h = 2, mt = 2, mx = -15, mb = -15,
					ui.div { ref = bar, h = 2, w = "0%", opacity = 0, bg = "#A9BEE6" },
				},
			},
		}))
	end)
	local function ProvidersPage()
		local cards = {}
		for i, p in PROVIDERS do
			cards[i] = ProviderCard { p = p }
		end
		return {
			K.Header("Providers", "Run the assistant on a plan you already pay for, or bring a key. Connected providers add their models to the picker."),
			K.SectionRow("PROVIDERS",
				ui.text { function()
					local n = 0
					for _, p in PROVIDERS do
						if status(p) == "connected" then
							n += 1
						end
					end
					return n .. " of " .. #PROVIDERS .. " connected"
				end, id = "providers_total", font = K.MONO, text_size = 10.5, color = C.muted }, true),
			ui.column { w_full = true, gap = 10, cards },
			K.Section("BILLING"),
			K.flag("Prefer subscriptions over API billing", "When a model is on both, use your plan's limits first and fall back to a key."),
			K.flag("Reuse sign-ins from installed CLIs", "Picks up Codex, Claude Code and OpenCode sign-ins found on this Mac."),
		}
	end

	return {
		Assistant = function()
			return { Assistant {} }
		end,
		Providers = ProvidersPage,
	}
end
