-- About Ask (SettingsSections.tsx About): version and update check, build
-- versions, open-source licences with a searchable package list, links.
return function(K)
	local model = K.model
	local PACKAGES = require("./licenses")
	local C = K.C

	local VERSIONS = {
		{ "Ask", "0.1.0 · 4eb5428 · Oct 11, 2026" }, { "Chromium", "154.0.8037.58" }, { "CEF", "154.0.32+g682c378 · cef-rs 154.3.0" },
		{ "Lumine", "0.1.0 · 81ee0e9" }, { "Rust", "1.98.1 (48a229cea 2026-09-01)" }, { "macOS", "27.2 · Apple silicon" },
	}
	local NOTICES = {
		{ "Chromium & CEF", "BSD-3-Clause", "Engine notices ship inside the app bundle" }, { "V8", "BSD-3-Clause", "Upstream notices retained" },
		{ "Lumine", "MIT (inherited zgui notice)", "Fork additions carry no separate licence" },
		{ "senxd/search", "MIT", "DOM driver, tool catalog and benchmark fixtures · rev 988d075" },
		{ "Geist & Geist Mono", "SIL OFL 1.1", "Interface typefaces" }, { "Inter", "SIL OFL 1.1", "Fallback typeface" },
		{ "Lucide", "ISC", "Interface icons" }, { "Search engine icons", "Trademarks", "Belong to their respective owners" },
	}
	local function has(s, w)
		return string.find(s, w, 1, true) ~= nil
	end
	local function family(l)
		if has(l, "MIT") and has(l, "Apache") then
			return "MIT or Apache-2.0"
		elseif (l == "MIT" or (has(l, "MIT") and not has(l, "MIT OR"))) and not has(l, "Apache") then
			return "MIT"
		elseif has(l, "Apache") then
			return "Apache-2.0"
		elseif has(l, "Unicode") then
			return "Unicode-3.0"
		elseif has(l, "BSD") then
			return "BSD"
		elseif has(l, "Zlib") then
			return "Zlib"
		elseif has(l, "ISC") then
			return "ISC"
		end
		return "Other"
	end
	local FAMILIES = {}
	do
		local counts = {}
		for _, p in PACKAGES do
			local f = family(p[3])
			counts[f] = (counts[f] or 0) + 1
		end
		for f, n in counts do
			table.insert(FAMILIES, { f, n })
		end
		table.sort(FAMILIES, function(a, b)
			if a[2] ~= b[2] then
				return a[2] > b[2]
			end
			return a[1] < b[1]
		end)
	end

	local checking = ui.state("about_checking", 0)
	local checked = ui.state("about_checked", "Checked today at 9:41")
	local copied = ui.state("about_copied", false)
	local query = ui.state("about_query", "")
	local all = ui.state("about_all", false)

	local Update = ui.component(function()
		local busy = function()
			return checking:get() > 0
		end
		local function check()
			if checking:peek() > 0 then
				return
			end
			checking:set(0.01)
			anime.animate(checking, { value = 1, duration = 1600, ease = "outQuint", onComplete = function()
				checking:set(0)
				checked:set("Checked just now")
			end })
		end
		return K.Row {
			label = function()
				return busy() and "Checking for updates…" or "Ask is up to date"
			end,
			setting = "Ask is up to date", desc = function()
				return checked:get()
			end,
			props = { id = "about_update" },
			child = K.button {
				id = "check_updates", h = 30, px = 12, min_w = 136, rounded = 7, justify = "center", shrink = 0, overflow = "hidden",
				bg = C.ctl, border = 1, border_color = C.ctl_line, hover = { bg = C.hover, border_color = C.hover_line },
				on_click = check,
				ui.overlay { absolute = true, top = 0, left = 0, bottom = 0, overflow = "hidden",
					w = function()
						return checking:get() * 100 .. "%"
					end,
					hidden = function()
						return not busy()
					end,
					ui.div { absolute = true, top = 0, left = 0, bottom = 0, w = 136, hidden = function()
						return K.amount("progress") == 0
					end,
						K.dither(function()
							return K.scaled({ ink = "#5A5850FF", pixel = 1.5, mode = 0, angle = 90, from = 0.75, to = 0.15, curve = 0.9, noise = 0.2, scale = 3 }, K.amount("progress"))
						end),
					},
					ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, bg = C.strong, hidden = function()
						return K.amount("progress") > 0
					end },
				},
				ui.text { function()
					return busy() and "Checking…" or "Check for updates"
				end, id = "check_updates_label", font = K.SANS, text_size = 12.5, color = C.text },
			},
		}
	end)

	local function Packages()
		local shown = K.computed(function()
			local q = string.lower(query:get())
			local out = {}
			for _, p in PACKAGES do
				if q == "" or has(string.lower(p[1] .. " " .. p[3]), q) then
					table.insert(out, p)
				end
			end
			return out
		end)
		local function visible()
			local n = #shown:get()
			if not all:get() and query:get() == "" then
				n = math.min(12, n)
			end
			return math.min(400, n)
		end
		local ROW = 27
		return ui.overlay(K.target("Open source", {
			w_full = true, mb = 8,
			K.flash_ring("Open source"),
			ui.column { w_full = true, gap = 8, items = "start",
				K.search_box { id = "package_search", value = query, placeholder = "Search " .. #PACKAGES .. " packages or licences", props = { w_full = true } },
				ui.column { w_full = true, rounded = 10, overflow = "hidden", border = 1, border_color = C.line,
					h = function()
						return math.max(1, math.min(280, visible() * ROW + 2))
					end,
					ui.list {
						id = "package_list", w_full = true, h_full = true, height = ROW, count = visible,
						row = function(i)
							local p = shown:peek()[i]
							if not p then
								return ui.div {}
							end
							return ui.row { w_full = true, h = ROW, px = 12, gap = 8, items = "center",
								K.text(p[1], 12, C.text), K.mono(p[2], 10.5, C.muted), K.spacer(), K.mono(p[3], 10.5, C.tertiary, { truncate = true, shrink = 1 }),
							}
						end,
					},
				},
				K.fixed(ui.show(function()
					return #shown:get() == 0
				end, ui.div { px = 8, py = 2, K.text("No packages match", 12, C.muted) })),
				K.fixed(ui.show(function()
					return query:get() == "" and not all:get()
				end, K.link("Show all " .. #PACKAGES .. " packages", function()
					all:set(true)
				end, { id = "show_all_packages" }))),
				K.fixed(ui.show(function()
					return (all:get() or query:get() ~= "") and #shown:get() > 400
				end, ui.text { function()
					return "Showing 400 of " .. #shown:get() .. ". Search to narrow."
				end, font = K.SANS, text_size = 11, color = C.muted })),
			},
		}))
	end

	local function About()
		local versions = {}
		for i, v in VERSIONS do
			versions[i] = ui.row { w_full = true, gap = 16, px = 14, py = 9, items = "center",
				K.text(v[1], 12.5, C.tertiary, { w = 96, shrink = 0 }), K.mono(v[2], 12, C.text) }
		end
		local families = {}
		local family_colors = { C.secondary, C.tertiary, C.muted }
		for i, f in FAMILIES do
			families[i] = ui.column { grow = f[2], basis = 0, min_w = 44, h = 44, pl = 1, gap = 1, justify = "end", overflow = "hidden",
				ui.div { absolute = true, top = 0, left = 0, right = 0, h = 6, rounded = 2, bg = family_colors[i] or C.hover_line },
				K.text(f[1], 11, C.secondary, { truncate = true }),
				K.mono(tostring(f[2]), 10.5, C.muted),
			}
		end
		local notices = {}
		for i, n in NOTICES do
			notices[i] = ui.column { min_w = 0, gap = 2, px = 12, py = 10, bg = C.inset,
				ui.row { w_full = true, gap = 8, K.text(n[1], 12.5), K.spacer(), K.mono(n[2], 10.5, C.tertiary) },
				K.text(n[3], 11, C.muted, { wrap = true }),
			}
		end
		local function links(labels, after, prefix)
			local out = {}
			for i, l in labels do
				out[i] = K.btn { label = l, after = after, id = prefix .. string.lower(l:gsub("%s", "_")) }
			end
			return ui.row { w_full = true, mt = 4, gap = 8, flex_wrap = true, out }
		end
		return {
			ui.row { w_full = true, gap = 16, pb = 20, items = "center",
				ui.div { p = 6, rounded = 18, shrink = 0, border = 1, border_color = C.ctl_line,
					gradient = { angle = 180, stops = { C.raised, C.panel } }, shadow = { x = 0, y = 8, blur = 24, color = "#00000066" },
					K.Orb { size = 56, lit = model.ai, active = true } },
				ui.column { gap = 4,
					K.text("Ask", 22, C.text, { weight = 500, letter_spacing = -0.3, line_height = 29 }),
					K.text("Version 0.1.0 (4eb5428) · Development build", 13, C.tertiary),
				},
			},
			Update {},
			K.SectionRow("VERSIONS",
				K.fixed(ui.presence(copied, function(done)
					return K.btn { small = true, id = "copy_versions", label = done and "Copied" or "Copy", icon = done and "check" or "copy",
						on_click = function()
							copied:set(true)
							K.after(1400, function()
								copied:set(false)
							end)
						end,
					}
				end))),
			ui.overlay(K.target("Versions", {
				w_full = true, mt = 6, mb = 4,
				K.flash_ring("Versions"),
				ui.column { id = "versions", w_full = true, rounded = 10, overflow = "hidden", border = 1, border_color = C.line, versions },
			})),
			K.Section("OPEN SOURCE"),
			ui.text { "Ask is built on open-source software. " .. #PACKAGES .. " Rust packages are bundled; their licences and notices ship in the app's licenses folder.",
				font = K.SANS, text_size = 12.5, line_height = 18.75, color = C.secondary, wrap = true, align = "left", mx = 2, mt = 2, mb = 12 },
			ui.row { w_full = true, gap = 2, h = 44, mb = 14, families },
			ui.grid { w_full = true, cols = 2, gap = 1, mb = 14, rounded = 10, overflow = "hidden", border = 1, border_color = C.line, bg = C.subtle, notices },
			Packages(),
			K.Section("MORE"),
			links({ "Release notes", "Privacy policy", "Terms of service" }, "arrow-up-right", "about_"),
			K.Section("DESIGN PREVIEWS"),
			links({ "Controls", "Tabs", "Composer", "Trace", "Navbar" }, nil, "preview_"),
			K.text("© 2026 Ask. Chromium is a trademark of Google LLC.", 11, C.muted, { mt = 22, mx = 2 }),
		}
	end

	return { ["About Ask"] = About }
end
