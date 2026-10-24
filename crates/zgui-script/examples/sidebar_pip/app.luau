-- Ask's sidebar and agent PiP, from ask.pen's "Sidebar · Navigation & PiP"
-- board, as one working browser window. The board's frames are states of
-- it: the expanded master (search, pinned trays, chats, tabs and groups,
-- the space switcher), the pinned trays' hover, active and focused states,
-- the switcher's hover tooltip, the favicon resolving while a tab loads,
-- and the PiP's master, stacked and stack-expanded states. Everything is
-- live: agents work through their steps, and every control does something.
local K = require("./kit")
local D = require("./data")
local shaders = require("./dither")
local C, SANS, MONO = K.C, K.SANS, K.MONO
local icon, text, mono, spacer, button, after = K.icon, K.text, K.mono, K.spacer, K.button, K.after

----------------------------------------------------------------------------
-- State
----------------------------------------------------------------------------

local space = ui.state("space", 1)
local collapsed = ui.state("collapsed", false)
local layout = ui.state("layout", {}) -- space id: { top, groups = { { id, name, open, tabs } }, bottom }
local pages = ui.state("pages", {}) -- pages opened while running: id -> { title, favicon, url }
local flags = ui.state("flags", {}) -- page id -> { asleep, unread, audio, muted, reading, loading = start ms }
local selected = ui.state("selected", {}) -- space id -> page id
local open_pins = ui.state("open_pins", {}) -- page id -> true: the pin's page is open (the active state)
local chats = ui.state("chats", {}) -- space id -> { { id, title, time } }
local chat_sel = ui.state("chat_sel", {}) -- space id -> chat id
local showing = ui.state("showing", {}) -- space id -> "page" | "chat"
local hovered = ui.state("hovered", "") -- "tab:id", "pin:id", "chat:id"
local kb_focus = ui.state("kb_focus", "") -- the pin keyboard focus is on (not by pointer)
local space_hover = ui.state("space_hover", 0)
local space_menu = ui.state("space_menu", false)
local query = ui.state("query", "")
local search_focused = ui.state("search_focused", false)
local agents = ui.state("agents", {}) -- front first: { id, state, step }
local expanded = ui.state("expanded", false)
local serial = ui.state("serial", 0)

local LOAD_MS = 3600

local function sp()
	return D.spaces[space:get()]
end
local function spp()
	return D.spaces[space:peek()]
end
local function next_id()
	serial:set(serial:peek() + 1)
	return serial:peek()
end
local function page(id)
	return D.pages[id] or pages:peek()[id] or { "New tab", nil, "" }
end
local function agent_info(id)
	for _, a in D.agents do
		if a.id == id then
			return a
		end
	end
	return D.agents[1]
end
-- State values are read-only snapshots: `update` hands `f` a mutable copy.
local function edit(s, f)
	s:update(f)
end
local function set_flag(id, key, value)
	edit(flags, function(all)
		all[id] = all[id] or {}
		all[id][key] = value
		return all
	end)
end

-- Loading: the favicon resolves over LOAD_MS. Views seek to the shared
-- start, so a row, the rail and the page's large favicon move together.
local function load(id)
	local start = anime.engine.currentTime
	ui.batch(function()
		set_flag(id, "loading", start)
		set_flag(id, "asleep", nil)
	end)
	after(LOAD_MS, function()
		local f = flags:peek()[id]
		if f and f.loading == start then
			set_flag(id, "loading", nil)
		end
	end)
end

local function reset()
	ui.batch(function()
		local l, sel, ch, cs, fl, show = {}, {}, {}, {}, {}, {}
		for _, s in D.spaces do
			local groups = {}
			for _, g in s.groups do
				table.insert(groups, { id = g.id, name = g.name, open = g.open, tabs = table.clone(g.tabs) })
			end
			l[s.id] = { top = table.clone(s.top), groups = groups, bottom = table.clone(s.bottom) }
			sel[s.id] = s.selected
			cs[s.id] = s.chat
			show[s.id] = "page"
			ch[s.id] = {}
			for _, c in s.chats do
				table.insert(ch[s.id], { id = c[1], title = c[2], time = c[3] })
			end
			for id, f in s.flags do
				fl[id] = table.clone(f)
				fl[id].loading = nil
			end
		end
		layout:set(l)
		selected:set(sel)
		chats:set(ch)
		chat_sel:set(cs)
		showing:set(show)
		flags:set(fl)
		pages:set({})
		open_pins:set({ gmail = true, p_youtube = true, r_wiki = true })
		local list = {}
		for _, a in D.agents do
			table.insert(list, { id = a.id, state = a.state, step = a.step })
		end
		agents:set(list)
		expanded:set(false)
		query:set("")
	end)
	for _, s in D.spaces do
		for id, f in s.flags do
			if f.loading then
				load(id)
			end
		end
	end
end

local booted = ui.state("booted", false)
if not booted:peek() then
	booted:set(true)
	reset()
end

----------------------------------------------------------------------------
-- Selection and editing
----------------------------------------------------------------------------

local function all_tabs(sid)
	local l = layout:peek()[sid]
	local out = {}
	if not l then
		return out
	end
	for _, id in l.top do
		table.insert(out, id)
	end
	for _, g in l.groups do
		for _, id in g.tabs do
			table.insert(out, id)
		end
	end
	for _, id in l.bottom do
		table.insert(out, id)
	end
	return out
end
local function is_pin(sid, id)
	for _, s in D.spaces do
		if s.id == sid then
			for _, tray in s.pins do
				if table.find(tray, id) then
					return true
				end
			end
		end
	end
	return false
end
local function exists(sid, id)
	return table.find(all_tabs(sid), id) ~= nil or is_pin(sid, id)
end

local function select_page(id)
	local sid = spp().id
	if not exists(sid, id) then
		return
	end
	local f = flags:peek()[id] or {}
	ui.batch(function()
		edit(selected, function(s)
			s[sid] = id
			return s
		end)
		edit(showing, function(s)
			s[sid] = "page"
			return s
		end)
		if f.unread then
			set_flag(id, "unread", nil)
		end
		if is_pin(sid, id) and not open_pins:peek()[id] then
			edit(open_pins, function(p)
				p[id] = true
				return p
			end)
		end
	end)
	if f.asleep or (is_pin(sid, id) and not f.loaded) then
		set_flag(id, "loaded", true)
		load(id)
	end
end

local function select_chat(id)
	local sid = spp().id
	ui.batch(function()
		edit(chat_sel, function(s)
			s[sid] = id
			return s
		end)
		edit(showing, function(s)
			s[sid] = "chat"
			return s
		end)
	end)
end

local function new_chat()
	local id = "chat" .. next_id()
	local sid = spp().id
	edit(chats, function(c)
		table.insert(c[sid], 1, { id = id, title = "New chat", time = "now" })
		return c
	end)
	select_chat(id)
end

