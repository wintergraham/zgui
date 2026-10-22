--!strict
-- Type definitions for zgui-script's globals, `ui` and `anime`, for
-- luau-lsp (editor completion and checking) and `luau-lsp analyze`:
--   luau-lsp analyze --definitions=crates/zgui-script/types/zgui.d.luau app.luau
-- In VS Code, set `luau-lsp.platform.type` to "standard" (requires resolve
-- by path), put `"languageMode": "strict"` in the project's .luaurc, add
-- this file to `luau-lsp.types.definitionFiles`, and turn on
-- Luau's new type solver (`"luau-lsp.fflags.override": { "LuauSolverV2":
-- "true" }`, or `--flag:LuauSolverV2=true`): it reports misspelled
-- properties, which the old solver lets through.
-- The style properties and animation keys are generated from the engine's
-- tables; `tests/types.rs` fails if they drift.

type Color = string | number
-- Points, or a string: "50%", "auto", "1.5rem".
type Len = number | string
type Pair = number | { number }
type Corners = { number } | { tl: number?, tr: number?, br: number?, bl: number? }
type ShadowSpec = { x: number?, y: number?, blur: number?, spread: number?, color: Color? }
type Shadow = ShadowSpec | { ShadowSpec } | false
type Gradient = { Color | { any } } | { angle: number?, stops: { Color | { any } } }

-- What a script keeps: Rust-owned state.
type Value = nil | boolean | number | string | { [any]: any }

declare extern type State with
	function get(self): any
	function peek(self): any
	function set(self, value: any): ()
	function update(self, f: (any) -> any): ()
	function toggle(self): ()
	function add(self, by: number?): ()
	function push(self, item: any): ()
	function remove(self, index: number): any
	function len(self): number
end
declare extern type Node with end
declare extern type Ref with end
declare extern type Action with end
declare extern type Binding with end
declare extern type Bitmap with
	w: number
	h: number
end
declare extern type Element with
	id: string?
	tag: string
	mounted: boolean
	width: number?
	height: number?
	x: number?
	y: number?
	function has_class(self, class: string): boolean
	-- Keyboard focus to the element, or the first focusable node inside it
	-- (an input's editor); false when nothing there takes focus.
	function focus(self): boolean
	function blur(self): ()
end
declare extern type Path with
	function move(self, x: number, y: number): Path
	function line(self, x: number, y: number): Path
	function quad(self, cx: number, cy: number, x: number, y: number): Path
	function cubic(self, c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number): Path
	function arc(self, cx: number, cy: number, r: number, start: number, sweep: number): Path
	function close(self): Path
end
-- `ui.web(opts)`: a web page's controller. Its fields are tracked, so a
-- text or computed reading `web.title` follows the page.
declare extern type Web with
	url: string
	title: string
	loading: boolean
	progress: number
	can_go_back: boolean
	can_go_forward: boolean
	focused: boolean
	error: string?
	function load(self, url: string): ()
	function html(self, html: string, base_url: string?): ()
	function back(self): ()
	function forward(self): ()
	function reload(self): ()
	function stop(self): ()
	function focus(self): ()
	function blur(self): ()
	function devtools(self): ()
	function zoom(self, zoom: number): ()
	function run(self, js: string): ()
	-- The script's value as Luau data, or nil and the error.
	function eval(self, js: string, reply: ((value: any, err: string?) -> ())?): ()
end
type WebOptions = {
	transparent: boolean?,
	-- "default" (shared cookies and storage), "ephemeral", or a name.
	profile: string?,
	inspectable: boolean?,
	user_agent: string?,
	-- `window.zgui.postMessage(value)` from the page reaches on_message.
	messages: boolean?,
	new_windows: boolean?,
}
type WebEvent = { kind: "started" | "committed" | "finished" | "failed" | "changed", url: string }
--- `ui.term(id, opts)`: a terminal session (a Ghostty surface with the
--- desktop host's `ghostty` feature). Sessions outlive reloads: the run that
--- asks for `id` again gets the same one. Its fields are tracked.
declare extern type Term with
	id: string
	-- What the running program set (OSC 0/2); "" until it does.
	title: string
	-- The shell's working directory (OSC 7); "" until reported.
	pwd: string
	-- The foreground process ("zsh", "nvim", "claude").
	process: string
	pid: number
	focused: boolean
	-- The bell rang since the terminal last had focus.
	bell: boolean
	-- A program's progress (OSC 9;4): 0 to 1, negative while indeterminate.
	progress: number?
	exited: number?
	columns: number
	rows: number
	-- The pointer over it, in points from its top left; nil outside.
	pointer: { x: number, y: number }?
	function focus(self): ()
	function blur(self): ()
	-- Text as if typed ("ls\r" runs ls).
	function input(self, text: string): ()
	function paste(self, text: string): ()
	-- A Ghostty binding action: "copy_to_clipboard", "clear_screen", ...
	function binding(self, action: string): ()
	-- Whether closing would end something other than an idle shell.
	function needs_confirm(self): boolean
	-- End the session (its process is hung up).
	function close(self): ()
end
type TermOptions = {
	-- A command line; nil runs the login shell.
	command: string?,
	-- Where it starts ("~" is home); nil is home.
	cwd: string?,
	env: { [string]: string }?,
	font_size: number?,
	-- Handlers (the latest run's are called).
	on_title: ((title: string) -> ())?,
	on_pwd: ((pwd: string) -> ())?,
	on_process: ((name: string) -> ())?,
	on_exit: ((code: number) -> ())?,
	-- The terminal asks to close: its process exited (alive = false), or
	-- a close binding ran with something still running.
	on_close: ((alive: boolean) -> ())?,
	on_bell: (() -> ())?,
	-- A shell-integrated command finished.
	on_command: ((exit_code: number?, duration_ms: number) -> ())?,
	on_notify: ((title: string, body: string) -> ())?,
}
--- `ui.command(opts)`: an app command, shown as a menu item; its key
--- reaches it wherever focus is (terminals and web pages included).
type CommandOptions = {
	id: string?,
	title: string?,
	-- "File", "Edit", "View", "Window" or any other menu.
	menu: string?,
	-- Modifiers and a key joined by "+": "cmd+shift+d", "cmd+alt+arrowleft".
	key: string?,
	run: (() -> ())?,
	-- A separator line in `menu` instead of a command.
	separator: boolean?,
}
declare extern type Brush with end
declare extern type Shader with end
type Paint = Color | Brush
declare extern type Canvas with
	function rect(self, x: number, y: number, w: number, h: number, paint: Paint, radius: number?): ()
	function circle(self, x: number, y: number, r: number, paint: Paint): ()
	function line(self, x1: number, y1: number, x2: number, y2: number, paint: Paint, width: number?, opts: { [string]: any }?): ()
	function fill(self, path: Path, paint: Paint, rule: string?): ()
	function stroke(self, path: Path, paint: Paint, width: number?, opts: { cap: string?, join: string?, dash: { number }?, dash_offset: number?, draw: (number | { number })? }?): ()
	function clip(self, path: Path): ()
	function restore(self): ()
end

-- A property's value: a literal, a state, a function of state (rerun when
-- what it reads changes) or an animation binding.
type P<T> = T | State | (() -> T) | Binding
-- Called with nothing; what it returns is ignored.
type Handler = () -> ...unknown
type Child = Node | string | (() -> string) | { Child } | false | nil

-- Plain styles, for `hover`, `active`, `focus` and `disabled`.
type Styles = {
	w: Len?,
	h: Len?,
	size: Pair?,
	min_w: Len?,
	max_w: Len?,
	min_h: Len?,
	max_h: Len?,
	w_full: boolean?,
	h_full: boolean?,
	aspect: number?,
	absolute: boolean?,
	relative: boolean?,
	top: Len?,
	left: Len?,
	right: Len?,
	bottom: Len?,
	p: Len?,
	px: Len?,
	py: Len?,
	pt: Len?,
	pr: Len?,
	pb: Len?,
	pl: Len?,
	m: Len?,
	mx: Len?,
	my: Len?,
	mt: Len?,
	mr: Len?,
	mb: Len?,
	ml: Len?,
	gap: number?,
	gap_x: Len?,
	gap_y: Len?,
	direction: string?,
	grow: number?,
	shrink: number?,
	basis: Len?,
	flex_wrap: boolean?,
	wrap_reverse: boolean?,
	items: string?,
	justify: string?,
	self: string?,
	content: string?,
	grid: boolean?,
	cols: number?,
	rows: number?,
	col_start: number?,
	row_start: number?,
	col_span: number?,
	row_span: number?,
	col_span_full: boolean?,
	row_span_full: boolean?,
	overflow: string?,
	overflow_x: string?,
	overflow_y: string?,
	hidden: boolean?,
	invisible: boolean?,
	bg: Color?,
	gradient: Gradient?,
	color: Color?,
	bg_image: string?,
	bg_fit: string?,
	rounded: number?,
	corners: Corners?,
	rounded_tl: number?,
	rounded_tr: number?,
	rounded_br: number?,
	rounded_bl: number?,
	border: number?,
	border_sides: Corners?,
	border_color: Color?,
	border_over: boolean?,
	border_dash: Pair?,
	shadow: Shadow?,
	opacity: number?,
	brightness: number?,
	contrast: number?,
	saturate: number?,
	blur: number?,
	backdrop_blur: number?,
	fade: Pair?,
	fade_x: Pair?,
	fade_sides: Corners?,
	clip_inset: Corners?,
	layer: boolean?,
	fit: string?,
	tint: Color?,
	cursor: string?,
	text_size: number?,
	weight: number?,
	bold: boolean?,
	italic: boolean?,
	align: string?,
	font: string?,
	wrap: boolean?,
	truncate: boolean?,
	lines: number?,
	line_height: number?,
	letter_spacing: number?,
	x: number?,
	y: number?,
	scale: number?,
	scale_x: number?,
	scale_y: number?,
	rotate: number?,
	skew: number?,
	skew_x: number?,
	skew_y: number?,
	origin: Pair?,
}

type Props = {
	w: P<Len>?,
	h: P<Len>?,
	size: P<Pair>?,
	min_w: P<Len>?,
	max_w: P<Len>?,
	min_h: P<Len>?,
	max_h: P<Len>?,
	w_full: P<boolean>?,
	h_full: P<boolean>?,
	aspect: P<number>?,
	absolute: P<boolean>?,
	relative: P<boolean>?,
	top: P<Len>?,
	left: P<Len>?,
	right: P<Len>?,
	bottom: P<Len>?,
	p: P<Len>?,
	px: P<Len>?,
	py: P<Len>?,
	pt: P<Len>?,
	pr: P<Len>?,
	pb: P<Len>?,
	pl: P<Len>?,
	m: P<Len>?,
	mx: P<Len>?,
	my: P<Len>?,
	mt: P<Len>?,
	mr: P<Len>?,
	mb: P<Len>?,
	ml: P<Len>?,
	gap: P<number>?,
	gap_x: P<Len>?,
	gap_y: P<Len>?,
	direction: P<string>?,
	grow: P<number>?,
	shrink: P<number>?,
	basis: P<Len>?,
	flex_wrap: P<boolean>?,
	wrap_reverse: P<boolean>?,
	items: P<string>?,
	justify: P<string>?,
	self: P<string>?,
	content: P<string>?,
	grid: P<boolean>?,
	cols: P<number>?,
	rows: P<number>?,
	col_start: P<number>?,
	row_start: P<number>?,
	col_span: P<number>?,
	row_span: P<number>?,
	col_span_full: P<boolean>?,
	row_span_full: P<boolean>?,
	overflow: P<string>?,
	overflow_x: P<string>?,
	overflow_y: P<string>?,
	hidden: P<boolean>?,
	invisible: P<boolean>?,
	bg: P<Color>?,
	gradient: P<Gradient>?,
	color: P<Color>?,
	bg_image: P<string>?,
	bg_fit: P<string>?,
	rounded: P<number>?,
	corners: P<Corners>?,
	rounded_tl: P<number>?,
	rounded_tr: P<number>?,
	rounded_br: P<number>?,
	rounded_bl: P<number>?,
	border: P<number>?,
	border_sides: P<Corners>?,
	border_color: P<Color>?,
	border_over: P<boolean>?,
	border_dash: P<Pair>?,
	shadow: P<Shadow>?,
	opacity: P<number>?,
	brightness: P<number>?,
	contrast: P<number>?,
	saturate: P<number>?,
	blur: P<number>?,
	backdrop_blur: P<number>?,
	fade: P<Pair>?,
	fade_x: P<Pair>?,
	fade_sides: P<Corners>?,
	clip_inset: P<Corners>?,
	layer: P<boolean>?,
	fit: P<string>?,
	tint: P<Color>?,
	cursor: P<string>?,
	text_size: P<number>?,
	weight: P<number>?,
	bold: P<boolean>?,
	italic: P<boolean>?,
	align: P<string>?,
	font: P<string>?,
	wrap: P<boolean>?,
	truncate: P<boolean>?,
	lines: P<number>?,
	line_height: P<number>?,
	letter_spacing: P<number>?,
	x: P<number>?,
	y: P<number>?,
	scale: P<number>?,
	scale_x: P<number>?,
	scale_y: P<number>?,
	rotate: P<number>?,
	skew: P<number>?,
	skew_x: P<number>?,
	skew_y: P<number>?,
	origin: P<Pair>?,
	-- Identity, for selectors, refs and animations.
	id: string?,
	class: string?,
	ref: (Ref | { Ref })?,
	focusable: boolean?,
	-- CSS pointer-events: "none" lets the pointer through to what is under
	-- the node (descendants with handlers still take it).
	pointer_events: ("none" | "auto" | boolean)?,
	disabled: (boolean | State | (() -> boolean) | Styles)?,
	hover: Styles?,
	active: Styles?,
	focus: Styles?,
	-- Size animation and masks.
	layout: ("height" | "width" | "size" | false)?,
	layout_ease: (string | any)?,
	layout_duration: number?,
	layout_enter: boolean?,
	layout_clip: boolean?,
	mask: (Bitmap | { any })?,
	bg_shader: Shader?,
	shader_params: { Color | { number } }?,
	-- Events.
	on_click: (Handler | Action)?,
	on_press: (Handler | Action)?,
	on_release: (Handler | Action)?,
	on_hover: ((inside: boolean) -> ())?,
	on_pointer_move: ((e: { x: number, y: number, local_x: number, local_y: number }) -> ())?,
	on_key: ((e: { key: string, shift: boolean, ctrl: boolean, alt: boolean, meta: boolean, ["repeat"]: boolean }) -> boolean?)?,
	on_key_up: ((key: string) -> ())?,
	on_focus: (Handler | Action)?,
	on_blur: (Handler | Action)?,
	on_wheel: ((e: { dx: number, dy: number }) -> ())?,
	on_mount: ((element: Element) -> ())?,
	on_resize: ((size: { w: number, h: number }, element: Element) -> ())?,
	-- Widget fields.
	label: string?,
	value: State?,
	min: number?,
	max: number?,
	multiline: boolean?,
	offset: State?,
	follow: boolean?,
	src: P<string>?,
	code: string?,
	draw: ((c: Canvas, w: number, h: number) -> ())?,
	count: P<number>?,
	height: number?,
	overscan: number?,
	row: ((index: number) -> Node)?,
	open: State?,
	-- popover and menu: the trigger, shown where the popover is written.
	anchor: Node?,
	-- Web views: a ui.web() controller (so the page outlives the node), or
	-- the options to make one; `src` is the page, or `html` with `base_url`.
	web: Web?,
	html: string?,
	base_url: string?,
	transparent: boolean?,
	profile: string?,
	inspectable: boolean?,
	user_agent: string?,
	messages: boolean?,
	new_windows: boolean?,
	on_navigate: ((e: WebEvent) -> ())?,
	on_load: ((url: string) -> ())?,
	on_title: ((title: string) -> ())?,
	on_message: ((value: any) -> ())?,
	on_error: ((url: string, err: string) -> ())?,
	on_new_window: ((url: string) -> ())?,
	-- Terminals: the session (`ui.term(id)`).
	term: Term?,
	-- Drag and drop within the window; payloads are plain values.
	-- A source: `on_drag` returns the payload once the pointer moves 4
	-- points; `drag_preview` builds what follows the pointer.
	on_drag: (() -> any)?,
	drag_preview: ((payload: any) -> Node)?,
	on_drag_end: ((payload: any, accepted: boolean) -> ())?,
	-- A destination: `drop_when` filters payloads (default: all).
	drop_when: ((payload: any) -> boolean)?,
	on_drag_over: ((payload: any, e: DragPoint) -> ())?,
	on_drag_leave: ((payload: any) -> ())?,
	on_drop: ((payload: any, e: DragPoint) -> ())?,
	-- A secondary click (control-click on macOS) shows the returned
	-- `menu_item`s, `submenu`s and separators as a menu at the pointer;
	-- returning nil (or nothing) shows none.
	context_menu: ((e: { x: number, y: number, local_x: number, local_y: number }) -> { Node }?)?,
	-- The context menu panel's look (plain values): bg, rounded, p, gap,
	-- backdrop_blur, border, shadow, w, …
	context_menu_style: Styles?,
	-- Open the context menu under the node, from its left edge (a tab's),
	-- rather than at the pointer.
	context_menu_below: boolean?,
	-- Children.
	[number]: Child,
}

type DragPoint = { x: number, y: number, local_x: number, local_y: number, w: number, h: number }

type Container = (props: Props) -> Node


-- Animation values: numbers, strings with units ("10px", "1turn", "+=10"),
-- colors, keyframe lists, tween tables, staggers and functions of
-- (target, index, total).
type AnimValue = any
type Ease = string | ((t: number) -> number) | any
type Callback = (anim: Animation) -> ()

type Timing = {
	duration: (number | ((any, number, number) -> number))?,
	delay: (number | ((any, number, number) -> number) | any)?,
	ease: Ease?,
	composition: ("replace" | "none" | "add")?,
	modifier: ((number) -> number)?,
	loop: (number | boolean)?,
	loopDelay: number?,
	alternate: boolean?,
	reversed: boolean?,
	autoplay: (boolean | any)?,
	frameRate: number?,
	playbackRate: number?,
	playbackEase: Ease?,
	keyframes: any?,
	onBegin: Callback?,
	onBeforeUpdate: Callback?,
	onUpdate: Callback?,
	onRender: Callback?,
	onLoop: Callback?,
	onPause: Callback?,
	onComplete: Callback?,
	-- No array part: an unknown key is an error (with the new solver).
	[number]: never,
}

-- An element animation: its properties are checked, so a misspelled one
-- (`bgColor`) is an error here rather than at the first frame.
type ElementParams = {
	aspect: AnimValue?,
	backdrop_blur: AnimValue?,
	background: AnimValue?,
	backgroundColor: AnimValue?,
	background_color: AnimValue?,
	basis: AnimValue?,
	bg: AnimValue?,
	blur: AnimValue?,
	border: AnimValue?,
	borderBottomLeftRadius: AnimValue?,
	borderBottomRightRadius: AnimValue?,
	borderColor: AnimValue?,
	borderRadius: AnimValue?,
	borderTopLeftRadius: AnimValue?,
	borderTopRightRadius: AnimValue?,
	borderWidth: AnimValue?,
	border_color: AnimValue?,
	border_dash: AnimValue?,
	border_radius: AnimValue?,
	border_width: AnimValue?,
	bottom: AnimValue?,
	brightness: AnimValue?,
	contrast: AnimValue?,
	saturate: AnimValue?,
	shaderTime: AnimValue?,
	shader_time: AnimValue?,
	shaderParam0: AnimValue?,
	shaderParam1: AnimValue?,
	shaderParam2: AnimValue?,
	shaderParam3: AnimValue?,
	shaderParam4: AnimValue?,
	shaderParam5: AnimValue?,
	shaderParam6: AnimValue?,
	shaderParam7: AnimValue?,
	shaderParam8: AnimValue?,
	shaderParam9: AnimValue?,
	shaderParam10: AnimValue?,
	shaderParam11: AnimValue?,
	shaderParam12: AnimValue?,
	shaderParam13: AnimValue?,
	shaderParam14: AnimValue?,
	shaderParam15: AnimValue?,
	clipBottom: AnimValue?,
	clipLeft: AnimValue?,
	clipRight: AnimValue?,
	clipTop: AnimValue?,
	clip_bottom: AnimValue?,
	clip_left: AnimValue?,
	clip_right: AnimValue?,
	clip_top: AnimValue?,
	col_span: AnimValue?,
	col_start: AnimValue?,
	color: AnimValue?,
	cols: AnimValue?,
	fade: AnimValue?,
	fade_x: AnimValue?,
	filter: AnimValue?,
	fontSize: AnimValue?,
	fontWeight: AnimValue?,
	font_size: AnimValue?,
	gap: AnimValue?,
	gap_x: AnimValue?,
	gap_y: AnimValue?,
	grow: AnimValue?,
	h: AnimValue?,
	height: AnimValue?,
	left: AnimValue?,
	letter_spacing: AnimValue?,
	line_height: AnimValue?,
	lines: AnimValue?,
	m: AnimValue?,
	margin: AnimValue?,
	marginBottom: AnimValue?,
	marginLeft: AnimValue?,
	marginRight: AnimValue?,
	marginTop: AnimValue?,
	maskX: AnimValue?,
	maskY: AnimValue?,
	mask_x: AnimValue?,
	mask_y: AnimValue?,
	maxHeight: AnimValue?,
	maxWidth: AnimValue?,
	max_h: AnimValue?,
	max_w: AnimValue?,
	mb: AnimValue?,
	minHeight: AnimValue?,
	minWidth: AnimValue?,
	min_h: AnimValue?,
	min_w: AnimValue?,
	ml: AnimValue?,
	mr: AnimValue?,
	mt: AnimValue?,
	mx: AnimValue?,
	my: AnimValue?,
	opacity: AnimValue?,
	origin: AnimValue?,
	p: AnimValue?,
	padding: AnimValue?,
	paddingBottom: AnimValue?,
	paddingLeft: AnimValue?,
	paddingRight: AnimValue?,
	paddingTop: AnimValue?,
	pb: AnimValue?,
	pl: AnimValue?,
	pr: AnimValue?,
	pt: AnimValue?,
	px: AnimValue?,
	py: AnimValue?,
	right: AnimValue?,
	rotate: AnimValue?,
	rotateZ: AnimValue?,
	rotate_z: AnimValue?,
	rotation: AnimValue?,
	rounded: AnimValue?,
	rounded_bl: AnimValue?,
	rounded_br: AnimValue?,
	rounded_tl: AnimValue?,
	rounded_tr: AnimValue?,
	row_span: AnimValue?,
	row_start: AnimValue?,
	rows: AnimValue?,
	scale: AnimValue?,
	scaleX: AnimValue?,
	scaleY: AnimValue?,
	scale_x: AnimValue?,
	scale_y: AnimValue?,
	shrink: AnimValue?,
	size: AnimValue?,
	skew: AnimValue?,
	skewX: AnimValue?,
	skewY: AnimValue?,
	skew_x: AnimValue?,
	skew_y: AnimValue?,
	text_size: AnimValue?,
	tint: AnimValue?,
	top: AnimValue?,
	translateX: AnimValue?,
	translateY: AnimValue?,
	translate_x: AnimValue?,
	translate_y: AnimValue?,
	w: AnimValue?,
	weight: AnimValue?,
	width: AnimValue?,
	x: AnimValue?,
	y: AnimValue?,
	duration: (number | ((any, number, number) -> number))?,
	delay: (number | ((any, number, number) -> number) | any)?,
	ease: Ease?,
	composition: ("replace" | "none" | "add")?,
	modifier: ((number) -> number)?,
	loop: (number | boolean)?,
	loopDelay: number?,
	alternate: boolean?,
	reversed: boolean?,
	autoplay: (boolean | any)?,
	frameRate: number?,
	playbackRate: number?,
	playbackEase: Ease?,
	keyframes: any?,
	onBegin: Callback?,
	onBeforeUpdate: Callback?,
	onUpdate: Callback?,
	onRender: Callback?,
	onLoop: Callback?,
	onPause: Callback?,
	onComplete: Callback?,
	-- No array part: an unknown key is an error (with the new solver).
	[number]: never,
}

type ElementTargets = string | Element | Ref | { Element | Ref | string }

declare extern type Animation with
	id: any
	currentTime: number
	progress: number
	iterationProgress: number
	currentIteration: number
	duration: number
	iterationDuration: number
	paused: boolean
	began: boolean
	completed: boolean
	reversed: boolean
	speed: number
	labels: { [string]: number }
	function play(self): Animation
	function pause(self): Animation
	function resume(self): Animation
	function restart(self): Animation
	function reverse(self): Animation
	function alternate(self): Animation
	function seek(self, time: number): Animation
	function complete(self): Animation
	function cancel(self): Animation
	function revert(self): Animation
	function reset(self): Animation
	function stretch(self, duration: number): Animation
	function refresh(self): Animation
	function done(self, f: (Animation) -> ()): Animation
	function add(self, targets: any, params: any?, position: (number | string)?): Animation
	function set(self, targets: any, params: any, position: (number | string)?): Animation
	function call(self, f: () -> (), position: (number | string)?): Animation
	function label(self, name: string, position: (number | string)?): Animation
	function sync(self, animation: Animation, position: (number | string)?): Animation
	function remove(self, targets: any, property: string?): Animation
end

declare extern type Draggable with
	x: number
	y: number
	isDragging: boolean
	velocity: number
	function enable(self): Draggable
	function disable(self): Draggable
	function reset(self): Draggable
	function revert(self): Draggable
	function stop(self): Draggable
	function setX(self, x: number): Draggable
	function setY(self, y: number): Draggable
end

declare extern type Scope with
	function add(self, f: () -> ()): Scope
	function revert(self): ()
end

declare ui: {
	column: Container,
	row: Container,
	div: Container,
	overlay: Container,
	button: Container,
	grid: Container,
	scroll: Container,
	scroll_x: Container,
	image: Container,
	svg: Container,
	canvas: Container,
	webview: Container,
	web: (opts: WebOptions?) -> Web,
	terminal: Container,
	term: (id: string, opts: TermOptions?) -> Term,
	command: (opts: CommandOptions) -> (),
	checkbox: Container,
	input: Container,
	textarea: Container,
	progress: Container,
	slider: Container,
	modal: Container,
	popover: Container,
	menu: Container,
	menu_item: Container,
	submenu: Container,
	list: Container,
	rich: Container,
	text: (content: string | (() -> string) | Props) -> Node,
	span: (content: string | { [any]: any }) -> any,
	show: (cond: boolean | State | (() -> boolean), node: Node, otherwise: Node?) -> Node,
	switch: (value: State | (() -> any), cases: { [any]: Node }, default: Node?) -> Node,
	each: (list: State, build: (item: any, index: number) -> Node, opts: { key: ((item: any) -> any)? }?) -> Node,
	presence: (value: State | (() -> any) | any, build: (value: any) -> Node, opts: (Props & {
		enter: ElementParams?,
		exit: ElementParams?,
		mode: "wait"?,
		initial: boolean?,
	})?) -> Node,
	slot: (name: string, props: Props?) -> Node,
	contribute: (name: string, node: Node) -> (),
	state: (key: string, initial: any) -> State,
	shared: (key: string, initial: any) -> State,
	use_state: (initial: any) -> State,
	computed: (f: () -> any) -> State,
	effect: (f: () -> ()) -> (),
	batch: (f: () -> ()) -> (),
	on_mount: (f: () -> ()) -> (),
	ref: () -> Ref,
	component: (build: (props: { [any]: any }) -> Node) -> ((props: { [any]: any }?) -> Node),
	action: (name: string, arg: any?) -> Action,
	-- Call a host action now (`ui.action` binds one to a handler instead).
	invoke: (name: string, arg: any?) -> (),
	set: (state: State, value: any) -> Action,
	toggle: (state: State) -> Action,
	add: (state: State, by: number?) -> Action,
	tween: (state: State, opts: { duration: number?, ease: Ease?, from: any?, to: any? }?) -> Binding,
	spring: (state: State, opts: { stiffness: number?, damping: number?, from: any?, to: any? }?) -> Binding,
	loop: (opts: { duration: number?, ease: Ease?, yoyo: boolean?, fps: number?, from: any?, to: any? }?) -> Binding,
	bitmap: (w: number, h: number, alpha: (x: number, y: number) -> number) -> Bitmap,
	-- A WGSL fill: `fn fill(input: FillInput) -> vec4<f32>`, with input.p,
	-- input.size, input.time, input.scale and input.params (four vec4s).
	shader: (source: string, opts: { fps: number? }?) -> Shader,
	path: () -> Path,
	linear: (x1: number, y1: number, x2: number, y2: number, colors: { Color }) -> Brush,
}

declare anime: {
	animate: ((targets: ElementTargets, params: ElementParams) -> Animation)
		& ((targets: State | { [string]: any }, params: { [string]: any }) -> Animation),
	createTimeline: (params: (Timing & { defaults: { [string]: any }? })?) -> Animation,
	createTimer: (params: Timing?) -> Animation,
	createAnimatable: (targets: any, params: { [string]: any }) -> any,
	createDraggable: (target: ElementTargets, params: { [string]: any }?) -> Draggable,
	onScroll: (params: { [string]: any }?) -> any,
	stagger: (value: number | string | { any }, params: { [string]: any }?) -> any,
	createSpring: (params: { mass: number?, stiffness: number?, damping: number?, velocity: number? }?) -> any,
	spring: (params: { bounce: number?, duration: number? }?) -> any,
	createScope: (params: { defaults: { [string]: any }? }?) -> Scope,
	eases: { [string]: any },
	engine: {
		speed: number,
		playbackRate: number,
		fps: number,
		timeUnit: "ms" | "s",
		currentTime: number,
		pause: () -> (),
		resume: () -> (),
	},
	utils: {
		set: (targets: any, params: { [string]: any }) -> (),
		get: (target: any, property: string, unit: string?) -> any,
		remove: (targets: any, animation: any?, property: string?) -> (),
		cleanInlineStyles: (targets: any, props: (string | { string })?) -> (),
		random: (min: number, max: number, decimals: number?) -> number,
		randomPick: (items: { any }) -> any,
		shuffle: (items: { any }) -> { any },
		round: (value: number?, decimals: number?) -> any,
		clamp: (value: number, min: number, max: number) -> number,
		snap: (value: number, step: number | { number }) -> number,
		wrap: (value: number, min: number, max: number) -> number,
		mapRange: (value: number, a1: number, b1: number, a2: number, b2: number) -> number,
		lerp: (a: number, b: number, t: number) -> number,
		damp: (a: number, b: number, dt: number, factor: number) -> number,
		degToRad: (deg: number) -> number,
		radToDeg: (rad: number) -> number,
		roundPad: (value: number, decimals: number) -> string,
		padStart: (value: any, length: number, pad: string) -> string,
		padEnd: (value: any, length: number, pad: string) -> string,
		createSeededRandom: (seed: number?) -> (min: number?, max: number?, decimals: number?) -> number,
	},
	svg: {
		createMotionPath: (d: string) -> { translateX: any, translateY: any, rotate: any },
		getTotalLength: (d: string) -> number,
	},
	text: {
		split: (text: string, params: {
			words: (boolean | { class: string? })?,
			chars: (boolean | { class: string? })?,
			class: string?,
			style: Styles?,
			props: Props?,
		}?) -> Node,
	},
}

