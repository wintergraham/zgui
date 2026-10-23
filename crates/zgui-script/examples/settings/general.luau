-- General (account, search engine, AI) and the plain pages: Passwords,
-- Spaces & profiles, Developer (Settings.tsx Generic, SettingsSections.tsx).
return function(K)
	local model = K.model
	local C = K.C

	---------------------------------------------------------------------------
	-- Account
	---------------------------------------------------------------------------
	local ACCOUNT = { name = "Robin Hale", email = "robin@hale.studio", initials = "RH", since = "March 2026" }
	local function Avatar(size)
		return ui.overlay {
			w = size, h = size, rounded = size / 2, overflow = "hidden", shrink = 0, bg = "#1F1E1B",
			items = "center", justify = "center", border = 1, border_color = "#FFFFFF14", border_over = true,
			K.dither(function()
				return K.scaled({ ink = "#EDEBE5FF", raw = true, pixel = size > 40 and 2 or 1, mode = 2, angle = 225, radius = 0.5, to = 0.34, curve = 1.6 }, K.amount("orbs"))
			end),
			ui.text { ACCOUNT.initials, font = K.SANS, text_size = size * 0.36, weight = 500, color = "#EDEBE5", letter_spacing = size * 0.36 * 0.02, self = "center" },
		}
	end
	-- Activity since joining: one cell per day, oldest week first.
	local WEEKS = 32
	local ACTIVITY = {}
	for i = 0, WEEKS * 7 - 1 do
		local w, day = i // 7, i % 7
		local n = math.sin(i * 12.9898 + 78.233) * 43758.5453
		local r = n - math.floor(n)
		local v = r * (0.45 + w / WEEKS * 0.75) * (day >= 5 and 0.55 or 1)
		ACTIVITY[i + 1] = v < 0.18 and 0 or v < 0.38 and 1 or v < 0.58 and 2 or v < 0.78 and 3 or 4
	end
	local function level(l)
		return l == 0 and C.raised or K.oklch(({ 0.36, 0.48, 0.62, 0.76 })[l], ({ 0.045, 0.065, 0.085, 0.1 })[l], 160)
	end
	local function ActivityGrid()
		local cols = {}
		for w = 0, WEEKS - 1 do
			local cells = {}
			for d = 0, 6 do
				cells[d + 1] = ui.div { class = "ac-cell", w_full = true, aspect = 1, rounded = 2, bg = level(ACTIVITY[w * 7 + d + 1]), opacity = K.reduced() and 1 or 0 }
			end
			cols[w + 1] = ui.column { grow = 1, basis = 0, min_w = 0, gap = 3, cells }
		end
		local legend = {}
		for l = 0, 4 do
			legend[l + 1] = ui.div { w = 9, h = 9, rounded = 2, bg = level(l) }
		end
		return ui.column {
			w_full = true, gap = 8, p = 13, rounded = 10, bg = C.panel, border = 1, border_color = C.subtle,
			on_mount = function()
				if not K.reduced() then
					anime.animate(".ac-cell", {
						opacity = { 0, 1 }, scale = { 0.4, 1 }, duration = 320, ease = K.EASE_OUT,
						delay = function(_, i)
							return (i // 7) * 14
						end,
					})
				end
			end,
			ui.row { w_full = true, gap = 3, cols },
			ui.row { w_full = true, items = "center",
				K.text("Most active on Tuesdays", 11, C.muted), K.spacer(),
				ui.row { gap = 3, items = "center", K.text("Less", 10.5, C.muted, { mr = 3 }), legend, K.text("More", 10.5, C.muted, { ml = 3 }) },
			},
		}
	end
	local function Aurora()
		local frame = ui.use_state(0)
		ui.on_mount(function()
			if K.reduced() then
				frame:set(6)
			else
				anime.animate(frame, { value = 6, duration = 6 * 1000 / 12, ease = "linear", frameRate = 12, modifier = anime.utils.round(0) })
			end
		end)
		local function k()
			return K.amount("background") * frame:get() / 6
		end
		return ui.overlay {
			absolute = true, top = 0, left = 0, right = 0, h = 170, overflow = "hidden", fade = { 0, 76 }, pointer_events = "none",
			K.dither(function()
				return K.scaled({ ink = "#4F5B6EFF", pixel = 2, mode = 3, from = 0.05, to = 0.7, curve = 1.4, scale = 2.2, seed = 3, fade = 0.75 }, k())
			end, nil, 0.08),
			K.dither(function()
				return K.scaled({ ink = "#EDEBE5FF", pixel = 2, mode = 1, cx = 0.5, cy = 1.15, radius = 0.75, from = 0.38, to = 0, curve = 1.5, noise = 0.5, scale = 4 }, k())
			end, nil, 0.3),
		}
	end
	local AccountDialog = ui.component(function()
		local devices = {}
		for i, d in { { "monitor", "MacBook Pro", "This device · Ask 0.1.0" }, { "smartphone", "iPhone 17 Pro", "Ask for iOS · 2 hours ago" } } do
			devices[i] = ui.row {
				w_full = true, h = 36, px = 10, gap = 10, items = "center", rounded = 8, bg = C.elevated, border = 1, border_color = C.subtle,
				K.icon(d[1], 15), K.text(d[2], 12.5), K.spacer(), K.text(d[3], 11.5, C.muted),
			}
		end
		local function label(text, trail)
			return ui.row { w_full = true, pt = 16, items = "center", K.label(text), K.spacer(), trail }
		end
		return ui.column {
			id = "account_dialog", w = 520, max_w = "100%", rounded = 14, overflow = "hidden", bg = C.page, border = 1, border_color = C.strong,
			shadow = { { x = 0, y = 24, blur = 64, color = "#00000080" }, { x = 0, y = 2, blur = 8, color = "#00000040" } },
			Aurora(),
			ui.column {
				w_full = true,
				ui.column {
					w_full = true, items = "center", gap = 12, pt = 64, px = 24, pb = 18,
					ui.div {
						rounded = 36, shadow = { { x = 0, y = 0, blur = 0, spread = 4, color = C.page }, { x = 0, y = 0, blur = 0, spread = 5, color = C.strong }, { x = 0, y = 12, blur = 32, color = "#00000080" } },
						Avatar(72),
					},
					ui.column { items = "center", gap = 3,
						K.text(ACCOUNT.name, 20, C.text, { weight = 500, letter_spacing = -0.3 }),
						K.text(ACCOUNT.email, 12.5, C.tertiary),
					},
					ui.row {
						h = 26, px = 11, gap = 7, items = "center", rounded = 13,
						border = 1, border_color = K.oklch(0.74, 0.07, 250, 0.28), bg = K.oklch(0.74, 0.07, 250, 0.08),
						K.text("Member since " .. ACCOUNT.since, 11.5, C.secondary),
					},
				},
				ui.column {
					w_full = true, gap = 10, pt = 4, px = 20, pb = 18,
					label("ACTIVITY", K.mono("12-day streak", 10, K.oklch(0.8, 0.1, 70), { letter_spacing = 0.4 })),
					ActivityGrid(),
					label("SIGNED IN ON"),
					devices,
				},
				ui.row {
					w_full = true, gap = 8, px = 20, py = 14, items = "center", bg = C.panel,
					K.btn { label = "Export data", id = "export_data" },
					K.spacer(),
					K.button {
						id = "sign_out", h = 30, px = 10, gap = 6, rounded = 7, hover = { bg = C.error_bg },
						K.icon("arrow-right-from-line", 13, C.error), K.text("Sign out", 12.5, C.error),
					},
				},
			},
			K.button {
				id = "account_close", absolute = true, top = 12, right = 12, w = 28, h = 28, rounded = 8, justify = "center",
				bg = "#12121199", backdrop_blur = 6, hover = { bg = C.selected },
				on_click = K.close_modal,
				K.icon("x", 14, C.secondary),
			},
		}
	end)
	local function AccountCard()
		return ui.overlay(K.target("Account", {
			w_full = true, mb = 6,
			K.flash_ring("Account", 12),
			K.button {
				id = "account", w_full = true, gap = 14, p = 15, rounded = 12, bg = C.card, border = 1, border_color = C.line,
				hover = { bg = C.elevated, border_color = C.hover_line },
				on_click = function()
					K.open_modal("account", function()
						return AccountDialog {}
					end)
				end,
				Avatar(40),
				ui.column { grow = 1, shrink = 1, min_w = 0, gap = 3,
					K.text(ACCOUNT.name, 14, C.text, { weight = 500 }),
					K.text(ACCOUNT.email, 12, C.tertiary),
				},
				ui.row { h = 30, px = 12, rounded = 7, items = "center", bg = C.ctl, border = 1, border_color = C.ctl_line, K.text("Manage", 12.5) },
			},
		}))
	end

	---------------------------------------------------------------------------
	-- Search engines
	---------------------------------------------------------------------------
	local ENGINES = {
		{ id = "google", name = "Google", url = "https://www.google.com/search?q=%s" },
		{ id = "duckduckgo", name = "DuckDuckGo", url = "https://duckduckgo.com/?q=%s" },
		{ id = "brave", name = "Brave Search", url = "https://search.brave.com/search?q=%s" },
		{ id = "ecosia", name = "Ecosia", url = "https://www.ecosia.org/search?q=%s" },
		{ id = "bing", name = "Bing", url = "https://www.bing.com/search?q=%s" },
		{ id = "kagi", name = "Kagi", url = "https://kagi.com/search?q=%s" },
		{ id = "startpage", name = "Startpage", url = "https://www.startpage.com/do/search?q=%s" },
		{ id = "perplexity", name = "Perplexity", url = "https://www.perplexity.ai/search?q=%s" },
	}
	local function host(url)
		local h = url:gsub("%%s", "q"):match("^%a[%w+.-]*://([^/?#:]+)")
		return h and (h:gsub("^www%.", "")) or url
	end
	local function EngineIcon(e, size)
		size = size or 16
		if not e.custom then
			return K.image("search-engines/" .. e.id, size, { rounded = 4 })
		end
		local hue = 0
		for i = 1, #e.name do
			hue += string.byte(e.name, i) * 7
		end
		return ui.div {
			w = size, h = size, rounded = 4, shrink = 0, bg = K.oklch(0.42, 0.07, hue % 360), items = "center", justify = "center",
			ui.text { string.upper(e.name:sub(1, 1)), font = K.SANS, text_size = size * 0.62, weight = 600, color = "#FFFFFF" },
		}
	end
	local function validate(name, url)
		local err = ""
		local scheme = url:gsub("%%s", "q"):match("^(%a[%w+.-]*):")
		if not scheme or ((scheme == "http" or scheme == "https") and not url:match("^%a+://[^/?#%s]+")) then
			err = "Enter the full address, starting with https://"
		elseif scheme ~= "http" and scheme ~= "https" then
			err = "Use an http or https address."
		elseif not url:find("%s", 1, true) then
			err = "Put %s where the search terms go."
		end
		if name:match("^%s*$") and err == "" then
			err = "Give it a name."
		end
		return err
	end
	local AddEngine = ui.component(function()
		local name, url, tried = ui.use_state(""), ui.use_state(""), ui.use_state(false)
		local err = K.computed(function()
			return validate(name:get(), url:get())
		end)
		local function submit()
			tried:set(true)
			if err:peek() == "" then
				local list = table.clone(model.engines:peek())
				local e = { id = "c-" .. #list + 1 .. "-" .. math.random(100000, 999999), name = (name:peek():match("^%s*(.-)%s*$")), url = url:peek(), custom = true }
				table.insert(list, e)
				ui.batch(function()
					model.engines:set(list)
					model.engine:set(e.id)
				end)
				K.close_modal()
			end
		end
		local function field(label, id, value, placeholder, mono, focus)
			local focused = ui.use_state(false)
			return ui.column { w_full = true, gap = 6,
				K.text(label, 12, C.secondary),
				ui.row {
					w_full = true, h = 32, px = 10, rounded = 8, bg = C.inset, border = 1, items = "center",
					border_color = K.tween(function()
						return focused:get() and C.focus or C.line
					end, K.FAST),
					K.input {
						id = id, value = value, placeholder = placeholder, size = 12.5, mono = mono,
						on_focus = function()
							focused:set(true)
						end,
						on_blur = function()
							focused:set(false)
						end,
						on_mount = focus and function(el)
							K.after(30, function()
								el:focus()
							end)
						end or nil,
						on_key = function(e)
							if e.key == "Enter" then
								submit()
								return true
							end
							return false
						end,
					},
				},
			}
		end
		return ui.column {
			id = "add_engine", w = 460, max_w = "100%", p = 23, gap = 12, rounded = 14, bg = C.page, border = 1, border_color = C.strong,
			shadow = { { x = 0, y = 24, blur = 64, color = "#00000080" }, { x = 0, y = 2, blur = 8, color = "#00000040" } },
			ui.column { w_full = true, gap = 4, mb = 6,
				K.text("Add search engine", 17, C.text, { weight = 500, letter_spacing = -0.2 }),
				K.text("Any site with a search results page works.", 13, C.tertiary),
			},
			field("Name", "engine_name", name, "Wikipedia", false, true),
			field("URL with %s in place of the query", "engine_url", url, "https://en.wikipedia.org/w/index.php?search=%s", false),
			ui.div { w_full = true, min_h = 16,
				ui.show(function()
					return tried:get() and err:get() ~= ""
				end, ui.text { function()
					return err:get()
				end, id = "engine_error", font = K.SANS, text_size = 11, color = C.error }, ui.presence(function()
					local u = url:get()
					return (u:find("%s", 1, true) and err:get() == "") and u or ""
				end, function(u)
					if u == "" then
						return ui.div {}
					end
					return ui.rich {
						id = "engine_preview", font = K.SANS, text_size = 11.5, color = C.muted, truncate = true,
						ui.span { "Searching " }, ui.span { "dither", color = C.secondary, weight = 500 }, ui.span { " opens " },
						ui.span { (u:gsub("%%s", "dither")), font = K.MONO, text_size = 11, color = C.tertiary },
					}
				end)),
			},
			ui.row { w_full = true, justify = "end", gap = 8, mt = 6,
				K.btn { label = "Cancel", id = "engine_cancel", on_click = K.close_modal },
				K.btn { label = "Add", id = "engine_add", primary = true, on_click = submit },
			},
		}
	end)
	local function SearchEngine()
		local a = K.anchor()
		local function all()
			local list = table.clone(ENGINES)
			for _, e in model.engines:get() do
				table.insert(list, e)
			end
			return list
		end
		local function current()
			for _, e in all() do
				if e.id == model.engine:get() then
					return e
				end
			end
			return ENGINES[1]
		end
		local function build()
			local items = {}
			local cur = current()
			for i, e in ENGINES do
				items[#items + 1] = K.menu_item {
					id = "engine_" .. e.id, label = e.name, lead = EngineIcon(e), meta = host(e.url), selected = e.id == cur.id,
					on_click = function()
						model.engine:set(e.id)
						K.close_menu()
					end,
				}
			end
			local custom = model.engines:peek()
			if #custom > 0 then
				items[#items + 1] = ui.div { h = 1, mx = 6, my = 4 }
			end
			for _, e in custom do
				local hovered = ui.use_state(false)
				items[#items + 1] = K.menu_item {
					id = "engine_" .. e.id, label = e.name, lead = EngineIcon(e), meta = host(e.url), selected = e.id == cur.id,
					on_click = function()
						model.engine:set(e.id)
						K.close_menu()
					end,
					trail = K.button {
						id = "engine_remove_" .. e.id, w = 20, h = 20, mr = -4, rounded = 5, justify = "center", hover = { bg = C.strong },
						opacity = K.tween(function()
							return hovered:get() and 1 or 0
						end, K.FAST),
						on_click = function()
							local rest = {}
							for _, c in model.engines:peek() do
								if c.id ~= e.id then
									table.insert(rest, c)
								end
							end
							ui.batch(function()
								model.engines:set(rest)
								if model.engine:peek() == e.id then
									model.engine:set("google")
								end
							end)
							K.close_menu()
						end,
						K.icon("x", 12, C.secondary),
					},
				}
				items[#items] = ui.div { w_full = true, on_hover = function(inside)
					hovered:set(inside)
				end, items[#items] }
			end
			items[#items + 1] = ui.div { h = 1, mx = 6, my = 4 }
			items[#items + 1] = K.menu_item {
				id = "engine_add_open", label = "Add search engine…",
				lead = ui.div { w = 16, h = 16, rounded = 4, border = 1, border_dash = { 2, 2 }, border_color = C.strong, items = "center", justify = "center", K.icon("plus", 12, C.secondary) },
				on_click = function()
					K.open_modal("add_engine", function()
						return AddEngine {}
					end)
				end,
			}
			return ui.column { ui.presence(function()
				return #model.engines:get()
			end, function()
				return K.menu_panel({ min_w = 280 }, items)
			end) }
		end
		return K.Row {
			label = "Search engine", desc = "Used when you type in the address bar. Add any site that takes a query.",
			child = K.button {
				id = "engine", h = 30, pl = 8, pr = 10, gap = 6, rounded = 7, shrink = 0, bg = function()
					return K.menu_open("engine") and C.hover or C.ctl
				end, border = 1, border_color = function()
					return K.menu_open("engine") and C.hover_line or C.ctl_line
				end, hover = { bg = C.hover, border_color = C.hover_line },
				on_mount = a.mount,
				on_click = function()
					K.toggle_menu("engine", a.el, build)
				end,
				K.fixed(ui.presence(function()
					return current().id
				end, function()
					local e = current()
					return ui.row { gap = 6, items = "center", EngineIcon(e), K.text(e.name, 12.5, C.soft, { id = "engine_value" }) }
				end)),
				K.icon("chevrons-up-down", 12, C.muted),
			},
		}
	end

	---------------------------------------------------------------------------
	-- Pages
	---------------------------------------------------------------------------
	local TITLES = {
		["Tabs & navigation"] = { "Tabs & navigation", "How tabs are laid out and where the address bar lives." },
		["Extensions"] = { "Extensions", "Chrome Web Store extensions installed in this profile." },
		["Downloads"] = { "Downloads", "Where files are saved and what happens when they finish." },
	}
	local function header(s)
		local t = TITLES[s]
		if t then
			return K.Header(t[1], t[2])
		end
		local wip = s == "Passwords" or s == "Spaces & profiles"
		return K.Header(s, wip and "This page is still in development." or "Preferences for this Ask profile.")
	end
	K.page_header = header

	local pages = {}
	pages.General = function()
		return {
			header("General"),
			AccountCard(),
			K.Section("BROWSING"),
			SearchEngine(),
			K.Section("ASSISTANT"),
			K.AiSwitch(),
			K.flag("Open assistant on launch", "Restore the assistant panel"),
		}
	end
	pages.Passwords = function()
		return { header("Passwords"), ui.row { mt = 16, gap = 8, K.btn { label = "Open Chromium settings", id = "chromium_passwords" } } }
	end
	pages["Spaces & profiles"] = function()
		return { header("Spaces & profiles"), K.text("Work · Personal", 13, C.secondary, { mb = 12 }) }
	end
	pages.Developer = function()
		return { header("Developer"), K.flag("Show frame counter", "Native UI FPS and presented frames; excludes the counter's own redraws") }
	end
	pages.Generic = function(s)
		return { header(s) }
	end
	return pages
end