local function close_tab(id)
	local sid = spp().id
	local order = all_tabs(sid)
	local at = table.find(order, id)
	ui.batch(function()
		edit(layout, function(l)
			local s = l[sid]
			local i = table.find(s.top, id)
			if i then
				table.remove(s.top, i)
			end
			i = table.find(s.bottom, id)
			if i then
				table.remove(s.bottom, i)
			end
			for _, g in s.groups do
				i = table.find(g.tabs, id)
				if i then
					table.remove(g.tabs, i)
				end
			end
			return l
		end)
		if selected:peek()[sid] == id then
			local nxt = order[(at or 0) + 1] or order[(at or 2) - 1]
			edit(selected, function(s)
				s[sid] = nxt
				return s
			end)
		end
		if hovered:peek() == "tab:" .. id then
			hovered:set("")
		end
	end)
end

local function close_group(gid)
	local sid = spp().id
	local l = layout:peek()[sid]
	for _, g in l.groups do
		if g.id == gid then
			for _, id in table.clone(g.tabs) do
				close_tab(id)
			end
		end
	end
	edit(layout, function(all)
		local s = all[sid]
		for i, g in s.groups do
			if g.id == gid then
				table.remove(s.groups, i)
				break
			end
		end
		return all
	end)
end

local function toggle_group(gid)
	local sid = spp().id
	edit(layout, function(all)
		for _, g in all[sid].groups do
			if g.id == gid then
				g.open = not g.open
			end
		end
		return all
	end)
end

local function close_pin(id)
	ui.batch(function()
		edit(open_pins, function(p)
			p[id] = nil
			return p
		end)
		set_flag(id, "loaded", nil)
		local sid = spp().id
		if selected:peek()[sid] == id then
			edit(selected, function(s)
				s[sid] = all_tabs(sid)[1]
				return s
			end)
		end
	end)
end

-- A page opened from the search field (an address or a Google search), or
-- by an agent: added to the top of the space's tabs, selected and loaded.
local function open_page(id, info)
	local sid = spp().id
	ui.batch(function()
		edit(pages, function(p)
			p[id] = info
			return p
		end)
		edit(layout, function(l)
			if not table.find(l[sid].top, id) then
				table.insert(l[sid].top, id)
			end
			return l
		end)
	end)
	select_page(id)
	load(id)
end

local function open_address(q)
	q = q:match("^%s*(.-)%s*$")
	if q == "" then
		return
	end
	local host = q:gsub("^https?://", "")
	local info
	if host:find("^[%w%-]+%.[%w%.%-]+") and not host:find("%s") then
		local domain = host:match("^([^/]+)"):gsub("^www%.", "")
		local fav
		for key, name in D.known do
			if domain:find(key, 1, true) then
				fav = name
			end
		end
		info = { domain, fav or false, host }
	else
		info = { q .. " – Google Search", "google", "google.com/search?q=" .. q:gsub("%s+", "+") }
	end
	open_page("open" .. next_id(), info)
end

local function switch_space(i)
	ui.batch(function()
		space:set(i)
		space_menu:set(false)
		hovered:set("")
	end)
end

----------------------------------------------------------------------------
-- Agents
----------------------------------------------------------------------------

local function agent_state(id)
	for _, a in agents:get() do
		if a.id == id then
			return a
		end
	end
	return { id = id, state = "done", step = 1 }
end

local function set_agent(id, f)
	edit(agents, function(list)
		for _, a in list do
			if a.id == id then
				f(a)
			end
		end
		return list
	end)
end

local function remove_agent(id)
	ui.batch(function()
		edit(agents, function(list)
			for i, a in list do
				if a.id == id then
					table.remove(list, i)
					break
				end
			end
			return list
		end)
		if #agents:peek() < 2 then
			expanded:set(false)
		end
		-- Its pages stop showing it at work.
		for pid, f in flags:peek() do
			if f.reading == id then
				set_flag(pid, "reading", nil)
			end
		end
	end)
end

local function to_front(id)
	edit(agents, function(list)
		for i, a in list do
			if a.id == id then
				table.remove(list, i)
				table.insert(list, 1, a)
				break
			end
		end
		return list
	end)
end

-- Every few seconds each working agent takes its next step.
anime.createTimer {
	duration = 4600, loop = true,
	onLoop = function()
		edit(agents, function(list)
			for _, a in list do
				if a.state == "working" or a.state == "reading" then
					local steps = #agent_info(a.id).steps
					if a.step >= steps then
						a.state = "done"
					else
						a.step += 1
					end
				end
			end
			return list
		end)
	end,
}

local function open_agent_tab(id)
	local a = agent_info(id)
	local pid = "agent_" .. id
	local first = a.tabs[1]
	open_page(pid, { first[1], first[2] or false, a.page })
	set_flag(pid, "reading", id)
end

----------------------------------------------------------------------------
-- Favicons: still, or resolving from the loading blob
----------------------------------------------------------------------------

local function still_favicon(id, size)
	local p = page(id)
	local function dim()
		local f = flags:get()[id]
		return f and f.asleep and 0.5 or 1
	end
	if p[2] then
		return K.favicon(p[2], size, { opacity = dim })
	end
	return icon("globe", size, C.tertiary, { opacity = dim })
end

-- favicon-resolve: 0–35% the dithered blob wobbles, 35–85% the favicon
-- focuses through it (blur to sharp, small to full, ink to colour).
local function resolving_favicon(id, size, start)
	local p = page(id)
	local blob, img = ui.ref(), ui.ref()
	local ink = K.rgba("#D3C3A0", 1)
	local T = LOAD_MS
	return ui.div {
		w = size, h = size, relative = true, shrink = 0,
		on_mount = function()
			local tl = anime.createTimeline { autoplay = false }
			tl:add(blob, { shaderTime = { 0, T / 1000 }, shaderParam4 = { 0, 1 }, duration = T, ease = "linear" }, 0)
			tl:add(img, { opacity = { 0, 1 }, duration = 0.2 * T, ease = "linear" }, 0.35 * T)
			tl:add(img, { blur = { size / 5, 0 }, scale = { 0.74, 1 }, duration = 0.5 * T, ease = "inOutSine" }, 0.35 * T)
			tl:add(img, { saturate = { 0, 1 }, duration = 0.25 * T, ease = "linear" }, 0.575 * T)
			tl:seek(math.clamp(anime.engine.currentTime - start, 0, T))
			tl:play()
		end,
		ui.div {
			ref = blob, absolute = true, left = 0, top = 0, w = size, h = size,
			bg_shader = shaders.favicon_blob, shader_params = { ink, { 0, (start % 97) / 7, 0, 0 } },
		},
		p[2] and K.favicon(p[2], size, { ref = img, absolute = true, left = 0, top = 0, opacity = 0, saturate = 0 })
			or icon("globe", size, C.tertiary, { ref = img, absolute = true, left = 0, top = 0, opacity = 0 }),
	}
end

local function favicon(id, size)
	return ui.presence(function()
		local f = flags:get()[id]
		return f and f.loading or 0
	end, function(start)
		if start ~= 0 then
			return resolving_favicon(id, size, start)
		end
		return still_favicon(id, size)
	end, { w = size, h = size, grow = 0, shrink = 0 })
