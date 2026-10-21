-- What a pane is called and which icon it shows, from what runs in it.
--
-- At a shell prompt the pane is its directory ("comet-native", "~"). With a
-- program running it is the title the program set, else the program's name;
-- a title that starts with a symbol (Claude Code's "✳ Fix the build") shows
-- that symbol as its icon. The program picks the icon otherwise.
local theme = require("./theme")

local SHELLS = {
	zsh = true,
	bash = true,
	fish = true,
	sh = true,
	dash = true,
	nu = true,
	tcsh = true,
	ksh = true,
	xonsh = true,
	pwsh = true,
	login = true,
}

-- Programs by kind: the icon each shows, and a tint for agents.
local KINDS = {}
local function kind(names: { string }, icon: string, tint: string?)
	for _, name in names do
		KINDS[name] = { icon = icon, tint = tint }
	end
end
kind({ "claude" }, "claude", theme.claude)
kind({ "codex" }, "openai", "#ffffff")
kind({ "omp", "pi", "opencode", "aider", "gemini", "copilot", "amp", "cursor-agent", "goose" }, "agent", theme.accent)
kind({ "nvim", "vim", "vi", "hx", "helix", "nano", "emacs", "micro", "kak", "zed" }, "editor")
kind({ "ssh", "mosh", "et", "telnet" }, "globe")
kind({ "htop", "btop", "top", "btm", "bottom", "glances", "atop" }, "gauge")
kind({ "git", "lazygit", "tig", "gitui", "gh" }, "branch")
kind({ "docker", "podman", "kubectl", "k9s", "lazydocker" }, "box")
kind({ "cargo", "rustc", "go", "make", "cmake", "just", "ninja", "zig", "gradle", "mvn", "xcodebuild", "swift" }, "code")
kind({ "less", "more", "man", "bat", "most", "glow" }, "doc")
kind({ "psql", "mysql", "sqlite3", "redis-cli", "mongosh", "duckdb" }, "database")

-- The symbols Claude Code's title starts with, idle (✳) and working.
local CLAUDE_SPINNER = {}
for _, glyph in { "✳", "✶", "✻", "✽", "✢", "·" } do
	CLAUDE_SPINNER[glyph] = true
end

local function trim(s: string): string
	return (s:gsub("^%s+", ""):gsub("%s+$", ""))
end

-- A file URL or path, as a path ("file://host/Users/a%20b" -> "/Users/a b").
local function path_of(pwd: string): string
	local path = pwd:gsub("^file://[^/]*", "")
	path = path:gsub("%%(%x%x)", function(hex)
		return string.char(tonumber(hex, 16) :: number)
	end)
	return path
end

-- The last part of a path; home is "~".
local function short(path: string, home: string): string
	path = path:gsub("/+$", "")
	if path == "" then
		return "/"
	end
	if path == home or path == "~" then
		return "~"
	end
	return path:match("([^/]+)$") or path
end

-- A shell's title is often "user@host:dir" or a directory: the directory.
local function directory_in(title: string): string?
	local dir = title:match(":%s*([~/][^:]*)$") or title:match("^([~/].*)$")
	return dir and trim(dir)
end

local M = {}

function M.is_shell(process: string): boolean
	return process == "" or SHELLS[process] == true
end

export type Description = {
	name: string,
	icon: string,
	tint: string?,
	glyph: string?,
	busy: boolean,
}

function M.describe(term: any, home: string): Description
	local process: string = term.process
	local title = trim(term.title)
	local pwd = path_of(term.pwd)
	if M.is_shell(process) then
		local dir = if pwd ~= "" then pwd else directory_in(title)
		return {
			name = if dir then short(dir, home) else (if title ~= "" then title else "Terminal"),
			icon = "terminal",
			busy = false,
		}
	end
	local known = KINDS[process] or { icon = "run" }
	-- A leading symbol (anything not ASCII) is the program's own icon,
	-- except Claude Code's spinner: Claude shows its mark.
	local glyph, rest = title:match("^([\128-\255]+)%s+(.+)$")
	if glyph and (known.icon == "claude" or CLAUDE_SPINNER[glyph]) then
		return { name = trim(rest), icon = "claude", tint = theme.claude, busy = true }
	end
	if glyph then
		return { name = trim(rest), icon = known.icon, tint = known.tint, glyph = glyph, busy = true }
	end
	local name = process
	if title ~= "" and not directory_in(title) then
		name = title
	end
	return { name = name, icon = known.icon, tint = known.tint, busy = true }
end

M.path_of = path_of
M.short = short

return M
