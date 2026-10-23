//! The `ui` library's breadth: components, derived state, every widget
//! kind, layout lengths and input events.
mod common;

use common::Harness;
use std::{cell::RefCell, rc::Rc};
use zgui::{
    input::{InputEvent, Key, Modifiers},
    scene::NodeKind,
};

impl Harness {
    fn kind(&mut self, id: &str) -> String {
        let node = self.find(id).unwrap_or_else(|| panic!("no node `{id}`"));
        let scene = self.ui.scene.borrow();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            match scene.kind(n) {
                NodeKind::Container(_) | NodeKind::Panel { .. } => {
                    stack.extend(scene.children(n).iter().rev())
                }
                other => return format!("{other:?}"),
            }
        }
        "empty".into()
    }
    fn width(&mut self, id: &str) -> f32 {
        let node = self.find(id).unwrap();
        self.ui.scene.borrow().bounds(node).width
    }
    fn count_text(&mut self, prefix: &str) -> usize {
        self.ui.prepare_frame();
        let scene = self.ui.scene.borrow();
        let mut stack = vec![scene.root()];
        let mut n = 0;
        while let Some(id) = stack.pop() {
            if let NodeKind::Text { text, .. } = scene.kind(id)
                && text.starts_with(prefix)
            {
                n += 1;
            }
            stack.extend(scene.children(id).iter().copied());
        }
        n
    }
}

#[test]
fn components_have_their_own_state_and_mount_hooks() {
    let mut h = Harness::inline(
        r##"local mounted = ui.state("mounted", 0)
        local Counter = ui.component(function(props)
            local n = ui.use_state(props.start or 0)
            ui.on_mount(function() mounted:add(1) end)
            return ui.column {
                ui.text { function() return props.name .. " " .. n:get() end, id = props.name },
                ui.button { id = props.name .. "+", w = 30, h = 20, on_click = function() n:add(1) end },
            }
        end)
        return ui.column {
            Counter { name = "a" }, Counter { name = "b", start = 10 },
            ui.text { "{mounted}", id = "mounted" },
        }"##,
    );
    assert_eq!(h.text("mounted"), "2");
    h.click("a+");
    h.click("a+");
    h.click("b+");
    assert_eq!(h.text("a"), "a 2");
    assert_eq!(h.text("b"), "b 11");
}

#[test]
fn computed_state_follows_its_inputs() {
    let mut h = Harness::inline(
        r##"local a = ui.state("a", 2)
        local b = ui.state("b", 3)
        local sum = ui.computed(function() return a:get() + b:get() end)
        return ui.column {
            ui.text { function() return "sum " .. sum:get() end, id = "sum" },
            ui.button { id = "inc", w = 20, h = 20, on_click = ui.add(a, 5) },
        }"##,
    );
    assert_eq!(h.text("sum"), "sum 5");
    h.click("inc");
    assert_eq!(h.text("sum"), "sum 10");
}

#[test]
fn rich_text_svg_and_canvas_render() {
    let mut h = Harness::inline(
        r##"local name = ui.state("name", "Ada")
        return ui.column {
            ui.rich { id = "rich", "Hello ", ui.span { "{name}", bold = true, color = "#f80" }, "!" },
            ui.svg { id = "svg", code = "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10'/></svg>", w = 10, h = 10 },
            ui.canvas { id = "canvas", w = 40, h = 40, draw = function(c, w, h)
                c:rect(0, 0, w, h, "#222", 6)
                c:stroke(ui.path():move(0, 0):line(w, h), "#fff", 2)
                c:circle(w / 2, h / 2, 5, ui.linear(0, 0, w, 0, { "#f00", "#00f" }))
            end },
        }"##,
    );
    assert!(h.kind("rich").contains("Hello Ada!"), "{}", h.kind("rich"));
    assert!(h.kind("svg").starts_with("Svg"), "{}", h.kind("svg"));
    let canvas = h.kind("canvas");
    assert!(
        canvas.starts_with("Canvas") && canvas.contains("Stroke") && canvas.contains("Linear"),
        "{canvas}"
    );
    assert_eq!(h.host.error().get(), None);
}

#[test]
fn virtual_lists_mount_only_visible_rows() {
    let mut h = Harness::inline(
        r##"return ui.list { id = "list", count = 100000, height = 20, w = 200, h = 200,
            row = function(i) return ui.text("row " .. i) end }"##,
    );
    let rows = h.count_text("row ");
    assert!(rows > 5 && rows < 30, "{rows} rows mounted of 100,000");
}

