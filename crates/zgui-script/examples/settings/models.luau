-- Assistant › Models & loadouts (SettingsModels.tsx; Pen 11 "Model picker &
-- loadouts"): provider usage, the loadout list and its editor with the model
-- browser, switching options and task models.
return function(K)
	local model = K.model
	local C = K.C

	local CATALOG = {
		{ id = "gpt-6", name = "GPT-6", provider = "Codex", logo = "openai", ctx = "400K", out = "128K", reasoning = "Minimal – Max", images = true, acts = true, cost = "Included in Pro", desc = "OpenAI's flagship reasoning model. Uses your ChatGPT Pro limits, not API credit." },
		{ id = "gpt-6-mini", name = "GPT-6 mini", provider = "Codex", logo = "openai", ctx = "400K", out = "128K", reasoning = "Minimal – High", images = true, acts = true, cost = "Included in Pro", desc = "A lighter GPT-6 for quick work. Counts for less of your 5-hour window." },
		{ id = "opus-5.5", name = "Opus 5.5", provider = "Anthropic", logo = "claude", ctx = "1M", out = "64K", reasoning = "Thinking up to 64K", images = true, acts = true, cost = "Included in Max", desc = "Anthropic's most capable model, for long and careful work across many tabs." },
		{ id = "sonnet-5.5", name = "Sonnet 5.5", provider = "Anthropic", logo = "claude", ctx = "1M", out = "64K", reasoning = "Thinking up to 64K", images = true, acts = true, cost = "Included in Max", desc = "Fast and thorough. A good default for agents acting on pages." },
		{ id = "haiku-4.5", name = "Haiku 4.5", provider = "Anthropic", logo = "claude", ctx = "200K", out = "64K", reasoning = "Thinking up to 32K", images = true, acts = false, cost = "Included in Max", desc = "The quickest Claude model, for summaries, titles and side work." },
		{ id = "grok-4.7", name = "Grok 4.7", provider = "Grok", logo = "grok", ctx = "2M", out = "64K", reasoning = "Low – High", images = true, acts = true, cost = "$3 / $15 per M tokens", desc = "xAI's reasoning model with a very long context. Billed to your API key." },
		{ id = "grok-4.7-fast", name = "Grok 4.7 Fast", provider = "Grok", logo = "grok", ctx = "2M", out = "30K", reasoning = "None", images = true, acts = false, cost = "$0.20 / $0.50 per M tokens", desc = "Cheap and immediate. No reasoning, so it answers right away." },
		{ id = "kimi-k3", name = "Kimi K3", provider = "OpenRouter", logo = "kimi", ctx = "256K", out = "32K", reasoning = "Thinking", images = false, acts = true, cost = "$0.60 / $2.50 per M tokens", desc = "Moonshot's open-weight agent model, served through OpenRouter." },
		{ id = "deepseek-v4", name = "DeepSeek V4", provider = "OpenRouter", logo = "deepseek", ctx = "128K", out = "32K", reasoning = "Thinking", images = false, acts = true, cost = "$0.27 / $1.10 per M tokens", desc = "Strong reasoning at a low price, served through OpenRouter." },
	}
	local function spec(id)
		for _, c in CATALOG do
			if c.id == id then
				return c
			end
		end
		return CATALOG[1]
	end
	local PROVIDERS = {
		{ name = "Codex", logo = "openai", auth = "ChatGPT sign-in · Pro", models = "GPT-6, GPT-6 mini", usage = { { "5-hour", 0.64, "64% · 2h 41m" }, { "Weekly", 0.72, "72% · Mon" } } },
		{ name = "Anthropic", logo = "claude", auth = "Claude sign-in · Max", models = "Opus 5.5, Sonnet 5.5, Haiku 4.5", usage = { { "5-hour", 0.12, "12% · 47m" }, { "Weekly", 0.58, "58% · Thu" } } },
		{ name = "Grok", logo = "grok", auth = "API key ••••c91e", models = "Grok 4.7, Grok 4.7 Fast", usage = { { "This month", 0.136, "$6.80 of $50" } } },
		{ name = "OpenRouter", logo = "openrouter", auth = "OAuth · personal", models = "6 of 312 models on", usage = { { "Credit", 0.37, "$18.40 left" } } },
	}
	local EFFORTS = { { "Minimal", "Answers right away" }, { "Low", "Light thinking, quick replies" }, { "Medium", "Balanced for most work" }, { "High", "Thinks longer on hard problems" }, { "Max", "Slowest. Uses your 5-hour window fastest" } }
	local LENGTHS = { "Default", "Concise", "Detailed" }
	local INHERIT = { "Inherit", "On", "Off" }
	local TASKS = {
		{ "Fast", "Quick, cheap side work: tab summaries, titles, pulling fields out of pages", "grok-4.7-fast" },
		{ "Standard", "Background tasks, automations and memory upkeep", "gpt-6-mini" },
		{ "Research", "Long reads across many tabs, with sources and citations", "opus-5.5" },
		{ "Deep", "Hard reasoning, planning, judging and synthesis", "gpt-6" },
		{ "Visual", "Screenshots, CAPTCHAs, charts and page layout", "gpt-6" },
	}
	local function bars(effort)
		for i, e in EFFORTS do
			if e[1] == effort then
				return i - 1
			end
		end
		return 0
	end

	local ms = model.models
	local function loadouts()
		return ms:get().loadouts
	end
	local function find(id)
		for _, l in ms:get().loadouts do
			if l.id == id then
				return l
			end
		end
		return nil
	end
	local function update(id, changes)
		local list = {}
		for i, l in ms:peek().loadouts do
			if l.id == id then
				l = table.clone(l)
				for k, v in changes do
					l[k] = v
				end
			end
			list[i] = l
		end
		model.patch(ms, { loadouts = list })
	end
	local function add(base)
		local cur = ms:peek()
		local id = 0
		for _, l in cur.loadouts do
			id = math.max(id, l.id)
		end
		id += 1
		local n = #cur.loadouts + 1
		local keys = n <= 9 and { "⌥", tostring(n) } or {}
		local l
		if base then
			l = table.clone(base)
			l.id, l.name, l.keys = id, base.name .. " copy", keys
		else
			l = model.loadout(id, "New loadout", "gpt-6", "Medium", "GPT-6 · Reasoning medium", { keys = keys })
		end
		local list = table.clone(cur.loadouts)
		table.insert(list, l)
		model.patch(ms, { loadouts = list, selected = id })
	end
	local function remove(id)
		local cur = ms:peek()
		if #cur.loadouts < 2 then
			return
		end
		local rest = {}
		for _, l in cur.loadouts do
			if l.id ~= id then
				table.insert(rest, l)
			end
		end
		model.patch(ms, { loadouts = rest, selected = rest[1].id, defaultId = cur.defaultId == id and rest[1].id or cur.defaultId })
	end

	---------------------------------------------------------------------------
	-- Small pieces
	---------------------------------------------------------------------------
	local function Meter(label, value, text)
		return ui.column { w_full = true, gap = 6,
			ui.row { w_full = true, K.text(label, 11, C.muted), K.spacer(), K.mono(text, 11, C.secondary) },
			ui.div { w_full = true, h = 3, rounded = 2, bg = C.line, overflow = "hidden", ui.div { w = value * 100 .. "%", h = 3, rounded = 2, bg = C.secondary } },
		}
	end
	local function EffortBars(n)
		local out = {}
		for i, h in { 4, 6, 8, 10 } do
			out[i] = ui.div { w = 2.5, h = h, rounded = 0.5, bg = i <= n and C.secondary or C.strong }
		end
		return ui.row { h = 10, gap = 1.5, items = "end", shrink = 0, out }
	end
	local function group(label, help)
		return ui.row { w_full = true, pt = 16, pb = 4, items = "center", K.label(label), K.spacer(), help and K.text(help, 11, C.muted) or nil }
	end
	local function ib(id, icon, on_click)
		return K.button { id = id, w = 28, h = 28, rounded = 7, justify = "center", shrink = 0, hover = { bg = C.hover }, on_click = on_click, K.icon(icon, 14, C.secondary) }
	end

	-- Options for a model field: provider headers, then each model; Auto first
	-- for task models.
	local function model_options(auto, except)
		local out = {}
		if auto then
			table.insert(out, { id = "auto", label = "Auto", desc = "Picks from your providers, skipping any near a limit" })
		end
		for _, p in { "Anthropic", "Codex", "Grok", "OpenRouter" } do
			table.insert(out, { id = "h-" .. p, label = string.upper(p), header = true })
			for _, c in CATALOG do
				if c.provider == p and c.id ~= except then
					table.insert(out, { id = c.id, label = c.name, logo = c.logo, meta = c.ctx })
				end
			end
		end
		return out
	end

	-- Field select (Pen "Box"). o: id, value (fn), meta (fn), logo (fn),
	-- options (fn or list of {id, label, desc, logo, meta, header}), pick(id).
	local function FieldSelect(o)
		local a = K.anchor()
		local function options()
			return type(o.options) == "function" and o.options() or o.options
		end
		local function build()
			local items, height = {}, 8
			local v = o.value()
			for i, opt in options() do
				if opt.header then
					items[i] = ui.div { w_full = true, pt = 8, px = 8, pb = 3, K.mono(opt.label, 10, C.muted, { letter_spacing = 1 }) }
					height += 25
				else
					local on = opt.label == v or opt.id == v
					items[i] = K.menu_item {
						id = o.id .. "_option_" .. opt.id:gsub("[^%w]", "_"), label = opt.label, desc = opt.desc, meta = opt.meta, checked = on,
						lead = opt.logo and K.tile(opt.logo, 16) or nil,
						on_click = function()
							o.pick(opt.id)
							K.close_menu()
						end,
					}
					height += opt.desc and 48 or 31
				end
			end
			return K.menu_panel({ min_w = 240, p = 0 }, {
				ui.scroll { w_full = true, h = math.min(height, 340), ui.column { w_full = true, p = 4, gap = 1, items } },
			})
		end
		return K.button {
			id = o.id, h = 30, px = 9, gap = 7, rounded = 7, shrink = 0, max_w = 260, bg = C.inset, border = 1,
			border_color = function()
				return K.menu_open(o.id) and C.hover_line or C.line
			end,
			hover = { border_color = C.hover_line },
			on_mount = a.mount,
			on_click = function()
				K.toggle_menu(o.id, a.el, build)
			end,
			o.logo and K.fixed(ui.presence(o.logo, function(logo)
				return K.tile(logo, 16)
			end)) or nil,
			ui.text { o.value, id = o.id .. "_value", font = K.SANS, text_size = 12.5, color = C.text, truncate = true, shrink = 1 },
			o.meta and ui.text { o.meta, font = K.SANS, text_size = 11.5, color = C.muted } or nil,
			K.icon("chevrons-up-down", 12, C.muted),
		}
	end

	---------------------------------------------------------------------------
	-- Model browser: search, provider filters and the list on the left, the
	-- highlighted model's specs on the right.
	---------------------------------------------------------------------------
	local function Browser(value, pick)
		local q, filter, hi = ui.use_state(""), ui.use_state("All"), ui.use_state(value)
		local list = K.computed(function()
			local out = {}
			local query = string.lower(q:get())
			for _, c in CATALOG do
				if (filter:get() == "All" or c.provider == filter:get()) and string.find(string.lower(c.name .. " " .. c.provider), query, 1, true) then
					table.insert(out, c)
				end
			end
			return out
		end)
		local function cur()
			local l = list:get()
			for _, c in l do
				if c.id == hi:get() then
					return c
				end
			end
			return l[1] or spec(value)
		end
		local function choose(id)
			pick(id)
			K.close_menu()
		end
		local chips = {}
		for i, f in { "All", "Codex", "Anthropic", "Grok", "OpenRouter" } do
			local function on()
				return filter:get() == f
			end
			chips[i] = K.button({
				id = "browser_filter_" .. string.lower(f), h = 22, px = 8, rounded = 11, border = 1,
				bg = function()
					return on() and C.text or C.clear
				end,
				border_color = function()
					return on() and C.text or C.line
				end,
				on_click = function()
					filter:set(f)
				end,
				ui.text { f, font = K.SANS, text_size = 11, color = function()
					return on() and C.panel or C.secondary
				end },
			}, false)
		end
		local spec_rows = function(c)
			local out = {}
			for i, kv in { { "Context", c.ctx .. " tokens" }, { "Max output", c.out .. " tokens" }, { "Reasoning", c.reasoning }, { "Images & PDFs", c.images and "Yes" or "No" }, { "Acts on pages", c.acts and "Yes" or "No" }, { "Cost", c.cost } } do
				out[i] = ui.row { w_full = true, K.text(kv[1], 11.5, C.muted), K.spacer(), K.text(kv[2], 11.5, kv[2] == "No" and C.muted or C.text) }
			end
			return out
		end
		return ui.column {
			id = "model_browser", w = 560, rounded = 10, overflow = "hidden", bg = C.elevated, border = 1, border_color = C.strong,
			shadow = { { x = 0, y = 12, blur = 28, color = "#00000066" }, { x = 0, y = 2, blur = 6, color = "#00000040" } },
			on_key = function(e)
				local l = list:peek()
				local i = 1
				for k, c in l do
					if c.id == cur().id then
						i = k
					end
				end
				if e.key == "ArrowDown" or e.key == "ArrowUp" then
					local n = l[(i - 1 + (e.key == "ArrowDown" and 1 or -1)) % math.max(1, #l) + 1]
					if n then
						hi:set(n.id)
					end
					return true
				end
				if e.key == "Enter" and #l > 0 then
					choose(cur().id)
					return true
				end
				return false
			end,
			ui.row { w_full = true, items = "stretch",
				ui.column { w = 330, p = 8, border = 1, border_sides = { 0, 1, 0, 0 }, border_color = C.selected,
					ui.row { w_full = true, h = 32, px = 8, gap = 8, rounded = 7, items = "center", bg = C.inset, border = 1, border_color = C.line,
						K.icon("search", 13, C.muted),
						K.input { id = "browser_search", value = q, placeholder = "Search 340 models", size = 12.5, on_mount = function(el)
							K.after(30, function()
								el:focus()
							end)
						end },
					},
					ui.div { pt = 12, px = 6, pb = 6, K.label("MODELS") },
					ui.row { w_full = true, gap = 4, px = 4, pb = 6, flex_wrap = true, chips },
					ui.scroll { w_full = true, h = 300,
						ui.column { w_full = true, gap = 1,
							ui.presence(function()
								local ids = {}
								for _, c in list:get() do
									table.insert(ids, c.id)
								end
								return table.concat(ids, ",") .. "|" .. q:get()
							end, function()
								local items = {}
								for i, c in list:peek() do
									local function on()
										return cur().id == c.id
									end
									items[i] = K.button({
										id = "browser_" .. c.id:gsub("[^%w]", "_"), w_full = true, h = 30, px = 8, gap = 9, rounded = 6, shrink = 0,
										bg = function()
											return on() and C.selected or C.clear
										end,
										on_hover = function(inside)
											if inside then
												hi:set(c.id)
											end
										end,
										on_click = function()
											choose(c.id)
										end,
										K.tile(c.logo, 18), K.text(c.name, 12.5), K.spacer(),
										c.id == value and K.icon("check", 12, C.secondary) or nil,
										K.mono(c.ctx, 10.5, C.muted, { min_w = 34, align = "right" }),
									}, false)
								end
								if #items == 0 then
									items[1] = ui.div { px = 8, py = 10, K.text("No models match “" .. q:peek() .. "”", 12, C.muted) }
								end
								return ui.column { w_full = true, gap = 1, items }
							end),
							K.button({ w_full = true, h = 34, mt = 4, px = 8, gap = 9, corners = { 0, 0, 6, 6 }, K.tile("openrouter", 18), K.text("Browse 312 models on OpenRouter", 12.5, C.secondary), K.spacer(), K.icon("arrow-right", 12, C.secondary) }, false),
						},
					},
				},
				ui.presence(function()
					return cur().id
				end, function()
					local c = cur()
					return ui.column {
						w = 230, h_full = true, p = 14, gap = 10, bg = C.page,
						on_mount = function(el)
							anime.animate(el, { opacity = { 0, 1 }, duration = K.BASE, ease = K.EASE_OUT })
						end,
						ui.row { gap = 10, items = "center", K.tile(c.logo, 30),
							ui.column { gap = 2, K.text(c.name, 14, C.text, { weight = 500, id = "browser_detail_name" }), K.mono(c.id .. " · " .. c.provider, 10.5, C.muted) } },
						ui.text { c.desc, font = K.SANS, text_size = 12, line_height = 17.4, color = C.secondary, wrap = true, align = "left", mb = 4 },
						spec_rows(c),
						K.spacer(),
						ui.text { "Loadout settings this model doesn't support are hidden.", font = K.SANS, text_size = 11, line_height = 15.4, color = C.muted, wrap = true, align = "left", pt = 10 },
					}
				end),
			},
			ui.row { w_full = true, gap = 6, px = 10, py = 7, items = "center",
				K.keycap("↑↓", "sm"), K.text("navigate", 11, C.muted), K.keycap("↵", "sm", { ml = 8 }), K.text("select", 11, C.muted),
			},
		}
	end
	local function ModelField(l)
		local a = K.anchor()
		local id = "loadout_model"
		return K.button {
			id = id, h = 30, px = 9, gap = 7, rounded = 7, shrink = 0, min_w = 250, max_w = 260, bg = C.inset, border = 1,
			border_color = function()
				return K.menu_open(id) and C.hover_line or C.line
			end,
			hover = { border_color = C.hover_line },
			on_mount = a.mount,
			on_click = function()
				local value = l().model
				K.toggle_menu(id, a.el, function()
					return Browser(value, function(m)
						update(l().id, { model = m })
					end)
				end)
			end,
			ui.presence(function()
				return l().model
			end, function(mid)
				local m = spec(mid)
				return ui.row { gap = 7, items = "center", grow = 1, shrink = 1, min_w = 0,
					K.tile(m.logo, 18), K.text(m.name, 12.5, C.text, { id = "loadout_model_value" }),
					K.text(m.provider .. " · " .. m.ctx .. " context", 11.5, C.muted, { truncate = true, shrink = 1 }),
				}
			end),
			K.spacer(),
			K.icon("chevrons-up-down", 12, C.muted),
		}
	end

	---------------------------------------------------------------------------
	-- Loadout editor
	---------------------------------------------------------------------------
	local function ERow(label, desc, child)
		return ui.row { w_full = true, gap = 16, py = 10, items = "center",
			ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
				K.text(label, 13.5),
				K.para(desc, 12, C.tertiary, 17.4),
			},
			child,
		}
	end
	local Editor = ui.component(function(props)
		local lid = props.id
		local function l()
			return find(lid) or loadouts()[1]
		end
		local renaming, recording = ui.use_state(false), ui.use_state(false)
		local name = ui.use_state(l().name)
		local instructions = ui.use_state(l().instructions)
		ui.effect(function()
			local v = instructions:get()
			if v ~= l().instructions then
				update(lid, { instructions = v:sub(1, 2000) })
			end
		end)
		local function finish_rename()
			local v = name:peek():match("^%s*(.-)%s*$")
			update(lid, { name = v ~= "" and v or l().name })
			renaming:set(false)
		end
		local function m()
			return spec(l().model)
		end
		local function inherit(key, label, desc, inherited)
			return ERow(label, desc, ui.row { gap = 4, items = "center", shrink = 0,
				K.fixed(ui.show(function()
					return l()[key] ~= "Inherit"
				end, ib("reset_" .. key, "rotate-ccw", function()
					update(lid, { [key] = "Inherit" })
				end))),
				FieldSelect {
					id = "loadout_" .. key,
					value = function()
						return l()[key]
					end,
					meta = function()
						return l()[key] == "Inherit" and "· " .. inherited or ""
					end,
					options = function()
						local out = {}
						for i, v in INHERIT do
							out[i] = { id = v, label = v }
						end
						return out
					end,
					pick = function(v)
						update(lid, { [key] = v })
					end,
				},
			})
		end
		local blink, blinking = ui.ref(), nil
		ui.effect(function()
			if blinking then
				blinking:cancel()
				blinking = nil
			end
			if recording:get() and not K.reduced() then
				blinking = anime.animate(blink, { opacity = { 1, 0.45 }, duration = 500, loop = true, alternate = true, ease = "steps(1)" })
			end
		end)
		return ui.column {
			id = "loadout_editor", w_full = true, gap = 6, p = 21, rounded = 12, bg = C.card, border = 1, border_color = C.line, min_w = 0,
			ui.row { w_full = true, gap = 12, items = "center",
				K.fixed(ui.presence(function()
					return m().logo
				end, function(logo)
					return K.tile(logo, 34)
				end)),
				ui.column { grow = 1, shrink = 1, min_w = 0, gap = 3,
					ui.presence(renaming, function(on)
						if on then
							return ui.row { self = "start", px = 6, h = 24, mx = -7, my = -2, rounded = 6, bg = C.inset, border = 1, border_color = C.focus, items = "center", w = 240,
								K.input { id = "loadout_rename", value = name, size = 16, on_mount = function(el)
									K.after(30, function()
										el:focus()
									end)
								end, on_blur = finish_rename, on_key = function(e)
									if e.key == "Enter" then
										finish_rename()
										return true
									elseif e.key == "Escape" then
										renaming:set(false)
										return true
									end
									return false
								end },
							}
						end
						return K.button({
							id = "loadout_name", self = "start", gap = 7, cursor = "text",
							on_click = function()
								name:set(l().name)
								renaming:set(true)
							end,
							ui.text { function()
								return l().name
							end, id = "loadout_name_value", font = K.SANS, text_size = 16, weight = 500, letter_spacing = -0.2, color = C.text },
							K.icon("pencil", 12, C.muted, { opacity = 0.7 }),
						}, false)
					end),
					ui.text { function()
						return ms:get().defaultId == lid and "Default for new chats" or l().note
					end, font = K.SANS, text_size = 11.5, color = C.tertiary, align = "left" },
				},
				ib("loadout_duplicate", "copy", function()
					add(l())
				end),
				ib("loadout_delete", "trash-2", function()
					remove(lid)
				end),
			},
			group("MODEL"),
			ERow("Model", function()
				return m().provider .. " · " .. m().ctx .. " context"
			end, ModelField(l)),
			ui.show(function()
				return m().reasoning ~= "None"
			end, ERow("Reasoning effort", "How long it thinks before answering", FieldSelect {
				id = "loadout_effort",
				value = function()
					return l().effort
				end,
				options = function()
					local out = {}
					for i, e in EFFORTS do
						out[i] = { id = e[1], label = e[1], desc = e[2] }
					end
					return out
				end,
				pick = function(v)
					update(lid, { effort = v })
				end,
			})),
			ERow("Response length", "Default lets the model decide", FieldSelect {
				id = "loadout_length",
				value = function()
					return l().length
				end,
				options = function()
					local out = {}
					for i, v in LENGTHS do
						out[i] = { id = v, label = v }
					end
					return out
				end,
				pick = function(v)
					update(lid, { length = v })
				end,
			}),
			ERow("Fallback", function()
				return "When " .. m().provider .. " hits its 5-hour or weekly limit"
			end, FieldSelect {
				id = "loadout_fallback",
				value = function()
					return spec(l().fallback).name
				end,
				meta = function()
					return spec(l().fallback).provider
				end,
				logo = function()
					return spec(l().fallback).logo
				end,
				options = function()
					return model_options(false, l().model)
				end,
				pick = function(v)
					update(lid, { fallback = v })
				end,
			}),
			group("BEHAVIOUR", "Inherit follows the composer"),
			inherit("tabs", "Read open tabs", "Tabs you @ mention, or all in the space", "On"),
			ui.show(function()
				return m().acts
			end, inherit("act", "Act on pages", "Click, type and submit forms", "Ask first")),
			inherit("memories", "Use saved memories", "Preferences and facts from past chats", "On"),
			group("SHORTCUT & INSTRUCTIONS"),
			ERow("Keybind", "Switches to this loadout anywhere in Ask", ui.row { gap = 8, items = "center", shrink = 0,
				K.fixed(ui.show(recording, ui.row { ref = blink, h = 20, px = 8, rounded = 4, items = "center", bg = C.selected, K.mono("Press keys…", 10.5, C.text) },
					ui.presence(function()
						return table.concat(l().keys, " ")
					end, function()
						return #l().keys > 0 and K.keycap(l().keys) or ui.div {}
					end))),
				K.btn {
					id = "loadout_record", small = true, label = function()
						return recording:get() and "Cancel" or "Record"
					end,
					props = { on_click = function()
						if recording:peek() then
							recording:set(false)
							K.capture = nil
							return
						end
						recording:set(true)
						-- The next key anywhere in the window becomes the keybind.
						K.capture = function(e)
							if e.key == "Alt" or e.key == "Meta" or e.key == "Control" or e.key == "Shift" then
								return
							end
							K.capture = nil
							recording:set(false)
							if e.key == "Escape" then
								return
							end
							local keys = {}
							for _, mod in { { e.meta, "⌘" }, { e.ctrl, "⌃" }, { e.alt, "⌥" }, { e.shift, "⇧" } } do
								if mod[1] then
									table.insert(keys, mod[2])
								end
							end
							table.insert(keys, #e.key == 1 and string.upper(e.key) or e.key)
							update(lid, { keys = keys })
						end
					end },
				},
			}),
			ui.column { w_full = true, gap = 10, pt = 12, pb = 6,
				ui.row { w_full = true, gap = 12, items = "end",
					ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
						K.text("Instructions", 13.5),
						K.text("Added before every message you send with this loadout", 12, C.tertiary),
					},
					ui.text { function()
						local n = #instructions:get()
						return (n >= 1000 and string.format("%d,%03d", n // 1000, n % 1000) or tostring(n)) .. " / 2,000"
					end, id = "instructions_count", font = K.MONO, text_size = 11, color = C.muted },
				},
				ui.row { w_full = true, min_h = 84, px = 12, py = 10, rounded = 8, bg = C.inset, border = 1, border_color = C.line, items = "start",
					K.input { id = "loadout_instructions", value = instructions, multiline = true, size = 12.5, h = "auto", placeholder = "Optional. For example: answer in British English.", props = { min_h = 64 } },
				},
			},
			ui.row { w_full = true, gap = 8, mt = 10, pt = 16, items = "center",
				K.icon("check", 13, "#C3CEB2"), K.text("Saved · applies to new messages", 12, C.tertiary), K.spacer(),
				K.btn { label = "Test in new chat", icon = "plus", keys = "⏎", id = "test_chat" },
			},
		}
	end)

	---------------------------------------------------------------------------
	-- Page
	---------------------------------------------------------------------------
	local Page = ui.component(function()
		local narrow = K.computed(function()
			return K.viewport:get().w < 1180
		end)
		local cards = {}
		for i, p in PROVIDERS do
			local meters = {}
			for j, u in p.usage do
				meters[j] = Meter(u[1], u[2], u[3])
			end
			cards[i] = ui.column {
				id = "model_provider_" .. string.lower(p.name), min_w = 0, gap = 14, p = 15, rounded = 10, bg = C.card, border = 1, border_color = C.line,
				ui.row { w_full = true, gap = 10, items = "center", K.tile(p.logo, 28),
					ui.column { grow = 1, shrink = 1, min_w = 0, gap = 3, K.text(p.name, 13.5, C.text, { weight = 500 }), K.text(p.auth, 11.5, C.tertiary, { truncate = true }) } },
				K.text(p.models, 12, C.secondary, { wrap = true }),
				K.spacer(),
				ui.column { w_full = true, gap = 10, meters },
			}
		end
		local list = ui.presence(function()
			local parts = {}
			for _, l in loadouts() do
				table.insert(parts, l.id .. ":" .. l.name .. ":" .. l.model .. ":" .. l.effort .. ":" .. table.concat(l.keys, ""))
			end
			return table.concat(parts, "|")
		end, function()
			local items = {}
			for i, l in ms:peek().loadouts do
				local s = spec(l.model)
				local function on()
					return ms:get().selected == l.id
				end
				items[i] = K.button({
					id = "loadout_" .. l.id, w_full = true, h = 48, pl = 6, pr = 10, gap = 9, rounded = 8, border = 1, shrink = 0,
					bg = function()
						return on() and C.ctl or C.clear
					end,
					border_color = function()
						return on() and C.ctl_line or C.clear
					end,
					hover = { bg = C.page },
					on_click = function()
						model.patch(ms, { selected = l.id })
					end,
					K.fixed(ui.show(on, K.icon("grip-vertical", 12, C.muted), K.icon("grip-vertical", 12, C.faint))),
					K.tile(s.logo, 24),
					ui.column { grow = 1, shrink = 1, min_w = 0, gap = 3,
						K.text(l.name, 13, C.text, { weight = 500, truncate = true }),
						K.text(s.name .. " · " .. l.note, 11.5, C.muted, { truncate = true }),
					},
					EffortBars(s.reasoning == "None" and 0 or bars(l.effort)),
					#l.keys > 0 and K.keycap(l.keys) or nil,
				}, false)
			end
			return ui.column { w_full = true, gap = 2, items }
		end)
		local star = K.button({
			id = "loadout_star", w_full = true, h = 40, mt = 6, gap = 8, justify = "center", rounded = 8, border = 1, border_dash = { 3, 3 }, border_color = C.strong,
			color = C.tertiary, hover = { border_color = C.focus, color = C.text },
			on_click = function()
				add()
			end,
			K.icon("star", 13, C.tertiary), K.text("Star any model in the picker to add it", 12, false),
		}, false)
		local editor = ui.presence(function()
			return ms:get().selected
		end, function(id)
			return Editor { id = id }
		end)
		local function default_loadout()
			local cur = ms:get()
			for _, l in cur.loadouts do
				if l.id == cur.defaultId then
					return l
				end
			end
			return cur.loadouts[1]
		end
		local function toggle(key, label)
			return K.Toggle { id = "models_" .. key, label = label, on = function()
				return ms:get()[key]
			end, set = function(v)
				model.patch(ms, { [key] = v })
			end }
		end
		local tasks = {}
		for i, t in TASKS do
			local task, desc, auto = t[1], t[2], t[3]
			local function v()
				return ms:get().tasks[task] or "auto"
			end
			local function s()
				return spec(v() == "auto" and auto or v())
			end
			tasks[i] = K.Row {
				label = task, desc = desc, setting = i == 1 and "Fast" or task,
				child = ui.row { gap = 8, items = "center", shrink = 0,
					K.fixed(ui.show(function()
						return v() == "auto"
					end, ui.row { h = 18, px = 6, rounded = 4, items = "center", bg = C.hover, border = 1, border_color = C.ctl_line,
						K.mono("AUTO", 9.5, C.tertiary, { weight = 500, letter_spacing = 0.8 }) })),
					FieldSelect {
						id = "task_" .. string.lower(task),
						value = function()
							return s().name
						end,
						logo = function()
							return s().logo
						end,
						options = model_options(true),
						pick = function(id)
							local next = table.clone(ms:peek().tasks)
							next[task] = id
							model.patch(ms, { tasks = next })
						end,
					},
				},
			}
		end
		return ui.column {
			w_full = true,
			K.Header("Models & loadouts", "Connect providers, then save a model with its settings as a loadout. Loadouts sit at the top of the composer picker and switch with ⌥1–9."),
			K.SectionRow("PROVIDERS", K.btn { small = true, icon = "plus", label = "Add provider", id = "add_provider" }, true),
			ui.overlay(K.target("Providers", {
				w_full = true, mt = 8, mb = 6,
				K.flash_ring("Providers"),
				ui.grid { w_full = true, gap = 10, cols = function()
					return narrow:get() and 2 or 4
				end, cards },
			})),
			ui.row { w_full = true, gap = 12, items = "center", pt = 26,
				ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
					ui.div { pt = 10, pb = 4, px = 2, K.label("LOADOUTS") },
					K.text("Favourites with their own settings and keybind. Order sets ⌥1–9.", 12, C.tertiary),
				},
				K.btn { small = true, icon = "plus", label = "New loadout", keys = "⏎", id = "new_loadout", on_click = function()
					add()
				end },
			},
			ui.overlay(K.target("Loadouts", {
				w_full = true, mt = 8, mb = 12,
				K.flash_ring("Loadouts", 12),
				ui.row {
					w_full = true, gap = 12, items = "start",
					direction = function()
						return narrow:get() and "column" or "row"
					end,
					-- 330fr : 562fr of the room beside the 12pt gap.
					ui.column { id = "loadout_list", min_w = 0, shrink = 0, w = function()
						return narrow:get() and "100%" or "36.5%"
					end, list, star },
					ui.column { w_full = true, min_w = 0, shrink = 1, editor },
				},
			})),
			K.Section("SWITCHING"),
			K.Row { label = "Default for new chats", desc = "Each new chat starts on this loadout. Switching mid-chat only affects later messages.", child = FieldSelect {
				id = "default_loadout",
				value = function()
					return default_loadout().name
				end,
				logo = function()
					return spec(default_loadout().model).logo
				end,
				options = function()
					local out = {}
					for i, l in loadouts() do
						out[i] = { id = tostring(l.id), label = l.name, logo = spec(l.model).logo }
					end
					return out
				end,
				pick = function(id)
					model.patch(ms, { defaultId = tonumber(id) })
				end,
			} },
			K.Row { label = "Hold ⌥ to show the loadout switcher", desc = "Shows your loadouts with their numbers after a short hold. Press a number to switch.", child = toggle("hold", "Hold to show switcher") },
			K.Row { label = "Cycle loadouts", desc = "Steps through loadouts in list order from the composer.", child = K.keycap({ "⌥", "Tab" }) },
			K.Row { label = "Remember loadout per space", desc = "Work starts on Opus 5.5 · Deep, Personal on GPT-6 Low.", child = toggle("perSpace", "Remember loadout per space") },
			K.Row { label = "Show which loadout answered", desc = "Adds the loadout name and response time under each reply.", child = toggle("answered", "Show which loadout answered") },
			K.Section("TASK MODELS"),
			ui.text { "Separate from loadouts. When an agent hands off part of a task, it uses the model set for that kind of work. Auto picks from your connected providers and skips any that are close to their 5-hour limit.",
				font = K.SANS, text_size = 12, line_height = 17.4, color = C.tertiary, wrap = true, align = "left", mx = 2, mb = 6 },
			K.Row { label = "Delegate sub-tasks automatically", desc = "Agents send side work to the models below. A model you name in your message, or the one your loadout uses, always comes first.", child = toggle("delegate", "Delegate sub-tasks automatically") },
			ui.column {
				id = "task_models", w_full = true,
				opacity = K.tween(function()
					return ms:get().delegate and 1 or 0.4
				end, K.BASE),
				disabled = function()
					return not ms:get().delegate
				end,
				tasks,
			},
		}
	end)

	return {
		Models = function()
			return { Page {} }
		end,
	}
end
