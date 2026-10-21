//! What a script builds: a description of components, state bindings and
//! animations. Evaluating a script produces this tree once; Rust lowers it to
//! retained zgui views and the script is not consulted again except by event
//! handlers, components as they mount, and explicitly computed (function)
//! properties.
use crate::{
    Vm,
    element::{Ref, RefUd},
    value::{State, Store, Value},
};
use mlua::{Function, Lua, Table, UserData};
use std::{
    any::Any,
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
};
use zgui::{
    canvas::GradientStop,
    decoration::Corners,
    layout::Length,
    reactive::{Runtime, Signal},
    scene::{BoxShadow, Color, Transform},
};

/// Plain styles for a pseudo-state such as `hover`.
pub(crate) type Styles = Rc<[(Prop, Lit)]>;

pub(crate) struct Node {
    /// The script that built this node. Lowered closures hold it strongly;
    /// the Lua heap does not, so a replaced script can be dropped.
    pub owner: Weak<Vm>,
    pub kind: Kind,
    pub props: Vec<(Prop, Source)>,
    pub pseudo: Vec<(Pseudo, Styles)>,
    pub children: Vec<Rc<Node>>,
    pub id: Option<Rc<str>>,
    pub classes: Rc<[Rc<str>]>,
    pub refs: Vec<Rc<Ref>>,
    pub events: Events,
    pub focusable: Option<bool>,
    pub disabled: Option<Source>,
    /// `layout = "height"`: the node's size follows its content, animated.
    pub layout: Option<LayoutAnim>,
    /// `mask = { bitmap, w, h, x, y }`: the content shows through the
    /// bitmap's alpha, drawn at `w`×`h` from (`x`, `y`).
    pub mask: Option<Mask>,
    /// `bg_shader = ui.shader(...)`: the box is filled by a WGSL function.
    pub shader: Option<Arc<zgui::decoration::FillShader>>,
    /// `shader_params`: the shader's sixteen values, as four vec4s.
    pub shader_params: Option<[[f32; 4]; 4]>,
}
/// An alpha mask over a node's content; the offset animates (`maskX`).
#[derive(Clone)]
pub(crate) struct Mask {
    pub image: Arc<zgui::image::ImageData>,
    pub size: (f32, f32),
    pub offset: (f32, f32),
}
/// A web view's page, controller and handlers.
pub(crate) struct WebSpec {
    /// Shared with `ui.web()` when given, so the page outlives this node.
    pub web: zgui::web_view::WebView,
    pub src: Option<Source>,
    pub html: Option<(String, Option<String>)>,
    pub on_navigate: Option<Function>,
    pub on_load: Option<Function>,
    pub on_title: Option<Function>,
    pub on_message: Option<Function>,
    pub on_error: Option<Function>,
    pub on_new_window: Option<Function>,
}

/// `ui.web(opts)`: a web view's controller, usable before and after its
/// view mounts.
#[derive(Clone)]
pub(crate) struct WebUd(pub zgui::web_view::WebView);
impl UserData for WebUd {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // Reads are tracked: a text or computed that reads them follows them.
        fields.add_field_method_get("url", |_, w| Ok(w.0.url()));
        fields.add_field_method_get("title", |_, w| Ok(w.0.title()));
        fields.add_field_method_get("loading", |_, w| Ok(w.0.loading()));
        fields.add_field_method_get("progress", |_, w| Ok(w.0.progress()));
        fields.add_field_method_get("can_go_back", |_, w| Ok(w.0.can_go_back()));
        fields.add_field_method_get("can_go_forward", |_, w| Ok(w.0.can_go_forward()));
        fields.add_field_method_get("focused", |_, w| Ok(w.0.focused()));
        fields.add_field_method_get("error", |_, w| Ok(w.0.state().get().error));
    }
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("load", |_, w, url: String| {
            w.0.load_url(url);
            Ok(())
        });
        methods.add_method("html", |_, w, (html, base): (String, Option<String>)| {
            w.0.load_html(html, base.as_deref());
            Ok(())
        });
        methods.add_method("back", |_, w, ()| {
            w.0.go_back();
            Ok(())
        });
        methods.add_method("forward", |_, w, ()| {
            w.0.go_forward();
            Ok(())
        });
        methods.add_method("reload", |_, w, ()| {
            w.0.reload();
            Ok(())
        });
        methods.add_method("stop", |_, w, ()| {
            w.0.stop();
            Ok(())
        });
        methods.add_method("focus", |_, w, ()| {
            w.0.focus();
            Ok(())
        });
        methods.add_method("blur", |_, w, ()| {
            w.0.blur();
            Ok(())
        });
        methods.add_method("devtools", |_, w, ()| {
            w.0.open_devtools();
            Ok(())
        });
        methods.add_method("zoom", |_, w, zoom: f64| {
            w.0.set_zoom(zoom);
            Ok(())
        });
        methods.add_method("run", |_, w, js: String| {
            w.0.run_js(js);
            Ok(())
        });
        // `web:eval(js, function(value, err) … end)`: the result as Luau data.
        methods.add_method("eval", |lua, w, (js, reply): (String, Option<Function>)| {
            let vm = env(lua)?.vm.clone();
            w.0.evaluate_js(js, move |result| {
                let (Some(reply), Some(vm)) = (reply, vm.upgrade()) else {
                    return;
                };
                let outcome = match result {
                    Ok(json) => vm.call::<()>(&reply, (json_value(&json), mlua::Value::Nil)),
                    Err(error) => vm.call::<()>(&reply, (Value::Nil, error)),
                };
                if let Err(e) = outcome {
                    vm.report(e);
                }
            });
            Ok(())
        });
    }
}
/// JSON as script data (objects as maps, arrays as lists).
pub(crate) fn json_value(json: &str) -> Value {
    fn convert(v: serde_json::Value) -> Value {
        match v {
            serde_json::Value::Null => Value::Nil,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(0.)),
            serde_json::Value::String(s) => Value::Str(s.into()),
            serde_json::Value::Array(a) => {
                Value::List(a.into_iter().map(convert).collect::<Vec<_>>().into())
            }
            serde_json::Value::Object(o) => Value::Map(
                o.into_iter()
                    .map(|(k, v)| (k.into(), convert(v)))
                    .collect::<Vec<_>>()
                    .into(),
            ),
        }
    }
    serde_json::from_str(json)
        .map(convert)
        .unwrap_or_else(|_| Value::Str(json.into()))
}
/// A controller's options from a script table.
fn web_options(table: &Table) -> mlua::Result<zgui::web_view::WebViewOptions> {
    use zgui::web_view::{WebProfile, WebViewOptions};
    let defaults = WebViewOptions::default();
    let profile = match table.get::<Option<String>>("profile")?.as_deref() {
        None | Some("default") => WebProfile::Default,
        Some("ephemeral") => WebProfile::Ephemeral,
        Some(name) => WebProfile::Named(name.to_owned()),
    };
    Ok(WebViewOptions {
        transparent: table
            .get::<Option<bool>>("transparent")?
            .unwrap_or(defaults.transparent),
        profile,
        inspectable: table
            .get::<Option<bool>>("inspectable")?
            .unwrap_or(defaults.inspectable),
        user_agent: table.get::<Option<String>>("user_agent")?,
        messages: table
            .get::<Option<bool>>("messages")?
            .unwrap_or(defaults.messages),
        new_windows: table
            .get::<Option<bool>>("new_windows")?
            .unwrap_or(defaults.new_windows),
    })
}

/// `ui.shader(source, { fps })`: a WGSL fill, compiled once per source.
#[derive(Clone)]
pub(crate) struct ShaderUd(pub Arc<zgui::decoration::FillShader>);
impl UserData for ShaderUd {}

/// `ui.bitmap(w, h, fn)`: pixels made by a script, for masks.
#[derive(Clone)]
pub(crate) struct BitmapUd(pub Arc<zgui::image::ImageData>);
impl UserData for BitmapUd {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("w", |_, b| Ok(b.0.width()));
        fields.add_field_method_get("h", |_, b| Ok(b.0.height()));
    }
}