end

-- The row's own dither while its page loads (tab-loading-bg).
local function loading_wash(id, rounded)
	return ui.presence(function()
		local f = flags:get()[id]
		return f and f.loading or nil
	end, function(start)
		return ui.div {
			w_full = true, h_full = true, rounded = rounded,
			bg_shader = shaders.tab_loading,
			shader_params = { K.rgba("#2C2B27", 1), { 0, 1.5, 0.75, 4 } },
			on_mount = function(el)
				local a = anime.animate(el, {
					shaderTime = { 0, LOAD_MS / 1000 }, shaderParam4 = { 0, 1 }, duration = LOAD_MS, ease = "linear",
				})
				a:seek(math.clamp(anime.engine.currentTime - start, 0, LOAD_MS))
			end,
		}
	end, {
		absolute = true, left = 0, top = 0, right = 0, bottom = 0, pointer_events = "none",
		exit = { opacity = 0, duration = 200 },
	})
end

----------------------------------------------------------------------------
-- Rows
----------------------------------------------------------------------------

local function hover_handler(key)
	return function(inside)
		if inside then
			hovered:set(key)
		elseif hovered:peek() == key then
			hovered:set("")
		end
	end
end

-- A small trailing control inside a row. A press here is the control's,
-- not the row's: `swallow` tells the row's click to stand down.
local swallow = false
local function row_action(id, name, size, color, f, props)
	local node = {
		id = id, w = size + 6, h = size + 6, rounded = 4, justify = "center",
		hover = { bg = "#FFFFFF14" },
		on_press = function() swallow = true end,
		on_click = function()
			swallow = true
			f()
		end,
		icon(name, size, color),
	}
	for k, v in props or {} do
		node[k] = v
	end
	return button(node, 0.88)
end
local function row_click(f)
	return function()
		if swallow then
			swallow = false
			return
		end
		f()
	end
end

local function tab_row(id)
	local p = page(id)
	local key = "tab:" .. id
	local function f()
		return flags:get()[id] or {}
	end
	local function on()
		local sid = sp().id
		return selected:get()[sid] == id
	end
	local function hot()
		return hovered:get() == key
	end
	return button({
		id = "tab_" .. id, w_full = true, h = 28, px = 7, gap = 8, relative = true,
		rounded = function() return on() and 8 or 6 end,
		bg = function()
			if on() then
				return C.tab_on
			end
			return hot() and "#FFFFFF0D" or C.clear
		end,
		border = 1,
		border_color = function() return on() and C.tab_on_line or C.clear end,
		on_hover = hover_handler(key),
		on_press = function() swallow = false end,
		on_click = row_click(function() select_page(id) end),
		context_menu = function()
			return {
				ui.menu_item { "Reload", on_click = function() load(id) end },
				ui.menu_item { "Close tab", on_click = function() close_tab(id) end },
			}
		end,
		loading_wash(id, 6),
		ui.div { w = 18, h = 18, items = "center", justify = "center", shrink = 0, favicon(id, 14) },
		text(p[1], 12.5, function()
			if on() then
				return C.text
			end
			return f().asleep and C.asleep or C.tab
		end, { weight = function() return on() and 500 or 400 end, truncate = true, grow = 1, shrink = 1, min_w = 0 }),
		-- Trailing state, hidden rather than shown so absent ones take no gap.
		K.orb("reading", 10, { hidden = function() return f().reading == nil end }),
		ui.div { w = 5, h = 5, rounded = 3, bg = C.unread, mx = 3.5, shrink = 0, hidden = function() return not f().unread or hot() end },
		icon("moon", 12, C.asleep, { hidden = function() return not f().asleep or hot() end }),
		row_action("mute_" .. id, "volume-2", 12, C.audio, function() set_flag(id, "muted", true) end, {
			hidden = function() return not f().audio or f().muted == true end,
		}),
		row_action("unmute_" .. id, "volume-x", 12, C.muted, function() set_flag(id, "muted", false) end, {
			hidden = function() return not f().audio or not f().muted end,
		}),
		row_action("close_" .. id, "x", 12, C.muted, function() close_tab(id) end, {
			hidden = function() return not (on() or hot()) end,
		}),
	}, 0.985)
end

local function new_tab_row(focus_search)
	return button({
		id = "new_tab", w_full = true, h = 28, px = 7, gap = 8, rounded = 6,
		hover = { bg = "#FFFFFF0D" },
		on_click = focus_search,
		ui.div { w = 18, h = 18, items = "center", justify = "center", icon("plus", 13, C.tertiary) },
		text("New tab", 12.5, C.tertiary),
	}, 0.985)
end

