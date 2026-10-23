-- Appearance: theme, tint, reduce motion and the dither card, a live preview
-- stage whose surfaces lift as their slider is hovered (Settings.tsx
-- DitherSettings, DitherStage, DitherSlider).
return function(K)
	local model = K.model
	local C = K.C

	local SURFACES = {
		{ "background", "Window background", "The glow in the corner and the drops along the top" },
		{ "progress", "Loading & progress", "Tab loads, saves and the connecting bar" },
		{ "highlight", "Agent highlight", "The focus drawn around what the agent is about to use" },
		{ "orbs", "Orbs", "The assistant's shaded sphere wherever it appears" },
	}
	-- 0 is "Disable dither": every surface draws as a solid shape instead.
	local PRESETS = { { 0, "Disable dither" }, { 0.5, "Subtle" }, { 1, "Default" }, { 1.6, "Rich" } }

	local function set_dither(changes)
		model.patch(model.dither, changes)
	end

	-- The preview: a miniature window with each surface at its amount.
	local function Stage(focus)
		local function cls(surface)
			return K.tween(function()
				local f = focus:get()
				return (f ~= nil and f ~= surface) and 0.3 or 1
			end, K.BASE)
		end
		local function ring(surface, radius)
			return ui.div {
				absolute = true, top = 0, left = 0, right = 0, bottom = 0, rounded = radius, pointer_events = "none",
				border = 1, border_color = K.P("EDEBE5", 0.67),
				hidden = function()
					return focus:get() ~= surface
				end,
			}
		end
		local ai = model.ai
		local wash = ui.ref()
		local function solid()
			return K.amount("progress") == 0
		end
		local tab_load = ui.overlay {
			h = 24, w = 132, rounded = 7, overflow = "hidden", bg = C.page, opacity = cls("progress"),
			ui.div {
				ref = wash, absolute = true, top = 0, left = 0, bottom = 0, w = "62%", overflow = "hidden",
				on_mount = function(el)
					if not K.reduced() then
						anime.animate(el, { w = { "0%", "100%" }, duration = 2600, ease = "outQuint", loop = true, loopDelay = 910 })
					end
				end,
				ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, bg = C.strong, hidden = function()
					return not solid()
				end },
				ui.div { absolute = true, top = 0, left = 0, bottom = 0, w = 132, hidden = solid,
					K.dither(function()
						return K.scaled(K.U.load, K.amount("progress"))
					end, nil, 0.37 * 12),
				},
			},
			ui.row { h_full = true, px = 8, gap = 6, items = "center",
				K.icon("loader-circle", 11, C.tertiary), K.text("Loading…", 11, C.tertiary),
			},
			ring("progress", 7),
		}
		local cta = ui.overlay {
			self = "start", mt = 10,
			ui.overlay {
				absolute = true, top = -22, left = -34, right = -34, bottom = -22, rounded = 16, overflow = "hidden", opacity = cls("highlight"),
				hidden = function()
					return not ai:get() or K.amount("highlight") == 0
				end,
				K.dither(function()
					return K.scaled(K.U.focus, K.amount("highlight"))
				end, nil, 0.53 * 12),
				ui.div { absolute = true, top = 0, left = 0, right = 0, bottom = 0, rounded = 16, border = 1, border_dash = { 4, 3 }, border_color = K.P("EDEBE5", 0.47), hidden = function()
					return focus:get() ~= "highlight"
				end },
			},
			ui.div {
				absolute = true, top = -5, left = -5, right = -5, bottom = -5, rounded = 11, border = 1.5, border_color = C.text, opacity = cls("highlight"),
				hidden = function()
					return not ai:get() or K.amount("highlight") > 0
				end,
			},
			ui.row { h = 28, px = 12, rounded = 7, bg = C.text, items = "center", K.text("Reserve · 8:00", 11.5, C.panel, { weight = 500 }) },
			ui.div { absolute = true, right = -14, bottom = -12, hidden = function()
				return not ai:get()
			end, K.icon("mouse-pointer-2", 14, C.text) },
		}
		local tag = {}
		for _, s in SURFACES do
			tag[s[1]] = s[2]
		end
		return ui.overlay {
			id = "dither_stage", w_full = true, h = 232, overflow = "hidden", bg = C.window,
			ui.overlay { absolute = true, top = 0, left = 0, right = 0, bottom = 0, opacity = cls("background"),
				K.dither(function()
					return K.scaled(K.U.page, K.amount("background"))
				end),
				ui.overlay { absolute = true, top = 0, left = 0, right = 0, h = 130,
					K.dither(function()
						return K.scaled(K.U.drops, K.amount("background"))
					end),
				},
			},
			ui.row { absolute = true, top = 14, left = 0, right = 0, justify = "center",
				ui.row { gap = 4, p = 3, rounded = 10, bg = C.panel, border = 1, border_color = C.line,
					ui.row { h = 24, w = 132, px = 8, gap = 6, items = "center", rounded = 7, bg = C.raised,
						K.icon("globe", 11, C.soft), K.text("opentable.com", 11, C.soft, { truncate = true }),
					},
					tab_load,
				},
			},
			ui.row { absolute = true, top = 60, bottom = -12, left = 0, right = 0, justify = "center",
				ui.column { w = "70%", max_w = 380, h_full = true, gap = 10, px = 22, py = 20, rounded = 10, bg = C.page, border = 1, border_color = C.subtle,
					ui.div { w = "46%", h = 10, rounded = 4, bg = C.strong, mb = 4 },
					ui.div { w = "72%", h = 7, rounded = 4, bg = C.selected },
					ui.div { w = "64%", h = 7, rounded = 4, bg = C.selected },
					cta,
				},
			},
			ui.overlay { absolute = true, right = 14, bottom = 14, opacity = cls("orbs"),
				ui.row { h = 30, pl = 7, pr = 12, gap = 8, items = "center", rounded = 15, bg = C.panel, border = 1, border_color = C.line,
					K.Orb { size = 18, lit = ai },
					ui.text { function()
						return ai:get() and "Reading page" or "AI off"
					end, font = K.SANS, text_size = 11.5, color = C.secondary },
				},
				ring("orbs", 15),
			},
			ui.div {
				absolute = true, left = 12, bottom = 10, px = 6, py = 3, rounded = 5, bg = K.P("121211", 0.8), border = 1, border_color = C.strong,
				hidden = function()
					return focus:get() == nil
				end,
				ui.text { function()
					local f = focus:get()
					return f and string.upper(tag[f]) or ""
				end, id = "stage_tag", font = K.MONO, text_size = 10, letter_spacing = 1.2, color = C.secondary },
			},
		}
	end

	-- A surface's amount: the dither ramp itself, lit up to the thumb; past
	-- the thumb it shows faintly as a preview of the range.
	local function Slider(surface, label, desc, focus)
		local value = ui.use_state(model.dither:peek()[surface])
		local width = ui.use_state(300)
		local hovered = ui.use_state(false)
		ui.effect(function()
			value:set(model.dither:get()[surface])
		end)
		ui.effect(function()
			local v = math.floor(value:get() / 0.05 + 0.5) * 0.05
			if math.abs(v - model.dither:peek()[surface]) > 1e-6 then
				set_dither { [surface] = v }
			end
		end)
		local function frac()
			return model.dither:get()[surface] / 2
		end
		local function solid()
			return model.dither:get().master == 0
		end
		local track_dither = function(alpha)
			return K.dither(function()
				local u = K.U.track
				return K.scaled(u, 1)
			end, { opacity = alpha })
		end
		return ui.column {
			w_full = true, gap = 8,
			on_hover = function(inside)
				hovered:set(inside)
				if inside then
					focus:set(surface)
				elseif focus:peek() == surface then
					focus:set(nil)
				end
			end,
			ui.row { w_full = true, items = "end",
				K.text(label, 13.5), K.spacer(),
				ui.text { function()
					return math.floor(frac() * 200 + 0.5) .. "%"
				end, id = "dither_" .. surface .. "_value", font = K.MONO, text_size = 11, color = C.secondary },
			},
			ui.overlay {
				id = "dither_" .. surface, w_full = true, h = 24, rounded = 7, overflow = "hidden", bg = C.panel, border = 1,
				border_color = K.tween(function()
					return hovered:get() and C.hover_line or C.line
				end, K.FAST),
				on_resize = function(size)
					width:set(size.w)
				end,
				ui.div {
					absolute = true, top = 0, left = 0, bottom = 0, corners = { 6, 0, 0, 6 }, bg = C.focus,
					hidden = function()
						return not solid()
					end,
					w = function()
						return frac() * 100 .. "%"
					end,
				},
				ui.overlay { absolute = true, top = 0, left = 0, right = 0, bottom = 0, hidden = solid,
					track_dither(0.16),
					ui.div {
						absolute = true, top = 0, left = 0, bottom = 0, overflow = "hidden",
						w = function()
							return frac() * 100 .. "%"
						end,
						ui.div { absolute = true, top = 0, left = 0, bottom = 0, w = function()
							return width:get()
						end, track_dither(1) },
					},
				},
				ui.div {
					absolute = true, top = 3, bottom = 3, w = 3, ml = -1.5, rounded = 2, bg = C.text,
					shadow = { x = 0, y = 0, blur = 0, spread = 2, color = C.panel },
					left = function()
						return frac() * math.max(0, width:get() - 2)
					end,
				},
				-- The engine's slider keeps dragging, keys and accessibility under the drawn track.
				ui.slider { id = "dither_" .. surface .. "_input", label = label, value = value, min = 0, max = 2, w_full = true, h_full = true, p = 0, opacity = 0.001, bg = C.clear, border = 0 },
			},
			ui.text { desc, align = "left", font = K.SANS, text_size = 12, line_height = 17.4, color = C.tertiary, wrap = true },
		}
	end

	local Dither = ui.component(function()
		local focus = ui.use_state(nil)
		local grid = {}
		for i, s in SURFACES do
			grid[i] = ui.div { w = "50%", px = 12, pb = 18, Slider(s[1], s[2], s[3], focus) }
		end
		local d = model.dither
		return ui.overlay(K.target("Dither", {
			w_full = true,
			K.flash_ring("Dither", 12),
			ui.column {
				id = "dither_card", w_full = true, rounded = 12, overflow = "hidden", bg = C.inset, border = 1, border_color = C.line,
				Stage(focus),
				ui.column { w_full = true, p = 16, gap = 18,
					ui.row { w_full = true, gap = 24, items = "center",
						ui.column { grow = 1, shrink = 1, min_w = 0, gap = 4,
							K.text("Intensity", 13.5), K.text("Scales every surface below", 12, C.tertiary),
						},
						K.Segmented {
							id = "intensity", danger = 0, options = PRESETS,
							value = function()
								local m = d:get().master
								for _, p in PRESETS do
									if math.abs(p[1] - m) < 0.001 then
										return p[1]
									end
								end
								return -1
							end,
							pick = function(v)
								set_dither { master = v }
							end,
						},
					},
					ui.row { w_full = true, mx = -12, mb = -18, flex_wrap = true, grid },
					ui.row { w_full = true, gap = 16, items = "center", pt = 2,
						K.Segmented {
							id = "grain", small = true, options = { { "fine", "Fine" }, { "default", "Default" }, { "coarse", "Coarse" } },
							value = function()
								return d:get().grain
							end,
							pick = function(v)
								set_dither { grain = v }
							end,
						},
						ui.row { gap = 8, items = "center",
							K.Toggle { id = "animate_dither", label = "Animate dither", on = function()
								return d:get().animate
							end, set = function(v)
								set_dither { animate = v }
							end },
							K.text("Animate at 12 fps", 12, C.secondary),
						},
						K.spacer(),
						K.link("Reset", function()
							d:set(table.clone(model.DEFAULT_DITHER))
						end, { id = "dither_reset" }),
					},
				},
			},
		}))
	end)

	local pages = {}
	pages.Appearance = function()
		local tints = {}
		for i, t in K.TINTS do
			local id, label, h = t[1], t[2], t[3]
			local function on()
				return model.tint:get() == id
			end
			tints[i] = K.button({
				id = "tint_" .. id, w = 26, h = 26, p = 3, rounded = 13, justify = "center", border = 1.5,
				border_color = function()
					return on() and C.text or C.clear
				end,
				hover = { border_color = C.strong },
				on_click = function()
					model.tint:set(id)
				end,
				ui.div { w = 20, h = 20, rounded = 10, bg = K.swatch("858379", h), border = 4, border_color = K.swatch("3A3934", h) },
			})
		end
		return {
			K.page_header("Appearance"),
			K.Row { label = "Theme", desc = "System follows the macOS appearance.", child = K.Segmented {
				id = "theme", options = K.THEMES,
				value = function()
					return model.theme:get()
				end,
				pick = function(v)
					model.theme:set(v)
				end,
			} },
			K.Row {
				label = "Tint",
				desc = function()
					for _, t in K.TINTS do
						if t[1] == model.tint:get() then
							return t[2] .. (t[1] == "stone" and " · the native warm grey" or " · every neutral leans toward this hue")
						end
					end
					return ""
				end,
				child = ui.row { gap = 6, shrink = 0, tints },
			},
			K.flag("Reduce motion", "Keep dither patterns static and shorten transitions", nil, "reduce_motion"),
			K.Section("DITHER"),
			Dither {},
			ui.row { mt = 16, gap = 8, K.btn { label = "Preview motion states", id = "preview_motion" } },
		}
	end
	return pages
end
