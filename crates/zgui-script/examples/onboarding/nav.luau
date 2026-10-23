-- Moving between steps, and what each step's footer buttons do.
local D = require("./data")
local S = require("./state")
local I = require("./import")
local Sh = require("./shaders")
local N = {}

local ORDER = { "splash", "welcome", "browsers", "data", "importing", "appearance", "agent", "per_action", "ready" }
local index = {}
for i, name in ORDER do
	index[name] = i
end

function N.go(page: string)
	local from = S.page:peek()
	if page == from then
		return
	end
	ui.batch(function()
		S.dir:set(if index[page] >= index[from] then 1 else -1)
		S.page:set(page)
		local n = D.steps[page].n
		if n > S.reached:peek() then
			S.reached:set(n)
		end
	end)
	-- The fills of the step that left stop their clocks once it has gone.
	ui.after(500, Sh.sweep)
end

-- Leaves setup: the window fades and closes.
function N.finish()
	if S.leaving:peek() then
		return
	end
	S.leaving:set(true)
	ui.after(if S.reduce_motion:peek() then 60 else 420, function()
		pcall(ui.invoke, "close")
	end)
end

function N.close()
	pcall(ui.invoke, "close")
end

-- Whether anything would come over from the Data step's choices.
function N.anything_to_import(): boolean
	for _, b in I.sources() do
		if #I.parts(b) > 0 then
			return true
		end
	end
	return false
end

export type Action = { label: string, run: () -> (), disabled: (() -> boolean)? }
export type Actions = { back: Action?, skip: Action?, primary: Action }

-- The footer's buttons for a step (read inside a computed, so tracked).
function N.actions(page: string): Actions
	if page == "welcome" then
		return {
			skip = { label = "Skip setup", run = function() N.go("ready") end },
			primary = { label = "Get started", run = function() N.go("browsers") end },
		}
	elseif page == "browsers" then
		return {
			back = { label = "Back", run = function() N.go("welcome") end },
			primary = {
				label = "Continue",
				run = function()
					if I.started() or #I.sources() > 0 then
						N.go("data")
					else
						N.go("appearance")
					end
				end,
			},
		}
	elseif page == "data" then
		local started = I.started()
		return {
			back = { label = "Back", run = function() N.go("browsers") end },
			primary = {
				label = if started then "Continue" else "Import",
				disabled = function()
					return not I.started() and not N.anything_to_import()
				end,
				run = function()
					if not I.started() then
						I.start()
					end
					N.go("importing")
				end,
			},
		}
	elseif page == "importing" then
		return {
			back = { label = "Back", run = function() N.go("data") end },
			primary = { label = "Continue", run = function() N.go("appearance") end },
		}
	elseif page == "appearance" then
		return {
			back = {
				label = "Back",
				run = function()
					N.go(if I.started() then "importing" else "browsers")
				end,
			},
			primary = { label = "Continue", run = function() N.go("agent") end },
		}
	elseif page == "agent" then
		return {
			back = { label = "Back", run = function() N.go("appearance") end },
			primary = { label = "Continue", run = function() N.go("ready") end },
		}
	elseif page == "per_action" then
		return {
			back = { label = "Back", run = function() N.go("agent") end },
			primary = { label = "Save and continue", run = function() N.go("ready") end },
		}
	elseif page == "ready" then
		return {
			back = { label = "Back", run = function() N.go("agent") end },
			primary = { label = "Open Ask", run = N.finish },
		}
	end
	return { primary = { label = "Continue", run = function() N.go("welcome") end } }
end

-- Enter: the step's primary button, unless it is disabled.
function N.primary()
	local a = N.actions(S.page:peek()).primary
	if not (a.disabled and a.disabled()) then
		a.run()
	end
end
function N.back()
	local a = N.actions(S.page:peek()).back
	if a then
		a.run()
	end
end

-- Choosing an agent level puts its rules in place.
function N.set_level(id: string)
	ui.batch(function()
		S.ai_off:set(false)
		S.level:set(id)
		S.rules:set(table.clone(D.presets[id]))
	end)
end

return N