local function group_view(sid, gid)
	local function g()
		for _, x in layout:get()[sid].groups do
			if x.id == gid then
				return x
			end
		end
		return { name = "", open = false, tabs = {} }
	end
	local ids = ui.computed(function() return g().tabs end)
	local name = g().name
	return ui.column {
		id = "group_" .. gid, w_full = true, p = 4, gap = 2, rounded = 8, bg = C.group,
		layout = "height", layout_clip = true,
		button({
			id = "group_header_" .. gid, w_full = true, h = 24, px = 4, gap = 7, rounded = 5,
			on_press = function() swallow = false end,
			on_click = row_click(function() toggle_group(gid) end),
			icon("folder", 13, C.secondary),
			text(name, 12, C.tab),
			icon("chevron-up", 10, C.secondary, { rotate = function() return g().open and 0 or 180 end }),
			spacer(),
			row_action("close_group_" .. gid, "x", 12, C.secondary, function() close_group(gid) end, {
				hidden = function() return not g().open end,
			}),
			mono(function() return tostring(#g().tabs) end, 10, C.asleep, { mr = 6, hidden = function() return g().open end }),
		}, 0.99),
		ui.column {
			w_full = true, pl = 8, gap = 2, hidden = function() return not g().open end,
			ui.each(ids, tab_row, { key = function(id) return id end }),
		},
	}
end

local function chat_row(c)
	local key = "chat:" .. c.id
	local function on()
		return chat_sel:get()[sp().id] == c.id
	end
	return button({
		id = "chat_" .. c.id, w_full = true, h = 28, px = 7, gap = 8,
		rounded = function() return on() and 8 or 6 end,
		bg = function()
			if on() then
				return C.selected
			end
			return hovered:get() == key and "#FFFFFF0D" or C.clear
		end,
		border = 1,
		border_color = function() return on() and C.line or C.clear end,
		on_hover = hover_handler(key),
		on_click = function() select_chat(c.id) end,
		ui.div {
			w = 18, h = 18, items = "center", justify = "center", shrink = 0,
			ui.show(on, K.orb("chat", 12), icon("message-circle", 12, C.muted)),
		},
		text(c.title, 12.5, function() return on() and C.text or C.secondary end,
			{ weight = function() return on() and 500 or 400 end, truncate = true, grow = 1, shrink = 1, min_w = 0 }),
		mono(c.time, 10, C.tertiary),
	}, 0.985)
end

----------------------------------------------------------------------------
-- Pins: one shared tray per row; hover, active (open) and focused
----------------------------------------------------------------------------

local function seed_of(id)
	local n = 0
	for i = 1, #id do
		n += string.byte(id, i) * i
	end
	return n % 60 + 7
end

local function fog_layer(id, rounded)
	local p = page(id)
	local b = D.brands[p[2]] or D.brands.notion
	return ui.div {
		absolute = true, left = 0, top = 0, right = 0, bottom = 0, rounded = rounded, bg = b.fill,
		pointer_events = "none", hidden = function() return not open_pins:get()[id] end,
		bg_shader = shaders.fog,
		shader_params = { K.rgba(b[1], 0.8), K.rgba(b[2], 0.42), K.rgba(b[3], 0.9), { 135, 2.2, seed_of(id), 0.5 } },
		on_mount = function(el)
			anime.animate(el, { opacity = { 0, 1 }, duration = 180, ease = "outQuad" })
			anime.animate(el, { shaderTime = { 0, 40 }, duration = 40000, loop = true, ease = "linear", frameRate = 12 })
		end,
	}
end

local pressing_pin = ""
local function pin(id, props)
	local p = page(id)
	local b = D.brands[p[2]] or D.brands.notion
	local key = "pin:" .. id
	local function active()
		return open_pins:get()[id] == true
	end
	local node = {
		id = "pin_" .. id, h = 34, rounded = 9, justify = "center", relative = true, focusable = true,
		border_over = true,
		bg = function() return not active() and hovered:get() == key and C.pin_hover or C.clear end,
		border = 1,
		border_color = function()
			if kb_focus:get() == id then
				return C.focus
			end
			return active() and b.line or C.clear
		end,
		on_hover = hover_handler(key),
		on_press = function() pressing_pin = id end,
		on_release = function() pressing_pin = "" end,
		on_focus = function()
			if pressing_pin ~= id then
				kb_focus:set(id)
			end
		end,
		on_blur = function()
			if kb_focus:peek() == id then
				kb_focus:set("")
			end
		end,
		on_key = function(e)
			if e.key == "Enter" or e.key == " " then
				select_page(id)
				return true
			end
			return false
		end,
		on_click = function() select_page(id) end,
		context_menu = function()
			if not open_pins:peek()[id] then
				return nil
			end
			return { ui.menu_item { "Close " .. p[1], on_click = function() close_pin(id) end } }
		end,
		fog_layer(id, 9),
		K.favicon(p[2], 17),
	}
	for k, v in props or {} do
		node[k] = v
	end
	return button(node, 0.94)
end

local function pin_tray(ids)
	local list = {}
	for _, id in ids do
		table.insert(list, pin(id, { grow = 1, basis = 0 }))
	end
	return ui.row { w_full = true, gap = 2, p = 3, bg = C.tray, border = 1, border_color = C.tray_line, rounded = 12, list }
end

----------------------------------------------------------------------------
-- Space switcher: the bottom pill row and the topbar menu
----------------------------------------------------------------------------

local function space_mark(s, size)
	local node = K.bayer({ ink = s.mark.ink, mode = 2, angle = s.mark.angle, cy = s.mark.cy, radius = s.mark.radius, curve = 1 })
	node.w, node.h, node.rounded, node.bg, node.border, node.border_color, node.shrink = size, size, 6, "#141413", 1, C.line, 0
	return ui.div(node)
end

local function switcher_button(i)
	local s = D.spaces[i]
	local function on()
		return space:get() == i
	end
	local function hot()
		return space_hover:get() == i and not on()
	end
	return button({
		id = "space_" .. s.id, h = 26, rounded = 7, gap = 6, justify = "center", relative = true,
		layout = "width", layout_duration = 0.28,
		pl = function() return on() and 8 or 7.5 end,
		pr = function() return on() and 10 or 7.5 end,
		bg = function()
			if on() then
				return C.tab_on
			end
			return hot() and C.raised or C.clear
		end,
		border = 1,
		border_color = function() return on() and C.line or C.clear end,
		on_hover = function(inside)
			if inside then
				space_hover:set(i)
			elseif space_hover:peek() == i then
				space_hover:set(0)
			end
		end,
		on_click = function() switch_space(i) end,
		icon(s.icon, 13, function() return (on() or hot()) and C.text or C.muted end),
		text(s.name, 12, C.text, { weight = 500, hidden = function() return not on() end }),
		-- The hovered icon's tooltip: its name and shortcut.
		ui.presence(function() return hot() and i or nil end, function()
			return ui.row {
				w_full = true, justify = "center",
				ui.row {
					gap = 6, px = 8, py = 5, rounded = 6, bg = C.tooltip, items = "center",
					shadow = { y = 6, blur = 16, color = "#00000066" },
					text(s.name, 11, C.text),
					mono(s.key, 10, C.tertiary),
				},
			}
		end, {
			absolute = true, bottom = 34, left = -60, w = 150, justify = "center", pointer_events = "none",
			enter = { opacity = { 0, 1 }, y = { 4, 0 }, duration = 140, ease = "outQuad" },
			exit = { opacity = 0, duration = 90 },
		}),
	}, 0.94)
end

local function space_switcher()
	local list = {}
	for i = 1, #D.spaces do
		table.insert(list, switcher_button(i))
	end
	return ui.row { gap = 2, p = 3, bg = C.panel, rounded = 10, items = "center", list }
end

local function space_menu_view()
	local rows = {}
	for i, s in D.spaces do
		table.insert(rows, button({
			id = "menu_space_" .. s.id, w_full = true, h = 28, px = 8, gap = 8, rounded = 6,
			hover = { bg = C.selected },
			on_click = function() switch_space(i) end,
			space_mark(s, 16),
			text(s.name, 12.5, C.text),
			spacer(),
			ui.show(function() return space:get() == i end, icon("check", 12, C.secondary)),
			mono(s.key, 10, C.muted),
		}, 0.98))
	end
	local marks = {}
	for i, s in D.spaces do
		marks[i] = space_mark(s, 22)
	end
	return ui.popover {
		open = space_menu, w = 200, p = 5, gap = 1, bg = "#1A1A18", rounded = 10, border = 1, border_color = C.strong,
		shadow = { y = 12, blur = 32, color = "#00000080" },
		anchor = button({
			id = "space_name", h = 30, gap = 8, rounded = 7, pr = 4,
			on_click = function() space_menu:set(not space_menu:peek()) end,
			ui.switch(space, marks),
			ui.text { function() return sp().name end, font = SANS, text_size = 13.5, weight = 600, color = C.text },
			icon("chevron-down", 12, C.secondary),
		}, 0.97),
		rows,
	}
end

----------------------------------------------------------------------------
-- The sidebar: expanded, and the collapsed rail in the same order
----------------------------------------------------------------------------

local search_el
local root_el
local function focus_search()
	-- Focusing the focused field again would drop its focus.
	if search_focused:peek() then
		return
	end
	if collapsed:peek() then
		collapsed:set(false)
		after(260, function()
			if search_el and search_el.mounted then
				search_el:focus()
			end
		end)
	elseif search_el and search_el.mounted then
		search_el:focus()
	end
end
local function blur_search()
	if root_el and search_focused:peek() then
		root_el:focus()
	end
end

local function search_field()
	return ui.row {
		id = "search", w_full = true, h = 30, px = 9, gap = 8, rounded = 8, bg = C.field, items = "center",
		border = 1, border_color = function() return search_focused:get() and C.strong or C.clear end,
		cursor = "text",
		on_click = focus_search,
		icon("search", 13, C.secondary),
		ui.overlay {
			grow = 1, shrink = 1, min_w = 0, h_full = true,
			ui.text {
				"Search or enter URL", absolute = true, left = 0, top = 7, font = SANS, text_size = 12.5, color = C.tertiary,
				hidden = function() return query:get() ~= "" end,
			},
			ui.input {
				id = "search_input", label = "Search or enter URL", value = query, w_full = true, h_full = true, p = 0,
				border = 0, bg = C.clear, text_size = 12.5, color = C.text, font = SANS, focus = { border_color = C.clear },
				on_mount = function(el) search_el = el end,
				on_focus = function() search_focused:set(true) end,
				on_blur = function() search_focused:set(false) end,
				on_key = function(e)
					if e.key == "Enter" then
						open_address(query:peek())
						query:set("")
						blur_search()
						return true
					elseif e.key == "Escape" then
						query:set("")
						blur_search()
						return true
					end
					return false
				end,
			},
		},
		mono("⌘K", 10, C.muted),
	}
end

-- Pins, chats and tabs of one space: what changes when the space does.
local function space_body(i)
	local s = D.spaces[i]
	local sid = s.id
	local top = ui.computed(function() return layout:get()[sid].top end)
	local groups = ui.computed(function() return layout:get()[sid].groups end)
	local bottom = ui.computed(function() return layout:get()[sid].bottom end)
	local chat_list = ui.computed(function() return chats:get()[sid] end)
	return ui.column {
		w_full = true, h_full = true, gap = 8,
		ui.column { id = "pinned", w_full = true, gap = 8, pin_tray(s.pins[1]), pin_tray(s.pins[2]) },
		ui.scroll {
			grow = 1, shrink = 1, min_h = 0, w_full = true,
			ui.column {
				w_full = true, gap = 8, pb = 8,
				ui.column {
					id = "chats", w_full = true, gap = 2,
					ui.row {
						w_full = true, h = 24, pl = 8, pr = 2, gap = 8, items = "center",
						mono("ASK", 10, C.tertiary, { letter_spacing = 1.2 }),
						spacer(),
						K.icon_button("new_chat", "edit", 12, C.tertiary, new_chat),
					},
					ui.each(chat_list, chat_row, { key = function(c) return c.id end }),
				},
				ui.div { w_full = true, h = 1, bg = C.line, shrink = 0 },
				ui.column {
					id = "tabs", w_full = true, gap = 6,
					ui.column {
						w_full = true,
						new_tab_row(focus_search),
						ui.each(top, tab_row, { key = function(id) return id end }),
					},
					ui.each(groups, function(g) return group_view(sid, g.id) end, { key = function(g) return g.id end }),
					ui.column { w_full = true, ui.each(bottom, tab_row, { key = function(id) return id end }) },
				},
			},
		},
	}
end

local function expanded_sidebar()
	return ui.column {
		id = "sidebar_expanded", w = 240, h_full = true, px = 8, pb = 8,
		ui.row {
			id = "topbar", w_full = true, h = 48, px = 4, gap = 8, items = "center",
			space_menu_view(),
			spacer(),
			K.icon_button("toggle_sidebar", "sidebar", 15, C.secondary, function() collapsed:set(true) end, { rounded = 6, w = 26, h = 26 }),
		},
		ui.column {
			w_full = true, grow = 1, min_h = 0, px = 4, pt = 2, gap = 8,
			search_field(),
			ui.presence(space, space_body, {
				w_full = true, grow = 1, min_h = 0,
				enter = { opacity = { 0, 1 }, y = { 6, 0 }, duration = 220, ease = "outCubic" },
				exit = { opacity = 0, duration = 110 },
			}),
			ui.row { id = "workspaces", w_full = true, pt = 4, pb = 8, px = 2, justify = "center", space_switcher() },
		},
	}
end

-- The rail: 30-point tiles in the expanded sidebar's order.
local function rail_tile(id)
	local function f()
		return flags:get()[id] or {}
	end
	local function on()
		return selected:get()[sp().id] == id
	end
	local key = "tab:" .. id
	return button({
		id = "rail_" .. id, w = 30, h = 30, rounded = 7, justify = "center", relative = true,
		bg = function()
			if on() then
				return C.tab_on
			end
			return hovered:get() == key and "#FFFFFF0D" or C.clear
		end,
		border = 1, border_color = function() return on() and C.tab_on_line or C.clear end,
		on_hover = hover_handler(key),
		on_click = function() select_page(id) end,
		loading_wash(id, 7),
		favicon(id, 14),
		K.orb("reading", 8, {
			absolute = true, top = 2, right = 2, border = 1, border_color = C.group,
			hidden = function() return f().reading == nil end,
		}),
		ui.div { absolute = true, top = 4, right = 3, w = 5, h = 5, rounded = 3, bg = C.unread, hidden = function() return not f().unread end },
	}, 0.92)
end

local function rail_body(i)
	local s = D.spaces[i]
	local sid = s.id
	local top = ui.computed(function() return layout:get()[sid].top end)
	local groups = ui.computed(function() return layout:get()[sid].groups end)
	local bottom = ui.computed(function() return layout:get()[sid].bottom end)
	local pins = {}
	for _, tray in s.pins do
		for _, id in tray do
			table.insert(pins, pin(id, { w = 36, h = 36, rounded = 7 }))
		end
	end
	return ui.scroll {
		w_full = true, h_full = true,
		ui.column {
			w_full = true, gap = 6, items = "center", pb = 8,
			pins,
			ui.div { w = 20, h = 1, bg = C.line, my = 1 },
			ui.column { gap = 2, items = "center", ui.each(top, rail_tile, { key = function(id) return id end }) },
			ui.each(groups, function(g)
				local gid = g.id
				local ids = ui.computed(function()
					for _, x in layout:get()[sid].groups do
						if x.id == gid then
							return x.open and x.tabs or {}
						end
					end
					return {}
				end)
				return ui.column {
					p = 3, gap = 2, rounded = 9, bg = C.group, items = "center",
					ui.each(ids, rail_tile, { key = function(id) return id end }),
					ui.show(function() return #ids:get() == 0 end, button({
						id = "rail_group_" .. gid, w = 30, h = 30, rounded = 7, justify = "center",
						hover = { bg = "#FFFFFF0D" },
						on_click = function() toggle_group(gid) end,
						icon("folder", 14, C.secondary),
					}, 0.92)),
				}
			end, { key = function(g) return g.id end }),
			ui.column { gap = 2, items = "center", ui.each(bottom, rail_tile, { key = function(id) return id end }) },
			K.icon_button("rail_new_tab", "plus", 15, C.secondary, focus_search, { w = 36, h = 30, rounded = 7 }),
			ui.div { w = 20, h = 1, bg = C.line, my = 1 },
			K.icon_button("rail_chats", "message-square", 15, C.secondary, function()
				select_chat(chat_sel:peek()[sid])
			end, { w = 36, h = 30, rounded = 7 }),
		},
	}
