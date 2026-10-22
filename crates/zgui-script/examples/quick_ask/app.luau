-- Quick ask, from ask.pen's "Ask · Quick ask · Ask from anywhere" board, as
-- one working element: the system-wide mini window summoned over any app,
-- reading the frontmost window and its selection as context. The board's
-- five cards are states of it: summoned over Chrome, typing over VS Code, an
-- answer over Mail, Finder files attached, and files dropped on it. Every
-- control does something (canned answers, no network), and every key on the
-- board's keyboard map works.
local K = require("./kit")
local D = require("./scenes")
local apps = require("./apps")
local dither = require("./dither")
local C, SANS, MONO = K.C, K.SANS, K.MONO
local icon, text, mono, spacer, kbd, button, after = K.icon, K.text, K.mono, K.spacer, K.kbd, K.button, K.after
local SCENES = D.scenes

----------------------------------------------------------------------------
-- State
----------------------------------------------------------------------------

local scene = ui.state("scene", 1)
local open = ui.state("open", true)
local query = ui.state("query", "")
local caps = ui.state("caps", { true, false, false })
local files = ui.state("files", {}) -- attached with + or dropped: { name, meta, icon, tone }
local progress = ui.state("progress", {}) -- upload progress by file name, 0 to 1
local picked = ui.state("picked", { "Lease agreement.pdf", "Q3 expenses.xlsx", "Floor plan.pdf" })
local sel = ui.state("sel", 1)
local dest = ui.state("dest", "ask")
local model = ui.state("model", "Auto")
local mode = ui.state("mode", "compose") -- "compose" | "answer"
local turns = ui.state("turns", {}) -- { id, q, a, done, ms }
local drop = ui.state("drop", nil) -- the payload dragged over the panel
local toast = ui.state("toast", nil)
local keymap = ui.state("keymap", false)
local listening = ui.state("listening", false)
local model_open = ui.state("model_open", false)
local actions_open = ui.state("actions_open", false)
local pasted = ui.state("pasted", nil)
local hovered = ui.state("hovered_chip", "")
local serial = ui.state("serial", 0)

local function sc()
	return SCENES[scene:get()]
end
local function scp()
	return SCENES[scene:peek()]
end
local function next_id()
	serial:set(serial:peek() + 1)
	return serial:peek()
end

local input_el -- the composer's text field, once mounted
local root_el -- the window, which takes the keys while the panel is closed
local input_focused = false
local function refocus()
	if input_el and input_el.mounted then
		input_el:focus()
	end
end

local function show_toast(message)
	local n = next_id()
	toast:set({ text = message, n = n })
	after(2400, function()
		local t = toast:peek()
		if t and t.n == n then
			toast:set(nil)
		end
	end)
end

local function close_menus()
	model_open:set(false)
	actions_open:set(false)
end

-- Each scene opens the way its card shows it.
local function enter_scene(i)
	local s = SCENES[i]
	ui.batch(function()
		scene:set(i)
		caps:set(table.clone(s.on))
		files:set({})
		query:set(s.draft or "")
		sel:set(1)
		pasted:set(nil)
		drop:set(nil)
		listening:set(false)
		close_menus()
		if s.start then
			turns:set({ { id = next_id(), q = s.start.q, a = s.start.a, done = true, ms = 1.4 } })
			mode:set("answer")
		else
			turns:set({})
			mode:set("compose")
		end
		open:set(true)
	end)
end