#[test]
fn switch_shows_the_matching_case() {
    let mut h = Harness::inline(
        r##"local tab = ui.state("tab", "one")
        return ui.column {
            ui.switch(tab, { one = ui.text { "first", id = "one" }, two = ui.text { "second", id = "two" } }),
            ui.button { id = "go", w = 20, h = 20, on_click = ui.set(tab, "two") },
        }"##,
    );
    assert!(h.find("one").is_some() && h.find("two").is_none());
    h.click("go");
    assert!(h.find("one").is_none() && h.find("two").is_some());
}

#[test]
fn percentage_and_auto_lengths() {
    let mut h = Harness::inline(
        r##"return ui.row { w = 400, h = 100,
            ui.div { id = "half", w = "50%", h = 10 },
            ui.div { id = "quarter", w = "25%", h = 10, ml = "auto" },
        }"##,
    );
    assert_eq!(h.width("half"), 200.);
    assert_eq!(h.width("quarter"), 100.);
    let node = h.find("quarter").unwrap();
    assert_eq!(
        h.ui.scene.borrow().bounds(node).x,
        300.,
        "auto margin pushes it to the end"
    );
}

#[test]
fn key_and_hover_events_reach_scripts() {
    let mut h = Harness::inline(
        r##"local log = ui.state("log", "")
        return ui.column {
            ui.div { id = "target", w = 100, h = 100, focusable = true,
                on_key = function(e) log:set(log:peek() .. e.key .. (e.shift and "!" or "")); return true end,
                on_hover = function(inside) log:set(log:peek() .. (inside and "[" or "]")) end,
                on_press = function(e) log:set(log:peek() .. "p" .. math.floor(e.local_x)) end },
            ui.text { "{log}", id = "log" },
        }"##,
    );
    h.ui.prepare_frame();
    h.ui.dispatch(InputEvent::PointerMove { x: 50., y: 50. });
    h.ui.dispatch(InputEvent::PointerDown {
        x: 30.,
        y: 50.,
        button: zgui::input::PointerButton::Primary,
    });
    h.ui.dispatch(InputEvent::PointerUp {
        x: 30.,
        y: 50.,
        button: zgui::input::PointerButton::Primary,
    });
    let node = h.find("target").unwrap();
    let scene = h.ui.scene.clone();
    h.ui.input.focus(&scene, Some(node));
    h.ui.dispatch(InputEvent::KeyDown {
        key: Key::Character("a".into()),
        modifiers: Modifiers {
            shift: true,
            ..Default::default()
        },
        repeat: false,
    });
    h.ui.dispatch(InputEvent::PointerMove { x: 300., y: 290. });
    assert_eq!(h.text("log"), "[p30a!]");
}

#[test]
fn keyed_rows_keep_their_state_when_the_list_reorders() {
    let mut h = Harness::inline(
        r##"local items = ui.state("items", { { id = 1, name = "x" }, { id = 2, name = "y" } })
        local Row = ui.component(function(props)
            local clicks = ui.use_state(0)
            return ui.button { id = "row" .. props.item.id, w = 50, h = 20,
                on_click = function() clicks:add(1) end,
                ui.text { function() return props.item.name .. clicks:get() end, id = "label" .. props.item.id } }
        end)
        return ui.column {
            ui.each(items, function(item) return Row { item = item } end, { key = function(item) return item.id end }),
            ui.button { id = "swap", w = 20, h = 20, on_click = function()
                local list = items:get()
                items:set({ list[2], list[1] })
            end },
        }"##,
    );
    h.click("row1");
    h.click("row1");
    h.click("swap");
    assert_eq!(h.text("label1"), "x2", "row 1 kept its clicks after moving");
    assert_eq!(h.text("label2"), "y0");
}

#[test]
fn modals_open_from_state() {
    let mut h = Harness::inline(
        r##"local open = ui.state("open", false)
        return ui.column {
            ui.button { id = "open", w = 30, h = 20, on_click = ui.set(open, true) },
            ui.modal { open = open, label = "Dialog", ui.text { "inside", id = "inside" } },
        }"##,
    );
    // A closed modal keeps its content mounted, hidden.
    let shown = |h: &mut Harness| {
        let node = h.find("inside").unwrap();
        h.ui.scene.borrow().visible_bounds(node).is_some()
    };
    assert!(!shown(&mut h));
    h.click("open");
    assert!(shown(&mut h));
}