/// How a node's size follows its content.
#[derive(Clone)]
pub(crate) struct LayoutAnim {
    pub width: bool,
    pub height: bool,
    /// ms; a spring's own settling time when `None`.
    pub duration: Option<f64>,
    pub ease: Option<crate::anime::ease::Ease>,
    /// Grows from zero when it first mounts, instead of starting at its size.
    pub enter: bool,
    /// Clips the content to the animated size.
    pub clip: bool,
}
impl Node {
    fn new(owner: Weak<Vm>, kind: Kind) -> Self {
        Self {
            owner,
            kind,
            props: Vec::new(),
            pseudo: Vec::new(),
            children: Vec::new(),
            id: None,
            classes: Rc::new([]),
            refs: Vec::new(),
            events: Events::default(),
            focusable: None,
            disabled: None,
            layout: None,
            mask: None,
            shader: None,
            shader_params: None,
        }
    }
    /// Whether scripts can reach this node (selectors, refs, `on_mount`):
    /// only those carry an element and its animated values.
    pub fn targetable(&self) -> bool {
        self.id.is_some()
            || !self.classes.is_empty()
            || !self.refs.is_empty()
            || self.events.mount.is_some()
            || self.events.resize.is_some()
    }
    /// Animations, scroll offsets, widget bindings and overlays need a
    /// component context.
    pub fn needs_context(&self) -> bool {
        matches!(
            self.kind,
            Kind::Scroll { .. }
                | Kind::Checkbox { .. }
                | Kind::Input { .. }
                | Kind::Progress { .. }
                | Kind::Slider { .. }
                | Kind::List { .. }
                | Kind::Modal { .. }
                | Kind::Popover { .. }
                | Kind::Menu { .. }
                | Kind::Submenu { .. }
                | Kind::Presence { .. }
        ) || self.props.iter().any(|(_, s)| matches!(s, Source::Anim(_)))
    }
    /// The element kind selectors match (`button`, `text`…).
    pub fn tag(&self) -> &'static str {
        match self.kind {
            Kind::Column => "column",
            Kind::Row => "row",
            Kind::Div => "div",
            Kind::Overlay => "overlay",
            Kind::Button => "button",
            Kind::Scroll { .. } => "scroll",
            Kind::Text(_) => "text",
            Kind::Rich(_) => "rich",
            Kind::Image { .. } => "image",
            Kind::Svg { .. } => "svg",
            Kind::Canvas(_) => "canvas",
            Kind::WebView(_) => "webview",
            Kind::Terminal(_) => "terminal",
            Kind::Checkbox { .. } => "checkbox",
            Kind::Input { .. } => "input",
            Kind::Progress { .. } => "progress",
            Kind::Slider { .. } => "slider",
            Kind::Show { .. } => "show",
            Kind::Each { .. } => "each",
            Kind::List { .. } => "list",
            Kind::Switch { .. } => "switch",
            Kind::Modal { .. } => "modal",
            Kind::Popover { .. } => "popover",
            Kind::Menu { .. } => "menu",
            Kind::MenuItem(_) => "menu_item",
            Kind::Submenu { .. } => "submenu",
            Kind::Component { .. } => "component",
            Kind::Presence { .. } => "presence",
            Kind::Slot(_) => "slot",
        }
    }
}

/// Script handlers for input and lifecycle events.
#[derive(Default)]
pub(crate) struct Events {
    pub click: Option<Handler>,
    /// Primary pointer-down: for what must start before release, such as
    /// dragging the window.
    pub press: Option<Handler>,
    pub release: Option<Handler>,
    /// `fn(inside)` on pointer enter and leave.
    pub hover: Option<Handler>,
    /// `fn({ x, y, local_x, local_y })`.
    pub pointer_move: Option<Handler>,
    /// `fn({ key, shift, ctrl, alt, meta, repeat })`; returning true
    /// consumes the key.
    pub key: Option<Handler>,
    pub key_up: Option<Handler>,
    pub focus: Option<Handler>,
    pub blur: Option<Handler>,
    /// `fn({ dx, dy })`.
    pub wheel: Option<Handler>,
    /// `fn(element)` once mounted: start animations here.
    pub mount: Option<Function>,
    /// `fn({ w, h }, element)` when layout gives the node a new size (and
    /// once at first layout), as a ResizeObserver reports.
    pub resize: Option<Function>,
    /// Drag and drop within the window (payloads are script values):
    /// `on_drag() -> payload` starts a drag after a 4-point move;
    /// `drag_preview(payload) -> node` follows the pointer;
    /// `on_drag_end(payload, accepted)` on the source when it ends.
    pub drag: Option<Function>,
    pub drag_preview: Option<Function>,
    pub drag_end: Option<Function>,
    /// On a destination: `drop_when(payload) -> bool` (default: all),
    /// `on_drag_over(payload, { x, y, local_x, local_y, w, h })` while an
    /// accepted drag hovers, `on_drag_leave(payload)` when it leaves,
    /// `on_drop(payload, { ... })` on release.
    pub drop: Option<Function>,
    pub drop_when: Option<Function>,
    pub drag_over: Option<Function>,
    pub drag_leave: Option<Function>,
    /// `fn({ x, y, local_x, local_y }) -> { menu_item, … }`: a secondary
    /// click (control-click on macOS) shows what it returns as a menu at
    /// the pointer; nothing returned, no menu.
    pub context_menu: Option<Function>,
    /// `context_menu_style = { bg, rounded, p, backdrop_blur, … }`: the
    /// menu panel's styles (plain values).
    pub context_menu_style: Option<Styles>,
    /// `context_menu_below = true`: the menu opens under the node (a tab's),
    /// not at the pointer.
    pub context_menu_below: bool,
}
impl Kind {
    /// Kinds that become a container of whatever they currently show.
    pub fn is_region(&self) -> bool {
        matches!(
            self,
            Kind::Show { .. }
                | Kind::Each { .. }
                | Kind::Switch { .. }
                | Kind::Presence { .. }
                | Kind::Slot(_)
        )
    }
}
impl Events {
    pub fn any_input(&self) -> bool {
        self.press.is_some()
            || self.release.is_some()
            || self.hover.is_some()
            || self.pointer_move.is_some()
            || self.key.is_some()
            || self.key_up.is_some()
            || self.focus.is_some()
            || self.blur.is_some()
            || self.wheel.is_some()
    }
    pub fn any_drag(&self) -> bool {
        self.drag.is_some()
            || self.drop.is_some()
            || self.drag_over.is_some()
            || self.drag_leave.is_some()
    }
}

pub(crate) enum Kind {
    Column,
    Row,
    Div,
    Overlay,
    Button,
    /// `offset`: state the scroll position follows and reports, in points.
    /// `follow`: stays at the end while the content grows, until scrolled away.
    Scroll {
        offset: Option<Signal<Value>>,
        horizontal: bool,
        follow: bool,
    },
    Text(Text),
    Rich(Vec<Span>),
    Image {
        label: String,
        src: Source,
    },
    Svg {
        label: String,
        src: Source,
    },
    /// `fn(canvas, width, height)`, run again when its size or the state it
    /// reads changes.
    Canvas(Function),
    /// `ui.webview { ... }`: a native web view laid out like a div.
    WebView(Box<WebSpec>),
    /// `ui.terminal { term = ui.term(id) }`: a terminal session, shown
    /// where the node is.
    Terminal(zgui::terminal::Terminal),
    Checkbox {
        label: String,
        value: Signal<Value>,
    },
    Input {
        label: String,
        value: Signal<Value>,
        multiline: bool,
    },
    Progress {
        label: String,
        value: Signal<Value>,
    },
    Slider {
        label: String,
        value: Signal<Value>,
        min: f32,
        max: f32,
    },
    Show {
        when: Source,
        then: Rc<Node>,
        otherwise: Option<Rc<Node>>,
    },
    Each {
        items: Signal<Value>,
        build: Function,
        key: Option<Function>,
    },
    /// A virtualized list: only visible rows are built.
    List {
        count: Source,
        height: f32,
        overscan: usize,
        build: Function,
        offset: Option<Signal<Value>>,
    },
    Switch {
        value: Source,
        cases: Vec<(Value, Rc<Node>)>,
        default: Option<Rc<Node>>,
    },
    Modal {
        label: String,
        open: Signal<Value>,
    },
    Popover {
        label: String,
        open: Signal<Value>,
        anchor: Rc<Node>,
    },
    Menu {
        label: String,
        open: Signal<Value>,
        anchor: Rc<Node>,
    },
    MenuItem(String),
    Submenu {
        label: String,
        open: Signal<Value>,
    },
    /// `ui.component(fn)`: `fn(props)` runs as each instance mounts, with
    /// its own `ui.use_state` state, effects and mount hooks.
    Component {
        build: Function,
        props: Table,
    },
    /// `ui.presence(value, build, { enter, exit, mode })`: content for the
    /// current value, entering and exiting with animations as it changes.
    Presence {
        value: Source,
        build: Function,
        enter: Option<Table>,
        exit: Option<Table>,
        /// `mode = "wait"`: the new content enters once the old has left.
        wait: bool,
        /// `initial = true`: the first content enters too.
        initial: bool,
    },
    Slot(String),
}
impl Kind {
    fn takes_children(&self) -> bool {
        matches!(
            self,
            Kind::Column
                | Kind::Row
                | Kind::Div
                | Kind::Overlay
                | Kind::Button
                | Kind::Scroll { .. }
                | Kind::Modal { .. }
                | Kind::Popover { .. }
                | Kind::Menu { .. }
                | Kind::Submenu { .. }
        )
    }
}

pub(crate) enum Text {
    Literal(String),
    /// `"Count: {count}"`: interpolated in Rust, no script call per update.
    Template(Vec<Segment>),
    /// A script function, rerun when the state it reads changes.
    Computed(Function),
}
pub(crate) enum Segment {
    Literal(String),
    State(Signal<Value>),
}

/// A rich text run.
pub(crate) struct Span {
    pub text: Text,
    pub color: Option<Color>,
    pub background: Option<Color>,
    pub size: Option<f32>,
    pub weight: Option<u16>,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub mono: bool,
    pub click: Option<Handler>,
}

