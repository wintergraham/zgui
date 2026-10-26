-- The Luau views probe (scripts/luau_views_smoke.py): a board canvas and a
-- measured list in one window, at fixed places, whose pixels the probe
-- reads. It prints what it does: each item a canvas draws, each view, each
-- jump.
--
-- Left, 400 by 400 points from the window's top left: a canvas following
-- `view`, over a magenta dot grid every 20 points, its two shapes drawn one
-- by one (`each`, `draw_item`). The wheel pans it; a click on a shape
-- recolours that shape alone.
-- Right, from x 400: Zoom (doubles the view's zoom), Jump (the list's
-- row 2000 to its centre) and a Drag chip, 100 by 40 each, over a measured
-- list 400 by 360 of rows one to three lines tall; row 2000 is five lines,
-- in green. The chip dragged onto the board springs it open once it rests
-- there (SPRUNG), and drops (DROPPED).
type Shape = { id: number, x: number, y: number, w: number, h: number, color: string }
type View = { x: number, y: number, zoom: number }

local view = ui.state("view", { x = 0, y = 0, zoom = 1 } :: View)
local shapes = ui.state(
	"shapes",
	{
		{ id = 1, x = 40, y = 40, w = 60, h = 40, color = "#ff0000" },
		{ id = 2, x = 200, y = 120, w = 40, h = 40, color = "#0000ff" },
	} :: { Shape }
)
local list = ui.ref()

local function report(v: View)
	print(string.format("VIEW %g %g %g", v.x, v.y, v.zoom))
end

local board = ui.overlay {
	w = 400,
	h = 400,
	bg = "#101010",
	overflow = "clip",
	on_wheel = function(e: WheelEvent)
		local v = view:peek()
		local moved = { x = v.x - e.dx, y = v.y - e.dy, zoom = v.zoom }
		view:set(moved)
		report(moved)
		return true
	end,
	-- A press on a shape (in the board's coordinates) recolours it.
	on_press = function(e: PointerEvent)
		local v = view:peek()
		local bx, by = (e.local_x - v.x) / v.zoom, (e.local_y - v.y) / v.zoom
		for i, s in shapes:peek() do
			if bx >= s.x and bx < s.x + s.w and by >= s.y and by < s.y + s.h then
				print(string.format("RECOLOR %d", s.id))
				shapes:set_key({ i, "color" }, "#00ff00")
			end
		end
	end,
	on_drag_spring = function(payload: string)
		print("SPRUNG " .. payload)
		return true
	end,
	on_drop = function(payload: string)
		print("DROPPED " .. payload)
	end,
	ui.canvas {
		w_full = true,
		h_full = true,
		view = view,
		bg_grid = { step = 20, dot = 2, color = "#ff00ff" },
		each = shapes,
		draw_item = function(c, s: Shape)
			print(string.format("DRAW_ITEM %d", s.id))
			c:rect(s.x, s.y, s.w, s.h, s.color)
		end,
	},
}

-- One to three lines a row; row 2000 five, in green.
local function lines(i: number): number
	return if i == 2000 then 5 else i % 3 + 1
end

local function row(i: number)
	local text = {}
	for n = 1, lines(i) do
		table.insert(text, string.format("Row %d line %d", i, n))
	end
	return ui.column {
		w_full = true,
		px = 8,
		py = 4,
		bg = if i == 2000 then "#00ff00" elseif i % 2 == 0 then "#202020" else "#303030",
		ui.text { table.concat(text, "\n"), literal = true, color = "#c0c0c0" },
	}
end

local function button(label: string, on_click: () -> ())
	return ui.button {
		w = 100,
		h = 40,
		px = 0,
		py = 0,
		rounded = 0,
		on_click = on_click,
		ui.text { label, literal = true },
	}
end

return ui.row {
	w_full = true,
	h_full = true,
	items = "start",
	bg = "#000000",
	text_size = 16,
	line_height = 20,
	board,
	ui.column {
		w = 400,
		h = 400,
		ui.row {
			h = 40,
			button("Zoom", function()
				local v = view:peek()
				local zoomed = { x = v.x, y = v.y, zoom = v.zoom * 2 }
				view:set(zoomed)
				report(zoomed)
			end),
			button("Jump", function()
				local el = list:peek()
				if el then
					el:scroll_to_row(2000, { align = "center" })
					print("JUMP 2000")
				end
			end),
			ui.div {
				w = 100,
				h = 40,
				bg = "#404060",
				on_drag = function()
					print("DRAGGING chip")
					return "chip"
				end,
				ui.text { "Drag", literal = true },
			},
		},
		ui.list {
			ref = list,
			w = 400,
			h = 360,
			count = 3000,
			measured = true,
			-- A guess no row is: rows show at their measured heights.
			height = 30,
			row = row,
		},
	},
}
