-- A tab's panes as a tree of splits. Plain data (kept in ui.state, so it
-- survives reloads); every change returns a new tree.
--
--   { kind = "pane", id = "p3" }
--   { kind = "split", id = "s2", axis = "row" | "column", a = node, b = node }
--
-- `row` lays a beside b, `column` a above b. Ratios live apart (keyed by
-- split id) so dragging a divider restyles without rebuilding the tree.
local M = {}

function M.pane(id: string)
	return { kind = "pane", id = id }
end

-- Pane ids in reading order.
function M.leaves(node, out: { string }?): { string }
	out = out or {}
	if node.kind == "pane" then
		table.insert(out :: { string }, node.id)
	else
		M.leaves(node.a, out)
		M.leaves(node.b, out)
	end
	return out :: { string }
end

function M.contains(node, pane: string): boolean
	return table.find(M.leaves(node), pane) ~= nil
end

-- Its structure, for rebuilding only when it changes.
function M.signature(node): string
	if node.kind == "pane" then
		return node.id
	end
	return `{node.id}{node.axis == "row" and "|" or "-"}({M.signature(node.a)},{M.signature(node.b)})`
end

-- `node` with the pane `pane` replaced by `replacement`.
function M.replace(node, pane: string, replacement)
	if node.kind == "pane" then
		return if node.id == pane then replacement else node
	end
	return {
		kind = "split",
		id = node.id,
		axis = node.axis,
		a = M.replace(node.a, pane, replacement),
		b = M.replace(node.b, pane, replacement),
	}
end

-- Split `pane`: `new` goes after it (right or below), or before when `before`.
function M.split(node, pane: string, axis: string, new: string, split_id: string, before: boolean?)
	local fresh = M.pane(new)
	local old = M.pane(pane)
	return M.replace(node, pane, {
		kind = "split",
		id = split_id,
		axis = axis,
		a = if before then fresh else old,
		b = if before then old else fresh,
	})
end

-- `node` without `pane`: its sibling takes its split's place. Returns the
-- new tree (nil when it was the last pane) and the pane next to where it was.
function M.remove(node, pane: string)
	if node.kind == "pane" then
		return if node.id == pane then nil else node, nil
	end
	if node.a.kind == "pane" and node.a.id == pane then
		return node.b, M.leaves(node.b)[1]
	end
	if node.b.kind == "pane" and node.b.id == pane then
		local leaves = M.leaves(node.a)
		return node.a, leaves[#leaves]
	end
	if M.contains(node.a, pane) then
		local a, near = M.remove(node.a, pane)
		return { kind = "split", id = node.id, axis = node.axis, a = a, b = node.b }, near
	end
	local b, near = M.remove(node.b, pane)
	return { kind = "split", id = node.id, axis = node.axis, a = node.a, b = b }, near
end

-- The path from the root to `pane`: each split and the side taken.
local function path_to(node, pane: string, path)
	if node.kind == "pane" then
		return if node.id == pane then path else nil
	end
	table.insert(path, { split = node, side = "a" })
	local found = path_to(node.a, pane, path)
	if found then
		return found
	end
	path[#path].side = "b"
	found = path_to(node.b, pane, path)
	if found then
		return found
	end
	table.remove(path)
	return nil
end

-- The pane beside `pane` in a direction: "left", "right", "up", "down".
function M.neighbor(node, pane: string, direction: string): string?
	local path = path_to(node, pane, {})
	if not path then
		return nil
	end
	local axis = if direction == "left" or direction == "right" then "row" else "column"
	local backward = direction == "left" or direction == "up"
	for i = #path, 1, -1 do
		local step = path[i]
		if step.split.axis == axis and step.side == (if backward then "b" else "a") then
			local other = if backward then step.split.a else step.split.b
			local leaves = M.leaves(other)
			return if backward then leaves[#leaves] else leaves[1]
		end
	end
	return nil
end

-- Put `subtree` beside `target`: after it (right or below), or before.
function M.attach(node, target: string, axis: string, subtree, split_id: string, before: boolean?)
	local old = M.pane(target)
	return M.replace(node, target, {
		kind = "split",
		id = split_id,
		axis = axis,
		a = if before then subtree else old,
		b = if before then old else subtree,
	})
end

export type Rect = { x: number, y: number, w: number, h: number }
export type Divider = Rect & { id: string, axis: string, length: number }

-- Where every pane and divider goes in a `w` x `h` box, `gap` apart: panes
-- by id, dividers in order. Sides round to whole points.
function M.frames(node, ratios: { [string]: number }, w: number, h: number, gap: number)
	local out = { panes = {}, dividers = {} }
	local function place(n, x: number, y: number, w: number, h: number)
		if n.kind == "pane" then
			out.panes[n.id] = { x = x, y = y, w = w, h = h }
			return
		end
		local r = ratios[n.id] or 0.5
		if n.axis == "row" then
			local length = math.max(w - gap, 0)
			local a = math.floor(length * r + 0.5)
			place(n.a, x, y, a, h)
			table.insert(out.dividers, { id = n.id, axis = "row", x = x + a, y = y, w = gap, h = h, length = length })
			place(n.b, x + a + gap, y, length - a, h)
		else
			local length = math.max(h - gap, 0)
			local a = math.floor(length * r + 0.5)
			place(n.a, x, y, w, a)
			table.insert(out.dividers, { id = n.id, axis = "column", x = x, y = y + a, w = w, h = gap, length = length })
			place(n.b, x, y + a + gap, w, length - a)
		end
	end
	place(node, 0, 0, w, h)
	return out
end

-- Split ids, for equalizing.
function M.splits(node, out: { string }?): { string }
	out = out or {}
	if node.kind == "split" then
		table.insert(out :: { string }, node.id)
		M.splits(node.a, out)
		M.splits(node.b, out)
	end
	return out :: { string }
end

return M