pub(crate) enum Source {
    Lit(Lit),
    State(Signal<Value>),
    Computed(Function),
    Anim(Rc<Anim>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Lit {
    Num(f32),
    Bool(bool),
    Str(Rc<str>),
    Color(Color),
    Pair(f32, f32),
    Len(Length),
    Shadows(Arc<[BoxShadow]>),
    Gradient(f32, Arc<[GradientStop]>),
    Corners(Corners),
}

#[derive(Clone)]
pub(crate) enum Handler {
    Lua(Function),
    Action(Action),
}
#[derive(Clone)]
pub(crate) enum Action {
    Set(Signal<Value>, Value),
    Toggle(Signal<Value>),
    Add(Signal<Value>, f64),
    /// A function the Rust application registered with `ScriptHost::action`.
    Host(Rc<str>, Value),
}

/// A Rust-driven animation of a scalar bound to state (`ui.tween`,
/// `ui.spring`, `ui.loop`). With `from`/`to`, the scalar is a 0..1 progress
/// mapped between them (numbers, colors or pairs).
pub(crate) struct Anim {
    pub source: Option<Signal<Value>>,
    pub curve: Curve,
    pub from: Option<Value>,
    pub to: Option<Value>,
}
#[derive(Clone)]
pub(crate) enum Curve {
    Tween {
        seconds: f32,
        ease: crate::anime::ease::Ease,
    },
    Spring {
        stiffness: f32,
        damping: f32,
    },
    /// `fps` caps the frames a decorative loop asks for.
    Loop {
        seconds: f32,
        ease: crate::anime::ease::Ease,
        yoyo: bool,
        fps: Option<f64>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pseudo {
    Hover,
    Active,
    Focus,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ty {
    Num,
    Bool,
    Color,
    Str,
    Pair,
    /// Points, `"50%"` or `"auto"`.
    Len,
    Shadow,
    Gradient,
    Corners,
}

macro_rules! props {
    ($($name:literal => $variant:ident: $ty:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum Prop { $($variant),* }
        impl Prop {
            pub(crate) fn parse(name: &str) -> Option<Self> {
                match name { $($name => Some(Prop::$variant),)* _ => None }
            }
            /// Numbers, lengths and colors interpolate; the rest switch.
            pub(crate) fn animatable(self) -> bool {
                matches!(self.ty(), Ty::Num | Ty::Color | Ty::Pair | Ty::Len)
            }
            pub fn ty(self) -> Ty {
                match self { $(Prop::$variant => Ty::$ty),* }
            }
        }
    };
}
props! {
    // Size and position.
    "w" => W: Len, "h" => H: Len, "size" => Size: Pair,
    "min_w" => MinW: Len, "max_w" => MaxW: Len, "min_h" => MinH: Len, "max_h" => MaxH: Len,
    "w_full" => WFull: Bool, "h_full" => HFull: Bool, "aspect" => Aspect: Num,
    "absolute" => Absolute: Bool, "relative" => Relative: Bool,
    "top" => Top: Len, "left" => Left: Len, "right" => Right: Len, "bottom" => Bottom: Len,
    // Spacing.
    "p" => P: Len, "px" => Px: Len, "py" => Py: Len,
    "pt" => Pt: Len, "pr" => Pr: Len, "pb" => Pb: Len, "pl" => Pl: Len,
    "m" => M: Len, "mx" => Mx: Len, "my" => My: Len,
    "mt" => Mt: Len, "mr" => Mr: Len, "mb" => Mb: Len, "ml" => Ml: Len,
    "gap" => Gap: Num, "gap_x" => GapX: Len, "gap_y" => GapY: Len,
    // Flex and grid.
    "direction" => Direction: Str, "grow" => Grow: Num, "shrink" => Shrink: Num, "basis" => Basis: Len,
    "flex_wrap" => FlexWrap: Bool, "wrap_reverse" => WrapReverse: Bool,
    "items" => Items: Str, "justify" => Justify: Str, "self" => AlignSelf: Str, "content" => AlignContent: Str,
    "grid" => Grid: Bool, "cols" => Cols: Num, "rows" => Rows: Num,
    "col_start" => ColStart: Num, "row_start" => RowStart: Num, "col_span" => ColSpan: Num, "row_span" => RowSpan: Num,
    "col_span_full" => ColSpanFull: Bool, "row_span_full" => RowSpanFull: Bool,
    "overflow" => Overflow: Str, "overflow_x" => OverflowX: Str, "overflow_y" => OverflowY: Str,
    "hidden" => Hidden: Bool, "invisible" => Invisible: Bool,
    // Paint.
    "bg" => Bg: Color, "gradient" => Gradient: Gradient, "color" => TextColor: Color,
    "bg_image" => BgImage: Str, "bg_fit" => BgFit: Str,
    "rounded" => Rounded: Num, "corners" => Corners: Corners,
    "rounded_tl" => RoundedTl: Num, "rounded_tr" => RoundedTr: Num,
    "rounded_br" => RoundedBr: Num, "rounded_bl" => RoundedBl: Num,
    "border" => Border: Num, "border_sides" => BorderSides: Corners, "border_color" => BorderColor: Color,
    "border_over" => BorderOver: Bool, "border_dash" => BorderDash: Pair,
    "shadow" => Shadow: Shadow, "opacity" => Opacity: Num,
    "brightness" => Brightness: Num, "contrast" => Contrast: Num, "saturate" => Saturate: Num, "blur" => Blur: Num, "backdrop_blur" => BackdropBlur: Num,
    "fade" => Fade: Pair, "fade_x" => FadeX: Pair, "fade_sides" => FadeSides: Corners,
    "clip_inset" => ClipInset: Corners, "layer" => Layer: Bool, "fit" => Fit: Str, "tint" => Tint: Color,
    "cursor" => Cursor: Str,
    // Text.
    "text_size" => TextSize: Num, "weight" => Weight: Num, "bold" => Bold: Bool, "italic" => Italic: Bool,
    "align" => Align: Str, "font" => Font: Str, "wrap" => Wrap: Bool, "truncate" => Truncate: Bool,
    "lines" => Lines: Num, "line_height" => LineHeight: Num, "letter_spacing" => LetterSpacing: Num,
    // Transforms: after layout, about `origin` (fractions of the node).
    "x" => X: Num, "y" => Y: Num,
    "scale" => Scale: Num, "scale_x" => ScaleX: Num, "scale_y" => ScaleY: Num,
    "rotate" => Rotate: Num, "skew" => Skew: Num, "skew_x" => SkewX: Num, "skew_y" => SkewY: Num,
    "origin" => Origin: Pair,
}

/// Per-script data the `ui` library reads.
pub(crate) struct Env {
    pub vm: Weak<Vm>,
    pub store: Rc<Store>,
    pub shared: Rc<Store>,
    pub contributions: RefCell<Vec<(String, Rc<Node>)>>,
    pub engine: Rc<crate::anime::Engine>,
    pub runtime: Runtime,
    /// The component instance being built, if any.
    pub scope: RefCell<Option<Rc<Scope>>>,
    /// Effects created outside components: they live as long as the script.
    pub retained: RefCell<Vec<Box<dyn Any>>>,
    /// The host's actions (`ui.action` binds them, `ui.invoke` calls them).
    pub actions: crate::Actions,
}

/// What a component instance owns while it is mounted.
#[derive(Default)]
pub(crate) struct Scope {
    pub retained: RefCell<Vec<Box<dyn Any>>>,
    pub mounted: RefCell<Vec<Function>>,
}

pub(crate) struct NodeUd(pub Rc<Node>);
impl UserData for NodeUd {}
pub(crate) struct AnimUd(pub Rc<Anim>);
impl UserData for AnimUd {}
pub(crate) struct ActionUd(pub Action);
impl UserData for ActionUd {}
pub(crate) struct SpanUd(pub RefCell<Option<Span>>);
impl UserData for SpanUd {}

pub(crate) fn env(lua: &Lua) -> mlua::Result<mlua::AppDataRef<'_, Env>> {
    lua.app_data_ref::<Env>()
        .ok_or_else(|| mlua::Error::runtime("ui library used outside a script host"))
}
fn node_ud(lua: &Lua, node: Node) -> mlua::Result<mlua::AnyUserData> {
    lua.create_userdata(NodeUd(Rc::new(node)))
}
fn state_of(value: &mlua::Value) -> Option<Signal<Value>> {
    match value {
        mlua::Value::UserData(ud) => ud.borrow::<State>().ok().map(|s| s.0.clone()),
        _ => None,
    }
}
fn expect_state(value: &mlua::Value, what: &str) -> mlua::Result<Signal<Value>> {
    state_of(value).ok_or_else(|| mlua::Error::runtime(format!("{what} must be a ui.state")))
}
fn expect_node(value: mlua::Value, what: &str) -> mlua::Result<Rc<Node>> {
    match value {
        mlua::Value::UserData(ud) if ud.is::<NodeUd>() => Ok(ud.borrow::<NodeUd>()?.0.clone()),
        other => Err(mlua::Error::runtime(format!(
            "{what} must be a ui node, not {}",
            other.type_name()
        ))),
    }
}

pub(crate) fn parse_color(text: &str) -> Option<Color> {
    crate::anime::value::parse_color(text).map(crate::anime::target::v_to_color)
}
fn color_of(value: &Value) -> Option<Color> {
    match value {
        Value::Num(n) => {
            let n = *n as u32;
            Some(Color((n >> 16) as u8, (n >> 8) as u8, n as u8, 255))
        }
        Value::Str(s) => parse_color(s),
        _ => None,
    }
}
fn length(value: &Value) -> Option<Length> {
    match value {
        Value::Num(n) => Some(Length::Px(*n as f32)),
        Value::Str(s) if &**s == "auto" => Some(Length::Auto),
        Value::Str(s) => match s.trim().strip_suffix('%') {
            Some(p) => p
                .trim()
                .parse::<f32>()
                .ok()
                .map(|p| Length::Percent(p / 100.)),
            None => s
                .trim()
                .trim_end_matches("px")
                .parse::<f32>()
                .ok()
                .map(Length::Px),
        },
        _ => None,
    }
}
fn num(value: &Value, key: &str) -> f32 {
    value.field(key).and_then(Value::as_f64).unwrap_or(0.) as f32
}
fn shadow(value: &Value) -> Option<BoxShadow> {
    Some(BoxShadow {
        color: value
            .field("color")
            .and_then(color_of)
            .unwrap_or(Color(0, 0, 0, 90)),
        offset: Transform {
            x: num(value, "x"),
            y: num(value, "y"),
        },
        // CSS's blur radius is twice the Gaussian sigma zgui takes.
        blur_radius: num(value, "blur") / 2.,
        spread: num(value, "spread"),
    })
}
/// Converts a value to a property's type, or `None` if it does not fit.
pub(crate) fn lit(value: &Value, ty: Ty) -> Option<Lit> {
    Some(match (ty, value) {
        (Ty::Num, v) => Lit::Num(v.as_f64()? as f32),
        (Ty::Bool, v) => Lit::Bool(v.truthy()),
        (Ty::Color, v) => Lit::Color(color_of(v)?),
        (Ty::Str, Value::Str(s)) => Lit::Str(s.clone()),
        (Ty::Pair, Value::List(items)) if items.len() == 2 => {
            Lit::Pair(items[0].as_f64()? as f32, items[1].as_f64()? as f32)
        }
        (Ty::Pair, Value::Num(n)) => Lit::Pair(*n as f32, *n as f32),
        (Ty::Len, v) => Lit::Len(length(v)?),
        (Ty::Shadow, Value::List(items)) => {
            Lit::Shadows(items.iter().map(shadow).collect::<Option<Vec<_>>>()?.into())
        }
        (Ty::Shadow, v @ Value::Map(_)) => Lit::Shadows(Arc::from([shadow(v)?])),
        (Ty::Shadow, Value::Bool(false) | Value::Nil) => Lit::Shadows(Arc::from([])),
        (Ty::Gradient, v) => {
            let (angle, stops) = match v {
                Value::List(stops) => (180., stops.clone()),
                Value::Map(_) => (
                    v.field("angle").and_then(Value::as_f64).unwrap_or(180.),
                    match v.field("stops")? {
                        Value::List(s) => s.clone(),
                        _ => return None,
                    },
                ),
                _ => return None,
            };
            let n = stops.len();
            if n < 2 {
                return None;
            }
            let stops: Vec<GradientStop> = stops
                .iter()
                .enumerate()
                .map(|(i, stop)| match stop {
                    Value::List(pair) if pair.len() == 2 => Some(GradientStop {
                        offset: pair[0].as_f64()? as f32,
                        color: color_of(&pair[1])?,
                    }),
                    other => Some(GradientStop {
                        offset: i as f32 / (n - 1) as f32,
                        color: color_of(other)?,
                    }),
                })
                .collect::<Option<_>>()?;
            // CSS angles (0 points up, clockwise; the default 180 runs top
            // to bottom); zgui's are clockwise from +x.
            Lit::Gradient(((angle - 90.) as f32).to_radians(), stops.into())
        }
        (Ty::Corners, Value::List(r)) if r.len() == 4 => Lit::Corners(Corners {
            top_left: r[0].as_f64()? as f32,
            top_right: r[1].as_f64()? as f32,
            bottom_right: r[2].as_f64()? as f32,
            bottom_left: r[3].as_f64()? as f32,
        }),
        (Ty::Corners, v @ Value::Map(_)) => Lit::Corners(Corners {
            top_left: num(v, "tl"),
            top_right: num(v, "tr"),
            bottom_right: num(v, "br"),
            bottom_left: num(v, "bl"),
        }),
        _ => return None,
    })
}

fn source(lua: &Lua, ty: Ty, name: &str, value: mlua::Value) -> mlua::Result<Source> {
    if let Some(state) = state_of(&value) {
        return Ok(Source::State(state));
    }
    match value {
        mlua::Value::Function(f) => Ok(Source::Computed(f)),
        mlua::Value::UserData(ud) if ud.is::<AnimUd>() => {
            Ok(Source::Anim(ud.borrow::<AnimUd>()?.0.clone()))
        }
        other => {
            let value = lua.unpack::<Value>(other)?;
            lit(&value, ty).map(Source::Lit).ok_or_else(|| {
                mlua::Error::runtime(format!("`{name}` expects a {ty:?}, got {value:?}"))
            })
        }
    }
}

fn handler(value: mlua::Value) -> mlua::Result<Handler> {
    match value {
        mlua::Value::Function(f) => Ok(Handler::Lua(f)),
        mlua::Value::UserData(ud) if ud.is::<ActionUd>() => {
            Ok(Handler::Action(ud.borrow::<ActionUd>()?.0.clone()))
        }
        other => Err(mlua::Error::runtime(format!(
            "handlers take a function or a ui action, not {}",
            other.type_name()
        ))),
    }
}

/// `{name}` binds state by name; `{{` and `}}` are literal braces. Anything
/// else in braces is literal, so data shown as text never needs escaping
/// unless it happens to look like `{identifier}`.
fn template(lua: &Lua, text: &str) -> mlua::Result<Text> {
    if !text.contains(['{', '}']) {
        return Ok(Text::Literal(text.to_owned()));
    }
    let env = env(lua)?;
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("{{").or_else(|| rest.strip_prefix("}}")) {
            literal.push(c);
            rest = after;
            continue;
        }
        let name = (c == '{')
            .then(|| rest[1..].split_once('}'))
            .flatten()
            .filter(|(name, _)| {
                name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        let Some((name, after)) = name else {
            literal.push(c);
            rest = &rest[c.len_utf8()..];
            continue;
        };
        let state = env
            .store
            .get(name)
            .or_else(|| env.shared.get(name))
            .ok_or_else(|| {
                mlua::Error::runtime(format!("text `{text}` names unknown state `{name}`"))
            })?;
        if !literal.is_empty() {
            segments.push(Segment::Literal(std::mem::take(&mut literal)));
        }
        segments.push(Segment::State(state));
        rest = after;
    }
    if segments.is_empty() {
        return Ok(Text::Literal(literal));
    }
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(Text::Template(segments))
}
fn text_of(lua: &Lua, value: mlua::Value, what: &str) -> mlua::Result<Text> {
    Ok(match value {
        mlua::Value::String(s) => template(lua, &s.to_string_lossy())?,
        mlua::Value::Function(f) => Text::Computed(f),
        mlua::Value::Number(n) => Text::Literal(Value::Num(n).to_string()),
        mlua::Value::Integer(n) => Text::Literal(n.to_string()),
        mlua::Value::Nil => Text::Literal(String::new()),
        other => {
            return Err(mlua::Error::runtime(format!(
                "{what} takes a string or function, not {}",
                other.type_name()
            )));
        }
    })
}

/// A table's array entries in order, skipping holes: `cond and node or
/// nil` in a child list leaves a nil, which must not end the list (Lua's
/// length and `ipairs` both stop at the first hole).
pub(crate) fn array_items(table: &Table) -> mlua::Result<Vec<mlua::Value>> {
    let mut items: Vec<(i64, mlua::Value)> = Vec::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        if let mlua::Value::Integer(i) = key
            && i >= 1
        {
            items.push((i, value));
        }
    }
    items.sort_unstable_by_key(|(i, _)| *i);
    Ok(items.into_iter().map(|(_, v)| v).collect())
}

/// Appends children from a table's array part: nodes, strings (text),
/// functions (computed text) and nested arrays, which flatten.
fn children(
    lua: &Lua,
    owner: &Weak<Vm>,
    table: &Table,
    out: &mut Vec<Rc<Node>>,
) -> mlua::Result<()> {
    for child in array_items(table)? {
        match child {
            mlua::Value::UserData(ud) if ud.is::<NodeUd>() => {
                out.push(ud.borrow::<NodeUd>()?.0.clone())
            }
            mlua::Value::String(s) => out.push(Rc::new(Node::new(
                owner.clone(),
                Kind::Text(template(lua, &s.to_string_lossy())?),
            ))),
            mlua::Value::Function(f) => out.push(Rc::new(Node::new(
                owner.clone(),
                Kind::Text(Text::Computed(f)),
            ))),
            mlua::Value::Table(nested) => children(lua, owner, &nested, out)?,
            mlua::Value::Nil | mlua::Value::Boolean(false) => {}
            other => {
                return Err(mlua::Error::runtime(format!(
                    "a child must be a ui node, string or function, not {}",
                    other.type_name()
                )));
            }
        }
    }
    Ok(())
}

/// Fields a constructor reads itself rather than as style properties.
const WIDGET_FIELDS: &[&str] = &[
    "label",
    "value",
    "min",
    "max",
    "multiline",
    "offset",
    "follow",
    "src",
    "code",
    "draw",
    "count",
    "height",
    "overscan",
    "row",
    "open",
    "anchor",
    "key",
    "enter",
    "exit",
    "mode",
    "initial",
];

fn set_handler(events: &mut Events, key: &str, value: mlua::Value) -> mlua::Result<bool> {
    let slot = match key {
        "on_click" => &mut events.click,
        "on_press" => &mut events.press,
        "on_release" => &mut events.release,
        "on_hover" => &mut events.hover,
        "on_pointer_move" => &mut events.pointer_move,
        "on_key" => &mut events.key,
        "on_key_up" => &mut events.key_up,
        "on_focus" => &mut events.focus,
        "on_blur" => &mut events.blur,
        "on_wheel" => &mut events.wheel,
        "on_mount" => {
            let mlua::Value::Function(f) = value else {
                return Err(mlua::Error::runtime("on_mount takes a function"));
            };
            events.mount = Some(f);
            return Ok(true);
        }
        "on_resize" => {
            let mlua::Value::Function(f) = value else {
                return Err(mlua::Error::runtime("on_resize takes a function"));
            };
            events.resize = Some(f);
            return Ok(true);
        }
        "context_menu" => {
            let mlua::Value::Function(f) = value else {
                return Err(mlua::Error::runtime("context_menu takes a function"));
            };
            events.context_menu = Some(f);
            return Ok(true);
        }
        "on_drag" | "drag_preview" | "on_drag_end" | "on_drop" | "drop_when" | "on_drag_over"
        | "on_drag_leave" => {
            let mlua::Value::Function(f) = value else {
                return Err(mlua::Error::runtime(format!("{key} takes a function")));
            };
            *match key {
                "on_drag" => &mut events.drag,
                "drag_preview" => &mut events.drag_preview,
                "on_drag_end" => &mut events.drag_end,
                "on_drop" => &mut events.drop,
                "drop_when" => &mut events.drop_when,
                "on_drag_over" => &mut events.drag_over,
                _ => &mut events.drag_leave,
            } = Some(f);
            return Ok(true);
        }
        _ => return Ok(false),
    };
    *slot = Some(handler(value)?);
    Ok(true)
}

/// Fills a node's properties, pseudo-states, handlers and children.
fn fill(lua: &Lua, mut node: Node, table: Option<&Table>) -> mlua::Result<Node> {
    let Some(table) = table else {
        return Ok(node);
    };
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        let mlua::Value::String(key) = key else {
            continue;
        };
        let key = key.to_string_lossy();
        if set_handler(&mut node.events, &key, value.clone())? {
            continue;
        }
        match key.as_str() {
            "id" => node.id = Some(lua.unpack::<String>(value)?.into()),
            "class" => {
                let classes: String = lua.unpack(value)?;
                node.classes = classes.split_whitespace().map(Rc::from).collect();
            }
            "ref" => match value {
                mlua::Value::UserData(ud) if ud.is::<RefUd>() => {
                    node.refs.push(ud.borrow::<RefUd>()?.0.clone())
                }
                mlua::Value::Table(list) => {
                    for r in list.sequence_values::<mlua::AnyUserData>() {
                        node.refs.push(r?.borrow::<RefUd>()?.0.clone());
                    }
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "`ref` takes ui.ref(), not {}",
                        other.type_name()
                    )));
                }
            },
            "focusable" => node.focusable = Some(lua.unpack(value)?),
            "bg_shader" => {
                let mlua::Value::UserData(ud) = &value else {
                    return Err(mlua::Error::runtime("`bg_shader` takes a ui.shader"));
                };
                node.shader = Some(ud.borrow::<ShaderUd>()?.0.clone());
            }
            "shader_params" => {
                // Up to four vec4s: lists of numbers, or colors (0 to 1).
                let mlua::Value::Table(list) = &value else {
                    return Err(mlua::Error::runtime(
                        "`shader_params` takes a list of up to four vec4s",
                    ));
                };
                let mut params = [[0.; 4]; 4];
                for (i, entry) in list.sequence_values::<mlua::Value>().enumerate().take(4) {
                    params[i] = match entry? {
                        mlua::Value::Table(v) => {
                            let mut out = [0.; 4];
                            for (j, n) in v.sequence_values::<f32>().enumerate().take(4) {
                                out[j] = n?;
                            }
                            out
                        }
                        other => {
                            let value: Value = lua.unpack(other)?;
                            let c = color_of(&value).ok_or_else(|| {
                                mlua::Error::runtime(
                                    "a shader parameter is a list of numbers or a color",
                                )
                            })?;
                            [
                                c.0 as f32 / 255.,
                                c.1 as f32 / 255.,
                                c.2 as f32 / 255.,
                                c.3 as f32 / 255.,
                            ]
                        }
                    };
                }
                node.shader_params = Some(params);
            }
            "mask" => {
                let (bitmap, t) = match &value {
                    mlua::Value::UserData(ud) => (ud.borrow::<BitmapUd>()?.0.clone(), None),
                    mlua::Value::Table(t) => (
                        t.get::<mlua::AnyUserData>(1)?
                            .borrow::<BitmapUd>()?
                            .0
                            .clone(),
                        Some(t),
                    ),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "`mask` takes a ui.bitmap, not {}",
                            other.type_name()
                        )));
                    }
                };
                let get = |key: &str, default: f32| -> mlua::Result<f32> {
                    Ok(match t {
                        Some(t) => t.get::<Option<f32>>(key)?.unwrap_or(default),
                        None => default,
                    })
                };
                let size = (
                    get("w", bitmap.width() as f32)?,
                    get("h", bitmap.height() as f32)?,
                );
                let offset = (get("x", 0.)?, get("y", 0.)?);
                node.mask = Some(Mask {
                    image: bitmap,
                    size,
                    offset,
                });
            }
            "layout" => {
                let (width, height) = match &value {
                    mlua::Value::Boolean(true) => (false, true),
                    mlua::Value::Boolean(false) | mlua::Value::Nil => (false, false),
                    mlua::Value::String(s) => match &*s.to_string_lossy() {
                        "height" => (false, true),
                        "width" => (true, false),
                        "size" => (true, true),
                        other => {
                            return Err(mlua::Error::runtime(format!(
                                "layout is \"height\", \"width\" or \"size\", not `{other}`"
                            )));
                        }
                    },
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "layout cannot be {}",
                            other.type_name()
                        )));
                    }
                };
                let layout = node.layout.get_or_insert(LayoutAnim {
                    width,
                    height,
                    duration: None,
                    ease: None,
                    enter: false,
                    clip: false,
                });
                layout.width = width;
                layout.height = height;
                if !width && !height {
                    node.layout = None;
                }
            }
            "layout_duration" | "layout_ease" | "layout_enter" | "layout_clip" => {
                let layout = node.layout.get_or_insert(LayoutAnim {
                    width: false,
                    height: true,
                    duration: None,
                    ease: None,
                    enter: false,
                    clip: false,
                });
                if key == "layout_enter" {
                    layout.enter = lua.unpack::<bool>(value)?;
                } else if key == "layout_clip" {
                    layout.clip = lua.unpack::<bool>(value)?;
                } else if key == "layout_duration" {
                    layout.duration = Some(lua.unpack::<f64>(value)?);
                } else {
                    layout.ease = match value {
                        mlua::Value::String(s) => Some(
                            crate::anime::ease::Ease::parse(&s.to_string_lossy())
                                .map_err(mlua::Error::runtime)?,
                        ),
                        mlua::Value::UserData(ud) => {
                            Some(ud.borrow::<crate::anime::api::EaseUd>()?.0.clone())
                        }
                        other => {
                            return Err(mlua::Error::runtime(format!(
                                "layout_ease cannot be {}",
                                other.type_name()
                            )));
                        }
                    };
                }
            }
            "disabled" if !matches!(value, mlua::Value::Table(_)) => {
                node.disabled = Some(source(lua, Ty::Bool, "disabled", value)?);
            }
            "context_menu_below" => {
                let mlua::Value::Boolean(below) = value else {
                    return Err(mlua::Error::runtime("`context_menu_below` takes a boolean"));
                };
                node.events.context_menu_below = below;
            }
            "context_menu_style" => {
                let mlua::Value::Table(styles) = value else {
                    return Err(mlua::Error::runtime(
                        "`context_menu_style` takes a table of styles",
                    ));
                };
                let mut lits = Vec::new();
                for pair in styles.pairs::<String, mlua::Value>() {
                    let (name, value) = pair?;
                    let prop = Prop::parse(&name).ok_or_else(|| {
                        mlua::Error::runtime(format!(
                            "unknown style `{name}` in `context_menu_style`"
                        ))
                    })?;
                    let Source::Lit(lit) = source(lua, prop.ty(), &name, value)? else {
                        return Err(mlua::Error::runtime(format!(
                            "`context_menu_style.{name}` must be a plain value"
                        )));
                    };
                    lits.push((prop, lit));
                }
                node.events.context_menu_style = Some(lits.into());
            }
            "hover" | "active" | "focus" | "disabled" => {
                let pseudo = match key.as_str() {
                    "hover" => Pseudo::Hover,
                    "active" => Pseudo::Active,
                    "focus" => Pseudo::Focus,
                    _ => Pseudo::Disabled,
                };
                let mlua::Value::Table(styles) = value else {
                    return Err(mlua::Error::runtime(format!(
                        "`{key}` takes a table of styles"
                    )));
                };
                let mut lits = Vec::new();
                for pair in styles.pairs::<String, mlua::Value>() {
                    let (name, value) = pair?;
                    let prop = Prop::parse(&name).ok_or_else(|| {
                        mlua::Error::runtime(format!("unknown style `{name}` in `{key}`"))
                    })?;
                    let Source::Lit(lit) = source(lua, prop.ty(), &name, value)? else {
                        return Err(mlua::Error::runtime(format!(
                            "`{key}.{name}` must be a plain value; animate the base style instead"
                        )));
                    };
                    lits.push((prop, lit));
                }
                node.pseudo.push((pseudo, lits.into()));
            }
            name if WIDGET_FIELDS.contains(&name) => {}
            name => {
                let prop = Prop::parse(name)
                    .ok_or_else(|| mlua::Error::runtime(format!("unknown property `{name}`")))?;
                node.props
                    .push((prop, source(lua, prop.ty(), name, value)?));
            }
        }
    }
    let mut kids = Vec::new();
    children(lua, &node.owner, table, &mut kids)?;
    if !kids.is_empty() && !node.kind.takes_children() {
        return Err(mlua::Error::runtime(format!(
            "`{}` does not take children",
            node.tag()
        )));
    }
    node.children = kids;
    Ok(node)
}