#[test]
fn rotated_buttons_are_hit_where_they_are_drawn() {
    let mut h = Harness::inline(
        r##"local n = ui.state("n", 0)
        return ui.column { p = 100,
            ui.button { id = "b", w = 100, h = 20, rotate = 90, on_click = ui.add(n) },
            ui.text { "{n}", id = "n" },
        }"##,
    );
    h.ui.prepare_frame();
    use zgui::input::PointerButton::Primary;
    // Rotated a quarter turn about its centre (150, 110): it now spans
    // y 60..160 at x 140..160, and no longer covers (110, 110).
    for (x, y) in [(150., 70.), (110., 110.)] {
        h.ui.dispatch(InputEvent::PointerDown {
            x,
            y,
            button: Primary,
        });
        h.ui.dispatch(InputEvent::PointerUp {
            x,
            y,
            button: Primary,
        });
    }
    assert_eq!(
        h.text("n"),
        "1",
        "only the point inside the rotated box clicked"
    );
}

#[test]
fn nil_children_leave_no_gap_in_the_list() {
    let mut h = common::Harness::inline(
        r##"return ui.row {
            ui.text { "a", id = "a" },
            false and ui.text { "b", id = "b" } or nil,
            ui.text { "c", id = "c" },
        }"##,
    );
    assert!(h.find("a").is_some() && h.find("b").is_none());
    assert!(h.find("c").is_some(), "a child after a nil one is kept");
}

#[test]
fn svg_tint_follows_state() {
    let mut h = common::Harness::inline(
        r##"local c = ui.state("c", "#ff0000")
        return ui.column {
            ui.svg { id = "i", w = 10, h = 10, tint = c,
                code = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#fff"/></svg>' },
            ui.button { id = "b", w = 10, h = 10, on_click = ui.set(c, "#00ff00") },
        }"##,
    );
    let svg = |h: &mut common::Harness| {
        let node = h.find("i").unwrap();
        let scene = h.ui.scene.borrow();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if let zgui::scene::NodeKind::Svg(data) = scene.kind(n) {
                return format!("{data:?}");
            }
            stack.extend(scene.children(n).iter().copied());
        }
        panic!("no svg")
    };
    let before = svg(&mut h);
    h.click("b");
    assert_ne!(before, svg(&mut h), "a new tint re-tints the icon");
}

#[test]
fn border_sides_set_each_width() {
    let mut h = common::Harness::inline(
        r##"return ui.div { id = "d", w = 40, h = 40, border_sides = { 1, 1, 0, 1 }, border_color = "#33322D" }"##,
    );
    let node = h.find("d").unwrap();
    let scene = h.ui.scene.borrow();
    let text = format!("{:?}", scene.kind(node));
    assert!(
        text.contains("bottom: 0.0") && text.contains("top: 1.0"),
        "{text}"
    );
}

#[test]
fn on_resize_reports_layout_sizes() {
    let mut h = common::Harness::inline(
        r##"local items = ui.state("items", { 1, 2 })
        local seen = ui.state("seen", "")
        return ui.column { w = 200,
            ui.row { flex_wrap = true, gap = 0,
                on_resize = function(size, el) seen:set(size.w .. "x" .. size.h .. " " .. tostring(el.height)) end,
                ui.each(items, function() return ui.div { w = 80, h = 10 } end) },
            ui.text { "{seen}", id = "seen" },
            ui.button { id = "add", w = 10, h = 10, on_click = function() items:push(3) end },
        }"##,
    );
    h.ui.prepare_frame();
    h.settle();
    assert_eq!(
        h.text("seen"),
        "160x10 10",
        "one row, as wide as its two items"
    );
    h.click("add");
    h.ui.prepare_frame();
    h.settle();
    assert_eq!(
        h.text("seen"),
        "200x20 20",
        "a third item wraps to a second row"
    );
}

#[test]
fn border_over_reaches_the_scene() {
    let mut h = common::Harness::inline(
        r##"return ui.div { id = "d", w = 20, h = 20, border = 1, border_color = "#fff", border_over = true, ui.div { w = 20, h = 20, bg = "#000" } }"##,
    );
    let node = h.find("d").unwrap();
    let text = format!("{:?}", h.ui.scene.borrow().kind(node));
    assert!(text.contains("border_over: true"), "{text}");
}