end

local function rail()
	local spaces = {}
	for i, s in D.spaces do
		table.insert(spaces, button({
			id = "rail_space_" .. s.id, w = 30, h = 26, rounded = 7, justify = "center",
			bg = function() return space:get() == i and C.tab_on or C.clear end,
			border = 1, border_color = function() return space:get() == i and C.line or C.clear end,
			hover = { bg = C.raised },
			on_click = function() switch_space(i) end,
			icon(s.icon, 13, function() return space:get() == i and C.text or C.muted end),
		}, 0.92))
	end
	return ui.column {
		id = "sidebar_rail", w = 60, h_full = true, px = 8, pb = 8, items = "center",
		ui.row {
			w_full = true, h = 48, justify = "center", items = "center",
			K.icon_button("expand_sidebar", "sidebar", 15, C.secondary, function() collapsed:set(false) end, { rounded = 6, w = 26, h = 26 }),
		},
		ui.column {
			w = 44, grow = 1, min_h = 0, pt = 2, gap = 6, items = "center",
			button({
				id = "rail_search", w = 36, h = 28, rounded = 7, justify = "center", bg = "#0F0F0E", border = 1, border_color = C.line,
				hover = { bg = C.raised },
				on_click = focus_search,
				icon("search", 15, C.secondary),
			}, 0.94),
			ui.presence(space, rail_body, {
				w_full = true, grow = 1, min_h = 0,
				enter = { opacity = { 0, 1 }, duration = 180 },
				exit = { opacity = 0, duration = 100 },
			}),
			ui.column { gap = 2, p = 3, rounded = 10, bg = C.panel, spaces },
		},
	}
