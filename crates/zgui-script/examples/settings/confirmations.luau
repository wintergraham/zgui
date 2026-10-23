-- Assistant › Confirmations (SettingsConfirm.tsx): which actions pause for
-- approval. The stage plays the agent doing that kind of action: the cursor
-- glides to the target, the dither focus lands, and either the approval
-- sheet rises (and is allowed) or the action simply goes through. It stays
-- pinned while the rows below scroll, so the hovered permission plays in view.
return function(K)
	local model = K.model
	local C = K.C
	local CATS = model.CATS

	local function cat_of(id)
		for _, k in CATS do
			if k.id == id then
				return k
			end
		end
		return CATS[1]
	end

	---------------------------------------------------------------------------
	-- Scenes: the page the agent is on, with one [data-target] element.
	---------------------------------------------------------------------------
	local function scene(id, manager, target)
		local function field(label, value, is_target)
			return ui.column(K.merge({ w_full = true, gap = 2, px = 9, py = 6, rounded = 6, bg = C.inset, border = 1, border_color = C.ctl_line,
				K.text(label, 9, C.muted), K.text(value, 11, C.text),
			}, is_target and target or nil))
		end
		local function h(text)
			return K.text(text, 14, C.text, { weight = 500, letter_spacing = -0.2, mb = 2 })
		end
		local function line(a, b, total)
			return ui.row { w_full = true, py = 2, pt = total and 7 or 2,
				K.text(a, 11, total and C.text or C.secondary, { weight = total and 500 or nil }), K.spacer(),
				K.text(b, 11, total and C.text or C.secondary, { weight = total and 500 or nil }) }
		end
		local function btn(label, danger, is_target)
			return ui.row(K.merge({ self = "start", mt = 4, px = 12, py = 6, rounded = 6, bg = danger and C.error_bg or C.ink,
				border = danger and 1 or 0, border_color = C.error_border,
				K.text(label, 11, danger and C.error or C.on_ink, { weight = 500 }) }, is_target and target or nil))
		end
		local function para(text)
			return ui.text { text, font = K.SANS, text_size = 11, line_height = 16, color = C.secondary, wrap = true, align = "left" }
		end
		if id == "payments" then
			return { site = "opentable.com", icon = "opentable", ask = "Pay £48.00 to OpenTable?", detail = "Deposit for 4 · Fri 19:45 · Visa •••• 4242", allow = "Pay", done = "Paid · receipt saved",
				body = { h("Confirm your deposit"), line("Table for 4", "£40.00"), line("Service", "£8.00"), line("Total", "£48.00", true), btn("Pay £48.00", false, true) } }
		elseif id == "messages" then
			return { site = "mail.google.com", icon = "gmail", ask = "Send this email to Sam Lee?", detail = "“Booked The River Café for Friday at 7:45…”", allow = "Send", done = "Sent to sam@lee.studio",
				body = { field("To", "Sam Lee"), field("Subject", "Friday dinner"), para("Booked The River Café for Friday at 7:45. Table for four, deposit paid."), btn("Send", false, true) } }
		elseif id == "forms" then
			return { site = "opentable.com", icon = "opentable", ask = "Submit this booking form?", detail = "Name, phone and party size for The River Café", allow = "Submit", done = "Booking request sent",
				body = { h("Your details"), field("Name", "Robin Hale"), field("Phone", "07700 900123"), field("Party", "4 people"), btn("Request booking", false, true) } }
		elseif id == "signingIn" then
			return { site = "github.com", icon = "github-mark", ask = "Sign in to github.com?", detail = "Using robin@hale.studio from " .. manager[2], allow = "Sign in", done = "Signed in with " .. manager[2],
				body = { h("Sign in to GitHub"), field("Username", "robin@hale.studio"),
					ui.overlay(K.merge({ w_full = true, field("Password", "••••••••••"),
						ui.row { absolute = true, right = 8, top = 0, bottom = 0, items = "center", K.ManagerIcon(manager[1], 14) } }, target)),
					ui.row { self = "start", mt = 4, px = 12, py = 6, rounded = 6, bg = C.ink, K.text("Sign in", 11, C.on_ink, { weight = 500 }) } } }
		elseif id == "sharing" then
			return { site = "notion.so", icon = "notion", ask = "Publish this page to the web?", detail = "Anyone with the link can view “Trip notes”", allow = "Publish", done = "Published · link copied",
				body = { h("Trip notes"), para("Day 1: Hammersmith, lunch by the river. Day 2: Kew Gardens."),
					ui.row { w_full = true, mt = 6, px = 10, py = 8, rounded = 6, border = 1, border_color = C.ctl_line, items = "center",
						K.text("Share to web", 11), K.spacer(), ui.div(K.merge({ w = 24, h = 14, rounded = 7, bg = C.active, id = "cf_switch" }, target)) } } }
		elseif id == "destructive" then
			local rows = { h("Stale branches") }
			for _, b in { "nav-motion", "old-tabs", "spike/dither" } do
				table.insert(rows, line(b, "3 months"))
			end
			table.insert(rows, btn("Delete 3 branches", true, true))
			return { site = "github.com", icon = "github-mark", ask = "Delete 3 branches?", detail = "nav-motion, old-tabs and spike/dither can't be restored", allow = "Delete", done = "3 branches deleted", danger = true, body = rows }
		elseif id == "account" then
			return { site = "accounts.google.com", icon = "google", ask = "Change your recovery email?", detail = "From r***@hale.studio to robin.hale@proton.me", allow = "Change", done = "Recovery email updated",
				body = { h("Recovery email"), field("Current", "r***@hale.studio"), field("New", "robin.hale@proton.me"), btn("Save", false, true) } }
		end
		local icons = {}
		for i = 1, 3 do
			icons[i] = ui.div(K.merge({ w = 30, h = 30, rounded = 8, bg = C.raised, border = 1, border_color = C.ctl_line }, i == 2 and target or nil))
		end
		return { site = "rtings.com", icon = "rtings", ask = "Click a button it can't identify?", detail = "The page changed while it was reading; the button has no label", allow = "Click", done = "Clicked",
			body = { h("Compare headphones"), para("Sony WH-1000XM6 vs Sennheiser Momentum 5"), ui.row { gap = 8, mt = 6, icons } } }
	end

	---------------------------------------------------------------------------
	-- Stage: one pass of a category, rebuilt for each pass.
	---------------------------------------------------------------------------
	local H = 264
	local Stage = ui.component(function(props)
		local cat, rule, ai = props.cat, props.rule, props.ai
		local asks = rule ~= "never"
		local manager = K.MANAGERS[1]
		for _, m in K.MANAGERS do
			if m[1] == model.manager:peek() then
				manager = m
			end
		end
		local reduced = K.reduced()
		local phase = ui.use_state(reduced and (asks and "ask" or "done") or "start")
		local target = ui.use_state({ x = 0, y = 0, w = 0, h = 0 })
		local width = ui.use_state(600)
		-- Offsets, not live bounds: measured once the pass's page has laid out
		-- (and again when the stage resizes), never from inside layout.
		local scene_el, target_el = nil, nil
		local target_props = {
			on_mount = function(el)
				target_el = el
			end,
		}
		local function measure()
			if scene_el and target_el and target_el.mounted then
				target:set {
					x = (target_el.x or 0) - (scene_el.x or 0), y = (target_el.y or 0) - (scene_el.y or 0),
					w = target_el.width or 0, h = target_el.height or 0,
				}
			end
		end
		local s = scene(cat, manager, target_props)
		local function at(...)
			local p = phase:get()
			for _, v in { ... } do
				if p == v then
					return true
				end
			end
			return false
		end
		local function asking()
			return at("ask", "allow")
		end
		-- The pass: glide (700 ms), focus lands, then ask → allowed → done, or
		-- straight to done; hold, then the next category.
		local steps = asks and { { "move", 250 }, { "focus", 800 }, { "ask", 650 }, { "allow", 1700 }, { "done", 550 }, { "start", 2000 } }
			or { { "move", 250 }, { "focus", 800 }, { "done", 750 }, { "start", 2000 } }
		local marks, total = {}, 0
		for i, st in steps do
			total += st[2]
			marks[i] = { total, st[1] }
		end
		local seed = ui.use_state(0)
		local function lift()
			if not asking() then
				return 0
			end
			local t = target:get()
			return math.max(0, t.y + t.h + 22 - (H - 118))
		end
		local function cursor()
			if at("start") then
				return width:get() - 60, 250
			end
			local t = target:get()
			return t.x + t.w * 0.62, t.y + t.h * 0.6
		end
		local danger = s.danger
		local function focus_opacity()
			return K.tween(function()
				return at("focus", "ask", "allow") and 1 or 0
			end, K.MEDIUM)
		end
		local actions = { ui.row { px = 10, py = 5, rounded = 6, border = 1, border_color = C.ctl_line, K.text("Deny", 11, C.secondary) } }
		if rule == "site" then
			table.insert(actions, ui.row { px = 10, py = 5, rounded = 6, border = 1, border_color = C.ctl_line, K.text("Always on " .. s.site:gsub("^mail%.", ""):gsub("^accounts%.", ""), 11, C.secondary) })
		end
		table.insert(actions, ui.row {
			px = 10, py = 5, rounded = 6, bg = danger and C.error or C.ink,
			scale = K.tween(function()
				return at("allow") and 0.94 or 1
			end, K.FAST, K.EASE_OUT, true),
			shadow = function()
				return at("allow") and { x = 0, y = 0, blur = 0, spread = 3, color = K.P("EDEBE5", 0.18) } or false
			end,
			K.text(s.allow, 11, danger and "#1C1513" or C.on_ink, { weight = 500 }),
		})
		local cx, cy = cursor()
		return ui.overlay {
			id = "confirm_stage", w_full = true, h = H, overflow = "hidden", bg = C.window,
			on_resize = function(size)
				width:set(size.w)
				K.after(16, measure)
			end,
			-- The pass's clock: an invisible node animated across the pass.
			ui.div {
				absolute = true, w = 1, h = 1, opacity = 0,
				on_mount = function(el)
					if reduced then
						return
					end
					anime.animate(el, { x = { 0, 1 }, duration = total, ease = "linear", frameRate = 30,
						onUpdate = function(a)
							local t = a.currentTime
							local now = "start"
							for _, m in marks do
								if t >= m[1] then
									now = m[2]
								end
							end
							if now == "move" and phase:peek() ~= "move" then
								measure()
							end
							phase:set(now)
							seed:set(math.floor(t / (1000 / 12)))
						end,
						onComplete = function()
							props.done()
						end,
					})
				end,
			},
			ui.overlay {
				absolute = true, top = 0, left = 0, right = 0, bottom = 0,
				y = K.tween(function()
					return -lift()
				end, K.SPRING_MS, K.SPRING, true),
				on_mount = function(el)
					scene_el = el
				end,
				ui.row { absolute = true, top = 18, bottom = -140, left = 0, right = 0, justify = "center",
					ui.column {
						w = "74%", max_w = 380, h_full = true, rounded = 10, overflow = "hidden", bg = C.page, border = 1, border_color = C.subtle,
						brightness = K.tween(function()
							return asking() and 0.7 or 1
						end, K.MEDIUM),
						on_mount = function(el)
							if not reduced then
								anime.animate(el, { opacity = { 0, 1 }, y = { 6, 0 }, duration = K.MEDIUM, ease = K.EASE_ENTER })
							end
						end,
						ui.row { w_full = true, h = 28, px = 10, gap = 6, items = "center",
							K.favicon(s.icon, 12, { rounded = 3 }), K.text(s.site, 10.5, C.tertiary), K.spacer(), K.icon("lock", 10, C.muted) },
						ui.column { w_full = true, gap = 7, px = 16, py = 14, s.body },
					},
				},
				ai and ui.overlay {
					absolute = true, left = 0, top = 0, rounded = 14, overflow = "hidden", pointer_events = "none",
					x = function()
						return target:get().x - 18
					end,
					y = function()
						return target:get().y - 14
					end,
					w = function()
						return target:get().w + 36
					end,
					h = function()
						return target:get().h + 28
					end,
					opacity = focus_opacity(),
					K.dither(function()
						local u = table.clone(K.U.focus)
						u.seed = (model.dither:get().animate and not K.reduced()) and seed:get() * 0.53 or 0
						return K.scaled(u, K.amount("highlight"))
					end, { hidden = function()
						return K.amount("highlight") == 0
					end }),
					ui.div { absolute = true, top = 12, left = 16, right = 16, bottom = 12, rounded = 9, border = 1.5, border_color = danger and C.error or C.text, hidden = function()
						return K.amount("highlight") > 0
					end },
				} or nil,
				ai and ui.div {
					absolute = true, left = 0, top = 0,
					x = K.tween(function()
						local x = cursor()
						return x
					end, 700, K.EASE_MOVE, true),
					y = K.tween(function()
						local _, y = cursor()
						return y
					end, 700, K.EASE_MOVE, true),
					opacity = K.tween(function()
						return at("start", "done") and 0 or 1
					end, K.MEDIUM),
					K.icon("mouse-pointer-2", 16, "#EDEBE5"),
				} or nil,
			},
			ui.row { absolute = true, bottom = 14, left = 0, right = 0, justify = "center", pointer_events = "none",
				ui.column {
					id = "confirm_sheet", w = "70%", max_w = 340, gap = 12, p = 14, rounded = 12, bg = C.elevated, border = 1, border_color = C.strong,
					shadow = { x = 0, y = 18, blur = 40, color = "#00000080" },
					opacity = K.tween(function()
						return asking() and 1 or 0
					end, 200, K.EASE_ENTER),
					y = K.tween(function()
						return asking() and 0 or 24
					end, K.SPRING_MS, K.SPRING, true),
					scale = K.tween(function()
						return asking() and 1 or 0.97
					end, K.SPRING_MS, K.SPRING, true),
					ui.row { w_full = true, gap = 10, items = "start",
						K.Orb { size = 22, lit = ai },
						ui.column { grow = 1, shrink = 1, min_w = 0, gap = 3,
							K.text(s.ask, 12.5, C.text, { weight = 500 }),
							ui.text { s.detail, font = K.SANS, text_size = 11, line_height = 15.4, color = C.tertiary, wrap = true, align = "left" },
						},
					},
					ui.row { w_full = true, justify = "end", gap = 6, actions },
				},
			},
			ui.row { absolute = true, bottom = 16, left = 0, right = 0, justify = "center", pointer_events = "none",
				ui.row {
					id = "confirm_toast", h = 30, px = 12, gap = 7, items = "center", rounded = 15, bg = C.raised, border = 1, border_color = C.strong,
					shadow = { x = 0, y = 10, blur = 24, color = "#00000066" },
					opacity = K.tween(function()
						return at("done") and 1 or 0
					end, 200, K.EASE_ENTER),
					y = K.tween(function()
						return at("done") and 0 or 10
					end, K.SPRING_MS, K.SPRING, true),
					K.icon("check", 13, "#C3CEB2"), K.text(s.done, 11, C.text),
					not asks and ui.row { pl = 7, border = 1, border_sides = { 0, 0, 0, 1 }, border_color = C.strong, K.text("No confirmation", 10, C.muted) } or nil,
				},
			},
		}
	end)

	---------------------------------------------------------------------------
	-- Page
	---------------------------------------------------------------------------
	local Page = ui.component(function()
		local c = model.confirm
		local hover = ui.use_state(nil)
		local auto = ui.use_state(0)
		local function p()
			return model.confirm_preset(c:get())
		end
		local function current()
			local h = hover:get()
			if h then
				return h
			end
			local order = model.confirm_asks(c:get())
			if #order == 0 then
				order = CATS
			end
			return order[auto:get() % #order + 1].id
		end
		local function rule_of(id)
			local cc = c:get()
			return cc.mode == "every" and "always" or cc.rules[id] or "always"
		end
		local function pick(v)
			if v == "read" then
				model.patch(c, { mode = "read" })
			elseif v == "every" then
				local rules = {}
				for _, k in CATS do
					rules[k.id] = "always"
				end
				c:set { mode = "every", rules = rules }
			elseif v == "balanced" then
				c:set { mode = "act", rules = table.clone(model.BALANCED) }
			end
		end
		local function read()
			return c:get().mode == "read"
		end

		-- Pinned under the window's top while the rows scroll (position: sticky).
		local slot = ui.use_state({ rel = 0, top = 0, h = 330 })
		local stage = ui.presence(function()
			return current() .. "/" .. rule_of(current()) .. "/" .. tostring(model.ai:get())
		end, function(key)
			local cat, rule = key:match("^(.-)/(.-)/")
			return Stage { cat = cat, rule = rule, ai = model.ai:peek(), done = function()
				if not hover:peek() then
					auto:set(auto:peek() + 1)
				end
			end }
		end)
		local card = ui.column {
			id = "confirm_card", w_full = true, rounded = 12, overflow = "hidden", bg = C.inset, border = 1, border_color = C.line,
			shadow = { x = 0, y = 14, blur = 28, spread = -10, color = "#00000099" },
			-- A fixed slot: while one pass hands over to the next both stages are
			-- mounted, and the rows under the pointer must not move.
			ui.overlay { w_full = true, h = H, overflow = "hidden", stage },
			ui.row { w_full = true, h = 36, px = 14, gap = 12, items = "center",
				ui.presence(current, function(id)
					local k = cat_of(id)
					return ui.row { gap = 6, items = "center", K.icon(k.icon, 12, C.muted), K.text(k.label, 11.5, C.secondary, { id = "confirm_now" }) }
				end),
			},
		}
		local rows = {}
		for i, k in CATS do
			rows[i] = ui.div {
				w_full = true,
				on_hover = function(inside)
					if inside then
						hover:set(k.id)
					elseif hover:peek() == k.id then
						hover:set(nil)
					end
				end,
				K.Row { label = k.label, desc = k.desc, child = K.Segmented {
					id = "rule_" .. k.id, small = true, danger = k.risky and "never" or nil,
					options = { { "always", "Always" }, { "site", "Per site" }, { "never", "Don't ask" } },
					value = function()
						local cc = c:get()
						return cc.mode == "every" and "always" or cc.rules[k.id]
					end,
					pick = function(v)
						local rules = table.clone(c:peek().rules)
						rules[k.id] = v
						c:set { mode = "act", rules = rules }
					end,
				} },
			}
		end
		return ui.column {
			w_full = true,
			K.Header("Confirmations", "Which actions pause for your approval before the agent goes ahead."),
			K.Row {
				label = "Approach",
				desc = function()
					local v = p()
					return v == "read" and "The agent reads pages but never clicks or types"
						or v == "every" and "Every click, keystroke and submit waits for you"
						or v == "custom" and "Your own mix, set per action below"
						or "Acts on its own, then pauses before anything below"
				end,
				child = K.Segmented {
					id = "approach",
					options = { { "read", "Read only" }, { "balanced", function()
						return p() == "custom" and "Custom" or "Risky actions"
					end }, { "every", "Every step" } },
					value = function()
						local v = p()
						return v == "custom" and "balanced" or v
					end,
					pick = pick,
				},
			},
			-- The card's place in the flow; the card itself draws last, over the rows.
			ui.div {
				w_full = true, mt = 4,
				h = function()
					return slot:get().h
				end,
				on_resize = function(_, el)
					local content = K.content_el
					if content then
						local rel = el.y - content.y
						local cur = slot:peek()
						slot:set { rel = rel, top = content.y + K.scroll:peek(), h = cur.h }
					end
				end,
			},
			K.Section("ASK BEFORE"),
			ui.column { id = "confirm_list", w_full = true,
				opacity = K.tween(function()
					return read() and 0.4 or 1
				end, K.BASE),
				disabled = read,
				rows,
			},
			K.Section("WHILE IT'S ACTING"),
			K.flag("Take over with Esc", "Pressing Esc on a page the assistant is driving hands control back to you immediately."),
			K.flag("Show agent cursor and dither highlight", "Draws the assistant's cursor and a dithered focus on the element it's about to use."),
			K.Row { label = "Site permissions", desc = "opentable.com allowed · amazon.co.uk read only · 2 blocked", child = K.btn { label = "Manage", id = "site_permissions" } },
			ui.column {
				id = "confirm_sticky", absolute = true, left = 0, right = 0, pt = 12, pb = 10,
				top = function()
					local sl = slot:get()
					return sl.rel + math.max(0, K.scroll:get() - (sl.top + sl.rel))
				end,
				on_resize = function(size)
					local cur = slot:peek()
					if math.abs(cur.h - size.h) > 0.5 then
						slot:set { rel = cur.rel, top = cur.top, h = size.h }
					end
				end,
				gradient = { angle = 180, stops = { { 0, C.window }, { 0.88, C.window }, { 1, K.P("0A0A09", 0) } } },
				card,
			},
		}
	end)

	return {
		Confirmations = function()
			return { Page {} }
		end,
	}
end