fn owner(lua: &Lua) -> mlua::Result<Weak<Vm>> {
    Ok(env(lua)?.vm.clone())
}
fn label(table: &Table) -> mlua::Result<String> {
    Ok(table.get::<Option<String>>("label")?.unwrap_or_default())
}

/// `ui.span { "text", bold = true, color = "#f80", on_click = fn }`.
fn span(lua: &Lua, arg: mlua::Value) -> mlua::Result<Span> {
    let (content, table) = match arg {
        mlua::Value::Table(t) => (t.get::<mlua::Value>(1)?, Some(t)),
        other => (other, None),
    };
    let mut span = Span {
        text: text_of(lua, content, "ui.span")?,
        color: None,
        background: None,
        size: None,
        weight: None,
        italic: false,
        underline: false,
        strike: false,
        mono: false,
        click: None,
    };
    if let Some(t) = table {
        let value = |k: &str| t.get::<Value>(k);
        span.color = color_of(&value("color")?);
        span.background = color_of(&value("bg")?);
        span.size = value("text_size")?.as_f64().map(|v| v as f32);
        span.weight = value("weight")?
            .as_f64()
            .map(|v| v as u16)
            .or(value("bold")?.truthy().then_some(700));
        span.italic = value("italic")?.truthy();
        span.underline = value("underline")?.truthy();
        span.strike = value("strike")?.truthy();
        span.mono = matches!(&value("font")?, Value::Str(f) if &**f == "mono");
        if let Some(h) = t.get::<Option<mlua::Value>>("on_click")? {
            span.click = Some(handler(h)?);
        }
    }
    Ok(span)
}

