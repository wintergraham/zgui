# Scripting zgui with Luau

`zgui-script` builds interfaces from [Luau](https://luau.org) scripts. A script runs once per load and returns a description of components, state bindings and animations; Rust lowers it to ordinary retained views. Layout, drawing, state and animation all stay in Rust. The script is called again only for event handlers, components as they mount, and properties it explicitly computes with a function. Text templates (`"Count: {count}"`) and every animation run without calling the script.

```lua
local count = ui.state("count", 0)
return ui.column {
    p = 24, gap = 12, bg = "#172030",
    ui.text { "Count: {count}", text_size = 18 },
    ui.button { on_click = ui.add(count), px = 16, py = 10, "Increment" },
}
```

```rust
let host = ScriptHost::new(&cx.ui.runtime, "app.luau").plugins("plugins");
host.action("save", |arg| { /* Rust, called by ui.action("save", arg) */ });
let _ = host.reload();                                // errors show over the window
cx.tasks.spawn(host.watch(Duration::from_millis(250))); // hot reload in development
cx.render(host.view());
```

State is Rust-owned and keyed, so a reload keeps it. Each script gets its own sandboxed VM (read-only globals, a 32 MiB heap limit and a time budget per call, counted in the CPU time the script uses, so a busy machine does not fail it), which makes plugins safe to load next to the application. A failed reload keeps the last good interface running and shows the error, with file and line, until a good save.

Examples: `hot_reload` (an app and two plugins), `frame_profiler` (a profiler window with its menus, popovers and tooltips), `stream_luau` / `stream_rust` (the same window in Luau and Rust, compared by `examples/stream/measure.sh`), `anime_showcase` (the animation API), `anime_bench` (per-frame engine cost), `quick_ask` (Ask's Quick ask panel over four mock apps: context chips, keyboard map, drag and drop).

## Types

`crates/zgui-script/types/zgui.d.luau` types the `ui` and `anime` globals for [luau-lsp](https://github.com/JohnnyMorganz/luau-lsp): editors complete property names, events and API calls, and report misspelled ones (`bgg`, `durtion`, `on_clik`, `state:sett`) before the script runs. In VS Code, list the file under `luau-lsp.types.definitionFiles` and turn on Luau's new type solver, which is the one that rejects unknown properties:

```json
{
    "luau-lsp.platform.type": "standard",
    "luau-lsp.types.definitionFiles": ["crates/zgui-script/types/zgui.d.luau"],
    "luau-lsp.fflags.override": { "LuauSolverV2": "true" }
}
```

Put `"languageMode": "strict"` in the project's `.luaurc` (non-strict mode reports little), and the `standard` platform resolves `require` by path, so types flow between modules. From a terminal: `luau-lsp analyze --platform standard --flag:LuauSolverV2=true --definitions=crates/zgui-script/types/zgui.d.luau app.luau`. The style properties and animatable keys in the file follow the engine's tables; `tests/types.rs` fails when they drift. At run time an unknown animation property is an error too, shown over the window.

## Elements

Elements take one table: string keys are properties, events and options; array entries are children (nodes, strings as text, functions as computed text, nested arrays, which flatten). `nil` and `false` entries are skipped, so `cond and node or nil` works anywhere in the list.

| Function | |
| --- | --- |
| `column`, `row`, `div`, `overlay`, `button`, `grid` | containers (`grid` sets `grid = true`) |
| `scroll`, `scroll_x` | scroll views; `offset = state` binds the position in points; `follow = true` keeps the view at its end while the content grows, until it is scrolled away (a transcript) |
| `text "…"`, `text { "…", … }`, `text(fn)` | `{name}` interpolates state in Rust; `{{` `}}` are literal braces |
| `rich { "plain", span { "bold", bold = true, on_click = … } }` | inline runs: `color bg text_size weight bold italic underline strike font on_click` |
| `image { src = "a.png", fit = "cover" }` | PNG or JPEG, relative to the script; `src` may be state |
| `svg { src = "icon.svg" }`, `svg { code = "<svg…>" }` | `tint = color` (a state or function re-tints it) |
| `canvas { draw = function(c, w, h) … end }` | runs again when its size or the state it reads changes |
| `webview { src = "https://…", … }` | a native web page (WKWebView on macOS) laid out, rounded, clipped and faded like any node, with zgui content (frosted popovers too) drawn over it; see [Web views](#web-views) |
| `terminal { term = ui.term(id) }` | a terminal session (a Ghostty surface, with the desktop host's `ghostty` feature) placed like a web view; see [Terminals](#terminals) |
| `checkbox`, `input`, `textarea`, `progress`, `slider` | `{ label, value = state }`; `slider` takes `min`, `max` |
| `show(cond, node, else)` | `cond`: state, function or boolean |
| `switch(state, { key = node, … }, default)` | |
| `presence(value, function(v) … end, { enter, exit, mode, initial, layout })` | shows what `value` (a state or function) builds; when it changes, the old content leaves out of the flow, playing `exit` (anime.js parameters), while the new plays `enter`. `mode = "wait"` enters after the exit; `nil` shows nothing; `initial = true` plays `enter` on the first value too |
| `each(list, function(item, i) … end, { key = fn })` | rows keep their state across reorders with `key` |
| `list { count, height, row = function(i) … end, offset }` | virtualized: only visible rows are built |
| `modal { open = state }`, `popover { open, anchor }`, `menu { open, anchor }`, `menu_item { "Copy", on_click }`, `submenu { open }` | overlays |
| `slot(name)`, `contribute(name, node)` | plugin extension points |

Canvas drawing: `c:rect(x, y, w, h, paint, radius)`, `c:circle(x, y, r, paint)`, `c:line(x1, y1, x2, y2, paint, width, opts)`, `c:fill(path, paint, "evenodd"?)`, `c:stroke(path, paint, width, { cap, join, dash, dash_offset, draw })`, `c:clip(path)`, `c:restore()`. Paths: `ui.path():move(x, y):line(…):quad(…):cubic(…):arc(cx, cy, r, start°, sweep°):close()`. Paint is a color or `ui.linear(x1, y1, x2, y2, { colors })`. `draw = t` (or `{ start, end }`) strokes a fraction of the path, as anime.js draws lines.

### Web views

`ui.webview` shows `src` (a URL, state or function: the page navigates when it changes) or `html` with `base_url`. Its options make the page: `transparent`, `profile` (`"default"` shares cookies and storage, `"ephemeral"` keeps nothing, any other string is a named, persistent store), `inspectable`, `user_agent`, `messages` and `new_windows`. To keep a page across the node (a tab switched away and back) or drive it from elsewhere, make its controller with `ui.web(options)` and pass `web = controller`.

```lua
local web = ui.web { profile = "ephemeral" }
local url = ui.state("url", "https://example.com")
return ui.column {
    ui.row { ui.button { "Back", on_click = function() web:back() end }, ui.text { function() return web.title end } },
    ui.webview {
        web = web, src = url, grow = 1, rounded = 12,
        on_navigate = function(e) print(e.kind, e.url) end,   -- started, committed, finished, failed, changed
        on_message = function(value) print(value.kind) end,   -- window.zgui.postMessage({ kind = … }) from the page
    },
}
```

The controller's `url`, `title`, `loading`, `progress`, `can_go_back`, `can_go_forward`, `focused` and `error` are tracked like state. Its methods: `load(url)`, `html(html, base_url)`, `back()`, `forward()`, `reload()`, `stop()`, `focus()`, `blur()`, `devtools()`, `zoom(z)`, `run(js)` and `eval(js, function(value, err) … end)`, which gets the script's value as Luau data. Events: `on_navigate({ kind, url })`, `on_load(url)`, `on_title(title)`, `on_message(value)`, `on_error(url, err)` and `on_new_window(url)`. Without a native host (headless, other platforms) the node shows a labelled box.

### Terminals

`ui.term(id, options)` returns the terminal session for `id`, starting it the first time: `command` (a command line; the login shell by default), `cwd` (`"~"` is home), `env` (a table) and `font_size`. Sessions belong to the host, not to a run: a reload keeps every shell running and the run that asks for `id` again gets the same session, its options' handlers replacing the previous run's (`ui.term(id)` without options only looks it up). Show one with `ui.terminal { term = t, … }`, sized and rounded like any node; moving the node elsewhere (another split, another tab) moves the terminal with its process.

```lua
local term = ui.term("p1", {
    cwd = "~/src",
    on_title = function(title) print(title) end,
    on_close = function(alive) term:close() end,   -- the process exited (alive = false)
})
return ui.terminal { term = term, grow = 1, rounded = 10, on_focus = function() print("typing here") end }
```

The session's `title` (what the program set), `pwd` (the shell's directory), `process` (the foreground program's name), `pid`, `focused`, `bell`, `progress`, `exited`, `columns` and `rows` are tracked like state. Methods: `focus()`, `blur()`, `input(text)` (as typed), `paste(text)`, `binding(action)` (a Ghostty binding action: `"copy_to_clipboard"`, `"clear_screen"`, `"increase_font_size:1"`), `needs_confirm()` (whether closing ends something other than an idle shell) and `close()`. Events, as options: `on_title`, `on_pwd`, `on_process`, `on_exit(code)`, `on_close(alive)`, `on_bell`, `on_command(exit_code, ms)` (shell integration) and `on_notify(title, body)`. The terminal reads its own keyboard, IME and mouse; app shortcuts reach the app through its menus (see [Commands](#commands-and-host-actions)). `zgui-desktop/examples/terminal` is a multiplexer written this way.

### Commands and host actions

`ui.command { id, title, menu, key, run }` declares an app command: the host shows it as a menu item (`ScriptHost::commands`, in declaration order) with `key` as its shortcut (`"cmd+shift+d"`, `"cmd+alt+arrowleft"`, `"cmd+["`) and calls `run` when it is chosen (`ScriptHost::run_command`). Menu shortcuts work wherever focus is, a terminal or a web page included. `ui.command { menu = "View", separator = true }` adds a separator. Each successful load replaces the commands; a failed one keeps the last. `ui.invoke(name, arg)` calls a host action now, from any handler (`ui.action` binds one to a handler instead).

### Drag and drop

A node becomes a drag source with `on_drag = function() return payload end` (any plain value), dragged once the pointer moves 4 points; `drag_preview = function(payload) return node end` builds what follows the pointer, and `on_drag_end(payload, accepted)` reports how it ended. A destination takes `on_drop(payload, e)`, optionally `drop_when(payload)` to accept only some payloads, and `on_drag_over(payload, e)` / `on_drag_leave(payload)` to show where a drop would land; `e` is `{ x, y, local_x, local_y, w, h }`. The nearest accepting destination gets the drag. A node with `on_press` and `on_pointer_move` or `on_release` keeps the pointer from press to release, wherever it goes (a divider being dragged).

### Context menus

`context_menu = function(e) return { … } end` on any node shows what it returns (`ui.menu_item`s, `ui.submenu`s, separators such as a 1-point `ui.div`) as a menu at the pointer on a secondary click, or a control-click on macOS; returning nothing shows none and lets the click through. `e` is `{ x, y, local_x, local_y }`. The innermost node with a context menu takes the click. `context_menu_style = { … }` styles the menu's panel with plain values (`bg`, `backdrop_blur`, `rounded`, `p`, `w`, `border`, `shadow`). `context_menu_below = true` opens it under the node, from its left edge, as a tab's does, wherever the click was. Terminals hand right clicks their program does not use to the node, so a terminal's context menu opens over it. Rust views have the same in `View::context_menu`, `View::context_menu_style` and `View::context_menu_below`.

### Properties

Any property can be a value, a state, a function (rerun when what it reads changes) or an animation binding.

- Size and position: `w h min_w max_w min_h max_h` (points, `"50%"`), `size w_full h_full aspect absolute relative top left right bottom`.
- Spacing: `p px py pt pr pb pl`, margins `m mx my mt mr mb ml` (also `"auto"`), `gap gap_x gap_y`.
- Flex and grid: `direction grow shrink basis flex_wrap wrap_reverse items justify self content`, `grid cols rows col_start row_start col_span row_span col_span_full row_span_full`.
- Paint: `bg gradient color rounded corners border border_color border_dash shadow opacity blur backdrop_blur fade layer fit tint cursor`, `overflow overflow_x overflow_y hidden invisible`. `blur` blurs the node's own content (CSS `filter: blur()`); `brightness`, `contrast` and `saturate` (CSS filter functions, 1 is unchanged) recolor the node and its subtree without a layer, and animate on the compositor (`hover` brightening: `anime.animate(el, { brightness = 1.12, duration = 150 })`); `backdrop_blur` blurs what is behind it (frosted glass). `fade = { top, bottom }`, `fade_x = { left, right }` or `fade_sides = { top, right, bottom, left }` fade a clipping view's edges over those widths (a horizontal row fading its right 28 points: `fade_x = { 0, 28 }`).
- Squircles: `corner_smoothing` (0 to 1) eases a node's rounded corners into superellipses whose curvature grows from the straight edges instead of jumping to a circle's: 0 keeps circles, 0.6 (`squircle = true`) reads as iOS's continuous corners. A corner of radius `r` runs `r · (1 + smoothing)` along each edge and keeps the circle's 45° point, so a squircle looks as round as the circle it replaces. Its fill, border, outline, shadows, background image, backdrop blur and the clip it gives its children (web views and terminals too) share it, and it animates as `cornerSmoothing`. `ui.corner_smoothing(0.6)` before the first frame makes it every node's default.
- Background images: `bg_image = "photo.jpg"` (a path, state or function) with `bg_fit = "cover"` (default), `"contain"`, `"fill"`, `"none"` or `"scale_down"`: drawn over `bg`, under the border and children, cut by `rounded`/`corners`.
- Masks: `mask = { ui.bitmap(w, h, function(x, y) return alpha end), w = …, h = …, x = …, y = … }` shows the content through the bitmap's alpha, drawn at `w`×`h` from (`x`, `y`), sampled without smoothing. Animating `maskX` / `maskY` slides it on the compositor (a shimmer band, a dithered fill, a sprite strip stepped with `ease = "steps(n)"`).
- Shader fills: `bg_shader = ui.shader(wgsl, { fps = 30 })` fills the box with a WGSL function run per pixel on the GPU, over `bg`, under the border and children, cut by the corners. The source defines `fn fill(input: FillInput) -> vec4<f32>` (straight alpha) and reads `input.p` (points from the box's top left), `input.size`, `input.time` (seconds), `input.scale` (pixels per point) and `input.params` (four `vec4`s). `shader_params = { "#e2c98f", { 1, 0.5, 0, 0 } }` sets the parameters (colors become 0 to 1); animate `shaderTime` and `shaderParam0` to `shaderParam15` like any property, on the compositor (`anime.animate(el, { shaderTime = { 0, 60 }, duration = 60000, ease = "linear", loop = true, frameRate = 12 })` turns a dither at 12 fps). `fps` caps how often time repaints it. A shader that does not compile fills with `bg`.
- Clip insets: `clip_inset = { top, right, bottom, left }` (CSS `clip-path: inset(...)`) paints the node and its children only inside its box shrunk by those amounts (negative reaches past it, rounded with the node's corners when it clips). Animate the sides as `clipTop`, `clipRight`, `clipBottom`, `clipLeft`, in points or as a percentage of the node's size: `{ clipBottom = { 0, "100%" } }` closes a node upward while its space closes, on the compositor.
- Size animation: `layout = "height" | "width" | "size"` lets the node's content size change freely while the space it takes eases to it (a spring of 0.4 s, bounce 0.1, or `layout_ease` / `layout_duration`). The content keeps its size and is not clipped (`layout_clip = true` clips); `layout_enter = true` grows it from nothing when it mounts. Combine with `presence` for content that swaps, folds or docks.
- Text: `text_size weight bold italic align font wrap truncate lines line_height letter_spacing`.
- Transforms: `x y scale scale_x scale_y rotate skew skew_x skew_y origin` (degrees; origin as fractions of the node).
- `hover`, `active`, `focus`, `disabled` take tables of plain values; `disabled = state` disables.
- `id`, `class = "a b"`, `ref = ui.ref()` make a node reachable by selectors and animations.
- `pointer_events = "none"` (CSS `pointer-events: none`) lets the pointer through the node, and through descendants without handlers of their own, to what is under it: a full-window layer that holds popovers and tooltips, a hover card over a chart.

Lua tables have no key order, so a property that refines another applies after it whatever order the keys come in: `p = 0, px = 8` pads the sides, `border = 1, border_sides = { 1, 0, 0, 0 }` draws only the top, and `rounded` gives way to `corners` and `rounded_tl`.

Colors are `"#rgb"`, `"#rrggbb"`, `"#rrggbbaa"`, `rgb()`, `rgba()`, `hsl()`, a few names, or `0xrrggbb`. `gradient = { angle = 135, stops = { "#f80", "#08f" } }` (CSS `linear-gradient` angles: 0 runs bottom to top, 90 left to right, the default 180 top to bottom; stops may be `{ offset, color }`); `shadow = { x = 0, y = 8, blur = 24, spread = 0, color = "#0006" }` (keyed) or a list of them (CSS `box-shadow` values: `blur` is the CSS blur radius); `corners = { tl, tr, br, bl }`, or single corners `rounded_tl rounded_tr rounded_br rounded_bl` (animatable as `borderTopLeftRadius` and so on); `border_sides = { top, right, bottom, left }` sets each side's width (a dock open at the bottom: `{ 1, 1, 0, 1 }`); `border_over = true` paints the border over the node's children (an image's frame) instead of under them.

### Events

`on_click`, `on_press` (primary pointer-down), `on_release`, `on_hover(inside)`, `on_pointer_move({ x, y, local_x, local_y })`, `on_key({ key, shift, ctrl, alt, meta, repeat })` (return `true` to consume it), `on_key_up(key)`, `on_focus`, `on_blur`, `on_wheel({ dx, dy })`, `on_mount(element)`, and `on_resize({ w, h }, element)`, called when layout gives the node a new size (and at its first layout), as a ResizeObserver reports: decide from real sizes (a row that wraps, text that overflows) rather than estimates. Elements read their laid-out `width`, `height`, `x` and `y`, and `el:focus()` / `el:blur()` move keyboard focus (to the element, or the first focusable node inside it, as an input's editor): a search field focuses itself in `on_mount`. Handlers take a function or a Rust-run action: `ui.set(s, v)`, `ui.toggle(s)`, `ui.add(s, n)`, or `ui.action(name, arg)` for a function registered with `ScriptHost::action`.

### State and components

- `require` loads modules as Luau resolves them: `require("./card")` and `require("../theme")` from the requiring file, `require("@components/card")` through the `aliases` in a `.luaurc` beside the script, `require("@self/x")` from the requiring file's own directory, and a directory as its `init.luau` (a bare `require("lib/theme")` resolves from the script's directory). Modules stay inside the script's directory, run once and return what they return; a change to any of them reloads the scripts, keeping state. A module's `ui.state` keys are its own (two modules' `ui.state("open")` are two states); `ui.shared` stays global, and `ui.use_state` is per component instance.

```lua
-- ui/components/card.luau
type CardProps = { title: string, tone: ("info" | "error")? }
return function(props: CardProps)
    return ui.column { p = 12, rounded = 10, ui.text { props.title } }
end

-- app.luau, with .luaurc: { "languageMode": "strict", "aliases": { "components": "./ui/components" } }
local Card = require("@components/card")
return Card { title = "Saved" }   -- `Card { titel = … }` is a type error in the editor
```
- `ui.state(key, initial)`: private to the script, kept across reloads. `ui.shared(key, initial)`: shared with Rust and every script.
- State values: `nil`, booleans, numbers, strings, lists and string-keyed tables. Handles: `get` (tracked), `peek`, `set`, `update(fn)`, `toggle`, `add(n)`, `push`, `remove(i)`, `len`.
- `ui.computed(fn)`: derived state. `ui.effect(fn)`: runs again when what it reads changes. `ui.batch(fn)`: one update for many writes.
- Functions that build part of the interface (`each` rows, `presence` content, components) own what they create: `ui.computed` and `ui.effect` made while building live as long as the part they built.
- `ui.component(function(props) … end)` returns a constructor: `Card { title = "…", child }` (children also arrive as `props.children`). Inside, `ui.use_state(initial)` is per instance, effects belong to the instance, and `ui.on_mount(fn)` runs once it is mounted.

## Animation: anime.js v4

The `anime` global follows anime.js v4's API. Animations run in a Rust engine: one task ticks every playing animation once per display frame, in one reactive batch, and writes translation, opacity, scale, rotation and skew straight to the scene, once per element and frame. Other properties (colors, sizes, radii, text size) go through the node's reactive style. Nothing runs while nothing plays: the engine asks for no frames at rest. Script code runs per frame only for callbacks a script sets (`onUpdate`, `onRender`), function eases and function modifiers. Methods work as `anim.play()` (as in JavaScript) and `anim:play()`.

```lua
anime.animate(".card", {
    x = { 0, 240 }, opacity = 0.6, rotate = "1turn",
    delay = anime.stagger(60, { from = "center" }),
    duration = 900, ease = "outElastic(1, .5)",
    loop = true, alternate = true,
    onComplete = function(a) print("done") end,
})
```

| anime.js | Luau |
| --- | --- |
| `animate(targets, params)` | same; targets are selectors (`"#id"`, `".class"`, `"button"`, `"button.primary"`, comma lists), refs, elements, states, plain tables, or lists of them |
| `createTimeline(params)` | `tl:add(targets, params, position)`, `tl:add(timerParams, position)`, `set`, `call`, `label`, `sync`, `remove`; positions `500`, `"+=100"`, `"-=100"`, `"<"`, `"<<"`, `"<+=50"`, `"label"`, `"label+=50"`; `defaults` |
| `createTimer(params)` | endless unless given a `duration` |
| `createAnimatable(targets, { x = 400, y = { duration = 600, ease = "out(3)" } })` | `a.x(value, duration?, ease?)` animates; `a.x()` reads |
| `createDraggable(target, params)` | `x`/`y` (`true`, `false`, `{ snap }`), `container` (selector, element or `{ top, right, bottom, left }`), `containerPadding`, `containerFriction`, `snap`, `dragSpeed`, `velocityMultiplier`, `minVelocity`, `maxVelocity`, `releaseStiffness`, `releaseDamping`, `releaseMass`, `cursor`, `onGrab`, `onDrag`, `onUpdate`, `onRelease`, `onSnap`, `onSettle`; `enable`, `disable`, `reset`, `revert`, `stop`, `setX`, `setY`; `x`, `y`, `isDragging`, `velocity` |
| `onScroll(params)`, as `autoplay` | `container` (default: the nearest scroll ancestor), `target`, `enter`/`leave` (`"<container edge> <target edge>"`: `start center end`, `%`, points, `+=`/`-=`), `sync` (`true`, a smoothing amount, or an ease), `repeat`, `onEnter`, `onLeave`, `onUpdate`, `onSync` |
| `stagger(value, params)` | ranges `{ -100, 100 }`, units, `start`, `from` (`"first"`, `"center"`, `"last"`, `"random"`, index), `reversed`, `ease`, `grid`, `axis`, `use`, `total`, `modifier` |
| `createSpring({ mass, stiffness, damping, velocity })`, `spring({ bounce, duration })` | an ease that sets the duration it settles in |
| `eases.*`, ease strings | `linear`, `in(p)`, `out(p)`, `inOut(p)`, `outIn(p)`, Quad to Quint, Sine, Circ, Expo, Bounce, `Back(overshoot)`, `Elastic(amplitude, period)`, `steps(n, fromStart)`, `cubicBezier(…)`, `linear(…)`, `irregular(…)`; functions; v3 names (`easeInOutQuad`) |
| `createScope({ defaults })` | `scope:add(fn)`, `scope:revert()` |
| `text.split(text, params)` | returns a node: one text per character (`chars`) or word (`words`), with a `class` to animate; `style` styles each piece, `props` the row |
| `svg.createMotionPath(d)` | SVG path data; returns `translateX`, `translateY`, `rotate` to animate along it; `svg.getTotalLength(d)` |
| `utils` | `set`, `get`, `remove`, `cleanInlineStyles(targets, props?)` (only the named properties when given; each returns to the node's own value), `random`, `randomPick`, `shuffle`, `round`, `clamp`, `snap`, `wrap`, `mapRange`, `lerp`, `damp`, `degToRad`, `radToDeg`, `roundPad`, `padStart`, `padEnd`, `createSeededRandom`; called without the value, `round` returns a modifier that runs in Rust |
| `engine` | `speed`/`playbackRate`, `fps`, `timeUnit` (`"ms"` or `"s"`), `currentTime`, `pause()`, `resume()` |
| `anime({ targets = …, … })` | anime.js v3's form |

Elements also animate `blur` (the content's), `maskX` and `maskY` on the compositor.

Parameters: `duration`, `delay`, `ease`, `composition` (`"replace"`, `"none"`, `"add"`), `modifier`, `loop` (count or `true`), `loopDelay`, `alternate`, `reversed`, `autoplay`, `frameRate`, `playbackRate`, `playbackEase`, `keyframes` (a list of property sets, or `{ ["0%"] = …, ["50%"] = … }`), callbacks `onBegin`, `onBeforeUpdate`, `onUpdate`, `onRender`, `onLoop`, `onPause`, `onComplete`. Property values: numbers, strings with units (`px`, `%`, `rem`, `em`, `deg`, `rad`, `turn`), relative (`"+=10"`, `"-=10"`, `"*=2"`), colors, strings with numbers in them, functions of `(target, index, total)`, staggers, value keyframes (`{ from, …, to }`), and tween objects (`{ to, from, duration, delay, ease, composition, modifier }`), alone or in lists. Bare numbers for angles are degrees, as in anime.js.

Controls: `play`, `pause`, `resume`, `restart`, `reverse`, `alternate`, `seek(time)`, `complete`, `cancel`, `revert`, `reset`, `stretch(duration)`, `refresh`, and `done(fn)` (anime.js `then`, a Luau keyword; `a["then"](fn)` also works). Fields: `id`, `currentTime` (settable), `progress` (settable), `iterationProgress`, `currentIteration`, `duration`, `iterationDuration`, `paused`, `began`, `completed`, `reversed`, `speed`, `labels`.

Scripts create animations before the elements they target mount: an animation resolves its targets the first time it plays, and draggables and scroll observers attach once their targets mount. Draggables and scroll-linked animations live without a script handle, until their script is replaced.

The `ui.tween(state, opts)`, `ui.spring(state, opts)` and `ui.loop(opts)` bindings remain for declarative transitions: a property bound to one eases whenever the state changes, with any anime.js ease.

### Cost

`anime_bench` animates 1,000 elements' `y` and `opacity` every frame, headless, and times one frame (Apple M5 Max, release):

| | per frame |
| --- | --- |
| Luau + anime engine | about 106 µs |
| The same motion in Rust, a signal and reactive style per element | about 217 µs |
| The same values written straight to the scene (no engine) | about 123 µs |

Writing to the scene directly is the floor for the work itself; the engine is below it because it skips values that did not change. What remains is the scene's own invalidation.

Rotation, scale and skew promote a node to a cached layer: its subtree is painted once into a texture, and each frame only composites that texture under a new matrix (no layout, no repaint). Scales re-rasterize at steps of √2, so a scale animation repaints a handful of times, not every frame. Each transformed node is its own texture and draw: thousands of them at once are bounded by the renderer's layer budget (64 MiB, 1,024 textures). Hit testing follows the transform; positions inside widgets that track the pointer themselves (a slider's thumb, a text caret) do not yet account for a rotated or scaled ancestor.

## Comparison with Rust views

The streaming example (`stream_luau` against `stream_rust`, 60 tokens a second into a transcript) measured on macOS: streaming CPU within noise (2.6% each), about 2% more instructions for Luau, and 0.9 MiB more memory (a Luau VM and the script's tree). Idle CPU is zero for both.