#[test]
fn squircle_corners_reach_the_scene() {
    let mut h = common::Harness::inline(
        r##"return ui.div {
            ui.div { id = "own", w = 20, h = 20, rounded = 6, bg = "#fff", corner_smoothing = 0.8 },
            ui.div { id = "ios", w = 20, h = 20, rounded = 6, bg = "#fff", squircle = true },
            ui.div { id = "plain", w = 20, h = 20, rounded = 6, bg = "#fff" },
        }"##,
    );
    let smoothing = |h: &mut common::Harness, id: &str| {
        let node = h.find(id).unwrap();
        match h.ui.scene.borrow().kind(node) {
            NodeKind::Panel { quad, .. } => quad.corner_smoothing,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(smoothing(&mut h, "own"), Some(0.8));
    assert_eq!(
        smoothing(&mut h, "ios"),
        Some(zgui::decoration::IOS_CORNER_SMOOTHING)
    );
    // Unset: the app's default decides when it draws.
    assert_eq!(smoothing(&mut h, "plain"), None);
}

#[test]
fn bg_image_paints_and_survives_an_animated_bg() {
    let png = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/images/components.png"
    );
    let mut h = common::Harness::inline(&format!(
        r##"return ui.div {{ id = "d", w = 40, h = 40, bg = "#000000", rounded = 8, bg_image = "{png}", bg_fit = "cover",
            on_mount = function(el) anime.animate(el, {{ backgroundColor = "#ff0000", duration = 50 }}) end }}"##
    ));
    for _ in 0..10 {
        h.frame();
    }
    let node = h.find("d").unwrap();
    let text = format!("{:?}", h.ui.scene.borrow().kind(node));
    assert!(
        text.contains("Image {"),
        "the background image is set: {text}"
    );
    assert!(
        text.contains("fill: Color(255, 0, 0"),
        "the animated bg sits under it: {text}"
    );
}

#[test]
fn fades_reach_each_edge() {
    let mut h = common::Harness::inline(
        r##"return ui.scroll_x { id = "s", w = 100, h = 20, fade_x = { 0, 28 }, ui.div { w = 300, h = 20 } }"##,
    );
    let node = h.find("s").unwrap();
    let scene = h.ui.scene.borrow();
    let mut stack = vec![node];
    let mut found = false;
    while let Some(n) = stack.pop() {
        if scene.style(n).fade_edges_x == [0., 28.] {
            found = true;
        }
        stack.extend(scene.children(n).iter().copied());
    }
    assert!(found, "a right fade of 28 points");
}

