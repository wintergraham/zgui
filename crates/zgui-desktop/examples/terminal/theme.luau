-- Dark glass, as the browser example: the window's blur shows through the
-- tint, and foregrounds are translucent white so they take the colour of
-- what is behind the glass, as AppKit's vibrant labels do.
return {
	tint = "#13141873",
	-- Each terminal's card: a little darker than the window, still glass.
	pane = "#0a0b0e4d",
	wash = "#ffffff0f",
	hover = "#ffffff0a",
	line = "#ffffff0d",
	ring = "#ffffff26",
	text = "#ffffffeb",
	secondary = "#ffffffa6",
	tertiary = "#ffffff66",
	quaternary = "#ffffff38",
	accent = "#a99cff",
	bell = "#f5c46b",
	claude = "#d97757",
	danger = "#ff7a6b",

	-- Space between the window's edge and the panes, and between panes
	-- (less: two borders meet there, which reads as more space).
	inset = 6,
	-- The interface's typeface (the host registers it).
	font = "Geist",
	gap = 4,
	titlebar = 40,
}
