-- The import, played: each chosen source in turn, part by part, pausing at
-- a Chromium browser's saved passwords until the Keychain coach is done.
-- Nothing is read; the counts are the canned ones in data.luau.
local D = require("./data")
local S = require("./state")
local I = {}

local TICK = 100 -- ms
local UNIT = 2200 -- ms one unit of weight takes, for Chrome's size
-- A host can play the import faster (tests do).
local SPEED = ui.shared("onboarding_speed", 1)

local weight_of = {}
for _, k in D.kinds do
	weight_of[k.id] = k.weight
end

-- The sources the user picked, in the grid's order.
function I.sources(): { any }
	local out = {}
	if S.skip_import:get() then
		return out
	end
	local picked = {}
	for _, id in S.picked:get() do
		picked[id] = true
	end
	for _, b in D.browsers do
		if picked[b.id] then
			table.insert(out, b)
		end
	end
	if S.file:get() then
		table.insert(out, D.file)
	end
	return out
end

-- Which parts a source has, of the ones chosen.
function I.parts(b: any): { string }
	local kinds, out = S.kinds:get(), {}
	local has = {
		bookmarks = b.bookmarks > 0,
		history = b.pages > 0,
		passwords = b.logins > 0,
		tabs = b.tabs > 0,
		extensions = b.compatible > 0,
	}
	for _, k in D.kinds do
		if kinds[k.id] and has[k.id] then
			table.insert(out, k.id)
		end
	end
	return out
end

local function job_weight(job): number
	local total = 0
	for _, part in job.parts do
		total += weight_of[part] * job.size
	end
	return total
end
local function job_done(job): number
	if job.finished then
		return job_weight(job)
	end
	local done = 0
	for i = 1, job.part - 1 do
		done += weight_of[job.parts[i]] * job.size
	end
	local current = job.parts[job.part]
	if current then
		done += weight_of[current] * job.size * job.done
	end
	return done
end

-- 0 to 1, over every source (tracked).
function I.progress(): number
	local total, done = 0, 0
	for _, job in S.jobs:get() do
		total += job_weight(job)
		done += job_done(job)
	end
	return if total > 0 then done / total else 0
end
function I.job_progress(job): number
	local w = job_weight(job)
	return if w > 0 then job_done(job) / w else 1
end
function I.percent(): string
	return math.floor(I.progress() * 100 + 0.5) .. "%"
end

-- "about a minute left", from what is left at the import's pace.
function I.time_left(): string
	local left = 0
	for _, job in S.jobs:get() do
		left += job_weight(job) - job_done(job)
	end
	local seconds = left * UNIT / 1000
	if seconds > 40 then
		return "about a minute left"
	elseif seconds > 8 then
		return "about " .. (math.ceil(seconds / 5) * 5) .. " seconds left"
	end
	return "almost done"
end

function I.started(): boolean
	return S.import_state:get() ~= "idle"
end

local stop: (() -> ())? = nil

-- A copy of a job with some fields changed.
local function with(job, changes)
	local out = table.clone(job)
	for k, v in changes do
		out[k] = v
	end
	return out
end

local function open_coach(job)
	ui.batch(function()
		S.import_state:set("waiting")
		S.coach_for:set(job.id)
		S.coach_step:set(1)
		S.coach_typed:set(0)
		S.coach_help:set(false)
		S.coach_open:set(true)
	end)
end

local function tick()
	if S.import_state:peek() ~= "running" then
		return
	end
	local jobs = S.jobs:peek()
	for i, job in jobs do
		if not job.finished then
			local part = job.parts[job.part]
			if part == "passwords" and job.chromium and not S.keychain:peek()[job.id] then
				open_coach(job)
				return
			end
			local done = job.done + TICK * SPEED:peek() / (UNIT * weight_of[part] * job.size)
			if done < 1 then
				S.jobs:set_key({ i, "done" }, done)
			elseif job.part < #job.parts then
				S.jobs:set_key(i, with(job, { part = job.part + 1, done = 0 }))
			else
				S.jobs:set_key(i, with(job, { done = 1, finished = true }))
			end
			return
		end
	end
	S.import_state:set("done")
	if stop then
		stop()
		stop = nil
	end
end

local function run()
	if stop then
		stop()
	end
	stop = ui.every(TICK, function()
		tick()
		return S.import_state:peek() ~= "done"
	end)
end

-- Starts the import of what the Data step chose.
function I.start()
	local jobs = {}
	for _, b in I.sources() do
		local parts = I.parts(b)
		if #parts > 0 then
			table.insert(jobs, {
				id = b.id,
				name = b.name,
				chromium = b.chromium or false,
				size = b.size,
				parts = parts,
				part = 1,
				done = 0,
				finished = false,
				skipped = false,
			})
		end
	end
	ui.batch(function()
		S.jobs:set(jobs)
		S.keychain:set({})
		S.import_state:set(if #jobs > 0 then "running" else "done")
	end)
	if #jobs > 0 then
		run()
	end
end

-- What the Keychain prompt came to: "always" or "once" reads the passwords,
-- "denied" skips them.
function I.keychain(answer: string)
	local id = S.coach_for:peek()
	ui.batch(function()
		S.keychain:set_key(id, answer)
		if answer == "denied" then
			for i, job in S.jobs:peek() do
				if job.id == id and job.parts[job.part] == "passwords" then
					local finished = job.part >= #job.parts
					S.jobs:set_key(i, with(job, {
						skipped = true,
						part = if finished then job.part else job.part + 1,
						done = if finished then 1 else 0,
						finished = finished,
					}))
				end
			end
		end
		S.coach_open:set(false)
		S.import_state:set("running")
	end)
	run()
end

-- Reopens the coach for the source waiting on it.
function I.resume_coach()
	for _, job in S.jobs:peek() do
		if not job.finished and job.parts[job.part] == "passwords" and not S.keychain:peek()[job.id] then
			open_coach(job)
			return
		end
	end
end

-- What a source's row says under its name.
function I.status(job): string
	local b = if job.id == "file" then D.file else D.by_id[job.id]
	if job.finished then
		local bits = {}
		for _, part in job.parts do
			if part == "bookmarks" then
				table.insert(bits, D.count(b.bookmarks) .. " bookmarks")
			elseif part == "tabs" then
				table.insert(bits, b.tabs .. " tabs")
			elseif part == "history" then
				table.insert(bits, "history")
			elseif part == "passwords" then
				table.insert(bits, if job.skipped then "passwords skipped" else "passwords")
			elseif part == "extensions" then
				table.insert(bits, b.compatible .. " extensions")
			end
		end
		return table.concat(bits, " · ")
	end
	local part = job.parts[job.part]
	if part == "passwords" and job.chromium and not S.keychain:get()[job.id] and S.import_state:get() == "waiting" then
		return "Waiting for Keychain access"
	end
	for _, k in D.kinds do
		if k.id == part then
			if job.part == 1 and job.done == 0 then
				local names = {}
				for _, id in job.parts do
					for _, kind in D.kinds do
						if kind.id == id then
							table.insert(names, kind.title:lower())
						end
					end
				end
				local list = D.join(names)
				return list:sub(1, 1):upper() .. list:sub(2)
			end
			return k.doing .. " · " .. I.time_left()
		end
	end
	return ""
end

return I