end

local sidebar_w = ui.state("sidebar_w", 240)
ui.effect(function()
	sidebar_w:set(collapsed:get() and 60 or 240)
end)

local function sidebar()
	return ui.div {
		id = "sidebar", h_full = true, shrink = 0, overflow = "hidden",
		w = ui.tween(sidebar_w, { duration = 260, ease = "outCubic" }),
		ui.presence(collapsed, function(c)
			return c and rail() or expanded_sidebar()
		end, {
			h_full = true,
			enter = { opacity = { 0, 1 }, duration = 200, delay = 60 },
			exit = { opacity = 0, duration = 90 },
		}),
	}
end

----------------------------------------------------------------------------
-- The page: what the selected tab, pin or chat shows
----------------------------------------------------------------------------

local function skeleton(w, h, color)
	return ui.div { w = w, h = h, rounded = h / 2, bg = color or C.mini, shrink = 0 }
end

local function page_view(id)
	local p = page(id)
	local function f()
		return flags:get()[id] or {}
	end
	return ui.column {
		id = "page", w_full = true, h_full = true,
		ui.row {
			w_full = true, h = 44, px = 12, gap = 4, items = "center", border = 1, border_sides = { 0, 0, 1, 0 }, border_color = C.line,
			K.icon_button("back", "arrow-left", 14, C.muted, function() end),
			K.icon_button("forward", "arrow-right", 14, C.faint, function() end),
			K.icon_button("reload", "rotate-cw", 13, C.muted, function() load(id) end),
			ui.row {
				id = "address", grow = 1, h = 28, ml = 6, px = 10, gap = 7, rounded = 8, bg = C.window, items = "center",
				border = 1, border_color = C.line, relative = true, overflow = "hidden",
				loading_wash(id, 8),
				icon("lock", 11, C.muted),
				mono(p[3], 11, C.secondary, { truncate = true, shrink = 1, min_w = 0 }),
			},
		},
		ui.column {
			w_full = true, grow = 1, items = "center", justify = "center", gap = 14, pb = 80,
			ui.div {
				w = 88, h = 88, rounded = 22, bg = "#161615", border = 1, border_color = C.line, items = "center", justify = "center",
				favicon(id, 48),
			},
			text(p[1], 20, C.text, { id = "page_title", weight = 500 }),
			mono(function()
				local x = f()
				if x.loading then
					return "loading " .. p[3] .. " …"
				elseif x.audio then
					return x.muted and "muted" or "playing audio"
				elseif x.reading then
					return "an agent is working on this page"
				end
				return p[3]
			end, 11, C.muted),
			ui.column {
				gap = 8, items = "center", pt = 18,
				skeleton(340, 8), skeleton(280, 8), skeleton(310, 8),
			},
		},
	}
end

local function chat_view(id)
	local c = { title = "Chat", time = "" }
	for _, x in chats:peek()[spp().id] or {} do
		if x.id == id then
			c = x
		end
	end
	return ui.column {
		id = "chat_page", w_full = true, h_full = true, items = "center", justify = "center", gap = 14, pb = 60,
		K.orb("chat", 44),
		text(c.title, 20, C.text, { weight = 500 }),
		mono("ASK · " .. c.time, 10, C.muted, { letter_spacing = 1 }),
		ui.row {
			w = 440, h = 46, mt = 18, px = 14, gap = 10, rounded = 12, bg = C.field, border = 1, border_color = C.strong, items = "center",
			text("Ask anything…", 13.5, C.tertiary),
			spacer(),
			mono("↵", 12, C.muted),
		},
	}
end

local function content_key()
	local sid = sp().id
	if showing:get()[sid] == "chat" and chat_sel:get()[sid] then
		return "chat:" .. chat_sel:get()[sid]
	end
	local id = selected:get()[sid]
	return id and ("page:" .. id) or "empty"
end

local function content_view(key)
	local kind, id = key:match("^(%a+):(.+)$")
	if kind == "chat" then
		return chat_view(id)
	elseif kind == "page" then
		return page_view(id)
	end
	return ui.column {
		w_full = true, h_full = true, items = "center", justify = "center", gap = 8,
		text("No tabs open", 15, C.secondary),
		mono("⌘T  new tab", 10, C.muted),
	}
end

----------------------------------------------------------------------------
-- The agent PiP: master, stacked, stack expanded
----------------------------------------------------------------------------

local function count()
	return #agents:get()
end
local function stacked()
	return count() > 1 and not expanded:get()
end

local function orb_kind(st)
	if st == "working" then
		return "working"
	end
	return st
