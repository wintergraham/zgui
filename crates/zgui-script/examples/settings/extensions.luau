-- Extensions (installed extensions with pins and switches) and Downloads
-- (location, behaviour, the assistant's downloads and recent files);
-- SettingsSections.tsx Extensions, Downloads.
return function(K)
	local model = K.model
	local C = K.C

	local function header_row(label, trail)
		return ui.row { w_full = true, gap = 12, items = "center", K.fixed(label), K.spacer(), trail }
	end
	local function letter_tile(letter, hue, size)
		return ui.row {
			w = size, h = size, rounded = 7, shrink = 0, items = "center", justify = "center", bg = K.oklch(0.42, 0.07, hue),
			ui.text { letter, font = K.SANS, text_size = 13, weight = 600, color = "#FFFFFF" },
		}
	end

	---------------------------------------------------------------------------
	-- Extensions: the pin puts an extension's button in the address bar;
	-- switching one off keeps it installed. Developer mode shows IDs.
	---------------------------------------------------------------------------
	local function edit(id, changes)
		local list = {}
		for i, x in model.extensions:peek() do
			if x.id == id then
				x = table.clone(x)
				for k, v in changes do
					x[k] = v
				end
			end
			list[i] = x
		end
		model.extensions:set(list)
	end
	local ExtensionRow = ui.component(function(props)
		local x0 = props.x
		local hovered = ui.use_state(false)
		local function x()
			for _, e in model.extensions:get() do
				if e.id == x0.id then
					return e
				end
			end
			return x0
		end
		local function dim()
			return K.tween(function()
				return x().on and 1 or 0.45
			end, K.BASE)
		end
		local function dev()
			return model.flag("Developer mode")
		end
		return ui.row {
			id = "extension_" .. x0.id, w_full = true, gap = 12, px = 14, py = 11, items = "center",
			on_hover = function(inside)
				hovered:set(inside)
			end,
			ui.div { opacity = dim(), letter_tile(x0.name:sub(1, 1), x0.hue, 28) },
			ui.column { grow = 1, shrink = 1, min_w = 0, gap = 2, opacity = dim(),
				ui.row { gap = 8, items = "end", K.text(x0.name, 13.5, C.text, { truncate = true, shrink = 1 }), K.mono(x0.version, 10.5, C.muted) },
				ui.text { function()
					return dev() and x0.id or x0.desc
				end, font = function()
					return dev() and K.MONO or K.SANS
				end, text_size = function()
					return dev() and 10.5 or 12
				end, color = function()
					return dev() and C.muted or C.tertiary
				end, truncate = true, align = "left" },
			},
			K.button {
				id = "pin_" .. x0.id, w = 26, h = 26, rounded = 6, justify = "center", shrink = 0, hover = { bg = C.hover },
				opacity = K.tween(function()
					local e = x()
					if not e.on then
						return 0
					end
					return (e.pinned or hovered:get()) and 1 or 0
				end, K.FAST),
				disabled = function()
					return not x().on
				end,
				on_click = function()
					edit(x0.id, { pinned = not x().pinned })
				end,
				K.icon("pin", 13, nil, { tint = function()
					return x().pinned and C.text or C.muted
				end }),
			},
			K.Toggle { id = "extension_toggle_" .. x0.id, label = x0.name, on = function()
				return x().on
			end, set = function(v)
				edit(x0.id, { on = v })
			end },
		}
	end)
	local function Extensions()
		local rows = {}
		for i, x in model.extensions:peek() do
			rows[i] = ExtensionRow { x = x }
		end
		return {
			K.page_header("Extensions"),
			header_row(ui.div { pt = 22, pb = 8, px = 2,
				ui.text { function()
					local on, n = 0, 0
					for _, x in model.extensions:get() do
						n += 1
						if x.on then
							on += 1
						end
					end
					return "INSTALLED · " .. on .. " OF " .. n .. " ON"
				end, id = "extensions_count", font = K.MONO, text_size = 10, letter_spacing = 1.2, color = C.muted },
			}, ui.row { gap = 12, items = "center",
				K.fixed(ui.show(function()
					return model.flag("Developer mode")
				end, K.btn { small = true, icon = "folder-input", label = "Load unpacked", id = "load_unpacked" })),
				K.btn { small = true, label = "Chrome Web Store", after = "arrow-up-right", id = "web_store" },
			}),
			ui.overlay(K.target("Installed extensions", {
				w_full = true,
				K.flash_ring("Installed extensions"),
				ui.column { id = "extension_list", w_full = true, rounded = 10, overflow = "hidden", border = 1, border_color = C.line, rows },
			})),
			K.Section("ACCESS"),
			K.flag("Run extensions in agent tabs", "Content scripts also run on pages the assistant drives. Off keeps agent tabs clean so extensions can't change what it reads."),
			K.flag("Developer mode", "Show extension IDs and load unpacked extensions from a folder.", nil, "developer_mode"),
		}
	end

	---------------------------------------------------------------------------
	-- Downloads
	---------------------------------------------------------------------------
	local RECENT = {
		{ name = "ask-0.1.1-arm64.dmg", icon = "file-down", size = "214 of 338 MB", site = "github.com", when = "", progress = 0.63 },
		{ name = "Q3 board deck.pdf", icon = "file-text", size = "4.8 MB", site = "drive.google.com", when = "9:12" },
		{ name = "opentable-receipt-oct-10.pdf", icon = "file-text", size = "182 KB", site = "opentable.com", when = "Yesterday" },
		{ name = "transactions-2026-09.csv", icon = "file-spreadsheet", size = "36 KB", site = "mercury.com", when = "Oct 2" },
		{ name = "geist-mono.zip", icon = "file", size = "1.1 MB", site = "vercel.com", when = "Sep 28" },
	}
	local AGENT_DOWNLOADS = { "Ask every time", "Allow from the current site", "Never" }
	local cleared = ui.state("downloads_cleared", false)
	local function recent()
		if not cleared:get() then
			return RECENT
		end
		local out = {}
		for _, x in RECENT do
			if x.progress then
				table.insert(out, x)
			end
		end
		return out
	end
	local function DownloadRow(x)
		local meta = {}
		for _, part in { x.size, x.site, x.when } do
			if part ~= "" then
				table.insert(meta, part)
			end
		end
		return ui.row {
			w_full = true, gap = 12, px = 14, py = 11, items = "center",
			ui.row { w = 28, h = 28, rounded = 7, shrink = 0, items = "center", justify = "center", bg = C.ctl, border = 1, border_color = C.ctl_line, K.icon(x.icon, 15, C.secondary) },
			ui.column { grow = 1, shrink = 1, min_w = 0, gap = 2,
				K.text(x.name, 13.5, C.text, { truncate = true }),
				K.text(table.concat(meta, " · "), 12, C.tertiary, { truncate = true }),
				x.progress and ui.overlay { w_full = true, h = 4, mt = 6, rounded = 2, overflow = "hidden", bg = C.subtle,
					ui.overlay { absolute = true, top = 0, left = 0, bottom = 0, w = x.progress * 100 .. "%", rounded = 2, overflow = "hidden",
						K.dither(function()
							return K.scaled({ ink = "#EDEBE5FF", pixel = 1.5, mode = 0, angle = 90, from = 0.15, to = 0.9, curve = 1 }, K.amount("progress"))
						end, { hidden = function()
							return K.amount("progress") == 0
						end }),
						ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, bg = C.secondary, hidden = function()
							return K.amount("progress") > 0
						end },
					},
				} or nil,
			},
			x.progress and K.btn { small = true, label = "Pause", id = "download_pause" }
				or K.button { w = 26, h = 26, rounded = 6, justify = "center", shrink = 0, hover = { bg = C.hover }, K.icon("folder", 13, C.muted) },
		}
	end
	local function Downloads()
		return {
			K.page_header("Downloads"),
			K.Row { label = "Location", desc = "~/Downloads", child = K.btn { icon = "folder", label = "Change…", id = "download_location" } },
			K.flag("Ask where to save each file", "Choose a folder and name every time instead of saving to Location."),
			K.flag("Open safe files after downloading", "PDFs, images and text. Archives and installers never open on their own."),
			K.flag("Show downloads as they start", "A bubble under the address bar with progress for each file."),
			K.Section("ASSISTANT"),
			K.Row {
				label = "Files the assistant downloads",
				desc = function()
					return (model.ai:get() and "" or "Needs AI features. ") .. "Saved to Location in an Ask Agent folder and listed in the chat."
				end,
				disabled = function()
					return not model.ai:get()
				end,
				child = ui.div {
					disabled = function()
						return not model.ai:get()
					end,
					K.Select { id = "agent_downloads", options = AGENT_DOWNLOADS, value = function()
						return model.agentDownloads:get()
					end, pick = function(v)
						model.agentDownloads:set(v)
					end },
				},
			},
			K.SectionRow("RECENT", K.fixed(ui.show(function()
				return not cleared:get()
			end, K.btn { small = true, label = "Clear", id = "downloads_clear", on_click = function()
				cleared:set(true)
			end }))),
			ui.overlay(K.target("Recent downloads", {
				w_full = true,
				K.flash_ring("Recent downloads"),
				ui.column { id = "download_list", w_full = true, rounded = 10, overflow = "hidden", border = 1, border_color = C.line,
					ui.presence(function()
						return #recent()
					end, function()
						local rows = {}
						for i, x in recent() do
							rows[i] = DownloadRow(x)
						end
						if #rows == 0 then
							rows[1] = ui.div { px = 8, py = 10, K.text("No downloads yet", 12, C.muted) }
						end
						return ui.column { w_full = true, rows }
					end),
				},
			})),
			ui.row { mt = 4, gap = 8, K.btn { label = "Open Downloads folder", after = "arrow-up-right", id = "open_downloads" } },
		}
	end

	return { Extensions = Extensions, Downloads = Downloads }
end
