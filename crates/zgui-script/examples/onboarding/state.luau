-- The onboarding's state, in one module so every screen shares it.
local D = require("./data")
local S = {}

S.page = ui.state("page", "splash")
S.dir = ui.state("dir", 1) -- 1 going forward, -1 going back: which way pages slide
S.reached = ui.state("reached", 0) -- the furthest step counted so far, 0 to 5

-- Import: which browsers, and what to bring.
S.picked = ui.state("picked", { "chrome", "aside" }) -- browser ids, in the grid's order
S.file = ui.state("file", false) -- the "From a file" tile
S.skip_import = ui.state("skip_import", false)
S.kinds = ui.state("kinds", { bookmarks = true, history = true, passwords = true, tabs = true, extensions = false })

-- The running import: one job per source, worked through in turn.
-- job: { id, name, phases = { kind ids }, phase (1-based), done (0..1 of the
-- current phase), finished, skipped_passwords, waiting }
S.jobs = ui.state("jobs", {} :: { any })
S.import_state = ui.state("import_state", "idle") -- "idle" | "running" | "waiting" | "done"
S.keychain = ui.state("keychain", {} :: { [string]: string }) -- browser id: "always" | "once" | "denied"

-- The Keychain coach.
S.coach_open = ui.state("coach_open", false)
S.coach_for = ui.state("coach_for", "chrome")
S.coach_step = ui.state("coach_step", 1)
S.coach_typed = ui.state("coach_typed", 0) -- how many characters: the password itself is never kept
S.coach_help = ui.state("coach_help", false)

-- Appearance.
S.theme = ui.state("theme", "Dark") -- "Dark" | "Light" | "System"
S.tint = ui.state("tint", "stone")
S.reduce_motion = ui.state("reduce_motion", false)
S.intensity = ui.state("intensity", "Default") -- "Off" | "Subtle" | "Default" | "Rich"
S.grain = ui.state("grain", "Default") -- "Fine" | "Default" | "Coarse"

-- The agent.
S.level = ui.state("level", "assist") -- a level id, or "custom" once a rule is changed by hand
S.ai_off = ui.state("ai_off", false)
S.rules = ui.state("rules", table.clone(D.presets.assist))

-- Ready.
S.made_default = ui.state("made_default", false)
S.leaving = ui.state("leaving", false)

return S