end
-- Presence and show wrappers grow in a row; a fixed box keeps them to size.
local function agent_orb(id, size)
	return ui.div {
		w = size, h = size, shrink = 0,
		ui.presence(function() return orb_kind(agent_state(id).state) end, function(kind)
			return K.orb(kind, size)
		end, { w = size, h = size }),
	}
end

local function tab_chip(t, first)
	if first then
		return ui.row {
			h = 20, pl = 5, pr = 6, gap = 4, rounded = 5, bg = C.field, border = 1, border_color = C.strong, items = "center", shrink = 0,
			t[2] and K.favicon(t[2], 10, { rounded = 2 }) or icon("globe", 10, C.tertiary),
			text(t[1], 10.5, C.text, { weight = 500 }),
		}
	end
	return ui.div {
		w = 20, h = 20, rounded = 5, bg = "#161615", border = 1, border_color = C.line, items = "center", justify = "center", shrink = 0,
		t[2] and K.favicon(t[2], 10, { rounded = 2 }) or icon("globe", 10, C.tertiary),
	}
end

-- The page the agent is on, in miniature, with its cursor at work.
local function live_page(a)
	local cursor, focus = ui.ref(), ui.ref()
	local function st()
		return agent_state(a.id)
	end
	local function target()
		local s = a.steps[math.clamp(st().step, 1, #a.steps)]
		return s[2], s[3]
	end
	local x0, y0 = target()
	local first = true
	ui.effect(function()
		local x, y = target()
		if first then
			first = false
			return
		end
		anime.animate(cursor, { x = x + 1, y = y + 6, duration = 700, ease = "inOutCubic" })
		anime.animate(focus, { x = x - 40, y = y - 40, duration = 800, ease = "inOutCubic" })
	end)
	local function field(n)
		return ui.div {
			grow = 1, basis = 0, h = 16, rounded = 3, bg = C.tray, border = 1,
			border_color = function()
				local s = st()
				if s.state == "done" then
					return C.strong
				end
				local tx = target()
				local on = math.abs(tx - ({ 39, 102, 165 })[n]) < 1
				if on then
					return s.state == "waiting" and C.gold or C.secondary
				end
				return C.strong
			end,
		}
	end
	local function live()
		return st().state ~= "done"
	end
	-- The board's focus dither: a radial falloff round where the agent works.
	local glow = K.bayer({ ink = C.text, alpha = 0.25, mode = 1, radius = 0.5, from = 0.55, to = 0, curve = 1.2 })
	glow.ref, glow.absolute, glow.left, glow.top, glow.w, glow.h = focus, true, 0, 0, 80, 80
	glow.x, glow.y, glow.pointer_events = x0 - 40, y0 - 40, "none"
	glow.hidden = function() return not live() end
	return ui.div {
		w_full = true, h = 124, px = 6, shrink = 0,
		ui.column {
			w_full = true, h_full = true, px = 10, py = 8, gap = 6, rounded = 6, bg = C.mini, border = 1, border_color = C.line,
			overflow = "hidden", relative = true,
			ui.row { w_full = true, gap = 6, items = "center", ui.div { w = 10, h = 10, rounded = 3, bg = a.accent }, ui.div { w = 46, h = 4, rounded = 2, bg = "#3A3933" } },
			ui.div { w = 110, h = 6, rounded = 2, bg = "#5A5850" },
			ui.row { w_full = true, gap = 5, field(1), field(2), field(3) },
			ui.div { w = 140, h = 4, rounded = 2, bg = C.strong },
			ui.div { w = 96, h = 4, rounded = 2, bg = C.strong },
			ui.div { w = 62, h = 14, rounded = 3, bg = a.accent },
			ui.div(glow),
			icon("mouse-pointer", 13, C.text, {
				ref = cursor, absolute = true, left = 0, top = 0, x = x0 + 1, y = y0 + 6, hidden = function() return not live() end,
			}),
		},
	}
end

local function stop_button(a)
	local function st()
		return agent_state(a.id).state
	end
	return button({
		id = "pip_action", h = 22, px = 8, gap = 5, rounded = 6, shrink = 0,
		bg = function() return st() == "waiting" and C.gold_button or C.field end,
		border = 1, border_color = function() return st() == "waiting" and C.gold_line or C.strong end,
		hover = { brightness = 1.15 },
		on_click = function()
			local s = st()
			if s == "waiting" then
				set_agent(a.id, function(x)
					x.state = "working"
					x.step = math.min(x.step + 1, #a.steps)
				end)
			else
				remove_agent(a.id)
			end
		end,
		ui.switch(function() return st() end, {
			waiting = text("Review", 11, C.gold, { weight = 500 }),
			done = ui.row { gap = 5, items = "center", icon("check", 11, C.done), text("Close", 11, C.text) },
		}, ui.row { gap = 5, items = "center", ui.div { w = 7, h = 7, rounded = 1.5, bg = C.text }, text("Stop", 11, C.text) }),
	}, 0.95)
end

local function front_card(id)
	local a = agent_info(id)
	local function st()
		return agent_state(id)
	end
	local others = ui.computed(function()
		local out = {}
		for i, x in agents:get() do
			if i > 1 then
				table.insert(out, x)
			end
		end
		return out
	end)
	return ui.column {
		id = "pip_front", w_full = true, bg = C.pip, border = 1, border_color = C.pip_line, rounded = 10, overflow = "hidden",
		shadow = { y = 8, blur = 24, color = "#00000066" },
		ui.row {
			w_full = true, h = 34, pl = 10, pr = 6, gap = 8, items = "center",
			agent_orb(id, 12),
			text(a.task, 12, C.text, { weight = 500, grow = 1, shrink = 1, min_w = 0, truncate = true, align = "left" }),
			ui.div { shrink = 0, ui.show(stacked, button({
				id = "pip_others", h = 20, pl = 5, pr = 7, gap = 3, rounded = 10, bg = C.field, border = 1, border_color = C.strong,
				hover = { bg = C.selected },
				on_click = function() expanded:set(true) end,
				ui.each(others, function(o) return agent_orb(o.id, 9) end, { key = function(o) return o.id end }),
				mono(function() return "+" .. (count() - 1) end, 10, C.soft, { weight = 500, ml = 1 }),
			}, 0.94), K.icon_button("pip_open", "maximize-2", 12, C.tertiary, function() open_agent_tab(id) end)) },
		},
		ui.show(function() return count() > 1 end, ui.row {
			w_full = true, px = 6, pb = 6, gap = 3, items = "center",
			tab_chip(a.tabs[1], true),
			a.tabs[2] and tab_chip(a.tabs[2]) or nil,
			a.tabs[3] and tab_chip(a.tabs[3]) or nil,
			spacer(),
			mono(#a.tabs .. " tabs", 9.5, C.muted),
		}),
		live_page(a),
		ui.row {
			w_full = true, h = 36, pl = 10, pr = 6, gap = 8, items = "center",
			ui.column {
				grow = 1, shrink = 1, min_w = 0, gap = 1,
				text(function()
					local s = st()
					if s.state == "done" then
						return a.finished
					end
					return a.steps[s.step][1]
				end, 11.5, C.secondary, { truncate = true }),
				mono(function()
					local s = st()
					if s.state == "waiting" then
						return a.domain .. " · " .. a.ask
					end
					return a.page .. " · " .. s.step .. "/" .. #a.steps
				end, 9.5, function() return st().state == "waiting" and C.gold or C.muted end, { truncate = true }),
			},
			stop_button(a),
		},
	}
end

local function agent_row(entry)
	local id = entry.id
	local a = agent_info(id)
	local function waiting()
		return agent_state(id).state == "waiting"
	end
	local thumb = K.bayer({ ink = "#8A877E", mode = 3, scale = 2.2, seed = 7, from = 0, to = 0.6 })
	thumb.w, thumb.h, thumb.rounded, thumb.bg, thumb.border, thumb.border_color, thumb.shrink = 36, 24, 4, C.panel, 1, C.line, 0
	thumb.on_mount = function(el)
		anime.animate(el, { shaderTime = { 0, 30 }, duration = 30000, loop = true, ease = "linear", frameRate = 8 })
	end
	return button({
		id = "pip_agent_" .. id, w_full = true, pt = 8, pb = 8, pl = 10, pr = 6, gap = 8, rounded = 10, border = 1,
		bg = function() return waiting() and C.gold_bg or C.pip end,
		border_color = function() return waiting() and C.gold_line or C.tray_line end,
		hover = { brightness = 1.12 },
		on_press = function() swallow = false end,
		on_click = row_click(function() to_front(id) end),
		agent_orb(id, 12),
		ui.column {
			grow = 1, shrink = 1, min_w = 0, gap = 2,
			text(a.task, 12, C.text, { weight = 500, truncate = true }),
			mono(function()
				local s = agent_state(id)
				if s.state == "waiting" then
					return a.domain .. " · " .. a.ask
				elseif s.state == "done" then
					return a.finished
				end
				return #a.tabs .. " tabs · " .. s.step .. "/" .. #a.steps
			end, 9.5, function() return waiting() and C.gold or C.muted end, { truncate = true }),
		},
		ui.show(waiting, button({
			id = "pip_review_" .. id, h = 22, px = 8, rounded = 6, bg = C.gold_button, border = 1, border_color = C.gold_line,
			hover = { brightness = 1.2 },
			on_press = function() swallow = true end,
			on_click = function()
				swallow = true
				to_front(id)
			end,
			text("Review", 11, C.gold, { weight = 500 }),
		}, 0.94), ui.div(thumb)),
	}, 0.985)
end

local function pip()
	local others = ui.computed(function()
		local out = {}
		for i, x in agents:get() do
			if i > 1 then
				table.insert(out, x)
			end
		end
		return out
	end)
	return ui.column {
		id = "pip", absolute = true, right = 16, bottom = 16, w = 216,
		hidden = function() return count() == 0 end,
		ui.show(expanded, ui.row {
			id = "pip_stack_header", w_full = true, h = 22, pl = 4, pr = 2, gap = 6, items = "center", mb = 6,
			text(function() return count() .. " agents" end, 11.5, C.secondary, { weight = 500 }),
			mono(function()
				local n = 0
				for _, x in agents:get() do
					n += #agent_info(x.id).tabs
				end
				return n .. " tabs"
			end, 9.5, C.muted),
			spacer(),
			K.icon_button("pip_collapse", "chevrons-up", 12, C.tertiary, function() expanded:set(false) end),
		}),
		ui.div {
			w_full = true, relative = true,
			pt = function() return stacked() and 14 or 0 end,
			-- The stack's peeks: the cards behind the front one.
			ui.div {
				absolute = true, left = 16, top = 0, w = 184, h = 40, rounded = 9, bg = "#0E0E0D", border = 1, border_color = "#24231F",
				hidden = function() return not stacked() or count() < 3 end,
			},
			ui.div {
				absolute = true, left = 8, top = 7, w = 200, h = 40, rounded = 10, bg = "#111110", border = 1, border_color = C.tray_line,
				hidden = function() return not stacked() end,
			},
			ui.presence(function()
				local list = agents:get()
				return list[1] and list[1].id or nil
			end, front_card, {
				w_full = true,
				enter = { opacity = { 0, 1 }, y = { 10, 0 }, duration = 240, ease = "outCubic" },
				exit = { opacity = 0, y = -6, duration = 140 },
			}),
		},
		ui.column {
			w_full = true, gap = 6, pt = 6, hidden = function() return not expanded:get() end,
			ui.each(others, agent_row, { key = function(x) return x.id end }),
		},
	}
end

----------------------------------------------------------------------------
-- Keys
----------------------------------------------------------------------------

local function step_tab(by)
	local sid = spp().id
	local order = all_tabs(sid)
	local at = table.find(order, selected:peek()[sid]) or 0
	local nxt = order[math.clamp(at + by, 1, #order)]
	if nxt then
		select_page(nxt)
	end
end

local function handle(e)
	local k = e.key
	local low = #k == 1 and k:lower() or k
	if e.ctrl and (k == "1" or k == "2" or k == "3") then
		switch_space(tonumber(k) :: number)
		return true
	end
	if e.meta then
		if low == "k" or low == "l" or low == "t" then
			focus_search()
		elseif low == "s" or k == "\\" then
			collapsed:set(not collapsed:peek())
		elseif low == "w" then
			local id = selected:peek()[spp().id]
			if id then
				close_tab(id)
			end
		elseif low == "r" and e.shift then
			reset()
		elseif low == "r" then
			local id = selected:peek()[spp().id]
			if id then
				load(id)
			end
		elseif low == "e" then
			if count() > 1 then
				expanded:set(not expanded:peek())
			end
		else
			return false
		end
		return true
	end
	if e.alt and (k == "ArrowUp" or k == "ArrowDown") then
		step_tab(k == "ArrowUp" and -1 or 1)
		return true
	end
	if k == "Escape" then
		if space_menu:peek() then
			space_menu:set(false)
		elseif expanded:peek() then
			expanded:set(false)
		else
			return false
		end
		return true
	end
	return false
end

return ui.overlay {
	id = "sidebar_pip", w_full = true, h_full = true, bg = C.window, color = C.text, font = SANS, text_size = 13,
	focusable = true, focus = {},
	on_mount = function(el)
		root_el = el
		el:focus()
	end,
	on_key = handle,
	ui.row {
		w_full = true, h_full = true,
		sidebar(),
		ui.div {
			grow = 1, shrink = 1, min_w = 0, h_full = true, pt = 8, pb = 8, pr = 8,
			ui.div {
				id = "content", w_full = true, h_full = true, rounded = 10, bg = C.panel, border = 1, border_color = C.swatch_line,
				overflow = "hidden", relative = true,
				ui.presence(content_key, content_view, {
					w_full = true, h_full = true,
					enter = { opacity = { 0, 1 }, duration = 160 },
					exit = { opacity = 0, duration = 90 },
				}),
				pip(),
			},
		},
	},
}
