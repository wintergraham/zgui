-- Sample data: 100,000 instruments, deterministic, and a price series.
export type Row = {
	id: number,
	symbol: string,
	sector: string,
	price: number,
	change: number,
	volume: number,
}

local SECTORS = { "Tech", "Energy", "Health", "Finance", "Retail", "Industry", "Media", "Utilities" }
local LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"

local M = {}
M.SECTORS = SECTORS

-- A small deterministic generator (the same rows every run).
local seed = 12345
local function random(): number
	seed = (seed * 1103515245 + 12345) % 2147483648
	return seed / 2147483648
end
M.random = random

-- `count` rows, each one's recent prices by its id (for its sparkline),
-- and the totals the charts start from: one pass, not a row per table.
function M.rows(count: number): ({ Row }, { [number]: { number } }, { [string]: number }, number)
	local rows: { Row } = table.create(count)
	local histories: { [number]: { number } } = table.create(count)
	-- The generator's arithmetic inline, the same stream `random` gives
	-- (its seed picked up where this leaves it): a call a value costs more
	-- than the two operations.
	local s = seed
	-- The strings a symbol is built from, made once, not per row.
	local letters = table.create(26)
	for i = 1, 26 do
		letters[i] = string.sub(LETTERS, i, i)
	end
	local digits = table.create(97)
	for i = 1, 97 do
		digits[i] = tostring(i - 1)
	end
	local floor, max = math.floor, math.max
	local sectors = SECTORS
	local volumes: { [string]: number } = {}
	local nsec = #sectors
	local value = 0
	for i = 1, count do
		s = (s * 1103515245 + 12345) % 2147483648
		local price = 5 + s / 2147483648 * 495
		local history = table.create(16)
		local p = price
		for h = 1, 16 do
			s = (s * 1103515245 + 12345) % 2147483648
			p = max(1, p * (1 + (s / 2147483648 - 0.5) * 0.04))
			history[h] = p
		end
		s = (s * 1103515245 + 12345) % 2147483648
		local change = (s / 2147483648 - 0.5) * 10
		s = (s * 1103515245 + 12345) % 2147483648
		local volume = floor(s / 2147483648 * 5_000_000)
		local sector = sectors[i % nsec + 1]
		volumes[sector] = (volumes[sector] or 0) + volume
		value += price * volume
		rows[i] = {
			id = i,
			symbol = letters[i % 26 + 1]
				.. letters[i // 26 % 26 + 1]
				.. letters[i // 676 % 26 + 1]
				.. digits[i % 97 + 1],
			sector = sector,
			price = price,
			change = change,
			volume = volume,
		}
		histories[i] = history
	end
	seed = s
	return rows, histories, volumes, value
end

function M.series(count: number): { number }
	local values: { number } = {}
	local v = 100
	for i = 1, count do
		v = math.max(10, v + (random() - 0.5) * 6)
		values[i] = v
	end
	return values
end

return M
