-- A terminal multiplexer: tabs in the titlebar, each a tree of split panes
-- running Ghostty terminals. Panes resize by their dividers and move by the
-- grip in their top padding, onto another pane's edge or onto a tab; tabs
-- reorder by dragging. Everything here is state that survives a reload,
-- and the shells keep running through one.
local theme = require("./theme")
local icons = require("./icons")
local names = require("./names")
local layout = require("./layout")

local home: string = ui.shared("home", ""):peek()
local launch_dir: string = ui.shared("launch_dir", ""):peek()
local window_radius = ui.shared("window_radius", 10)

local next_id = ui.state("next_id", 1)
-- { { id = "t1", root = <layout> } }, in strip order.
local tabs = ui.state("tabs", {})
local active = ui.state("active", "")
-- Tab id -> its focused pane.
local focus = ui.state("focus", {})
-- Split id -> the first side's share (0 to 1).
local ratios = ui.state("ratios", {})
-- Tab id -> the pane shown alone.
local zoom = ui.state("zoom", {})
-- "pane:<id>" or "tab:<id>" awaiting confirmation to close.
local confirm = ui.state("confirm", "")
local hovered_tab = ui.state("hovered_tab", "")
-- Where a drag would land: { pane, zone } over a pane, { tab, side } over a tab.
local drop = ui.state("drop", {})
-- What is being dragged: { kind = "pane" | "tab", id }, or {}.
local dragging = ui.state("dragging", {})
-- The tab a dragged pane hovers (dropping moves it there).
local tab_drop = ui.state("tab_drop", "")
-- Where a dragged pane would open as a tab of its own: { pane, before }
-- (the tab it goes before, "" for the end), or {}.
local tab_slot = ui.state("tab_slot", {})

-- State values are read-only: changes build new tables.
local function copy(t: { [any]: any }): { [any]: any }
	local out = {}
	for k, v in t do
		out[k] = v
	end
	return out
end

local function fresh(prefix: string): string
	local n = next_id:peek()
	next_id:set(n + 1)
	return prefix .. n
end

local function tab_index(id: string): number?
	for i, tab in tabs:peek() do
		if tab.id == id then
			return i
		end
	end
	return nil
end
local function tab_of(pane: string): string?
	for _, tab in tabs:peek() do
		if layout.contains(tab.root, pane) then
			return tab.id
		end
	end
	return nil
end
local function root_of(id: string)
	local i = tab_index(id)
	return if i then tabs:peek()[i].root else nil
end
local function set_root(id: string, root)
	local list = {}
	for _, tab in tabs:peek() do
		table.insert(list, if tab.id == id then { id = tab.id, root = root } else tab)
	end
	tabs:set(list)
end
local function set_map(state, key: string, value)
	local map = copy(state:peek())
	map[key] = value
	state:set(map)
end

-- Sessions ----------------------------------------------------------------

-- Where a new pane's shell starts, until it has.
local spawn_dir: { [string]: string } = {}
local close_pane: (pane: string, force: boolean?) -> ()

local function session(pane: string)
	return ui.term(pane, {
		cwd = spawn_dir[pane],
		-- The process exited (no question), or a close binding asked.
		on_close = function(alive)
			close_pane(pane, not alive)
		end,
	})
end
local function describe(pane: string?): names.Description
	if not pane then
		return { name = "", icon = "terminal", busy = false }
	end
	return names.describe(ui.term(pane), home)
end
-- The directory a pane's shell is in, for what opens next to it.
local function cwd_of(pane: string?): string
	if pane then
		local pwd = names.path_of(ui.term(pane).pwd)
		if pwd ~= "" then
			return pwd
		end
	end
	return launch_dir
end

-- Focus and tabs ------------------------------------------------------------

local function focus_pane(tab: string, pane: string)
	if (focus:peek())[tab] ~= pane then
		set_map(focus, tab, pane)
	end
	if active:peek() == tab then
		ui.term(pane):focus()
	end
end
local function activate(tab: string)
	active:set(tab)
	local pane = (focus:peek())[tab]
	if pane then
		ui.term(pane):focus()
	end
end
local function new_tab(cwd: string, pane: string?, before: string?)
	local id = fresh("t")
	if not pane then
		pane = fresh("p")
		spawn_dir[pane :: string] = cwd
		session(pane :: string)
	end
	local list = copy(tabs:peek())
	local at = #list + 1
	for i, tab in list do
		if tab.id == before then
			at = i
		end
	end
	table.insert(list, at, { id = id, root = layout.pane(pane :: string) })
	tabs:set(list)
	set_map(focus, id, pane)
	activate(id)
end
local function current(): string?
	return (focus:peek())[active:peek()]
end

local function split(axis: string)
	local tab = active:peek()
	local pane = current()
	local root = root_of(tab)
	if not pane or not root then
		return
	end
	local new = fresh("p")
	spawn_dir[new] = cwd_of(pane)
	session(new)
	set_map(zoom, tab, nil)
	set_root(tab, layout.split(root, pane, axis, new, fresh("s")))
	focus_pane(tab, new)
end