/// Installs the read-only `ui` global.
pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    let ui = lua.create_table()?;
    let container = |kind: fn() -> Kind| {
        lua.create_function(move |lua, table: Option<Table>| {
            let node = fill(lua, Node::new(owner(lua)?, kind()), table.as_ref())?;
            node_ud(lua, node)
        })
    };
    ui.set("column", container(|| Kind::Column)?)?;
    ui.set("row", container(|| Kind::Row)?)?;
    ui.set("div", container(|| Kind::Div)?)?;
    ui.set("overlay", container(|| Kind::Overlay)?)?;
    ui.set("button", container(|| Kind::Button)?)?;
    ui.set(
        "grid",
        lua.create_function(|lua, table: Option<Table>| {
            let mut node = fill(lua, Node::new(owner(lua)?, Kind::Div), table.as_ref())?;
            node.props
                .insert(0, (Prop::Grid, Source::Lit(Lit::Bool(true))));
            node_ud(lua, node)
        })?,
    )?;
    for (name, horizontal) in [("scroll", false), ("scroll_x", true)] {
        ui.set(
            name,
            lua.create_function(move |lua, table: Option<Table>| {
                let offset = match &table {
                    Some(t) => t.get::<mlua::Value>("offset")?,
                    None => mlua::Value::Nil,
                };
                let offset = (!offset.is_nil())
                    .then(|| expect_state(&offset, "`offset`"))
                    .transpose()?;
                let follow = match &table {
                    Some(t) => t.get::<Option<bool>>("follow")?.unwrap_or(false),
                    None => false,
                };
                let node = fill(
                    lua,
                    Node::new(
                        owner(lua)?,
                        Kind::Scroll {
                            offset,
                            horizontal,
                            follow,
                        },
                    ),
                    table.as_ref(),
                )?;
                node_ud(lua, node)
            })?,
        )?;
    }

    ui.set(
        "text",
        lua.create_function(|lua, arg: mlua::Value| {
            let owner = owner(lua)?;
            let (content, table) = match arg {
                mlua::Value::Table(t) => (t.get::<mlua::Value>(1)?, Some(t)),
                other => (other, None),
            };
            let mut node = Node::new(owner, Kind::Text(text_of(lua, content, "ui.text")?));
            if let Some(table) = &table {
                // The content is element 1; the rest are styles.
                table.raw_remove(1)?;
                node = fill(lua, node, Some(table))?;
            }
            node_ud(lua, node)
        })?,
    )?;
    ui.set(
        "span",
        lua.create_function(|lua, arg: mlua::Value| {
            Ok(SpanUd(RefCell::new(Some(span(lua, arg)?))))
        })?,
    )?;
    ui.set(
        "rich",
        lua.create_function(|lua, table: Table| {
            let mut spans = Vec::new();
            for item in table.sequence_values::<mlua::Value>() {
                match item? {
                    mlua::Value::UserData(ud) if ud.is::<SpanUd>() => spans.push(
                        ud.borrow::<SpanUd>()?
                            .0
                            .borrow_mut()
                            .take()
                            .ok_or_else(|| mlua::Error::runtime("a span can be used once"))?,
                    ),
                    other => spans.push(span(lua, other)?),
                }
            }
            for i in (1..=table.raw_len()).rev() {
                table.raw_remove(i)?;
            }
            node_ud(
                lua,
                fill(lua, Node::new(owner(lua)?, Kind::Rich(spans)), Some(&table))?,
            )
        })?,
    )?;
    for name in ["image", "svg"] {
        ui.set(
            name,
            lua.create_function(move |lua, table: Table| {
                let src = match table.get::<mlua::Value>("src")? {
                    mlua::Value::Nil => match table.get::<mlua::Value>("code")? {
                        mlua::Value::Nil => {
                            return Err(mlua::Error::runtime(format!("ui.{name} needs `src`")));
                        }
                        code => {
                            let code: String = lua.unpack(code)?;
                            Source::Lit(Lit::Str(format!("code:{code}").into()))
                        }
                    },
                    src => source(lua, Ty::Str, "src", src)?,
                };
                let label = label(&table)?;
                let kind = if name == "image" {
                    Kind::Image { label, src }
                } else {
                    Kind::Svg { label, src }
                };
                node_ud(lua, fill(lua, Node::new(owner(lua)?, kind), Some(&table))?)
            })?,
        )?;
    }
    ui.set(
        "canvas",
        lua.create_function(|lua, table: Table| {
            let draw: Function = table
                .get("draw")
                .map_err(|_| mlua::Error::runtime("ui.canvas needs `draw`"))?;
            node_ud(
                lua,
                fill(
                    lua,
                    Node::new(owner(lua)?, Kind::Canvas(draw)),
                    Some(&table),
                )?,
            )
        })?,
    )?;
    crate::canvas::install(lua, &ui)?;

    let widget = |build: fn(String, Signal<Value>, &Table) -> mlua::Result<Kind>| {
        lua.create_function(move |lua, table: Table| {
            let label: Option<String> = table.get("label")?;
            let value = expect_state(&table.get("value")?, "`value`")?;
            let kind = build(label.unwrap_or_default(), value, &table)?;
            node_ud(lua, fill(lua, Node::new(owner(lua)?, kind), Some(&table))?)
        })
    };
    ui.set(
        "checkbox",
        widget(|label, value, _| Ok(Kind::Checkbox { label, value }))?,
    )?;
    ui.set(
        "input",
        widget(|label, value, t| {
            Ok(Kind::Input {
                label,
                value,
                multiline: t.get::<Option<bool>>("multiline")?.unwrap_or(false),
            })
        })?,
    )?;
    ui.set(
        "textarea",
        widget(|label, value, _| {
            Ok(Kind::Input {
                label,
                value,
                multiline: true,
            })
        })?,
    )?;
    ui.set(
        "progress",
        widget(|label, value, _| Ok(Kind::Progress { label, value }))?,
    )?;
    ui.set(
        "slider",
        widget(|label, value, t| {
            let min = t.get::<Option<f32>>("min")?.unwrap_or(0.);
            let max = t.get::<Option<f32>>("max")?.unwrap_or(1.);
            if !(min.is_finite() && max.is_finite() && min < max) {
                return Err(mlua::Error::runtime("slider needs finite min < max"));
            }
            Ok(Kind::Slider {
                label,
                value,
                min,
                max,
            })
        })?,
    )?;

    ui.set(
        "show",
        lua.create_function(
            |lua, (when, then, otherwise): (mlua::Value, mlua::Value, mlua::Value)| {
                let when = source(lua, Ty::Bool, "show condition", when)?;
                let then = expect_node(then, "ui.show's content")?;
                let otherwise = (!otherwise.is_nil())
                    .then(|| expect_node(otherwise, "ui.show's alternative"))
                    .transpose()?;
                node_ud(
                    lua,
                    Node::new(
                        owner(lua)?,
                        Kind::Show {
                            when,
                            then,
                            otherwise,
                        },
                    ),
                )
            },
        )?,
    )?;
    ui.set(
        "each",
        lua.create_function(
            |lua, (items, build, table): (mlua::Value, Function, Option<Table>)| {
                let items = expect_state(&items, "ui.each's list")?;
                let key = match &table {
                    Some(t) => t.get::<Option<Function>>("key")?,
                    None => None,
                };
                node_ud(
                    lua,
                    fill(
                        lua,
                        Node::new(owner(lua)?, Kind::Each { items, build, key }),
                        table.as_ref(),
                    )?,
                )
            },
        )?,
    )?;
    ui.set(
        "list",
        lua.create_function(|lua, table: Table| {
            let count = source(lua, Ty::Num, "count", table.get("count")?)?;
            let build: Function = table
                .get("row")
                .map_err(|_| mlua::Error::runtime("ui.list needs `row`"))?;
            let height = table.get::<Option<f32>>("height")?.unwrap_or(28.);
            if !(height.is_finite() && height > 0.) {
                return Err(mlua::Error::runtime("ui.list `height` must be positive"));
            }
            let offset = match table.get::<mlua::Value>("offset")? {
                mlua::Value::Nil => None,
                o => Some(expect_state(&o, "`offset`")?),
            };
            let overscan = table.get::<Option<usize>>("overscan")?.unwrap_or(2);
            node_ud(
                lua,
                fill(
                    lua,
                    Node::new(
                        owner(lua)?,
                        Kind::List {
                            count,
                            height,
                            overscan,
                            build,
                            offset,
                        },
                    ),
                    Some(&table),
                )?,
            )
        })?,
    )?;
    ui.set(
        "presence",
        lua.create_function(
            |lua, (value, build, table): (mlua::Value, Function, Option<Table>)| {
                let value = source(lua, Ty::Str, "presence value", value)?;
                let (enter, exit, wait, initial) = match &table {
                    Some(t) => (
                        t.get::<Option<Table>>("enter")?,
                        t.get::<Option<Table>>("exit")?,
                        t.get::<Option<String>>("mode")?.as_deref() == Some("wait"),
                        t.get::<Option<bool>>("initial")?.unwrap_or(false),
                    ),
                    None => (None, None, false, false),
                };
                let kind = Kind::Presence {
                    value,
                    build,
                    enter,
                    exit,
                    wait,
                    initial,
                };
                node_ud(
                    lua,
                    fill(lua, Node::new(owner(lua)?, kind), table.as_ref())?,
                )
            },
        )?,
    )?;
    ui.set(
        "switch",
        lua.create_function(
            |lua, (value, cases, default): (mlua::Value, Table, mlua::Value)| {
                let value = source(lua, Ty::Str, "switch value", value)?;
                let mut list = Vec::new();
                for pair in cases.pairs::<Value, mlua::Value>() {
                    let (key, node) = pair?;
                    list.push((key, expect_node(node, "a switch case")?));
                }
                let default = (!default.is_nil())
                    .then(|| expect_node(default, "a switch default"))
                    .transpose()?;
                node_ud(
                    lua,
                    Node::new(
                        owner(lua)?,
                        Kind::Switch {
                            value,
                            cases: list,
                            default,
                        },
                    ),
                )
            },
        )?,
    )?;
    ui.set(
        "modal",
        lua.create_function(|lua, table: Table| {
            let open = expect_state(&table.get("open")?, "`open`")?;
            node_ud(
                lua,
                fill(
                    lua,
                    Node::new(
                        owner(lua)?,
                        Kind::Modal {
                            label: label(&table)?,
                            open,
                        },
                    ),
                    Some(&table),
                )?,
            )
        })?,
    )?;
    for name in ["popover", "menu"] {
        ui.set(
            name,
            lua.create_function(move |lua, table: Table| {
                let open = expect_state(&table.get("open")?, "`open`")?;
                let anchor = expect_node(table.get("anchor")?, "`anchor`")?;
                let label = label(&table)?;
                let kind = if name == "menu" {
                    Kind::Menu {
                        label,
                        open,
                        anchor,
                    }
                } else {
                    Kind::Popover {
                        label,
                        open,
                        anchor,
                    }
                };
                node_ud(lua, fill(lua, Node::new(owner(lua)?, kind), Some(&table))?)
            })?,
        )?;
    }
    ui.set(
        "menu_item",
        lua.create_function(|lua, table: Table| {
            let label = match table.get::<mlua::Value>(1)? {
                mlua::Value::String(s) => s.to_string_lossy(),
                _ => label(&table)?,
            };
            table.raw_remove(1)?;
            node_ud(
                lua,
                fill(
                    lua,
                    Node::new(owner(lua)?, Kind::MenuItem(label)),
                    Some(&table),
                )?,
            )
        })?,
    )?;
    ui.set(
        "submenu",
        lua.create_function(|lua, table: Table| {
            let open = expect_state(&table.get("open")?, "`open`")?;
            node_ud(
                lua,
                fill(
                    lua,
                    Node::new(
                        owner(lua)?,
                        Kind::Submenu {
                            label: label(&table)?,
                            open,
                        },
                    ),
                    Some(&table),
                )?,
            )
        })?,
    )?;

    // Components: `local Card = ui.component(function(props) … end)`, then
    // `Card { title = "…", child, … }`.
    ui.set(
        "component",
        lua.create_function(|lua, build: Function| {
            lua.create_function(move |lua, props: Option<Table>| {
                let props = match props {
                    Some(p) => p,
                    None => lua.create_table()?,
                };
                // Children stay in the props table's array part and are
                // also given as `props.children`.
                let kids = lua.create_table()?;
                for (i, child) in array_items(&props)?.into_iter().enumerate() {
                    kids.raw_set(i + 1, child)?;
                }
                props.raw_set("children", kids)?;
                let mut node = Node::new(
                    owner(lua)?,
                    Kind::Component {
                        build: build.clone(),
                        props: props.clone(),
                    },
                );
                if let Some(id) = props.get::<Option<String>>("id")? {
                    node.id = Some(id.into());
                }
                if let Some(class) = props.get::<Option<String>>("class")? {
                    node.classes = class.split_whitespace().map(Rc::from).collect();
                }
                node_ud(lua, node)
            })
        })?,
    )?;
    // `local` is a Luau keyword: component state is `ui.use_state`.
    ui.set(
        "use_state",
        lua.create_function(|lua, initial: Value| {
            let env = env(lua)?;
            if env.scope.borrow().is_none() {
                return Err(mlua::Error::runtime(
                    "ui.use_state is for component state; use ui.state at the top level",
                ));
            }
            Ok(State(env.runtime.signal(initial)))
        })?,
    )?;
    ui.set(
        "effect",
        lua.create_function(|lua, f: Function| {
            let env = env(lua)?;
            let vm = env.vm.clone();
            let effect = env.runtime.effect(move || {
                if let Some(vm) = vm.upgrade()
                    && let Err(e) = vm.call::<()>(&f, ())
                {
                    vm.report(e);
                }
            });
            retain(&env, Box::new(effect));
            Ok(())
        })?,
    )?;
    ui.set(
        "computed",
        lua.create_function(|lua, f: Function| {
            let env = env(lua)?;
            let signal = env.runtime.signal(Value::Nil);
            let (vm, out) = (env.vm.clone(), signal.clone());
            let effect = env.runtime.effect(move || {
                if let Some(vm) = vm.upgrade() {
                    match vm.call::<Value>(&f, ()) {
                        Ok(v) => {
                            out.set(v);
                        }
                        Err(e) => vm.report(e),
                    }
                }
            });
            retain(&env, Box::new(effect));
            Ok(State(signal))
        })?,
    )?;
    ui.set(
        "on_mount",
        lua.create_function(|lua, f: Function| {
            let env = env(lua)?;
            match &*env.scope.borrow() {
                Some(scope) => scope.mounted.borrow_mut().push(f),
                None => {
                    return Err(mlua::Error::runtime(
                        "ui.on_mount is for components; give a node `on_mount`",
                    ));
                }
            }
            Ok(())
        })?,
    )?;
    ui.set(
        "batch",
        lua.create_function(|lua, f: Function| {
            let (runtime, vm) = {
                let env = env(lua)?;
                (env.runtime.clone(), env.vm.upgrade())
            };
            // The writes inside count; the updates after them do not.
            match vm {
                Some(vm) => vm.off_clock(|| runtime.batch(|| vm.call::<mlua::MultiValue>(&f, ()))),
                None => runtime.batch(|| f.call::<mlua::MultiValue>(())),
            }
        })?,
    )?;
    ui.set(
        "ref",
        lua.create_function(|_, ()| Ok(RefUd(Rc::default())))?,
    )?;
    ui.set(
        "web",
        lua.create_function(|lua, opts: Option<Table>| {
            let options = match &opts {
                Some(t) => web_options(t)?,
                None => Default::default(),
            };
            Ok(WebUd(zgui::web_view::WebView::new(
                &env(lua)?.runtime,
                options,
            )))
        })?,
    )?;
    ui.set(
        "webview",
        lua.create_function(|lua, table: Table| {
            let web = match table.get::<mlua::Value>("web")? {
                mlua::Value::Nil => {
                    zgui::web_view::WebView::new(&env(lua)?.runtime, web_options(&table)?)
                }
                mlua::Value::UserData(ud) => ud.borrow::<WebUd>()?.0.clone(),
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "`web` takes a ui.web(), not {}",
                        other.type_name()
                    )));
                }
            };
            let src = match table.get::<mlua::Value>("src")? {
                mlua::Value::Nil => None,
                value => Some(source(lua, Ty::Str, "src", value)?),
            };
            let html = table
                .get::<Option<String>>("html")?
                .map(|html| -> mlua::Result<_> {
                    Ok((html, table.get::<Option<String>>("base_url")?))
                })
                .transpose()?;
            let handler = |key: &str| table.get::<Option<Function>>(key);
            let spec = WebSpec {
                web,
                src,
                html,
                on_navigate: handler("on_navigate")?,
                on_load: handler("on_load")?,
                on_title: handler("on_title")?,
                on_message: handler("on_message")?,
                on_error: handler("on_error")?,
                on_new_window: handler("on_new_window")?,
            };
            // What the constructor read is not a style.
            for key in [
                "web",
                "src",
                "html",
                "base_url",
                "transparent",
                "profile",
                "inspectable",
                "user_agent",
                "messages",
                "new_windows",
                "on_navigate",
                "on_load",
                "on_title",
                "on_message",
                "on_error",
                "on_new_window",
            ] {
                table.raw_set(key, mlua::Value::Nil)?;
            }
            let node = fill(
                lua,
                Node::new(owner(lua)?, Kind::WebView(Box::new(spec))),
                Some(&table),
            )?;
            node_ud(lua, node)
        })?,
    )?;
    // `ui.invoke(name, arg)`: call a host action now, from any handler.
    ui.set(
        "invoke",
        lua.create_function(|lua, (name, arg): (String, Option<Value>)| {
            let action = env(lua)?.actions.borrow().get(&name).cloned();
            match action {
                Some(action) => {
                    action(arg.unwrap_or(Value::Nil));
                    Ok(())
                }
                None => Err(mlua::Error::runtime(format!(
                    "no host action named `{name}`"
                ))),
            }
        })?,
    )?;
    ui.set(
        "terminal",
        lua.create_function(|lua, table: Table| {
            let term = match table.get::<mlua::Value>("term")? {
                mlua::Value::UserData(ud) if ud.is::<crate::term::TermUd>() => {
                    ud.borrow::<crate::term::TermUd>()?.term.clone()
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "ui.terminal needs `term = ui.term(id)`, not {}",
                        other.type_name()
                    )));
                }
            };
            table.raw_set("term", mlua::Value::Nil)?;
            let node = fill(
                lua,
                Node::new(owner(lua)?, Kind::Terminal(term)),
                Some(&table),
            )?;
            node_ud(lua, node)
        })?,
    )?;
    ui.set(
        "shader",
        lua.create_function(|_, (source, opts): (String, Option<Table>)| {
            let fps = match &opts {
                Some(t) => t.get::<Option<f32>>("fps")?,
                None => None,
            };
            // One shader per source and rate: rebuilding a view does not
            // compile it again.
            type Shaders =
                std::collections::HashMap<(String, Option<u32>), Arc<zgui::decoration::FillShader>>;
            thread_local! {
                static SHADERS: RefCell<Shaders> = RefCell::default();
            }
            // A shader that does not compile is the script's error, shown
            // with its line, not a plain fill.
            zgui_gpu::check_fill_shader(&source)
                .map_err(|e| mlua::Error::runtime(format!("ui.shader: {e}")))?;
            let key = (source, fps.map(f32::to_bits));
            let shader = SHADERS.with(|shaders| {
                shaders
                    .borrow_mut()
                    .entry(key.clone())
                    .or_insert_with(|| zgui::decoration::FillShader::new(key.0.clone(), fps))
                    .clone()
            });
            Ok(ShaderUd(shader))
        })?,
    )?;
    ui.set(
        "bitmap",
        lua.create_function(|_, (w, h, f): (u32, u32, Function)| {
            if w == 0 || h == 0 || w > 4096 || h > 4096 {
                return Err(mlua::Error::runtime(
                    "ui.bitmap takes 1 to 4096 pixels a side",
                ));
            }
            // White, with the alpha the function gives each pixel (0 to 1).
            let mut pixels = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let a: f64 = f.call((x, y))?;
                    let a = (a.clamp(0., 1.) * 255.).round() as u8;
                    pixels.extend_from_slice(&[a, a, a, a]);
                }
            }
            let image = zgui::image::ImageData::new(w, h, pixels)
                .map_err(|e| mlua::Error::runtime(format!("ui.bitmap: {e}")))?;
            Ok(BitmapUd(Arc::new(image)))
        })?,
    )?;

    ui.set(
        "slot",
        lua.create_function(|lua, (name, table): (String, Option<Table>)| {
            node_ud(
                lua,
                fill(
                    lua,
                    Node::new(owner(lua)?, Kind::Slot(name)),
                    table.as_ref(),
                )?,
            )
        })?,
    )?;
    ui.set(
        "contribute",
        lua.create_function(|lua, (slot, node): (String, mlua::Value)| {
            let node = expect_node(node, "a contribution")?;
            env(lua)?.contributions.borrow_mut().push((slot, node));
            Ok(())
        })?,
    )?;
    ui.set(
        "state",
        lua.create_function(|lua, (key, initial): (String, Value)| {
            let env = env(lua)?;
            // A required module's keys are its own: two modules' "open" are
            // two states. The script's own keys stay as they are.
            let chunk = lua
                .inspect_stack(1, |d| d.source().source.map(|s| s.into_owned()))
                .flatten();
            let script = env.vm.upgrade().map(|vm| format!("@{}", vm.name));
            let key = match chunk {
                Some(chunk) if Some(&chunk) != script.as_ref() && chunk.starts_with('@') => {
                    format!("{}::{key}", &chunk[1..])
                }
                _ => key,
            };
            Ok(State(env.store.state(&key, initial)))
        })?,
    )?;
    ui.set(
        "shared",
        lua.create_function(|lua, (key, initial): (String, Value)| {
            Ok(State(env(lua)?.shared.state(&key, initial)))
        })?,
    )?;

    let anim = |lua: &Lua,
                source: Option<Signal<Value>>,
                opts: Option<Table>,
                default: Curve|
     -> mlua::Result<mlua::AnyUserData> {
        let opts = match opts {
            Some(t) => t,
            None => lua.create_table()?,
        };
        let ease = match opts.get::<mlua::Value>("ease")? {
            mlua::Value::Nil => None,
            mlua::Value::String(s) => Some(
                crate::anime::ease::Ease::parse(&s.to_string_lossy())
                    .map_err(mlua::Error::runtime)?,
            ),
            mlua::Value::UserData(ud) => Some(ud.borrow::<crate::anime::api::EaseUd>()?.0.clone()),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "ease must be a name or ease, not {}",
                    other.type_name()
                )));
            }
        };
        let seconds = opts.get::<Option<f32>>("duration")?;
        let curve = match default {
            Curve::Tween {
                seconds: s,
                ease: e,
            } => Curve::Tween {
                seconds: seconds.unwrap_or(s),
                ease: ease.unwrap_or(e),
            },
            Curve::Spring { stiffness, damping } => Curve::Spring {
                stiffness: opts.get::<Option<f32>>("stiffness")?.unwrap_or(stiffness),
                damping: opts.get::<Option<f32>>("damping")?.unwrap_or(damping),
            },
            Curve::Loop {
                seconds: s,
                ease: e,
                yoyo,
                fps,
            } => Curve::Loop {
                seconds: seconds.unwrap_or(s),
                ease: ease.unwrap_or(e),
                yoyo: opts.get::<Option<bool>>("yoyo")?.unwrap_or(yoyo),
                fps: opts.get::<Option<f64>>("fps")?.or(fps),
            },
        };
        if let Curve::Tween { seconds, .. } | Curve::Loop { seconds, .. } = curve
            && !(seconds.is_finite() && seconds > 0.)
        {
            return Err(mlua::Error::runtime("duration must be positive"));
        }
        let from: Option<Value> = opts.get("from")?;
        let to: Option<Value> = opts.get("to")?;
        if from.is_some() != to.is_some() {
            return Err(mlua::Error::runtime(
                "give both `from` and `to`, or neither",
            ));
        }
        lua.create_userdata(AnimUd(Rc::new(Anim {
            source,
            curve,
            from,
            to,
        })))
    };
    use crate::anime::ease::{Base, Ease, Mode};
    ui.set(
        "tween",
        lua.create_function(move |lua, (state, opts): (mlua::Value, Option<Table>)| {
            let curve = Curve::Tween {
                seconds: 0.25,
                ease: Ease::Curve(Mode::Out, Base::Power(3.)),
            };
            anim(
                lua,
                Some(expect_state(&state, "ui.tween's source")?),
                opts,
                curve,
            )
        })?,
    )?;
    ui.set(
        "spring",
        lua.create_function(move |lua, (state, opts): (mlua::Value, Option<Table>)| {
            let curve = Curve::Spring {
                stiffness: 260.,
                damping: 24.,
            };
            anim(
                lua,
                Some(expect_state(&state, "ui.spring's source")?),
                opts,
                curve,
            )
        })?,
    )?;
    ui.set(
        "loop",
        lua.create_function(move |lua, opts: Option<Table>| {
            let curve = Curve::Loop {
                seconds: 1.,
                ease: Ease::Curve(Mode::InOut, Base::Power(2.)),
                yoyo: true,
                fps: None,
            };
            anim(lua, None, opts, curve)
        })?,
    )?;

    let action = |lua: &Lua, action: Action| lua.create_userdata(ActionUd(action));
    ui.set(
        "set",
        lua.create_function(move |lua, (state, value): (mlua::Value, Value)| {
            action(
                lua,
                Action::Set(expect_state(&state, "ui.set's target")?, value),
            )
        })?,
    )?;
    ui.set(
        "toggle",
        lua.create_function(move |lua, state: mlua::Value| {
            action(
                lua,
                Action::Toggle(expect_state(&state, "ui.toggle's target")?),
            )
        })?,
    )?;
    ui.set(
        "add",
        lua.create_function(move |lua, (state, by): (mlua::Value, Option<f64>)| {
            action(
                lua,
                Action::Add(expect_state(&state, "ui.add's target")?, by.unwrap_or(1.)),
            )
        })?,
    )?;
    ui.set(
        "action",
        lua.create_function(move |lua, (name, arg): (String, Value)| {
            action(lua, Action::Host(name.into(), arg))
        })?,
    )?;
    // Read-only once the host's own parts are in (see `ScriptHost::run`).
    lua.globals().set("ui", ui)
}

/// Keeps `resource` for the component being built, or for the script.
fn retain(env: &Env, resource: Box<dyn Any>) {
    match &*env.scope.borrow() {
        Some(scope) => scope.retained.borrow_mut().push(resource),
        None => env.retained.borrow_mut().push(resource),
    }
}