/// Records a page's commands and answers evaluations.
struct Page(RefCell<Vec<String>>);
impl zgui::web_view::WebViewBackend for Page {
    fn run(&self, command: zgui::web_view::WebViewCommand) {
        self.0.borrow_mut().push(format!("{command:?}"));
        if let zgui::web_view::WebViewCommand::Evaluate {
            reply: Some(reply), ..
        } = command
        {
            zgui::native_view::post(move || reply(Ok(r#"{"n":42,"tags":["a","b"]}"#.into())));
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn web_views_load_their_source_and_report_page_events() {
    use zgui::{native_view::NativeContent, web_view::WebViewEvent};
    let mut h = Harness::inline(
        r##"local web = ui.web { profile = "ephemeral" }
        local tab = ui.state("tab", "a")
        local log = ui.state("log", "")
        local function note(s) log:set(log:get() .. s .. ";") end
        return ui.column {
            ui.webview {
                id = "page", web = web, w = 200, h = 100, rounded = 8,
                src = function() return "https://" .. tab:get() .. ".test/" end,
                on_navigate = function(e) note(e.kind .. " " .. e.url) end,
                on_load = function(url) note("load " .. url) end,
                on_title = function(t) note("title " .. t) end,
                on_message = function(m) note("message " .. m.kind .. " " .. m.count) end,
                on_error = function(url, err) note("error " .. err) end,
            },
            ui.text { function() return web.title end, id = "title" },
            ui.button { id = "b", w = 20, h = 20, on_click = function() tab:set("b") end },
            ui.button { id = "eval", w = 20, h = 20, on_click = function()
                web:eval("x", function(v) note("eval " .. v.n .. " " .. v.tags[2]) end)
            end },
            ui.text { "{log}", id = "log" },
        }"##,
    );
    let page = h.find("page").unwrap();
    assert_eq!(h.natives.nodes(), vec![page]);
    let Some(NativeContent::Web(web)) = h.natives.content(page) else {
        panic!("no page")
    };
    assert!(web.options().profile == zgui::web_view::WebProfile::Ephemeral);
    // Unsupported headless: a labelled box, sized like any node.
    assert_eq!(h.width("page"), 200.);
    let backend = Rc::new(Page(RefCell::new(Vec::new())));
    web.attach(backend.clone());
    assert_eq!(
        *backend.0.borrow(),
        vec!["LoadUrl(\"https://a.test/\")".to_string()]
    );
    h.click("b");
    assert_eq!(
        backend.0.borrow().last().unwrap(),
        "LoadUrl(\"https://b.test/\")"
    );

    web.emit(WebViewEvent::NavigationStarted {
        url: "https://b.test/".into(),
    });
    web.emit(WebViewEvent::NavigationFinished {
        url: "https://b.test/".into(),
    });
    web.update_state(|s| s.title = "B".into());
    web.emit(WebViewEvent::TitleChanged { title: "B".into() });
    web.emit(WebViewEvent::Message {
        body: r#"{"kind":"ping","count":3}"#.into(),
    });
    web.emit(WebViewEvent::NavigationFailed {
        url: "https://c.test/".into(),
        error: "offline".into(),
    });
    h.settle();
    assert_eq!(h.text("title"), "B");
    assert_eq!(
        h.text("log"),
        "started https://b.test/;finished https://b.test/;load https://b.test/;title B;\
         message ping 3;failed https://c.test/;error offline;"
    );
    h.click("eval");
    zgui::native_view::deliver_posted();
    h.settle();
    assert!(h.text("log").ends_with("eval 42 b;"), "{}", h.text("log"));
}

#[test]
fn longhand_properties_refine_their_shorthand_in_any_table_order() {
    // Lua tables have no order: `p = 0, px = 20` must pad the sides
    // whatever order the keys come out in, so vary the keys around them.
    let mut h = Harness::inline(
        r##"local rows = {}
        for i = 1, 24 do
            local props = { id = "row" .. i, p = 0, px = 20, py = 3, ui.div { id = "cell" .. i, w = 10, h = 10 } }
            props["h"] = 16 + i % 3
            if i % 2 == 0 then props.gap = i end
            if i % 3 == 0 then props.rounded = i end
            table.insert(rows, ui.row(props))
        end
        return ui.column { rows }"##,
    );
    for i in 1..=24 {
        let row = h.find(&format!("row{i}")).unwrap();
        let cell = h.find(&format!("cell{i}")).unwrap();
        let scene = h.ui.scene.borrow();
        let (r, c) = (scene.bounds(row), scene.bounds(cell));
        assert_eq!(c.x - r.x, 20., "row {i}: px refines p");
        assert_eq!(c.y - r.y, 3., "row {i}: py refines p");
    }
}

#[test]
fn pointer_events_none_lets_presses_through_to_what_is_under() {
    let mut h = Harness::inline(
        r##"local hits = ui.shared("hits", 0)
        return ui.overlay { w = 200, h = 100, on_key = function() return false end,
            ui.button { id = "under", w = 200, h = 100, on_click = function() hits:set(hits:peek() + 1) end, "Under" },
            ui.div { id = "layer", absolute = true, top = 0, left = 0, right = 0, bottom = 0, pointer_events = "none",
                ui.div { w = 10, h = 10 } },
        }"##,
    );
    let hits = h.host.shared("hits", 0.);
    h.click("under");
    assert_eq!(
        hits.with_untracked(|v| v.as_f64()),
        Some(1.),
        "the layer passes the press through"
    );
}

#[test]
fn elements_can_take_keyboard_focus() {
    let mut h = Harness::inline(
        r##"local focused = ui.shared("focused", false)
        local text = ui.state("text", "")
        return ui.column {
            ui.input { id = "field", label = "Field", value = text, on_focus = function() focused:set(true) end,
                on_mount = function(el) el:focus() end },
        }"##,
    );
    let focused = h.host.shared("focused", false);
    h.settle();
    assert_eq!(
        focused.with_untracked(|v| v.clone()),
        zgui_script::Value::Bool(true)
    );
    let _ = h.find("field");
}