-- Take `pane` out of its tab without ending its session; the tab goes when
-- it was its last pane. Returns whether it was found.
local function detach(pane: string): boolean
	local tab = tab_of(pane)
	if not tab then
		return false
	end
	local root, near = layout.remove(root_of(tab), pane)
	if (zoom:peek())[tab] == pane then
		set_map(zoom, tab, nil)
	end
	if root then
		set_root(tab, root)
		if (focus:peek())[tab] == pane then
			focus_pane(tab, near or layout.leaves(root)[1])
		end
		return true
	end
	-- Its last pane: the tab goes, a neighbour takes over.
	local i = tab_index(tab) :: number
	local remaining = copy(tabs:peek())
	table.remove(remaining, i)
	tabs:set(remaining)
	set_map(focus, tab, nil)
	local list = tabs:peek()
	if #list == 0 then
		active:set("")
	elseif active:peek() == tab then
		activate(list[math.min(i, #list)].id)
	end
	return true
end

close_pane = function(pane: string, force: boolean?)
	if not force and ui.term(pane):needs_confirm() then
		ui.term(pane):blur()
		confirm:set("pane:" .. pane)
		return
	end
	if confirm:peek() == "pane:" .. pane then
		confirm:set("")
	end
	detach(pane)
	ui.term(pane):close()
	if #tabs:peek() == 0 then
		ui.invoke("close_window")
	end
end

local function close_tab(tab: string, force: boolean?)
	local root = root_of(tab)
	if not root then
		return
	end
	local panes = layout.leaves(root)
	if not force then
		for _, pane in panes do
			if ui.term(pane):needs_confirm() then
				ui.term(pane):blur()
				confirm:set("tab:" .. tab)
				return
			end
		end
	end
	confirm:set("")
	for _, pane in panes do
		close_pane(pane, true)
	end
end

-- Move `pane` beside `target` (zone "left", "right", "top" or "bottom").
local function move_pane(pane: string, target: string, zone: string)
	if pane == target or not tab_of(target) then
		return
	end
	detach(pane)
	local tab = tab_of(target) :: string
	local axis = if zone == "left" or zone == "right" then "row" else "column"
	local before = zone == "left" or zone == "top"
	set_root(tab, layout.split(root_of(tab), target, axis, pane, fresh("s"), before))
	set_map(zoom, tab, nil)
	active:set(tab)
	focus_pane(tab, pane)
end
-- Move `pane` into `tab`, beside its focused pane.
-- Preview `pane` opening as a tab before `before` ("" : at the end).
local function show_slot(pane: string, before: string)
	local s = tab_slot:peek()
	if s.pane ~= pane or s.before ~= before then
		tab_slot:set({ pane = pane, before = before })
	end
	if tab_drop:peek() ~= "" then
		tab_drop:set("")
	end
end
local function hide_slot()
	if (tab_slot:peek()).pane then
		tab_slot:set({})
	end
end
-- Open the previewed tab: the pane leaves its tab for one of its own.
local function commit_slot(): boolean
	local s = tab_slot:peek()
	tab_slot:set({})
	if not s.pane then
		return false
	end
	-- Its tab may go with it (its last pane): place it by a neighbour.
	local before = s.before
	if before == tab_of(s.pane) and #layout.leaves(root_of(before)) == 1 then
		return true
	end
	if detach(s.pane) then
		new_tab("", s.pane, before)
	end
	return true
end
local function move_pane_to_tab(pane: string, tab: string)
	local target = (focus:peek())[tab]
	if not target or tab_of(pane) == tab then
		activate(tab)
		return
	end
	move_pane(pane, target, "right")
end
-- Tabs' widths and elements, for sliding them when the order changes.
local tab_widths: { [string]: number } = {}
local tab_elements: { [string]: any } = {}
local function tab_positions(list): { [string]: number }
	local out, x = {}, 0
	for _, tab in list do
		out[tab.id] = x
		x += (tab_widths[tab.id] or 0) + 3
	end
	return out
end
-- Move `tab` before or after `target` now, the others sliding to make room.
local function reorder(tab: string, target: string, side: string)
	local list = tabs:peek()
	local next = {}
	local moved
	for _, item in list do
		if item.id == tab then
			moved = item
		else
			table.insert(next, item)
		end
	end
	if not moved then
		return
	end
	for i, item in next do
		if item.id == target then
			table.insert(next, if side == "before" then i else i + 1, moved)
			break
		end
	end
	local same = true
	for i, item in next do
		if list[i].id ~= item.id then
			same = false
		end
	end
	if same then
		return
	end
	local before, after = tab_positions(list), tab_positions(next)
	tabs:set(next)
	for id, element in tab_elements do
		local delta = (before[id] or 0) - (after[id] or 0)
		if delta ~= 0 then
			anime.animate(element, { x = { delta, 0 }, duration = 240, ease = "outQuint" })
		end
	end
end

local function step_pane(by: number)
	local root = root_of(active:peek())
	local pane = current()
	if not root or not pane then
		return
	end
	local leaves = layout.leaves(root)
	local i = table.find(leaves, pane) or 1
	focus_pane(active:peek(), leaves[(i - 1 + by) % #leaves + 1])
end
local function step_tab(by: number)
	local list = tabs:peek()
	local i = tab_index(active:peek()) or 1
	if #list > 0 then
		activate(list[(i - 1 + by) % #list + 1].id)
	end
end
local function go(direction: string)
	local root = root_of(active:peek())
	local pane = current()
	local other = root and pane and layout.neighbor(root, pane, direction)
	if other then
		focus_pane(active:peek(), other)
	end
end

-- Start: one tab where the app was launched; every session's handlers are
-- this run's.
if #tabs:peek() == 0 then
	new_tab(launch_dir)
else
	for _, tab in tabs:peek() do
		for _, pane in layout.leaves(tab.root) do
			session(pane)
		end
	end
	activate(if tab_index(active:peek()) then active:peek() else tabs:peek()[1].id)
end

-- Commands (the menus) --------------------------------------------------------

local function command(id: string, title: string, menu: string, key: string?, run: () -> ())
	ui.command({ id = id, title = title, menu = menu, key = key, run = run })
end
command("new_tab", "New Tab", "Shell", "cmd+t", function()
	new_tab(cwd_of(current()))
end)
command("split_right", "Split Right", "Shell", "cmd+d", function()
	split("row")
end)
command("split_down", "Split Down", "Shell", "cmd+shift+d", function()
	split("column")
end)
ui.command({ menu = "Shell", separator = true })
command("close_pane", "Close Pane", "Shell", "cmd+w", function()
	local pane = current()
	if pane then
		close_pane(pane)
	end
end)
command("close_tab", "Close Tab", "Shell", "cmd+alt+w", function()
	close_tab(active:peek())
end)

local function binding(action: string)
	return function()
		local pane = current()
		if pane then
			ui.term(pane):binding(action)
		end
	end
end
command("copy", "Copy", "Edit", "cmd+c", binding("copy_to_clipboard"))
command("paste", "Paste", "Edit", "cmd+v", binding("paste_from_clipboard"))
command("select_all", "Select All", "Edit", "cmd+a", binding("select_all"))
ui.command({ menu = "Edit", separator = true })
command("clear", "Clear", "Edit", "cmd+k", binding("clear_screen"))

command("zoom_pane", "Zoom Pane", "View", "cmd+shift+enter", function()
	local tab, pane = active:peek(), current()
	if pane then
		set_map(zoom, tab, if (zoom:peek())[tab] == pane then nil else pane)
		ui.term(pane):focus()
	end
end)
command("equalize", "Equalize Panes", "View", "cmd+ctrl+=", function()
	local root = root_of(active:peek())
	if root then
		local map = copy(ratios:peek())
		for _, id in layout.splits(root) do
			map[id] = nil
		end
		ratios:set(map)
	end
end)
ui.command({ menu = "View", separator = true })
command("bigger", "Bigger", "View", "cmd+=", binding("increase_font_size:1"))
command("smaller", "Smaller", "View", "cmd+-", binding("decrease_font_size:1"))
command("actual_size", "Actual Size", "View", "cmd+0", binding("reset_font_size"))

command("next_pane", "Next Pane", "Window", "cmd+]", function()
	step_pane(1)
end)
command("previous_pane", "Previous Pane", "Window", "cmd+[", function()
	step_pane(-1)
end)
for _, direction in { "left", "right", "up", "down" } do
	command(
		"focus_" .. direction,
		"Focus Pane " .. direction:sub(1, 1):upper() .. direction:sub(2),
		"Window",
		"cmd+alt+arrow" .. direction,
		function()
			go(direction)
		end
	)
end
ui.command({ menu = "Window", separator = true })
command("next_tab", "Next Tab", "Window", "cmd+shift+]", function()
	step_tab(1)
end)
command("previous_tab", "Previous Tab", "Window", "cmd+shift+[", function()
	step_tab(-1)
end)
for n = 1, 9 do
	command("tab_" .. n, if n == 9 then "Last Tab" else "Tab " .. n, "Window", "cmd+" .. n, function()
		local list = tabs:peek()
		local tab = if n == 9 then list[#list] else list[n]
		if tab then
			activate(tab.id)
		end
	end)
end

-- The window's title follows the active tab (Mission Control, the Dock).
ui.effect(function()
	local name = describe((focus:get())[active:get()]).name
	ui.invoke("set_title", if name ~= "" then name else "Terminal")
end)

-- Views ----------------------------------------------------------------------

local CLEAR = "#00000000"

local function glyph(desc: () -> names.Description, size: number, dim: () -> boolean)
	return ui.overlay({
		w = size + 2,
		h = size + 2,
		shrink = 0,
		items = "center",
		justify = "center",
		ui.svg({
			src = function()
				return "code:" .. icons[desc().icon]
			end,
			hidden = function()
				return desc().glyph ~= nil
			end,
			w = size,
			h = size,
			tint = function()
				local d = desc()
				return d.tint or (if dim() then theme.tertiary else theme.secondary)
			end,
		}),
		ui.text({
			function()
				return desc().glyph or ""
			end,
			hidden = function()
				return desc().glyph == nil
			end,
			text_size = size,
			color = function()
				return desc().tint or theme.secondary
			end,
		}),
	})
end

local function tool(label: string, svg: string, on_click: () -> (), size: number?)
	return ui.button({
		label = label,
		w = size or 28,
		h = size or 28,
		rounded = 8,
		items = "center",
		justify = "center",
		shrink = 0,
		bg = CLEAR,
		hover = { bg = "#ffffff14" },
		focus = { border = 0 },
		on_click = on_click,
		ui.svg({ code = svg, w = 15, h = 15, tint = theme.secondary }),
	})
end

-- Empty titlebar space moves the window.
local function drag_space(props: { [string]: any })
	props.h_full = true
	props.on_press = ui.action("drag_window")
	return ui.div(props)
end

-- Context menus: frosted like the rest of the window.
local MENU_STYLE = {
	font = theme.font,
	w = 180,
	p = 4,
	gap = 0,
	rounded = 10,
	bg = "#24252a99",
	backdrop_blur = 30,
	border = 1,
	border_color = "#ffffff14",
	shadow = { y = 6, blur = 18, color = "#00000040" },
}
local function entry(label: string, run: () -> ())
	return ui.menu_item({
		label,
		h = 24,
		px = 8,
		rounded = 6,
		text_size = 12.5,
		color = theme.text,
		bg = CLEAR,
		hover = { bg = "#ffffff12" },
		focus = { bg = "#ffffff12", border = 0 },
		on_click = run,
	})
end
local function separator()
	return ui.div({ h = 1, mx = 8, my = 4, bg = "#ffffff0f" })
end

-- A pane's icon in a tab: on a little tile of its own, tinted for agents.
local function badge(pane: string, dim: () -> boolean)
	local desc = function()
		return describe(pane)
	end
	-- A row, not an overlay: an overlay stacks from its top left.
	return ui.row({
		w = 20,
		h = 20,
		shrink = 0,
		rounded = 5,
		items = "center",
		justify = "center",
		bg = function()
			local tint = desc().tint
			if tint then
				return tint .. (if dim() then "1f" else "2e")
			end
			return if dim() then "#ffffff0d" else "#ffffff17"
		end,
		border = 1,
		border_color = function()
			local tint = desc().tint
			return if tint then tint .. "33" else "#ffffff0f"
		end,
		glyph(desc, 12, dim),
	})
end

-- More panes than fit: "+3" on a tile like the icons'.
local function more_badge(count: number, dim: () -> boolean)
	return ui.row({
		h = 20,
		min_w = 20,
		px = 5,
		shrink = 0,
		rounded = 5,
		items = "center",
		justify = "center",
		bg = function()
			return if dim() then "#ffffff0d" else "#ffffff17"
		end,
		border = 1,
		border_color = "#ffffff0f",
		ui.text({
			`+{count}`,
			text_size = 10.5,
			weight = 600,
			color = function()
				return if dim() then theme.tertiary else theme.secondary
			end,
		}),
	})
end

-- A tab shows an icon per pane, three at most: past that, two and "+n".
local function tab_slots(id: string)
	return ui.computed(function()
		local root = nil
		for _, tab in tabs:get() do
			if tab.id == id then
				root = tab.root
			end
		end
		local panes = if root then layout.leaves(root) else {}
		local slots = {}
		for i, pane in panes do
			if #panes > 3 and i == 3 then
				table.insert(slots, { key = "more", more = #panes - 2 })
				break
			end
			table.insert(slots, { key = pane, pane = pane })
		end
		return slots
	end)
end

-- What follows the pointer while dragging a tab or a pane: the tab as it
-- shows active in the strip (a pane alone, its own icon). Blurred behind,
-- as the strip's glass is, so it reads over a terminal.
local function chip(pane: string?, tab: string?)
	local never = function()
		return false
	end
	local icons = if tab
		then ui.each(tab_slots(tab), function(slot)
			if slot.more then
				return more_badge(slot.more, never)
			end
			return badge(slot.pane, never)
		end, {
			key = function(slot)
				return if slot.more then `more{slot.more}` else slot.key
			end,
		})
		else (if pane then badge(pane, never) else nil)
	return ui.row({
		h = 28,
		pl = 4,
		pr = 11,
		gap = 7,
		items = "center",
		rounded = 8,
		max_w = 210,
		font = theme.font,
		bg = theme.wash,
		backdrop_blur = 24,
		border = 1,
		border_color = theme.line,
		ui.row({ gap = 3, shrink = 0, items = "center", icons }),
		ui.text({
			function()
				local name = describe(pane).name
				return if name ~= "" then name else "Terminal"
			end,
			shrink = 1,
			min_w = 0,
			text_size = 12.5,
			truncate = true,
			color = theme.text,
		}),
	})
end

local function tab_button(tab, _index)
	local id: string = tab.id
	local function pane(): string?
		return (focus:get())[id]
	end
	local function desc()
		return describe(pane())
	end
	local function is_active()
		return active:get() == id
	end
	local function hovered()
		return hovered_tab:get() == id
	end
	return ui.row({
		h = 28,
		-- The icons' tiles sit as far in from the tab's edge as from its
		-- top; the name, without a tile, a little more.
		pl = 4,
		pr = 11,
		gap = 7,
		items = "center",
		rounded = 8,
		max_w = 210,
		shrink = 1,
		cursor = "default",
		on_mount = function(element)
			tab_elements[id] = element
		end,
		on_resize = function(size)
			tab_widths[id] = size.w
		end,
		-- Where it was while dragged: a dim slot.
		opacity = function()
			local d = dragging:get()
			return if d.kind == "tab" and d.id == id then 0.35 else 1
		end,
		bg = function()
			if tab_drop:get() == id then
				return "#ffffff1f"
			end
			if is_active() then
				return theme.wash
			end
			return if hovered() then theme.hover else CLEAR
		end,
		border = 1,
		border_color = function()
			return if is_active() then theme.line else CLEAR
		end,
		on_click = function()
			activate(id)
		end,
		context_menu_style = MENU_STYLE,
		context_menu_below = true,
		context_menu = function()
			local others = #tabs:peek() > 1
			local items = {
				entry("New Tab", function()
					new_tab(cwd_of((focus:peek())[id]))
				end),
				separator(),
				entry("Close Tab", function()
					close_tab(id)
				end),
			}
			if others then
				table.insert(
					items,
					entry("Close Other Tabs", function()
						for _, tab in tabs:peek() do
							if tab.id ~= id then
								close_tab(tab.id)
							end
						end
					end)
				)
			end
			return items
		end,
		on_hover = function(inside)
			if inside then
				hovered_tab:set(id)
			elseif hovered_tab:peek() == id then
				hovered_tab:set("")
			end
		end,
		on_drag = function()
			dragging:set({ kind = "tab", id = id })
			return { kind = "tab", id = id }
		end,
		drag_preview = function()
			return chip(pane(), id)
		end,
		on_drag_end = function()
			dragging:set({})
			drop:set({})
		end,
		drop_when = function(payload)
			return payload.kind == "pane" or (payload.kind == "tab" and payload.id ~= id)
		end,
		-- A tab takes this one's place as it passes. A pane over its middle
		-- marks it (dropping moves the pane in); near an edge, a new tab
		-- opens up beside it.
		on_drag_over = function(payload, e)
			if payload.kind == "tab" then
				reorder(payload.id, id, if e.local_x < e.w / 2 then "before" else "after")
				return
			end
			local edge = math.min(28, e.w * 0.3)
			if e.local_x < edge then
				show_slot(payload.id, id)
			elseif e.local_x > e.w - edge then
				local list = tabs:peek()
				local after = ""
				for i, tab in list do
					if tab.id == id and list[i + 1] then
						after = list[i + 1].id
					end
				end
				show_slot(payload.id, after)
			else
				hide_slot()
				if tab_drop:peek() ~= id then
					tab_drop:set(id)
				end
			end
		end,
		on_drag_leave = function()
			if tab_drop:peek() == id then
				tab_drop:set("")
			end
		end,
		on_drop = function(payload)
			tab_drop:set("")
			if payload.kind == "pane" and not commit_slot() then
				move_pane_to_tab(payload.id, id)
			end
		end,
		ui.row({
			gap = 3,
			shrink = 0,
			items = "center",
			ui.each(tab_slots(id), function(slot)
				local dim = function()
					return not is_active()
				end
				if slot.more then
					return more_badge(slot.more, dim)
				end
				return badge(slot.pane, dim)
			end, {
				key = function(slot)
					return if slot.more then `more{slot.more}` else slot.key
				end,
			}),
		}),
		ui.text({
			function()
				local name = desc().name
				return if name ~= "" then name else "Terminal"
			end,
			shrink = 1,
			min_w = 0,
			text_size = 12.5,
			truncate = true,
			color = function()
				return if is_active() then theme.text else theme.secondary
			end,
		}),
		-- The bell rang in a pane of a background tab.
		ui.div({
			hidden = function()
				local p = pane()
				return is_active() or p == nil or not ui.term(p).bell
			end,
			w = 6,
			h = 6,
			rounded = 3,
			bg = theme.bell,
			shrink = 0,
		}),
	})
end

-- The tab strip: the tabs, and the slot a dragged pane would open in.
local strip = ui.computed(function()
	local list = tabs:get()
	local s = tab_slot:get()
	if not s.pane then
		return list
	end
	local out = {}
	for _, tab in list do
		if tab.id == s.before then
			table.insert(out, { id = "slot", slot = s.pane })
		end
		table.insert(out, tab)
	end
	if s.before == "" or #out == #list then
		table.insert(out, { id = "slot", slot = s.pane })
	end
	return out
end)

-- Where a dragged pane would open as a tab: a ghost of that tab, opening
-- up so the tabs after it slide over.
local function slot_tab(pane: string)
	local always = function()
		return false
	end
	return ui.row({
		h = 28,
		w = 132,
		pl = 4,
		pr = 11,
		gap = 7,
		shrink = 0,
		items = "center",
		rounded = 8,
		overflow = "hidden",
		bg = "#ffffff0f",
		border = 1,
		border_color = "#ffffff29",
		on_mount = function(el)
			anime.animate(el, { width = { 0, 132 }, opacity = { 0, 1 }, duration = 200, ease = "outQuint" })
		end,
		drop_when = function(payload)
			return payload.kind == "pane"
		end,
		on_drop = function()
			commit_slot()
		end,
		badge(pane, always),
		ui.text({
			function()
				local name = describe(pane).name
				return if name ~= "" then name else "Terminal"
			end,
			shrink = 1,
			min_w = 0,
			text_size = 12.5,
			truncate = true,
			color = theme.secondary,
		}),
	})
end

local titlebar = ui.row({
	w_full = true,
	h = theme.titlebar,
	items = "center",
	-- A pane anywhere else on it (the new tab button, the tools) opens as
	-- a tab where the slot shows, at the end if none does yet.
	drop_when = function(payload)
		return payload.kind == "pane"
	end,
	on_drag_over = function(payload)
		if not (tab_slot:peek()).pane then
			show_slot(payload.id, "")
		end
	end,
	on_drop = function()
		commit_slot()
	end,
	gap = 3,
	pr = 6,
	-- Room for the traffic lights, and some air after them.
	drag_space({ w = 80, shrink = 0 }),
	ui.row({
		gap = 3,
		items = "center",
		shrink = 1,
		min_w = 0,
		ui.each(strip, function(item, index)
			if item.slot then
				return slot_tab(item.slot)
			end
			return tab_button(item, index)
		end, {
			key = function(item)
				return item.id
			end,
		}),
	}),
	tool("New tab", icons.plus, function()
		new_tab(cwd_of(current()))
	end),
	-- Dropping a pane here gives it a tab of its own.
	drag_space({
		grow = 1,
		min_w = 16,
		drop_when = function(payload)
			return payload.kind == "pane"
		end,
		on_drag_over = function(payload)
			show_slot(payload.id, "")
		end,
		on_drop = function()
			commit_slot()
		end,
	}),
	tool("Split right", icons.split_right, function()
		split("row")
	end),
	tool("Split down", icons.split_down, function()
		split("column")
	end),
})

-- Panes ------------------------------------------------------------------------
--
-- The active tab's panes are cards placed absolutely, one per pane for as
-- long as it is in the tab, at frames worked out from its split tree. A
-- change of structure (a split, a close, a drop, a drag's preview) eases
-- every card to its new frame; a divider drag or a window resize moves
-- them at once.

local function radius(): number
	return math.max(window_radius:get() - theme.inset, 6)
end

-- The content area's size.
local area = ui.state("area", { w = 0, h = 0 })

local function live_root(id: string)
	for _, tab in tabs:get() do
		if tab.id == id then
			return tab.root
		end
	end
	return nil
end

-- The tree a drag hovering a pane's edge would make, shown while it hovers.
local function previewed(root)
	local d = drop:get()
	if not d.target or not layout.contains(root, d.target) then
		return root
	end
	local axis = if d.zone == "left" or d.zone == "right" then "row" else "column"
	local before = d.zone == "left" or d.zone == "top"
	if d.kind == "pane" then
		local base = root
		if layout.contains(root, d.id) then
			base = layout.remove(root, d.id)
		end
		if not base or not layout.contains(base, d.target) then
			return root
		end
		return layout.split(base, d.target, axis, d.id, "preview", before)
	end
	local other = live_root(d.id)
	if not other then
		return root
	end
	return layout.attach(root, d.target, axis, other, "preview", before)
end

local last_shape: string? = nil
local frames = ui.computed(function()
	local id = active:get()
	local root = live_root(id)
	local size = area:get()
	if not root then
		return { panes = {}, dividers = {}, order = {}, smooth = false }
	end
	local shown = previewed(root)
	local zoomed = (zoom:get())[id]
	local out
	if zoomed and layout.contains(shown, zoomed) and not (drop:get()).target then
		out = { panes = { [zoomed] = { x = 0, y = 0, w = size.w, h = size.h } }, dividers = {}, order = { zoomed } }
	else
		out = layout.frames(shown, ratios:get(), size.w, size.h, theme.gap)
		out.order = layout.leaves(shown)
	end
	-- A new shape eases; the same shape resized follows at once.
	local shape = `{id}#{layout.signature(shown)}#{zoomed or ""}`
	out.smooth = last_shape ~= nil and shape ~= last_shape and last_shape:match("^[^#]+") == id
	last_shape = shape
	return out
end)
local pane_ids = ui.computed(function()
	return (frames:get()).order
end)
local divider_ids = ui.computed(function()
	local ids = {}
	for _, d in (frames:get()).dividers do
		table.insert(ids, d.id)
	end
	return ids
end)

-- Ease (or set) an element's box.
local function place(element, r, smooth: boolean)
	anime.animate(element, {
		left = r.x,
		top = r.y,
		width = r.w,
		height = r.h,
		duration = if smooth then 280 else 0,
		ease = "outQuint",
	})
end

-- The pane under a point of the content area, by the committed layout (not
-- a preview's), and the edge of it the point is nearest.
local function target_at(x: number, y: number): (string?, string?)
	local id = active:peek()
	local root = root_of(id)
	if not root then
		return nil, nil
	end
	local size = area:peek()
	local zoomed = (zoom:peek())[id]
	local placed = if zoomed and layout.contains(root, zoomed)
		then { [zoomed] = { x = 0, y = 0, w = size.w, h = size.h } }
		else layout.frames(root, ratios:peek(), size.w, size.h, theme.gap).panes
	for pane, r in placed do
		if x >= r.x and x < r.x + r.w and y >= r.y and y < r.y + r.h then
			local fx, fy = (x - r.x) / math.max(r.w, 1), (y - r.y) / math.max(r.h, 1)
			local zone, best = "right", 1 - fx
			for name, distance in { left = fx, top = fy, bottom = 1 - fy } do
				if distance < best then
					zone, best = name, distance
				end
			end
			return pane, zone
		end
	end
	return nil, nil
end

-- Commit what the drop preview showed.
local function commit_drop(d)
	local tab = active:peek()
	local root = root_of(tab)
	if not d.target or not root or not layout.contains(root, d.target) then
		return
	end
	if d.kind == "pane" then
		move_pane(d.id, d.target, d.zone)
		return
	end
	-- A whole tab: its panes join this one beside the target.
	local other = root_of(d.id)
	if not other or d.id == tab then
		return
	end
	local axis = if d.zone == "left" or d.zone == "right" then "row" else "column"
	local i = tab_index(d.id) :: number
	local list = copy(tabs:peek())
	table.remove(list, i)
	tabs:set(list)
	set_map(focus, d.id, nil)
	set_root(tab, layout.attach(root, d.target, axis, other, fresh("s"), d.zone == "left" or d.zone == "top"))
	focus_pane(tab, d.target)
end

-- Six dots, two rows of three.
local function dots()
	local row = function()
		local out = { gap = 3, items = "center" }
		for _ = 1, 3 do
			table.insert(out, ui.div({ w = 3, h = 3, rounded = 2, bg = "#ffffff99" }))
		end
		return ui.row(out)
	end
	return ui.column({ gap = 3, items = "center", row(), row() })
end

local Pane = ui.component(function(props)
	local pane: string = props.pane
	local tab = active:peek()
	local term = session(pane)
	local element = nil
	local start = (frames:peek()).panes[pane] or { x = 0, y = 0, w = 0, h = 0 }
	local function focused()
		return (focus:get())[tab] == pane
	end
	local function several()
		local root = live_root(tab)
		return root ~= nil and root.kind == "split"
	end
	local function moving()
		local d = drop:get()
		return d.target ~= nil and d.kind == "pane" and d.id == pane
	end
	-- The terminal takes its new size at once and stays put in the card's
	-- top left while the card eases around it (clipping it): it redraws
	-- once, not on every frame of the move.
	local size = ui.use_state({ w = math.max(start.w - 2, 0), h = math.max(start.h - 2, 0) })
	ui.effect(function()
		local f = frames:get()
		local r = f.panes[pane]
		if not r then
			return
		end
		local s = size:peek()
		if s.w ~= r.w - 2 or s.h ~= r.h - 2 then
			size:set({ w = math.max(r.w - 2, 0), h = math.max(r.h - 2, 0) })
		end
		if element then
			place(element, r, f.smooth)
		end
	end)
	-- The grip shows while the pointer is near the top edge, the grip
	-- included (the terminal hears the pointer over what zgui draws on it,
	-- zgui does not hear it leave for the terminal), when the pane has
	-- somewhere to go: beside another pane, or into another tab.
	-- The focused terminal hears the pointer anywhere in the window, so
	-- only points over it count.
	local grip = ui.use_state(0)
	ui.effect(function()
		local p = term.pointer
		local near = p ~= nil and p.y >= 0 and p.y < 32 and p.x >= 0 and p.x <= size:get().w
		local d = dragging:get()
		-- While dragging, only the dragged pane's grip (hiding the source
		-- would end the drag).
		local show = if d.kind ~= nil
			then d.kind == "pane" and d.id == pane
			else near and (several() or #tabs:get() > 1)
		grip:set(if show then 1 else 0)
	end)
	local inner = function()
		return radius() - 1
	end
	return ui.column({
		absolute = true,
		left = start.x,
		top = start.y,
		w = start.w,
		h = start.h,
		on_mount = function(el)
			element = el
			-- Where it goes now (the frames may have moved on since it was
			-- built); arriving with a change, it fades in there.
			local f = frames:peek()
			local r = f.panes[pane]
			if r then
				place(el, r, false)
			end
			if f.smooth then
				anime.animate(el, { opacity = { 0, 1 }, duration = 220, ease = "outQuad" })
			end
		end,
		items = "stretch",
		rounded = radius,
		overflow = "hidden",
		bg = theme.pane,
		border = 1,
		border_color = function()
			if moving() then
				return "#ffffff59"
			end
			return if several() and focused() then theme.ring else theme.line
		end,
		ui.overlay({
			grow = 1,
			min_h = 0,
			ui.terminal({
				term = term,
				absolute = true,
				-- Inside the card's border.
				left = 1,
				top = 1,
				w = function()
					return size:get().w
				end,
				h = function()
					return size:get().h
				end,
				rounded = inner,
				on_focus = function()
					focus_pane(tab, pane)
				end,
				context_menu_style = MENU_STYLE,
				context_menu = function()
					focus_pane(tab, pane)
					local zoomed = (zoom:peek())[tab] == pane
					local items = {
						entry("Copy", function()
							term:binding("copy_to_clipboard")
						end),
						entry("Paste", function()
							term:binding("paste_from_clipboard")
						end),
						entry("Clear", function()
							term:binding("clear_screen")
						end),
						separator(),
						entry("Split Right", function()
							split("row")
						end),
						entry("Split Down", function()
							split("column")
						end),
					}
					if several() or zoomed then
						table.insert(
							items,
							entry(if zoomed then "Unzoom Pane" else "Zoom Pane", function()
								set_map(zoom, tab, if zoomed then nil else pane)
								term:focus()
							end)
						)
					end
					table.insert(items, separator())
					table.insert(
						items,
						entry("Close Pane", function()
							close_pane(pane)
						end)
					)
					return items
				end,
			}),

			ui.row({
				absolute = true,
				top = 6,
				left = 0,
				w_full = true,
				justify = "center",
				hidden = function()
					return grip:get() == 0
				end,
				ui.column({
					w = 36,
					h = 22,
					rounded = 7,
					items = "center",
					justify = "center",
					-- Frosted, as the window: the terminal shows through, blurred.
					bg = "#5a5c6440",
					backdrop_blur = 14,
					border = 1,
					border_color = "#ffffff1a",
					shadow = { y = 2, blur = 8, color = "#00000033" },
					cursor = "grab",
					-- (Seconds, unlike anime.animate's milliseconds.)
					opacity = ui.tween(grip, { duration = 0.16 }),
					on_click = function()
						focus_pane(tab, pane)
					end,
					on_drag = function()
						dragging:set({ kind = "pane", id = pane })
						return { kind = "pane", id = pane }
					end,
					drag_preview = function()
						return chip(pane)
					end,
					on_drag_end = function()
						dragging:set({})
						hide_slot()
						tab_drop:set("")
					end,
					dots(),
				}),
			}),
		}),
	})
end)

local Divider = ui.component(function(props)
	local id: string = props.id
	local element = nil
	local function find(list)
		for _, d in list do
			if d.id == id then
				return d
			end
		end
		return nil
	end
	local first = find((frames:peek()).dividers) or { axis = "row", x = 0, y = 0, w = 0, h = 0, length = 0 }
	local row = first.axis == "row"
	ui.effect(function()
		local f = frames:get()
		local d = find(f.dividers)
		if element and d then
			place(element, d, f.smooth)
		end
	end)
	local start: { at: number, ratio: number, length: number }? = nil
	return ui.div({
		absolute = true,
		left = first.x,
		top = first.y,
		w = first.w,
		h = first.h,
		on_mount = function(el)
			element = el
			local d = find((frames:peek()).dividers)
			if d then
				place(el, d, false)
			end
		end,
		rounded = 3,
		cursor = if row then "col_resize" else "row_resize",
		bg = CLEAR,
		hover = { bg = "#ffffff12" },
		on_press = function(e)
			local d = find((frames:peek()).dividers)
			if d then
				start = { at = if row then e.x else e.y, ratio = (ratios:peek())[id] or 0.5, length = d.length }
			end
		end,
		on_pointer_move = function(e)
			local s = start
			if not s or s.length <= 0 then
				return
			end
			local moved = (if row then e.x else e.y) - s.at
			local least = math.min(0.45, 160 / s.length)
			set_map(ratios, id, math.clamp(s.ratio + moved / s.length, least, 1 - least))
		end,
		on_release = function()
			start = nil
		end,
	})
end)

-- The active tab's panes. A pane or a tab dragged over them shows, live,
-- where it would go; dropping keeps it.
local content = ui.div({
	grow = 1,
	min_h = 0,
	mx = theme.inset,
	mb = theme.inset,
	on_resize = function(size)
		local a = area:peek()
		if a.w ~= size.w or a.h ~= size.h then
			area:set({ w = size.w, h = size.h })
		end
	end,
	drop_when = function(payload)
		return payload.kind == "pane" or (payload.kind == "tab" and payload.id ~= active:peek())
	end,
	on_drag_over = function(payload, e)
		hide_slot()
		local target, zone = target_at(e.local_x, e.local_y)
		if target == payload.id then
			target = nil
		end
		local d = drop:peek()
		if d.target ~= target or d.zone ~= zone or d.id ~= payload.id then
			drop:set(if target then { target = target, zone = zone, kind = payload.kind, id = payload.id } else {})
		end
	end,
	on_drag_leave = function()
		if (drop:peek()).target then
			drop:set({})
		end
	end,
	on_drop = function()
		local d = drop:peek()
		drop:set({})
		commit_drop(d)
	end,
	ui.each(pane_ids, function(id)
		return Pane({ pane = id })
	end, {
		key = function(id)
			return id
		end,
	}),
	ui.each(divider_ids, function(id)
		return Divider({ id = id })
	end, {
		key = function(id)
			return id
		end,
	}),
})

-- Closing something still running asks first.
local function confirm_sheet()
	local function what(): (string, string)
		local c = confirm:get()
		local kind, id = c:match("^(%w+):(.+)$")
		if kind == "pane" then
			return `Close “{describe(id).name}”?`, "The process running in it will be ended."
		end
		return "Close this tab?", "Processes running in its panes will be ended."
	end
	local function decide(yes: boolean)
		local c = confirm:peek()
		confirm:set("")
		local kind, id = c:match("^(%w+):(.+)$")
		if not yes then
			local pane = current()
			if pane then
				ui.term(pane):focus()
			end
			return
		end
		if kind == "pane" then
			close_pane(id, true)
		elseif kind == "tab" then
			close_tab(id, true)
		end
	end
	local function button(label: string, primary: boolean, on_click: () -> ())
		return ui.button({
			label,
			h = 30,
			px = 14,
			rounded = 8,
			text_size = 13,
			color = if primary then "#ffffff" else theme.text,
			bg = if primary then "#e5534bd9" else theme.wash,
			hover = { bg = if primary then "#f0645c" else "#ffffff1f" },
			focus = { border = 0 },
			on_click = on_click,
		})
	end
	return ui.div({
		hidden = function()
			return confirm:get() == ""
		end,
		absolute = true,
		left = 0,
		top = 0,
		w_full = true,
		h_full = true,
		bg = "#0000004d",
		items = "center",
		justify = "center",
		on_click = function()
			decide(false)
		end,
		ui.column({
			w = 340,
			p = 18,
			gap = 6,
			rounded = 14,
			bg = "#1d1e22d9",
			backdrop_blur = 28,
			border = 1,
			border_color = "#ffffff1f",
			shadow = { y = 18, blur = 48, color = "#00000080" },
			-- Clicks on the sheet stay on it.
			on_click = function() end,
			ui.text({
				function()
					return (what())
				end,
				text_size = 14,
				bold = true,
				color = theme.text,
			}),
			ui.text({
				function()
					local _, body = what()
					return body
				end,
				text_size = 12.5,
				color = theme.secondary,
			}),
			ui.row({
				mt = 12,
				gap = 8,
				justify = "end",
				button("Cancel", false, function()
					decide(false)
				end),
				button("Close", true, function()
					decide(true)
				end),
			}),
		}),
	})
end

return ui.column({
	w_full = true,
	h_full = true,
	font = theme.font,
	items = "stretch",
	bg = theme.tint,
	on_key = function(e)
		if confirm:peek() == "" then
			return false
		end
		if e.key == "Escape" or e.key == "Enter" then
			local c = confirm:peek()
			confirm:set("")
			if e.key == "Enter" then
				local kind, id = c:match("^(%w+):(.+)$")
				if kind == "pane" then
					close_pane(id, true)
				else
					close_tab(id, true)
				end
			end
			return true
		end
		return false
	end,
	titlebar,
	content,
	confirm_sheet(),
})