local booted = ui.state("booted", false)
if not booted:peek() then
	booted:set(true)
	local first = ui.shared("quick_ask_scene", 1):peek()
	enter_scene(math.clamp(math.floor(first), 1, #SCENES))
end

local function close_panel(clear)
	-- The text field leaves with the panel: keep the keys (⌥Space) coming.
	-- Only from the field: a click has already focused the window, and
	-- focusing a focused element again drops focus.
	if root_el and input_focused then
		root_el:focus()
	end
	ui.batch(function()
		open:set(false)
		close_menus()
		listening:set(false)
		drop:set(nil)
		if clear then
			query:set("")
			turns:set({})
			mode:set("compose")
			files:set({})
		end
	end)
end

----------------------------------------------------------------------------
-- What the panel shows: chips, rows
----------------------------------------------------------------------------

local function file_info(name)
	for _, f in D.files do
		if f.name == name then
			return f
		end
	end
	for _, f in D.picks do
		if f.name == name then
			return f
		end
	end
	return { name = name, meta = "File", icon = "file", tone = "#8B8D98" }
end

-- The chips in the input box: what the captures take, then attached files.
local chips = ui.computed(function()
	local s, out, seen = sc(), {}, {}
	for i, on in caps:get() do
		local c = s.chips[i]
		if on and c.kind == "files" then
			for _, name in picked:get() do
				local f = file_info(name)
				seen[name] = true
				table.insert(out, { key = "cap:" .. name, kind = "file", name = name, meta = f.meta, icon = f.icon,
					tone = f.tone, cap = i, ctx = "selection" })
			end
		elseif on then
			table.insert(out, { key = "cap:" .. i, kind = c.kind, name = c.name, icon = c.icon, cap = i, ctx = c.ctx })
		end
	end
	for j, f in files:get() do
		if not seen[f.name] then
			table.insert(out, { key = "file:" .. f.name, kind = "file", name = f.name, meta = f.meta, icon = f.icon,
				tone = f.tone, file = j, ctx = "file" })
		end
	end
	return out
end)

-- The omnibox rows: suggestions while empty; Ask then Google rows (or the
-- other way round, when Google is the default) once there is a query.
local items = ui.computed(function()
	local s, q, out = sc(), query:get(), {}
	local n = 0
	local function group(label)
		table.insert(out, { key = "h:" .. label, header = label })
	end
	local function row(r)
		n += 1
		r.n = n
		-- Keyed rows keep what they built: the rows that show the query keep
		-- one key and read it live, so typing doesn't rebuild them; a new
		-- detail is a new row.
		r.key = r.kind .. ":" .. (r.live and "query" or r.title) .. ":" .. (r.detail or "")
		table.insert(out, r)
	end
	if q:match("^%s*$") then
		group(s.group)
		for _, r in s.suggest do
			row({ kind = "ask", icon = r.icon, title = r.title, detail = r.detail })
		end
		return out
	end
	local list = chips:get()
	local detail = #list == 1 and "— with " .. list[1].ctx or #list > 1 and "— reads all " .. #list or nil
	local function asks()
		group(#list > 1 and "ASK · " .. #list .. " ATTACHMENTS" or "ASK")
		row({ kind = "ask", orb = true, title = q, live = true, detail = detail })
		for _, r in s.typed do
			if r.title:lower() ~= q:lower() then
				row({ kind = "ask", icon = r.icon, title = r.title, detail = r.detail or detail })
			end
		end
	end
	local function googles()
		group("GOOGLE")
		row({ kind = "google", icon = "search", title = q, live = true })
		row({ kind = "google", icon = "search", title = s.google })
	end
	if dest:get() == "google" then
		googles()
		asks()
	else
		asks()
		googles()
	end
	return out
end)

local function rows()
	local out = {}
	for _, it in items:peek() do
		if not it.header then
			table.insert(out, it)
		end
	end
	return out
end

-- The selection stays on a row as the rows change.
ui.effect(function()
	query:get()
	dest:get()
	sel:set(1)
end)

----------------------------------------------------------------------------
-- Actions
----------------------------------------------------------------------------

local function answer_for(q)
	local s = scp()
	local a = s.answers[q:lower()]
	if a then
		return a
	end
	return "Here's a short take on “" .. q .. "”, read against " .. s.window .. ". This is a stand-in answer: "
		.. "in Ask it streams in here with the sources it read underneath, ready to paste back into "
		.. s.app .. " or open as a thread."
end

local function ask(q)
	q = q:gsub("^%s+", ""):gsub("%s+$", "")
	if q == "" then
		return
	end
	local id = next_id()
	local list = table.clone(turns:peek())
	table.insert(list, { id = id, q = q, done = false })
	ui.batch(function()
		turns:set(list)
		mode:set("answer")
		query:set("")
		close_menus()
	end)
	local ms = model:peek() == "Instant" and 500 or model:peek() == "Deep" and 2200 or 1100
	after(ms, function()
		local now = table.clone(turns:peek())
		for i, t in now do
			if t.id == id then
				now[i] = { id = id, q = q, a = answer_for(q), done = true, ms = ms / 1000 + 0.3 }
			end
		end
		turns:set(now)
	end)
end

local function google(q)
	if q == "" then
		return
	end
	show_toast("Searching Google for “" .. q .. "” in your default browser")
	close_panel(false)
end

local function run(r)
	if not r then
		return
	end
	local title = r.live and query:peek() or r.title
	if r.kind == "google" then
		google(title)
	else
		ask(title)
	end
end

local function last_answer()
	local list = turns:peek()
	local t = list[#list]
	return t and t.done and t.a or nil
end

local function paste()
	local a = last_answer()
	if not a then
		return
	end
	if scp().id == "mail" then
		pasted:set(a)
	end
	show_toast("Pasted into " .. scp().app)
	close_panel(true)
end

local function open_in_ask()
	if not last_answer() then
		return
	end
	show_toast("Opened the thread in Ask with " .. #chips:peek() .. " context items")
	close_panel(true)
end

local function copy_answer()
	if last_answer() then
		show_toast("Answer copied")
	end
end

local function upload(name)
	local p = table.clone(progress:peek())
	p[name] = 0
	progress:set(p)
	-- 12 updates a second: each one repaints the frosted panel.
	anime.createTimer {
		duration = 1600,
		frameRate = 12,
		onUpdate = function(t)
			local now = table.clone(progress:peek())
			now[name] = math.min(1, t.progress)
			progress:set(now)
		end,
		onComplete = function()
			local now = table.clone(progress:peek())
			now[name] = 1
			progress:set(now)
		end,
	}
end

local function attach(names)
	local have = {}
	for _, c in chips:peek() do
		have[c.name] = true
	end
	local list = table.clone(files:peek())
	for _, name in names do
		if not have[name] then
			local f = file_info(name)
			table.insert(list, { name = name, meta = f.meta, icon = f.icon, tone = f.tone })
			upload(name)
		end
	end
	files:set(list)
end

local picks = 0
local function attach_pick()
	picks += 1
	attach({ D.picks[(picks - 1) % #D.picks + 1].name })
	refocus()
end

local function remove_chip(c)
	if c.file then
		local list = table.clone(files:peek())
		table.remove(list, c.file)
		files:set(list)
	elseif c.kind == "file" then
		local list = table.clone(picked:peek())
		local at = table.find(list, c.name)
		if at then
			table.remove(list, at)
		end
		picked:set(list)
	else
		local now = table.clone(caps:peek())
		now[c.cap] = false
		caps:set(now)
	end
	refocus()
end

local function toggle_cap(i)
	local now = table.clone(caps:peek())
	now[i] = not now[i]
	caps:set(now)
	refocus()
end

local DICTATION = {
	chrome = "how long should it rest after searing",
	code = "is this timezone safe",
	mail = "suggest three rooftop restaurants",
	finder = "when is the first rent due",
}
local function dictate()
	if listening:peek() then
		listening:set(false)
		return
	end
	listening:set(true)
	local phrase = DICTATION[scp().id]
	anime.createTimer {
		duration = #phrase * 45,
		onUpdate = function(t)
			if listening:peek() then
				query:set(phrase:sub(1, math.floor(t.progress * #phrase + 0.5)))
			end
		end,
		onComplete = function()
			listening:set(false)
			refocus()
		end,
	}
end

local function flip_dest()
	dest:set(dest:peek() == "ask" and "google" or "ask")
	refocus()
end

-- ⌘↵: Google when Ask is the default (Ask when Google is), Paste on an answer.
local function alternate()
	local q = query:peek()
	if mode:peek() == "answer" and q == "" then
		paste()
	elseif q ~= "" then
		if dest:peek() == "ask" then
			google(q)
		else
			ask(q)
		end
	else
		local r = rows()[sel:peek()]
		if r then
			google(r.title)
		end
	end
end

local function send()
	local q = query:peek()
	if mode:peek() == "answer" then
		ask(q)
	elseif q ~= "" or #rows() > 0 then
		run(rows()[sel:peek()])
	end
	refocus()
end

----------------------------------------------------------------------------
-- Keys: the board's keyboard map
----------------------------------------------------------------------------

-- Option+digit arrives as the character it types on a US layout.
local OPTION_DIGITS = { ["1"] = 1, ["2"] = 2, ["3"] = 3, ["¡"] = 1, ["™"] = 2, ["£"] = 3 }

local function handle(e)
	local k = e.key
	local low = #k == 1 and k:lower() or k
	if keymap:peek() then
		if k == "Escape" or (e.meta and (k == "/" or k == "?")) then
			keymap:set(false)
			return true
		end
		return false
	end
	if e.meta and (k == "/" or k == "?") then
		keymap:set(true)
		return true
	end
	if e.alt and (k == " " or k == " " or k == "Space") then
		if open:peek() then
			close_panel(false)
		else
			open:set(true)
		end
		return true
	end
	if not open:peek() then
		return false
	end
	if k == "Escape" then
		if model_open:peek() or actions_open:peek() then
			close_menus()
		elseif drop:peek() then
			drop:set(nil)
		else
			close_panel(false)
		end
		return true
	end
	if e.meta then
		if low == "w" then
			close_panel(true)
		elseif low == "n" then
			ui.batch(function()
				turns:set({})
				mode:set("compose")
				query:set("")
			end)
		elseif k == "Enter" then
			alternate()
		elseif low == "o" then
			open_in_ask()
		elseif low == "c" and e.shift then
			copy_answer()
		elseif k == "[" then
			if #turns:peek() > 0 then
				mode:set("compose")
			end
		elseif k == "." then
			actions_open:set(false)
			model_open:set(not model_open:peek())
		elseif low == "k" then
			model_open:set(false)
			actions_open:set(not actions_open:peek())
		elseif low == "u" then
			attach_pick()
		elseif low == "d" then
			dictate()
		elseif tonumber(k) and tonumber(k) >= 1 then
			if mode:peek() == "compose" then
				run(rows()[tonumber(k)])
			end
		else
			return false
		end
		return true
	end
	if e.alt and OPTION_DIGITS[k] then
		toggle_cap(OPTION_DIGITS[k])
		return true
	end
	if k == "Tab" then
		flip_dest()
		return true
	end
	if mode:peek() == "compose" and (k == "ArrowDown" or k == "ArrowUp") then
		local n = #rows()
		if n > 0 then
			sel:set((sel:peek() - 1 + (k == "ArrowDown" and 1 or -1)) % n + 1)
		end
		return true
	end
	if k == "Enter" and not e.shift then
		if actions_open:peek() then
			actions_open:set(false)
		end
		send()
		return true
	end
	if k == "Backspace" and query:peek() == "" then
		local list = chips:peek()
		if #list > 0 then
			remove_chip(list[#list])
			return true
		end
	end
	return false
end

----------------------------------------------------------------------------
-- Pieces
----------------------------------------------------------------------------

-- The Ask orb: the design system's Bayer-dithered sphere.
local function orb(size, color, spinning)
	return ui.div {
		w = size, h = size, rounded = size / 2, shrink = 0, bg = "#1F1E1B",
		bg_shader = dither, shader_params = { color or C.text, { 1, 0, 1.1, 0 } },
		on_mount = spinning and function(el)
			anime.animate(el, { shaderTime = { 0, 6 }, duration = 1500, loop = true, frameRate = 12, ease = "linear" })
		end or nil,
	}
end

local function app_tile(id, size)
	local r = size / 4
	if id == "chrome" then
		return ui.div { w = size, h = size, rounded = r, bg = "#FFFFFF", items = "center", justify = "center", shrink = 0,
			ui.image { src = "assets/chrome.png", w = size * 0.75, h = size * 0.75, fit = "contain" } }
	end
	local look = ({
		code = { "#2489D8", "#0E63B5", "code" },
		mail = { "#5AC8FA", "#1E7CF2", "mail" },
		finder = { "#5AC8FA", "#1E7CF2", "folder" },
	})[id]
	return ui.div {
		w = size, h = size, rounded = r, shrink = 0, items = "center", justify = "center",
		gradient = { angle = 180, stops = { look[1], look[2] } },
		icon(look[3], size * 0.6, "#FFFFFF"),
	}
end

local function capture(s, i)
	local function on()
		return caps:get()[i]
	end
	local function label()
		if s.id == "finder" and i == 2 then
			local n = #picked:get()
			return n > 0 and n .. " files" or "Selection"
		end
		return s.caps[i]
	end
	return button({
		id = "cap_" .. i, h = 26, rounded = 13, pl = 8, pr = 4, gap = 6,
		bg = function() return on() and C.wash or C.clear end,
		border = 1, border_color = function() return on() and "#FFFFFF24" or C.clear end,
		hover = { bg = C.wash },
		on_click = function() toggle_cap(i) end,
		icon(s.cap_icons[i], 12, function() return on() and C.text or C.muted end),
		ui.text { label, font = SANS, text_size = 11.5,
			color = function() return on() and C.text or C.secondary end,
			weight = function() return on() and 500 or 400 end },
		ui.row {
			h = 18, px = 5, border = 1, border_color = C.key_line, items = "center",
			corners = function() return on() and { 6, 9, 9, 6 } or { 6, 6, 6, 6 } end,
			mono("⌥" .. i, 10, C.keycap, { letter_spacing = 0.5 }),
		},
	}, 0.96)
end

local function dock(s)
	local list = {}
	for i = 1, 3 do
		list[i] = capture(s, i)
	end
	return ui.row {
		w_full = true, h = 40, pl = 12, pr = 8, gap = 8, items = "center",
		bg = C.dock, corners = { 18, 18, 0, 0 }, border = 1, border_sides = { 1, 1, 0, 1 }, border_color = C.dock_line,
		app_tile(s.id, 20),
		text(s.app, 12.5, C.text, { weight = 500 }),
		mono(s.window, 10.5, C.hint, { truncate = true, shrink = 1, min_w = 0 }),
		spacer(),
		ui.row { gap = 2, items = "center", list },
	}
end

local function remove_badge(c)
	return ui.button {
		id = "remove_" .. c.key, absolute = true, top = -5, right = -5, w = 16, h = 16, p = 0, rounded = 8,
		bg = "#2A2926", border = 1.5, border_color = C.window, items = "center", justify = "center",
		hover = { bg = "#3A3934" }, focus = {}, focusable = false, cursor = "pointer",
		invisible = function() return hovered:get() ~= c.key end,
		on_click = function() remove_chip(c) end,
		icon("x", 8, C.text),
	}
end

local function chip_view(c)
	local body
	if c.kind == "file" then
		body = ui.row {
			h = 36, pl = 5, pr = 10, gap = 8, rounded = 9, items = "center",
			bg = C.elevated, border = 1, border_color = C.line,
			ui.div { w = 26, h = 26, rounded = 6, bg = c.tone .. "29", items = "center", justify = "center",
				icon(c.icon, 13, c.tone) },
			ui.column {
				gap = 2, max_w = 150,
				text(c.name, 11.5, C.text, { weight = 500, truncate = true }),
				ui.text {
					function()
						local p = progress:get()[c.name]
						if p and p < 1 then
							return "Uploading " .. math.floor(p * 100) .. "%"
						end
						return c.meta
					end,
					font = MONO, text_size = 9.5, color = C.muted,
				},
			},
		}
	elseif c.kind == "image" then
		body = ui.div {
			w = 36, h = 36, rounded = 8, items = "center", justify = "center",
			gradient = { angle = 160, stops = { "#3A3D52", "#7A5A4A", "#C79A6B" } },
			border = 1, border_color = C.line,
			icon(c.icon, 14, "#FFFFFFCC"),
		}
	else
		body = ui.row {
			h = 26, pl = 4, pr = 9, gap = 6, rounded = 8, items = "center",
			bg = C.elevated, border = 1, border_color = C.line,
			ui.div { w = 18, h = 18, rounded = 5, bg = C.panel, items = "center", justify = "center", icon(c.icon, 11, C.text) },
			text(c.name, 11.5, C.text, { weight = 500 }),
		}
	end
	return ui.overlay {
		on_hover = function(inside)
			if inside then
				hovered:set(c.key)
			elseif hovered:peek() == c.key then
				hovered:set("")
			end
		end,
		body,
		remove_badge(c),
	}
end

local function drop_zone(payload)
	local cards = {}
	for i, name in payload.names do
		local f = file_info(name)
		cards[i] = chip_view({ key = "drop:" .. name, kind = "file", name = name, meta = f.meta, icon = f.icon, tone = f.tone })
	end
	local n = #payload.names
	return ui.column {
		w_full = true, p = 10, gap = 10, rounded = 12, bg = "#FFFFFF0A", border = 1, border_color = "#FFFFFF2E",
		ui.row {
			w_full = true, gap = 10, items = "center",
			ui.div { w = 22, h = 22, rounded = 11, bg = C.text, items = "center", justify = "center", icon("arrow-down", 12, C.ink) },
			ui.column { gap = 2,
				text("Release to attach " .. n .. (n == 1 and " file" or " files"), 12.5, C.text, { weight = 500 }),
				mono("Up to 20 files · 50 MB each", 9.5, C.muted) },
			spacer(),
			kbd("esc"), text("Cancel", 11.5, C.muted),
		},
		ui.row { gap = 6, flex_wrap = true, cards },
	}
end

local function round_button(id, name, size, props)
	props.id = id
	props.w, props.h, props.rounded = 34, 34, 17
	props.justify = "center"
	table.insert(props, icon(name, size, props.tint or C.secondary))
	props.tint = nil
	return button(props, 0.92)
end

local function input_box(s)
	local function ready()
		return query:get() ~= ""
	end
	return ui.column {
		w_full = true, p = 6, gap = 4, bg = C.well, corners = { 0, 0, 18, 18 }, border = 1, border_color = C.dock_line,
		ui.presence(drop, function(payload)
			return drop_zone(payload)
		end, { layout = "height", layout_clip = true, enter = { opacity = { 0, 1 }, duration = 160 }, exit = { opacity = 0, duration = 120 } }),
		ui.row {
			w_full = true, pt = 4, px = 4, pb = 2, gap = 6, flex_wrap = true, items = "center",
			hidden = function() return #chips:get() == 0 or drop:get() ~= nil end,
			ui.each(chips, chip_view, { key = function(c) return c.key end }),
		},
		ui.row {
			w_full = true, h = 36, gap = 8, items = "center",
			round_button("add", "plus", 16, { bg = C.wash, hover = { bg = "#FFFFFF1F" }, tint = C.soft, on_click = attach_pick }),
			ui.overlay {
				grow = 1, shrink = 1, min_w = 0, h_full = true, pl = 4,
				ui.text {
					function()
						if listening:get() then
							return "Listening…"
						end
						return mode:get() == "answer" and "Ask a follow-up…" or s.placeholder
					end,
					absolute = true, left = 4, top = 8, font = SANS, text_size = 15,
					color = function() return listening:get() and C.error or C.hint end,
					hidden = function() return query:get() ~= "" end,
				},
				ui.input {
					id = "qa_input", label = s.placeholder, value = query, w_full = true, h_full = true, p = 0,
					border = 0, bg = C.clear, text_size = 15, color = C.text, font = SANS, focus = { border_color = C.clear },
					on_mount = function(el)
						input_el = el
						el:focus()
					end,
					on_key = handle,
					on_focus = function() input_focused = true end,
					on_blur = function() input_focused = false end,
				},
			},
			round_button("mic", "mic", 15, {
				bg = function() return listening:get() and "#E8A89829" or C.clear end,
				tint = function() return listening:get() and C.error or C.secondary end,
				on_click = dictate,
			}),
			button({
				id = "send", w = 34, h = 34, rounded = 17, justify = "center",
				bg = function() return ready() and C.text or "#3A3934" end,
				hover = {},
				on_click = send,
				icon("arrow-up", 16, function() return ready() and C.ink or C.tertiary end),
			}, 0.92),
		},
	}
end

local function row_view(r)
	if r.header then
		return ui.row { w_full = true, pt = 8, px = 10, pb = 4, mono(r.header, 10, C.hint, { letter_spacing = 0.6 }) }
	end
	local function on()
		return sel:get() == r.n
	end
	local action = r.kind == "google" and "Search" or "Ask"
	return button({
		id = "row_" .. r.n, w_full = true, h = 34, px = 10, gap = 10, rounded = 7,
		bg = function() return on() and C.wash or C.clear end,
		hover = { bg = "#FFFFFF0D" },
		on_hover = function(inside)
			if inside then
				sel:set(r.n)
			end
		end,
		on_click = function() run(r) end,
		r.orb and ui.div { w = 20, h = 20, items = "center", justify = "center", orb(12, C.text) }
			or ui.div { w = 20, h = 20, rounded = 5, bg = C.raised, items = "center", justify = "center",
				icon(r.icon, 13, C.secondary) },
		text(r.live and function() return query:get() end or r.title, 13, C.text, { truncate = true, shrink = 1, min_w = 0 }),
		r.detail and text(r.detail, 12.5, C.muted, { shrink = 0 }) or nil,
		spacer(),
		ui.text { function() return (on() or r.kind == "google") and action or "" end,
			font = MONO, text_size = 10.5, color = C.tertiary },
		ui.row {
			h = 18, px = 5, rounded = 4, border = 1, border_color = C.key_line, items = "center",
			ui.text { function() return on() and "↵" or "⌘" .. r.n end, font = MONO, text_size = 10.5, color = C.tertiary },
		},
	}, 0.99)
end

local function suggestions()
	return ui.column {
		w_full = true, gap = 1, pb = 2,
		ui.each(items, row_view, { key = function(it) return it.key end }),
	}
end

local function source_chip(src)
	return ui.row {
		h = 24, pl = 6, pr = 9, gap = 6, rounded = 12, items = "center", cursor = "pointer",
		bg = C.elevated, border = 1, border_color = "#2E2D29", hover = { bg = C.raised },
		on_click = function() show_toast("Opening " .. src[1]) end,
		ui.div { w = 12, h = 12, rounded = 6, bg = src[2] },
		text(src[1], 11.5, C.secondary),
	}
end

local function answer_action(id, name, label, keys, primary, on_click)
	return button({
		id = id, h = 28, rounded = 14, pl = 9, pr = 5, gap = 6,
		bg = primary and C.text or C.clear, hover = { bg = primary and "#FFFFFF" or C.wash },
		on_click = on_click,
		icon(name, 13, primary and C.ink or C.secondary),
		text(label, 12.5, primary and C.ink or C.soft, { weight = 500 }),
		kbd(keys, primary and { line = "#1212113D", color = "#121211B3" } or nil),
	}, 0.96)
end

local function turn_view(t)
	local s = scp()
	local function last()
		local list = turns:get()
		return list[#list] and list[#list].id == t.id
	end
	local body
	if t.done then
		local sources = {}
		for i, src in s.sources do
			sources[i] = source_chip(src)
		end
		body = {
			ui.row { gap = 8, items = "center",
				orb(14, C.done),
				mono(string.format("Answered in %.1fs · %d sources", t.ms or 1.4, #s.sources), 10.5, C.hint) },
			ui.text { t.a, font = SANS, text_size = 14, color = C.text, wrap = true, line_height = 21, w_full = true },
			ui.row { gap = 6, items = "center", flex_wrap = true, sources },
			ui.row {
				w_full = true, gap = 6, pt = 2, items = "center", hidden = function() return not last() end,
				answer_action("paste", "corner-down-left", "Paste into " .. s.app, "⌘↵", true, paste),
				answer_action("open_in_ask", "maximize-2", "Open in Ask", "⌘O", false, open_in_ask),
				answer_action("copy", "copy", "Copy", "⌘⇧C", false, copy_answer),
			},
		}
	else
		body = {
			ui.row { gap = 8, items = "center",
				orb(14, C.text, true),
				mono(s.reading .. " · " .. model:peek(), 10.5, C.hint),
			},
			ui.column {
				w_full = true, gap = 7,
				ui.div { w = "92%", h = 9, rounded = 4, bg = "#FFFFFF0F" },
				ui.div { w = "74%", h = 9, rounded = 4, bg = "#FFFFFF0A" },
			},
		}
	end
	return ui.column {
		w_full = true, gap = 12,
		ui.row { w_full = true, justify = "end",
			ui.div { max_w = 460, px = 13, py = 8, rounded = 16, bg = "#232321",
				ui.text { t.q, font = SANS, text_size = 13.5, color = C.text, wrap = true } } },
		body,
	}
end

-- Scroll views default to 240 points tall: the thread takes its content's
-- height, up to 330, and scrolls past that.
local thread_h = ui.state("thread_h", 0)
local function thread()
	return ui.scroll {
		id = "thread", w_full = true, follow = true,
		h = function() return math.min(thread_h:get(), 330) end,
		ui.column {
			w_full = true, gap = 20, pt = 12, px = 14, pb = 10,
			on_resize = function(size) thread_h:set(size.h) end,
			ui.each(turns, turn_view, { key = function(t) return t.id .. (t.done and ":done" or ":thinking") end }),
		},
	}
end

local function menu_row(id, label, keys, on_click, checked)
	return button({
		id = id, w_full = true, h = 30, px = 8, gap = 8, rounded = 6, hover = { bg = C.selected },
		on_click = on_click,
		text(label, 12.5, C.text), spacer(),
		checked and ui.text { function() return checked() and "✓" or "" end, text_size = 12, color = C.secondary } or nil,
		keys and kbd(keys) or nil,
	}, 0.98)
end

local MODELS = {
	{ "Instant", "Fastest, for quick lookups" },
	{ "Auto", "Picks the depth per question" },
	{ "Think", "Reasons before answering" },
	{ "Deep", "Researches across sources" },
}

local function model_picker()
	local options = {}
	for i, m in MODELS do
		options[i] = button({
			id = "model_" .. m[1], w_full = true, px = 8, py = 6, gap = 8, rounded = 6, hover = { bg = C.selected },
			on_click = function()
				model:set(m[1])
				model_open:set(false)
				refocus()
			end,
			ui.column { grow = 1, gap = 2, text(m[1], 12.5, C.text), text(m[2], 11, C.muted) },
			ui.text { function() return model:get() == m[1] and "✓" or "" end, text_size = 12, color = C.secondary },
		}, 0.98)
	end
	return ui.popover {
		open = model_open, w = 236, p = 5, gap = 1, bg = C.elevated, rounded = 10, border = 1, border_color = C.strong,
		shadow = { y = 12, blur = 32, color = "#00000080" },
		anchor = button({
			id = "model", h = 28, rounded = 13, pl = 8, pr = 4, gap = 6,
			on_click = function()
				actions_open:set(false)
				model_open:set(not model_open:peek())
			end,
			icon("zap", 12, C.secondary),
			ui.text { function() return model:get() end, font = SANS, text_size = 12.5, color = C.secondary },
			icon("chevron-down", 11, C.muted),
			kbd("⌘."),
		}, 0.97),
		options,
	}
end

local function actions_menu()
	local function target()
		return rows()[sel:peek()]
	end
	return ui.popover {
		open = actions_open, w = 250, p = 5, gap = 1, bg = C.elevated, rounded = 10, border = 1, border_color = C.strong,
		shadow = { y = 12, blur = 32, color = "#00000080" },
		anchor = button({
			id = "actions", h = 28, rounded = 13, pl = 8, pr = 4, gap = 6,
			on_click = function()
				model_open:set(false)
				actions_open:set(not actions_open:peek())
			end,
			text("Actions", 12.5, C.secondary),
			kbd("⌘K"),
		}, 0.97),
		menu_row("action_run", "Run selected row", "↵", function()
			actions_open:set(false)
			run(target())
		end),
		menu_row("action_google", "Search Google instead", "⌘↵", function()
			actions_open:set(false)
			local r = target()
			google(r and r.title or query:peek())
		end),
		menu_row("action_copy", "Copy prompt", nil, function()
			actions_open:set(false)
			show_toast("Prompt copied")
		end),
		menu_row("action_pin", "Pin to suggestions", nil, function()
			actions_open:set(false)
			show_toast("Pinned to " .. scp().app .. " suggestions")
		end),
	}
end

local function footer()
	return ui.row {
		w_full = true, h = 28, px = 4, gap = 4, items = "center",
		button({
			id = "destination", h = 28, rounded = 14, pl = 8, pr = 4, gap = 7,
			on_click = flip_dest,
			ui.switch(dest, { ask = orb(12, C.text), google = icon("search", 12, C.text) }),
			ui.text { function() return dest:get() == "ask" and "Ask" or "Google" end,
				font = SANS, text_size = 12.5, weight = 500, color = C.text },
			icon("chevron-down", 11, C.muted),
			kbd("⇥"),
		}, 0.97),
		button({
			id = "alternate", h = 28, px = 8, gap = 6, rounded = 14,
			on_click = alternate,
			kbd("⌘↵", { h = 20, px = 6, size = 10.5, color = C.secondary }),
			ui.text {
				function()
					if mode:get() == "answer" and query:get() == "" then
						return "Paste into " .. sc().app
					end
					return dest:get() == "ask" and "Search Google" or "Ask"
				end,
				font = SANS, text_size = 12, color = C.secondary,
			},
		}, 0.97),
		spacer(),
		model_picker(),
		actions_menu(),
	}
end

local function panel(s)
	return ui.column {
		id = "quick_ask", w = 640, p = 8, gap = 6,
		bg = C.glass, rounded = 26, border = 1, border_color = C.glass_line, backdrop_blur = 40,
		shadow = { { y = 24, blur = 60, color = "#00000066" }, { y = 2, blur = 6, color = "#00000033" } },
		layout = "height", layout_duration = 0.32,
		-- Clicks on the glass stay on it rather than closing it.
		on_press = function() end,
		on_drag_over = function(payload)
			if not drop:peek() then
				drop:set(payload)
			end
		end,
		on_drag_leave = function()
			drop:set(nil)
		end,
		on_drop = function(payload)
			drop:set(nil)
			attach(payload.names)
			show_toast("Attached " .. #payload.names .. (#payload.names == 1 and " file" or " files"))
		end,
		ui.show(function() return mode:get() == "answer" end, thread()),
		ui.column { w_full = true, dock(s), input_box(s) },
		ui.show(function() return mode:get() == "compose" end, suggestions()),
		footer(),
	}
end

----------------------------------------------------------------------------
-- The desktop under it
----------------------------------------------------------------------------

local function menu_bar(s)
	local menus = {}
	for i, m in s.menus do
		menus[i] = text(m, 12.5, "#FFFFFFD9")
	end
	return ui.row {
		absolute = true, top = 0, left = 0, right = 0, h = 26, px = 14, gap = 18, items = "center",
		bg = "#0000004D", backdrop_blur = 20,
		icon("apple", 12, "#FFFFFF"),
		text(s.menu, 12.5, "#FFFFFF", { weight = 600 }),
		menus,
		spacer(),
		button({
			id = "status_item", w = 22, h = 18, rounded = 4, justify = "center", hover = { bg = "#FFFFFF26" },
			bg = function() return open:get() and "#FFFFFF33" or C.clear end,
			on_click = function()
				if open:peek() then
					close_panel(false)
				else
					open:set(true)
				end
			end,
			orb(12, "#FFFFFF"),
		}, 0.9),
		icon("wifi", 13, "#FFFFFF"),
		icon("battery", 15, "#FFFFFF"),
		text("Sat 17 Oct  9:41", 12.5, "#FFFFFF"),
	}
end

local function drag_preview(payload)
	local n = #payload.names
	return ui.overlay {
		w = 48, h = 56,
		ui.column {
			absolute = true, top = 4, left = 4, w = 36, h = 46, rounded = 4, bg = "#FFFFFF", justify = "end", pb = 8, px = 5,
			shadow = { y = 6, blur = 16, color = "#00000059" }, rotate = -4,
			ui.div { w_full = true, h = 4, rounded = 1, bg = file_info(payload.names[1]).tone },
		},
		n > 1 and ui.div {
			absolute = true, top = -2, right = 0, w = 18, h = 18, rounded = 9, bg = "#E5484D",
			border = 1.5, border_color = "#FFFFFF", items = "center", justify = "center",
			text(tostring(n), 10, "#FFFFFF", { weight = 600 }),
		} or nil,
	}
end

local function stage(i)
	local s = SCENES[i]
	local look = apps.frame[s.id]
	local ctx = {
		files = D.files,
		picked = picked,
		pasted = pasted,
		selection_on = function() return caps:get()[2] end,
		drag = function(name)
			local list = picked:peek()
			local names = table.find(list, name) and table.clone(list) or { name }
			if not open:peek() then
				open:set(true)
			end
			if mode:peek() == "answer" then
				mode:set("compose")
			end
			return { names = names }
		end,
		drag_preview = drag_preview,
		on_drag_end = function()
			drop:set(nil)
		end,
	}
	return ui.overlay {
		w_full = true, h_full = true, rounded = 12, overflow = "hidden", border = 1, border_color = C.line,
		gradient = { angle = 170, stops = { { 0, "#2B2A3A" }, { 0.55, "#5A3F3A" }, { 1, "#B07A55" } } },
		ui.div {
			absolute = true, top = 50, left = 44, right = 44, bottom = -40, rounded = 10, overflow = "hidden",
			bg = look.bg, border = 1, border_color = look.line, border_over = true,
			shadow = { y = 20, blur = 50, color = "#00000066" },
			apps.view(s.id, ctx),
		},
		ui.presence(function() return open:get() or nil end, function()
			-- A click outside closes it; over Finder the files stay live under
			-- the dim, to be picked and dragged onto the panel.
			local finder = s.id == "finder"
			return ui.div {
				w_full = true, h_full = true, bg = "#00000040",
				-- The wrapper lets the pointer through; the dim takes it back to
				-- close the panel, except over Finder.
				pointer_events = finder and "none" or "auto",
				on_click = not finder and function() close_panel(false) end or nil,
			}
		end, {
			-- Drag hit testing doesn't skip pointer-through nodes: the
			-- wrapper is hidden while there is nothing in it.
			absolute = true, top = 0, left = 0, right = 0, bottom = 0, pointer_events = "none",
			hidden = function() return not open:get() end,
			enter = { opacity = { 0, 1 }, duration = 180 }, exit = { opacity = 0, duration = 160 },
		}),
		menu_bar(s),
		ui.presence(function() return open:get() or nil end, function()
			-- Centred without a full-width wrapper, which would take the
			-- clicks and drops beside it.
			return ui.div { absolute = true, top = s.panel_top or 66, left = "50%", x = -320, panel(s) }
		end, {
			-- No scale: it would promote the frosted panel to a cached layer,
			-- repainting the blur into a texture on every step.
			enter = { opacity = { 0, 1 }, y = { -10, 0 }, duration = 240, ease = "outQuart" },
			exit = { opacity = 0, y = -6, duration = 140, ease = "inQuad" },
			initial = true,
		}),
		ui.presence(toast, function(t)
			return ui.row {
				absolute = true, bottom = 28, left = 0, right = 0, justify = "center", pointer_events = "none",
				ui.row {
					h = 36, px = 14, gap = 8, rounded = 18, items = "center",
					bg = "#1F1F1DEB", border = 1, border_color = C.glass_line, backdrop_blur = 20,
					shadow = { y = 10, blur = 30, color = "#00000066" },
					icon("check", 13, C.done), text(t.text, 12.5, C.text),
				},
			}
		end, {
			enter = { opacity = { 0, 1 }, y = { 10, 0 }, duration = 220, ease = "outQuart" },
			exit = { opacity = 0, y = 6, duration = 160 },
		}),
		ui.show(function() return not open:get() end, ui.row {
			absolute = true, top = 40, right = 16, gap = 6, items = "center", px = 10, h = 28, rounded = 14,
			bg = "#00000059", pointer_events = "none",
			kbd("⌥Space", { color = "#FFFFFFCC", line = "#FFFFFF40" }),
			text("to summon Quick ask", 12, "#FFFFFFCC"),
		}),
	}
end

----------------------------------------------------------------------------
-- The keyboard map
----------------------------------------------------------------------------

local KEYMAP = {
	{ "SUMMON & DISMISS", { { "⌥Space", "Open / close from any app" }, { "esc", "Close, keep draft" },
		{ "⌘W", "Close and clear draft" }, { "⌘N", "New question, drop thread" } } },
	{ "SEND", { { "↵", "Ask" }, { "⌘↵", "Search Google (or Paste on an answer)" }, { "⇥", "Flip default destination" },
		{ "⇧↵", "New line" } } },
	{ "CONTEXT", { { "⌥1", "Page / File / Message" }, { "⌥2", "Selection" }, { "⌥3", "Screen" },
		{ "⌫", "Remove last chip (empty input)" }, { "⌘U", "Attach file" }, { "drag", "Drop files onto the panel" } } },
	{ "RESULTS", { { "↑ ↓", "Move selection" }, { "⌘1–9", "Run row directly" }, { "⌘K", "Actions for selected row" },
		{ "⌘.", "Model picker" }, { "⌘D", "Dictate" } } },
	{ "ANSWER", { { "⌘↵", "Paste into frontmost app" }, { "⌘O", "Open thread in Ask" }, { "⌘⇧C", "Copy answer" },
		{ "⌘[", "Back to results" } } },
}

local function keymap_view()
	local cols = {}
	for i, col in KEYMAP do
		local lines = {}
		for j, entry in col[2] do
			lines[j] = ui.row { gap = 10, items = "center",
				ui.row { w = 58, kbd(entry[1]) }, text(entry[2], 12, C.body, { wrap = true, shrink = 1 }) }
		end
		cols[i] = ui.column { grow = 1, basis = 0, gap = 10, mono(col[1], 10, C.hint, { letter_spacing = 0.6, mb = 2 }), lines }
	end
	return ui.column {
		w_full = true, h_full = true, bg = "#000000B3", items = "center", justify = "center",
		pointer_events = "auto", on_click = function() keymap:set(false) end,
		ui.column {
			id = "keymap", w = 1080, p = 28, gap = 22, rounded = 16, bg = C.panel, border = 1, border_color = C.line,
			shadow = { y = 30, blur = 80, color = "#000000A0" },
			on_press = function() end,
			ui.row { gap = 12, items = "center",
				text("Keyboard map", 15, C.text, { weight = 500 }),
				text("Every control has a key — hands never leave the keyboard", 12.5, C.muted),
				spacer(), kbd("esc"), text("Close", 12, C.muted) },
			ui.row { w_full = true, gap = 28, items = "start", cols },
		},
	}
end

----------------------------------------------------------------------------
-- The window
----------------------------------------------------------------------------

local function scene_tab(i)
	local s = SCENES[i]
	local function on()
		return scene:get() == i
	end
	return button({
		id = "scene_" .. s.id, h = 28, pl = 6, pr = 10, gap = 7, rounded = 8,
		bg = function() return on() and C.selected or C.clear end,
		hover = { bg = C.raised },
		on_click = function() enter_scene(i) end,
		app_tile(s.id, 16),
		ui.text { s.app, font = SANS, text_size = 12.5, color = function() return on() and C.text or C.secondary end },
	}, 0.96)
end

local tabs = {}
for i = 1, #SCENES do
	tabs[i] = scene_tab(i)
end

return ui.overlay {
	id = "quick_ask_lab", w_full = true, h_full = true, bg = C.window, color = C.text, font = SANS, text_size = 13,
	focusable = true, focus = {},
	on_mount = function(el) root_el = el end,
	on_key = handle,
	ui.column {
		w_full = true, h_full = true,
		ui.row {
			w_full = true, h = 52, pl = 88, pr = 20, gap = 14, items = "center",
			ui.row { h_full = true, gap = 12, items = "center", on_press = ui.action("drag_window"),
				mono("QUICK ASK", 10, C.muted, { letter_spacing = 1.2 }),
				text("Ask from anywhere", 14, C.text),
			},
			ui.row { gap = 2, ml = 10, tabs },
			ui.div { grow = 1, h_full = true, on_press = ui.action("drag_window") },
			kbd("⌥Space"), text("summon", 12, C.muted),
			button({
				id = "keymap_button", h = 28, px = 10, gap = 7, rounded = 8, hover = { bg = C.raised },
				on_click = function() keymap:set(true) end,
				icon("keyboard", 14, C.secondary), text("Keyboard map", 12.5, C.secondary), kbd("⌘/"),
			}, 0.96),
		},
		ui.div {
			grow = 1, shrink = 1, min_h = 0, w_full = true, px = 20, pb = 20,
			-- The desktop swaps at once: crossfading two of them redraws both
			-- frosted panels every frame. The new panel fades in instead.
			ui.presence(scene, stage, { w_full = true, h_full = true }),
		},
	},
	ui.presence(function() return keymap:get() or nil end, keymap_view, {
		absolute = true, top = 0, left = 0, right = 0, bottom = 0, pointer_events = "none",
		hidden = function() return not keymap:get() end,
		enter = { opacity = { 0, 1 }, duration = 160 },
		exit = { opacity = 0, duration = 120 },
	}),
}
