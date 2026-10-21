//! Lowers a script's node tree to retained zgui views. Literal styles become
//! ordinary fluent styles; state, computed and animated properties share one
//! reactive style per node; text templates interpolate state in Rust.
//! Targetable nodes (`id`, `class`, `ref`, `on_mount`) also carry an element:
//! what animations write, resolved over the node's own styles.
use crate::{
    Actions, Vm, anim,
    anime::{Key as Property, target::Target, value::V},
    element::{Element, ElementUd},
    node::{
        Action, Handler, Kind, LayoutAnim, Lit, Node, NodeUd, Prop, Pseudo, Scope, Segment, Source,
        Span, Text, Ty, lit,
    },
    value::Value,
};
use mlua::FromLua;
use std::{cell::OnceCell, collections::HashMap, rc::Rc, sync::Arc};
use zgui::{
    compose::{Context, NodeHandle, TextSpan, View, prelude::*},
    image::ImageData,
    input::{EventPhase, InputEvent, Key, PointerButton},
    layout::{ContentAlign, Length},
    reactive::Signal,
    scene::{Align, Color},
    svg::SvgData,
};

/// Decoded images and SVGs by source, kept across reloads.
#[derive(Default)]
pub(crate) struct Assets {
    images: std::cell::RefCell<HashMap<String, Arc<ImageData>>>,
    svgs: std::cell::RefCell<HashMap<String, Arc<SvgData>>>,
    /// Each file's path and modification time when it was read, so a
    /// watching host sees an edited image (see `changed`).
    files: std::cell::RefCell<HashMap<String, (std::path::PathBuf, Option<std::time::SystemTime>)>>,
}
impl Assets {
    fn read(&self, src: &str, path: &std::path::Path) -> std::io::Result<Vec<u8>> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let bytes = std::fs::read(path)?;
        self.files
            .borrow_mut()
            .insert(src.to_owned(), (path.to_path_buf(), modified));
        Ok(bytes)
    }
    /// Forgets the files edited since they were read, so the next build
    /// reads them again; whether there were any. Only stats files.
    pub fn changed(&self) -> bool {
        let stale: Vec<String> = self
            .files
            .borrow()
            .iter()
            .filter(|(_, (path, modified))| {
                std::fs::metadata(path).and_then(|m| m.modified()).ok() != *modified
            })
            .map(|(src, _)| src.clone())
            .collect();
        for src in &stale {
            self.files.borrow_mut().remove(src);
            self.images.borrow_mut().remove(src);
            self.svgs.borrow_mut().remove(src);
        }
        !stale.is_empty()
    }
}

pub(crate) struct Lowering {
    pub slots: HashMap<String, Vec<Rc<Node>>>,
    pub actions: Actions,
    pub elements: Rc<crate::element::Elements>,
    pub runtime: zgui::reactive::Runtime,
    pub assets: Rc<Assets>,
    /// The window's layout inspector, once the host view mounted: nodes
    /// with a recorded source are labelled for it while it records.
    pub inspector: Rc<std::cell::RefCell<Option<zgui::inspect::Inspector>>>,
}

pub(crate) fn lower(node: &Rc<Node>, low: &Rc<Lowering>) -> View {
    let Some(vm) = node.owner.upgrade() else {
        return div();
    };
    let view = if node.needs_context() {
        let (node, low, vm) = (node.clone(), low.clone(), vm.clone());
        component(move |cx| build(&node, &vm, &low, Some(cx)))
    } else {
        build(node, &vm, low, None)
    };
    // Labelled for the layout inspector while it records.
    let view = match (&node.source, low.inspector.borrow().as_ref()) {
        (Some(source), Some(inspector)) if inspector.recording() => {
            let (inspector, source, name) = (inspector.clone(), source.clone(), node.label());
            view.on_mount(move |handle| inspector.label(handle.node(), name, Some(source)))
        }
        _ => view,
    };
    match &node.theme {
        None => view,
        Some(spec) => themed(spec, &vm, view),
    }
}

/// `view` under the theme scope `theme = …` asks for.
fn themed(spec: &crate::node::ThemeSpec, vm: &Rc<Vm>, view: View) -> View {
    use crate::node::ThemeSpec;
    let compute = |value: Value, vm: &Vm| {
        crate::theme::override_of(&value).unwrap_or_else(|e| {
            vm.report(mlua::Error::runtime(format!("theme: {e}")));
            zgui::theme::ThemeOverride::new()
        })
    };
    match spec {
        ThemeSpec::Fixed(over) => zgui::compose::theme_scope(over.clone(), view),
        ThemeSpec::State(state) => {
            let (state, vm) = (state.clone(), vm.clone());
            zgui::compose::theme_scope_with(move || compute(state.get(), &vm), view)
        }
        ThemeSpec::Computed(f) => {
            let (f, vm) = (f.clone(), vm.clone());
            zgui::compose::theme_scope_with(
                move || {
                    let value = vm.call::<Value>(&f, ()).unwrap_or_else(|e| {
                        vm.report(e);
                        Value::Nil
                    });
                    compute(value, &vm)
                },
                view,
            )
        }
    }
}
/// The style property a theme token in `prop` sets.
fn token_field(prop: Prop) -> Option<zgui::theme::Field> {
    use Prop::*;
    use zgui::theme::Field;
    Some(match prop {
        Bg => Field::Background,
        TextColor => Field::TextColor,
        BorderColor => Field::BorderColor,
        Rounded => Field::Radius,
        Border => Field::BorderWidth,
        P => Field::Padding(15),
        Px => Field::Padding(10),
        Py => Field::Padding(5),
        Pt => Field::Padding(1),
        Pr => Field::Padding(2),
        Pb => Field::Padding(4),
        Pl => Field::Padding(8),
        M => Field::Margin(15),
        Mx => Field::Margin(10),
        My => Field::Margin(5),
        Mt => Field::Margin(1),
        Mr => Field::Margin(2),
        Mb => Field::Margin(4),
        Ml => Field::Margin(8),
        Gap => Field::Gap,
        TextSize => Field::TextSize,
        LineHeight => Field::LineHeight,
        LetterSpacing => Field::LetterSpacing,
        Weight => Field::FontWeight,
        Font => Field::FontFamily,
        Shadow => Field::Shadow,
        Opacity => Field::Opacity,
        BackdropBlur => Field::Blur,
        W => Field::Width,
        H => Field::Height,
        _ => return None,
    })
}

fn bridge<T: PartialEq + Clone + 'static>(
    cx: &mut Context,
    value: &Signal<Value>,
    from: fn(&Value) -> T,
    to: fn(T) -> Value,
) -> Signal<T> {
    let typed = cx.state(value.with_untracked(from));
    let runtime = cx.runtime();
    let (v, t) = (value.clone(), typed.clone());
    cx.retain(runtime.effect(move || {
        t.set(from(&v.get()));
    }));
    let (v, t) = (value.clone(), typed.clone());
    // The widget writes back only after its first change, so mounting never
    // rewrites the script's value (for example 0.1 through f32).
    let mut first = true;
    cx.retain(runtime.effect(move || {
        let next = t.get();
        if !std::mem::take(&mut first) {
            v.set(to(next));
        }
    }));
    typed
}

fn need<'a>(cx: &'a mut Option<&mut Context>) -> &'a mut Context {
    cx.as_deref_mut()
        .expect("node lowered without a component context")
}

fn number(value: &Value) -> f32 {
    value.as_f64().unwrap_or(0.) as f32
}

/// Calls a script function that builds part of the interface, as a
/// component's body: the effects and computed values it creates live as long
/// as what it built (not as long as the script), and its `ui.on_mount`
/// hooks run once that is mounted.
pub(crate) fn build_scoped(
    vm: &Rc<Vm>,
    cx: &mut Context,
    low: &Rc<Lowering>,
    f: &mlua::Function,
    args: impl mlua::IntoLuaMulti,
) -> View {
    let scope = Rc::new(Scope::default());
    let previous = vm
        .lua
        .app_data_ref::<crate::node::Env>()
        .map(|env| env.scope.replace(Some(scope.clone())));
    let result = vm.call::<mlua::Value>(f, args);
    if let (Some(previous), Some(env)) = (previous, vm.lua.app_data_ref::<crate::node::Env>()) {
        *env.scope.borrow_mut() = previous;
    }
    for resource in scope.retained.borrow_mut().drain(..) {
        cx.retain(resource);
    }
    let mounted: Vec<mlua::Function> = scope.mounted.borrow_mut().drain(..).collect();
    let view = built(vm, result, low);
    if mounted.is_empty() {
        return view;
    }
    let vm = vm.clone();
    view.on_mount(move |_| {
        for f in mounted {
            if let Err(e) = vm.call::<()>(&f, ()) {
                vm.report(e);
            }
        }
    })
}

/// A web view: its page from `src` (followed) or `html`, its handlers
/// subscribed for as long as the view is mounted.
fn web_view_of(spec: &crate::node::WebSpec, vm: &Rc<Vm>, low: &Rc<Lowering>) -> View {
    use zgui::web_view::{WebViewEvent, web_view, web_view_src};
    let web = spec.web.clone();
    let view = match (&spec.src, &spec.html) {
        (Some(src), _) => {
            let (src, vm) = (resolve_owned(src), vm.clone());
            web_view_src(&web, move || match resolve(&src, Ty::Str, None, &vm) {
                Some(Lit::Str(url)) => url.to_string(),
                _ => String::new(),
            })
        }
        (None, Some((html, base))) => {
            web.load_html(html.clone(), base.as_deref());
            web_view(&web)
        }
        (None, None) => web_view(&web),
    };
    let handlers = [
        spec.on_navigate.clone(),
        spec.on_load.clone(),
        spec.on_title.clone(),
        spec.on_message.clone(),
        spec.on_error.clone(),
        spec.on_new_window.clone(),
    ];
    if handlers.iter().all(Option::is_none) {
        return view;
    }
    let (vm, _low) = (vm.clone(), low.clone());
    view.on_mount(move |handle| {
        let [navigate, load, title, message, error, new_window] = handlers;
        let vm = Rc::downgrade(&vm);
        let subscription = web.on_event(move |event| {
            let Some(vm) = vm.upgrade() else { return };
            let call = |f: &Option<mlua::Function>, args: mlua::MultiValue| {
                if let Some(f) = f
                    && let Err(e) = vm.call::<()>(f, args)
                {
                    vm.report(e);
                }
            };
            let lua = &vm.lua;
            let pack = |args: &[&str]| -> mlua::MultiValue {
                args.iter()
                    .filter_map(|a| lua.create_string(a).ok().map(mlua::Value::String))
                    .collect()
            };
            if navigate.is_some() {
                let (kind, url) = match event {
                    WebViewEvent::NavigationStarted { url } => ("started", url.as_str()),
                    WebViewEvent::NavigationCommitted { url } => ("committed", url.as_str()),
                    WebViewEvent::NavigationFinished { url } => ("finished", url.as_str()),
                    WebViewEvent::NavigationFailed { url, .. } => ("failed", url.as_str()),
                    WebViewEvent::UrlChanged { url } => ("changed", url.as_str()),
                    _ => ("", ""),
                };
                if !kind.is_empty()
                    && let Ok(t) = lua.create_table()
                {
                    let _ = t.set("kind", kind);
                    let _ = t.set("url", url);
                    call(
                        &navigate,
                        mlua::MultiValue::from_vec(vec![mlua::Value::Table(t)]),
                    );
                }
            }
            match event {
                WebViewEvent::NavigationFinished { url } => call(&load, pack(&[url])),
                WebViewEvent::TitleChanged { title: t } => call(&title, pack(&[t])),
                WebViewEvent::NavigationFailed { url, error: e } => call(&error, pack(&[url, e])),
                WebViewEvent::NewWindowRequested { url } => call(&new_window, pack(&[url])),
                WebViewEvent::ProcessTerminated => {
                    call(&error, pack(&["", "the page's process ended"]))
                }
                WebViewEvent::Message { body } => {
                    if let Some(f) = &message {
                        let value = crate::node::json_value(body);
                        if let Err(e) = vm.call::<()>(f, value) {
                            vm.report(e);
                        }
                    }
                }
                _ => {}
            }
        });
        handle.retain(subscription);
    })
}

/// Builds a node a script function returned.
fn built(vm: &Rc<Vm>, result: mlua::Result<mlua::Value>, low: &Rc<Lowering>) -> View {
    match result.and_then(|v| match v {
        mlua::Value::UserData(ud) => ud.borrow::<NodeUd>().map(|n| n.0.clone()),
        mlua::Value::Nil => Err(mlua::Error::runtime(
            "the function returned nothing; return a ui node",
        )),
        other => Err(mlua::Error::runtime(format!(
            "expected a ui node, got {}",
            other.type_name()
        ))),
    }) {
        Ok(node) => lower(&node, low),
        Err(error) => {
            vm.report(error);
            div()
        }
    }
}

fn build(node: &Rc<Node>, vm: &Rc<Vm>, low: &Rc<Lowering>, mut cx: Option<&mut Context>) -> View {
    // Layout moves animate through an element too, one selectors do not find.
    let element = (node.targetable() || node.tracks_layout()).then(|| {
        Element::new(
            &low.runtime,
            node.id.clone(),
            node.classes.clone(),
            node.tag(),
        )
    });
    let mut view = match &node.kind {
        Kind::Column => column(),
        Kind::Row => row(),
        Kind::Div => div(),
        Kind::Overlay => overlay(),
        Kind::Button => button(),
        Kind::Scroll {
            offset,
            horizontal,
            follow,
        } => {
            let offset = match offset {
                Some(offset) => bridge(need(&mut cx), offset, number, |x| Value::Num(x as f64)),
                None => need(&mut cx).state(0_f32),
            };
            if let Some(element) = &element {
                let _ = element.scroll.set((offset.clone(), *horizontal));
            }
            let view = if *horizontal {
                scroll_x(offset)
            } else {
                scroll(offset)
            };
            view.follow_end(*follow)
        }
        Kind::Text(text) => lower_text(text, vm),
        Kind::Rich(spans) => lower_rich(spans, vm, low),
        Kind::Image { label, src } => {
            let (src, vm2, low2) = (resolve_owned(src), vm.clone(), low.clone());
            let label = label.clone();
            image_signal(label, move || {
                let path = match resolve(&src, Ty::Str, None, &vm2) {
                    Some(Lit::Str(path)) => path,
                    _ => return placeholder(),
                };
                load_image(&vm2, &low2, &path).unwrap_or_else(placeholder)
            })
        }
        Kind::Svg { label, src } => {
            let (src, vm2, low2) = (resolve_owned(src), vm.clone(), low.clone());
            let theme = cx.as_deref().map(Context::theme);
            // The tint may be state or computed: the icon re-tints when it changes.
            let tint = node
                .props
                .iter()
                .find(|(p, _)| *p == Prop::Tint)
                .map(|(_, source)| resolve_owned(source));
            svg_signal(label.clone(), move || {
                let source = match resolve(&src, Ty::Str, None, &vm2) {
                    Some(Lit::Str(path)) => path,
                    _ => "code:<svg xmlns='http://www.w3.org/2000/svg'/>".into(),
                };
                let data = load_svg(&vm2, &low2, &source);
                match tint
                    .as_ref()
                    .and_then(|t| resolve(t, Ty::Color, None, &vm2))
                {
                    Some(Lit::Color(c)) => Arc::new(data.tinted(c)),
                    Some(Lit::Token(t)) => match theme.as_ref().map(|theme| theme.get(t).color()) {
                        Some(Some(c)) => Arc::new(data.tinted(c)),
                        _ => data,
                    },
                    _ => data,
                }
            })
        }
        Kind::WebView(spec) => web_view_of(spec, vm, low),
        Kind::Terminal(term) => zgui::terminal::terminal(term),
        Kind::Canvas(draw) => {
            let (draw, vm) = (draw.clone(), vm.clone());
            canvas(move |(w, h)| {
                let recorder = crate::canvas::Recorder(Default::default());
                let result = vm.lua.create_userdata(recorder).and_then(|ud| {
                    vm.call::<()>(&draw, (ud.clone(), w, h))?;
                    ud.take::<crate::canvas::Recorder>()
                });
                match result {
                    Ok(recorder) => recorder.0.into_inner(),
                    Err(error) => {
                        vm.report(error);
                        Default::default()
                    }
                }
            })
        }
        Kind::Checkbox { label, value } => checkbox(
            label.clone(),
            bridge(need(&mut cx), value, Value::truthy, Value::Bool),
        ),
        Kind::Input {
            label,
            value,
            multiline,
            placeholder,
            placeholder_color,
            select_on_focus,
        } => {
            let value = bridge(
                need(&mut cx),
                value,
                |v| v.to_string(),
                |s| Value::Str(s.into()),
            );
            let mut view = if *multiline {
                text_area(label.clone(), value)
            } else {
                text_input(label.clone(), value)
            };
            if let Some(text) = placeholder {
                let (text, vm) = (resolve_owned(text), vm.clone());
                view = view.placeholder_when(move || match resolve(&text, Ty::Str, None, &vm) {
                    Some(Lit::Str(text)) => text.to_string(),
                    _ => String::new(),
                });
            }
            match placeholder_color {
                Some(Source::Lit(Lit::Color(color))) => view = view.placeholder_color(*color),
                Some(color) => {
                    // A token (`"$text.tertiary"`) follows the element's theme;
                    // anything unresolved shows the default placeholder colour.
                    let (color, vm) = (resolve_owned(color), vm.clone());
                    let theme = match cx.as_deref().map(Context::theme) {
                        Some(theme) => theme,
                        None => zgui::theme::Themes::of(&low.runtime).root().clone(),
                    };
                    view = view.placeholder_color_when(move || {
                        let muted = || theme.color(zgui::theme::tok::TEXT_TERTIARY);
                        match resolve(&color, Ty::Color, None, &vm) {
                            Some(Lit::Color(color)) => color,
                            Some(Lit::Token(t)) => theme.get(t).color().unwrap_or_else(muted),
                            _ => muted(),
                        }
                    });
                }
                None => {}
            }
            if *select_on_focus {
                view = view.select_on_focus(true);
            }
            view
        }
        Kind::Progress { label, value } => progress(
            label.clone(),
            bridge(need(&mut cx), value, number, |x| Value::Num(x as f64)),
        ),
        Kind::Slider {
            label,
            value,
            min,
            max,
        } => slider(
            label.clone(),
            bridge(need(&mut cx), value, number, |x| Value::Num(x as f64)),
            *min..=*max,
        ),
        Kind::Show {
            when,
            then,
            otherwise,
        } => {
            let (when_vm, vm) = (vm.clone(), vm.clone());
            let (then, otherwise, low) = (then.clone(), otherwise.clone(), low.clone());
            let when = Rc::new(resolve_owned(when));
            switch(
                move || {
                    resolve(&when, Ty::Bool, None, &when_vm).is_some_and(|l| l == Lit::Bool(true))
                },
                move |on, _| {
                    let _keep = &vm;
                    match (on, &otherwise) {
                        (true, _) => lower(&then, &low),
                        (false, Some(otherwise)) => lower(otherwise, &low),
                        (false, None) => div(),
                    }
                },
            )
            // Lays out as what it shows, as if in its place.
            .contents()
        }
        Kind::Switch {
            value,
            cases,
            default,
        } => {
            let (value_vm, vm) = (vm.clone(), vm.clone());
            let cases: Vec<(Value, Rc<Node>)> = cases.clone();
            let (default, low) = (default.clone(), low.clone());
            let value = Rc::new(resolve_owned(value));
            switch(
                move || resolve_value(&value, &value_vm),
                move |key, _| {
                    let _keep = &vm;
                    let found = cases
                        .iter()
                        .find(|(case, _)| *case == key || case.to_string() == key.to_string());
                    match (found, &default) {
                        (Some((_, node)), _) => lower(node, &low),
                        (None, Some(node)) => lower(node, &low),
                        (None, None) => div(),
                    }
                },
            )
            .contents()
        }
        Kind::Each {
            items,
            build: builder,
            key,
            motion: Some(motion),
        } => crate::motion::each(
            need(&mut cx),
            items.clone(),
            builder.clone(),
            key.clone(),
            motion.clone(),
            vm,
            low,
        ),
        Kind::Each {
            items,
            build: builder,
            key,
            motion: None,
        } => {
            // Rows are keyed by the script's `key` (they keep their state as
            // the list reorders), else by position and value. The key pass
            // notes each row's index for the build that follows it.
            let indices: Rc<std::cell::RefCell<HashMap<String, usize>>> = Rc::default();
            let (keys_from, items) = (items.clone(), items.clone());
            let (key_fn, key_vm, noted) = (key.clone(), vm.clone(), indices.clone());
            let (builder, vm, low) = (builder.clone(), vm.clone(), low.clone());
            keyed(
                move || {
                    keys_from.with(|list| {
                        let mut noted = noted.borrow_mut();
                        noted.clear();
                        let mut seen: HashMap<String, usize> = HashMap::new();
                        list.list()
                            .iter()
                            .enumerate()
                            .map(|(i, item)| {
                                let base = match &key_fn {
                                    Some(f) => key_vm.call::<Value>(f, item.clone()).map_or_else(
                                        |e| {
                                            key_vm.report(e);
                                            i.to_string()
                                        },
                                        |k| k.to_string(),
                                    ),
                                    None => format!("{i}:{item:?}"),
                                };
                                let n = seen.entry(base.clone()).or_default();
                                *n += 1;
                                let identity = format!("{base}#{n}");
                                noted.insert(identity.clone(), i);
                                identity
                            })
                            .collect::<Vec<_>>()
                    })
                },
                move |identity, cx| {
                    let index = indices.borrow().get(&identity).copied().unwrap_or(0);
                    let item = items
                        .with_untracked(|list| list.list().get(index).cloned().unwrap_or_default());
                    build_scoped(&vm, cx, &low, &builder, (item, index + 1))
                },
            )
        }
        Kind::List {
            count,
            height,
            overscan,
            build: builder,
            offset,
        } => {
            let offset = match offset {
                Some(offset) => bridge(need(&mut cx), offset, number, |x| Value::Num(x as f64)),
                None => need(&mut cx).state(0_f32),
            };
            let (count, count_vm) = (Rc::new(resolve_owned(count)), vm.clone());
            let (builder, vm, low) = (builder.clone(), vm.clone(), low.clone());
            virtual_list(
                offset,
                *height,
                *overscan,
                move || match resolve(&count, Ty::Num, None, &count_vm) {
                    Some(Lit::Num(n)) => n.max(0.) as usize,
                    _ => 0,
                },
                |index| index,
                // Each row owns what it makes, as `each` rows do.
                move |_, index, cx| build_scoped(&vm, cx, &low, &builder, index + 1),
            )
        }
        Kind::Modal { label, open } => modal(
            label.clone(),
            bridge(need(&mut cx), open, Value::truthy, Value::Bool),
        ),
        Kind::Popover {
            label,
            open,
            anchor,
        } => popover(
            label.clone(),
            bridge(need(&mut cx), open, Value::truthy, Value::Bool),
            lower(anchor, low),
        ),
        Kind::Menu {
            label,
            open,
            anchor,
        } => menu(
            label.clone(),
            bridge(need(&mut cx), open, Value::truthy, Value::Bool),
            lower(anchor, low),
        ),
        Kind::MenuItem(label) => menu_item(label.clone()),
        Kind::Submenu { label, open } => submenu(
            label.clone(),
            bridge(need(&mut cx), open, Value::truthy, Value::Bool),
        ),
        Kind::Component {
            build: builder,
            props,
        } => {
            let (builder, props, vm, low) =
                (builder.clone(), props.clone(), vm.clone(), low.clone());
            component(move |cx| build_scoped(&vm, cx, &low, &builder, props.clone()))
        }
        Kind::Presence {
            value,
            build: builder,
            enter,
            exit,
            wait,
            initial,
        } => presence(
            need(&mut cx),
            Rc::new(resolve_owned(value)),
            builder.clone(),
            enter.clone(),
            exit.clone(),
            *wait,
            *initial,
            node.props.iter().any(|(p, _)| BOX.contains(p)),
            vm,
            low,
        ),
        Kind::Slot(name) => column().children(
            low.slots
                .get(name)
                .into_iter()
                .flatten()
                .map(|n| lower(n, low)),
        ),
    };
    if let Some(mask) = &node.mask {
        view = view
            .mask(mask.image.clone(), mask.size)
            .mask_offset(mask.offset.0, mask.offset.1);
    }
    if !node.children.is_empty() {
        let flow = flow(node);
        view = view.children(node.children.iter().map(|child| {
            let view = lower(child, low);
            if child.kind.flows() {
                apply_all(view, &flow)
            } else {
                view
            }
        }));
    }

    // Literal styles apply once. Anything that can change, and an element's
    // animated values, share one reactive style.
    let mut statics = Vec::new();
    let mut dynamic = Vec::new();
    for (prop, source) in &node.props {
        match source {
            Source::Lit(lit) => statics.push((*prop, lit.clone())),
            Source::Anim(spec) => {
                let scalar = anim::drive(need(&mut cx), spec);
                dynamic.push((*prop, resolve_owned(source), Some(scalar)));
            }
            _ => dynamic.push((*prop, resolve_owned(source), None)),
        }
    }
    // A dynamic part of a translation or transform carries its literal
    // parts along: each is set as a whole.
    let groups: Vec<&[Prop]> = TRANSFORM_GROUPS
        .iter()
        .copied()
        .filter(|g| dynamic.iter().any(|(p, _, _)| g.contains(p)))
        .collect();
    for group in groups {
        {
            let moved: Vec<_> = statics
                .iter()
                .filter(|(p, _)| group.contains(p))
                .cloned()
                .collect();
            statics.retain(|(p, _)| !group.contains(p));
            dynamic.extend(moved.into_iter().map(|(p, l)| (p, Owned::Lit(l), None)));
        }
    }
    if let Some(element) = &element {
        // What the node's own literals place it at, for animations that
        // start before the scene has them (an `on_mount` animation of
        // `scale` keeps a literal `rotate`).
        let (translate, local) = literal_transform(&statics);
        let literal = |prop| {
            statics.iter().find_map(|(p, l)| match (p == &prop, l) {
                (true, Lit::Num(v)) => Some(*v),
                _ => None,
            })
        };
        let effects = (
            literal(Prop::Opacity).unwrap_or(1.),
            literal(Prop::Blur).unwrap_or(0.),
        );
        element.set_base(translate, local, effects);
        element.set_base_color([
            literal(Prop::Brightness).unwrap_or(1.),
            literal(Prop::Contrast).unwrap_or(1.),
            literal(Prop::Saturate).unwrap_or(1.),
        ]);
        let mut corners = None;
        for (prop, lit) in &statics {
            match (prop, lit) {
                (Prop::Rounded, Lit::Num(r)) => corners = Some(zgui::decoration::Corners::all(*r)),
                (Prop::Corners, Lit::Corners(c)) => corners = Some(*c),
                _ => {}
            }
        }
        element.set_base_corners(corners);
        element.set_base_shader(
            node.shader_params
                .map(|params| zgui::decoration::ShaderUniforms { time: 0., params }),
        );
        element.set_base_clip(statics.iter().find_map(|(p, l)| match (p, l) {
            (Prop::ClipInset, Lit::Corners(c)) => {
                Some([c.top_left, c.top_right, c.bottom_right, c.bottom_left])
            }
            _ => None,
        }));
    }
    view = apply_all(view, &statics);
    // A background image: over `bg`, under the border and children, cut by
    // the corners. It goes in the node's one reactive style, after anything
    // animated, so an animated `bg` keeps it.
    let bg_image = node
        .props
        .iter()
        .find(|(p, _)| *p == Prop::BgImage)
        .map(|(_, source)| {
            let fit = node.props.iter().find_map(|(p, s)| match (p, s) {
                (Prop::BgFit, Source::Lit(Lit::Str(v))) => Some(match &**v {
                    "contain" => ObjectFit::Contain,
                    "fill" => ObjectFit::Fill,
                    "none" => ObjectFit::None,
                    "scale_down" => ObjectFit::ScaleDown,
                    _ => ObjectFit::Cover,
                }),
                _ => None,
            });
            (
                resolve_owned(source),
                fit.unwrap_or(ObjectFit::Cover),
                low.clone(),
            )
        });
    let shader = node
        .shader
        .clone()
        .map(|shader| (shader, node.shader_params.unwrap_or_default()));
    if !dynamic.is_empty() || element.is_some() || bg_image.is_some() || shader.is_some() {
        let vm = vm.clone();
        let element = element.clone();
        view = view.reactive_style(move || {
            let lits: Vec<(Prop, Lit)> = dynamic
                .iter()
                .filter_map(|(prop, source, scalar)| {
                    Some((*prop, resolve(source, prop.ty(), scalar.as_ref(), &vm)?))
                })
                .collect();
            let styles = match &element {
                Some(element) => with_animated(element, lits),
                None => apply_all(Styles::new(), &lits),
            };
            let image = bg_image.as_ref().and_then(|(src, fit, low)| {
                match resolve(src, Ty::Str, None, &vm) {
                    Some(Lit::Str(path)) => load_image(&vm, low, &path).map(|image| (image, *fit)),
                    _ => None,
                }
            });
            let styles = match image {
                Some((image, fit)) => styles.bg_image(image, fit),
                None => styles,
            };
            // A shader fill, with its literal parameters unless animated.
            match &shader {
                Some((fill, params)) => {
                    let animated = element.as_ref().and_then(|e| e.animated.borrow().shader);
                    let uniforms = animated.unwrap_or(zgui::decoration::ShaderUniforms {
                        time: 0.,
                        params: *params,
                    });
                    styles.bg_shader(fill.clone()).shader_uniforms(uniforms)
                }
                None => styles,
            }
        });
    }
    for (pseudo, lits) in &node.pseudo {
        let lits = lits.clone();
        let style = move |s: Styles| apply_all(s, &lits);
        view = match pseudo {
            Pseudo::Hover => view.hover(style),
            Pseudo::Active => view.active(style),
            Pseudo::Focus => view.focus(style),
            Pseudo::Disabled => view.disabled_style(style),
        };
    }
    if let Some(disabled) = &node.disabled {
        let (disabled, vm) = (resolve_owned(disabled), vm.clone());
        view = view.disabled_when(move || {
            resolve(&disabled, Ty::Bool, None, &vm) == Some(Lit::Bool(true))
        });
    }
    if let Some(focusable) = node.focusable {
        view = view.focusable(focusable);
    }
    if node.window_drag {
        view = view.window_drag();
    }
    if let Some(enabled) = node.pointer_events {
        view = view.pointer_events(enabled);
    }
    if let Some(handler) = &node.events.click {
        let (handler, vm, actions) = (handler.clone(), vm.clone(), low.actions.clone());
        view = view.on_click(move || {
            run(&handler, &vm, &actions, ());
        });
    }
    // The node's handle, for local pointer coordinates and elements.
    let handle: Rc<OnceCell<NodeHandle>> = Rc::default();
    if node.events.any_input() {
        view = with_events(view, node, vm, low, handle.clone());
    }
    if node.events.any_drag() || node.events.drag_preview.is_some() {
        view = with_drag(view, node, vm, low, handle.clone());
    }
    if let Some(style) = &node.events.context_menu_style {
        view = view.context_menu_style(apply_all(Styles::new(), style));
    }
    if node.events.context_menu_below {
        view = view.context_menu_below(true);
    }
    if let Some(build) = node.events.context_menu.clone() {
        let (vm, low) = (vm.clone(), low.clone());
        view = view.context_menu(move |e| {
            let point = vm.lua.create_table().and_then(|t| {
                t.set("x", e.x)?;
                t.set("y", e.y)?;
                t.set("local_x", e.local_x)?;
                t.set("local_y", e.local_y)?;
                Ok(t)
            });
            let items = point.and_then(|p| {
                vm.runtime
                    .clone()
                    .batch(|| vm.call::<Option<mlua::Table>>(&build, p))
            });
            match items {
                Ok(Some(items)) => items
                    .sequence_values::<mlua::AnyUserData>()
                    .filter_map(|item| {
                        match item.and_then(|ud| ud.borrow::<NodeUd>().map(|n| n.0.clone())) {
                            Ok(node) => Some(lower(&node, &low)),
                            Err(error) => {
                                vm.report(error);
                                None
                            }
                        }
                    })
                    .collect(),
                Ok(None) => Vec::new(),
                Err(error) => {
                    vm.report(error);
                    Vec::new()
                }
            }
        });
    }
    // Listeners scripts add later (a draggable) receive the element's events.
    if let Some(element) = &element {
        let element = element.clone();
        view = view.on_event(move |event| {
            if element.listeners.borrow().is_empty() {
                return;
            }
            let listeners = element.listeners.borrow().clone();
            for listener in listeners {
                listener(event);
            }
        });
    }
    // Input and drag handlers read the node's handle (local coordinates).
    if element.is_some() || node.events.any_input() || node.events.any_drag() {
        let (element, elements, refs) = (element.clone(), low.elements.clone(), node.refs.clone());
        let targetable = node.targetable();
        // Moves are measured against the parent; with a size box, against
        // the box's parent.
        let track = node.tracks_layout().then(|| {
            let timing = crate::motion::Timing {
                ease: node.layout.as_ref().and_then(|l| l.ease.clone()),
                duration: node.layout.as_ref().and_then(|l| l.duration),
            };
            let depth = if node.sizes_layout() { 2 } else { 0 };
            (node.layout_id.clone(), timing, depth)
        });
        let (mount, resize, vm) = (
            node.events.mount.clone(),
            node.events.resize.clone(),
            vm.clone(),
        );
        view = view.on_mount(move |mounted| {
            let _ = handle.set(mounted.clone());
            let Some(element) = element else { return };
            if targetable {
                elements.register(&element, mounted.clone());
            } else {
                element.attach(mounted.clone());
            }
            if let Some((layout_id, timing, depth)) = track {
                crate::motion::track(&element, &mounted, layout_id, timing, depth, &vm);
            }
            for r in &refs {
                r.attach(&element, &mounted);
            }
            if let Some(f) = mount {
                let arg = vm.lua.create_userdata(ElementUd(element.clone()));
                if let Err(e) = arg.and_then(|arg| vm.call::<()>(&f, arg)) {
                    vm.report(e);
                }
            }
            if let (Some(f), Some(bounds)) = (resize.clone(), mounted.observe_bounds()) {
                // Size changes only: moving does not report.
                let (vm, element, runtime) = (vm.clone(), element.clone(), vm.runtime.clone());
                let last = std::cell::Cell::new(None::<(f32, f32)>);
                let effect = runtime.effect(move || {
                    let b = bounds.get();
                    let size = (b.width, b.height);
                    // Nothing to report before the first layout.
                    if last.get().is_none() && size == (0., 0.) {
                        return;
                    }
                    if last.replace(Some(size)) == Some(size) {
                        return;
                    }
                    let call = || -> mlua::Result<()> {
                        let size = vm.lua.create_table()?;
                        size.set("w", b.width)?;
                        size.set("h", b.height)?;
                        let el = vm.lua.create_userdata(ElementUd(element.clone()))?;
                        vm.call::<()>(&f, (size, el))
                    };
                    if let Err(e) = vm.runtime.untracked(call) {
                        vm.report(e);
                    }
                });
                mounted.retain(effect);
            }
        });
    }
    if let Some(id) = &node.id {
        view = view.id(id.to_string());
    }
    if let Some(layout) = node.layout.as_ref().filter(|_| node.sizes_layout()) {
        view = layout_box(view, layout, vm, low);
    }
    view
}

/// How a container lays out its children, for the regions among them that
/// hold any number (`each`, `slot`): a region is a box of its own, and lays
/// its children out as its parent would, stretched across the parent's
/// cross axis in a column. (`show`, `switch` and `presence` hold one view
/// at a time and are contents nodes: CSS `display: contents`, exactly.)
fn flow(node: &Node) -> Vec<(Prop, Lit)> {
    let mut row = matches!(node.kind, Kind::Row);
    let mut out = Vec::new();
    for (prop, source) in &node.props {
        let Source::Lit(lit) = source else { continue };
        match prop {
            Prop::Direction => {
                row = matches!(lit, Lit::Str(v) if v.starts_with("row"));
                out.push((*prop, lit.clone()));
            }
            Prop::Items | Prop::Justify | Prop::Gap | Prop::GapX | Prop::GapY | Prop::FlexWrap => {
                out.push((*prop, lit.clone()));
            }
            _ => {}
        }
    }
    if row {
        // In a row the region takes the row's room, so its children wrap
        // and shrink against the row's width, not their own.
        out.insert(0, (Prop::Direction, Lit::Str("row".into())));
        out.push((Prop::Grow, Lit::Num(1.)));
        out.push((Prop::Shrink, Lit::Num(1.)));
        out.push((Prop::MinW, Lit::Len(zgui::layout::Length::Px(0.))));
    } else {
        out.push((Prop::AlignSelf, Lit::Str("stretch".into())));
    }
    out
}

/// Resolves an element's animated values over `lits`: what an animation
/// writes wins, as the inline styles anime.js writes do.
pub(crate) fn with_animated(element: &Element, mut lits: Vec<(Prop, Lit)>) -> Styles {
    element.revision.get();
    let animated = element.animated.borrow();
    if animated.styles.iter().any(|(p, _)| is_corner(*p)) {
        // A single animated corner keeps the node's other corners.
        lits.extend(
            element
                .base_corners()
                .map(|c| (Prop::Corners, Lit::Corners(c))),
        );
    }
    for (prop, lit) in &animated.styles {
        lits.retain(|(p, _)| p != prop);
        lits.push((*prop, lit.clone()));
    }
    let mut styles = apply_all(Styles::new(), &lits);
    if let Some((x, y)) = element.shown_translate(literal_transform(&lits).0) {
        styles = styles.translate(x, y);
    }
    if let Some(opacity) = animated.opacity {
        styles = styles.opacity(opacity);
    }
    if let Some(blur) = animated.blur {
        styles = styles.filter_blur(blur);
    }
    if let Some([brightness, contrast, saturate]) = animated.color {
        styles = styles
            .brightness(brightness)
            .contrast(contrast)
            .saturate(saturate);
    }
    if let Some((x, y)) = animated.mask {
        styles = styles.mask_offset(x, y);
    }
    if let Some([t, r, b, l]) = animated.clip {
        styles = styles.clip_inset(t, r, b, l);
    }
    if let Some(uniforms) = animated.shader {
        styles = styles.shader_uniforms(uniforms);
    }
    if let Some(local) = animated.local {
        styles = crate::transform::style(styles, local);
    }
    styles
}

/// A view carrying `element`'s animated values, registered once mounted.
fn element_view(
    view: View,
    element: Rc<Element>,
    low: &Rc<Lowering>,
    mounted: impl FnOnce(NodeHandle) + 'static,
) -> View {
    let (style_element, elements) = (element.clone(), low.elements.clone());
    view.reactive_style(move || with_animated(&style_element, Vec::new()))
        .on_mount(move |handle| {
            elements.register(&element, handle.clone());
            mounted(handle);
        })
}

/// The springy default for size changes, as for moves.
fn layout_ease(layout: &LayoutAnim) -> (crate::anime::ease::Ease, f64) {
    crate::motion::Timing {
        ease: layout.ease.clone(),
        duration: layout.duration,
    }
    .resolve()
}

/// Wraps `inner` in a box whose size follows the inner content's, animated.
/// The box is a contents node: `inner` lays out exactly as it would without
/// it, at its new size at once (its percentages, `grow` and the scroll views
/// inside it included), and what animates is the space it takes, as a margin
/// of the box's own (the lab's technique): a negative bottom (or right)
/// margin holds the space at its last size and eases to zero. The content's
/// constraints never change, so a frame of the animation lays out only the
/// box and its ancestors. Nothing is clipped unless `layout_clip` asks: then
/// a clip inset follows the margin, cutting the content to the space.
fn layout_box(inner: View, layout: &LayoutAnim, vm: &Rc<Vm>, low: &Rc<Lowering>) -> View {
    let slack = Element::new(&low.runtime, None, Rc::new([]), "layout");
    let (enter, grow_h, grow_w, clip) = (layout.enter, layout.height, layout.width, layout.clip);
    let (layout, target, vm2) = (layout.clone(), slack.clone(), Rc::downgrade(vm));
    let engine = vm
        .lua
        .app_data_ref::<crate::node::Env>()
        .map(|env| env.engine.clone());
    let inner = inner.on_mount(move |handle| {
        let (Some(bounds), Some(engine)) = (handle.observe_bounds(), engine) else {
            return;
        };
        let last: std::cell::Cell<Option<(f32, f32)>> = Default::default();
        let runtime = engine.runtime.clone();
        let effect = runtime.effect(move || {
            let b = bounds.get();
            let size = (b.width, b.height);
            if size.0 <= 0. && size.1 <= 0. && last.get().is_none() {
                return;
            }
            let previous = last.replace(Some(size));
            // The margin now: what the box shows beyond (or short of) the
            // content's last size.
            let current = |prop| {
                target
                    .animated
                    .borrow()
                    .styles
                    .iter()
                    .find(|(p, _)| *p == prop)
                    .and_then(|(_, lit)| match lit {
                        Lit::Len(zgui::layout::Length::Px(v)) => Some(*v),
                        Lit::Num(v) => Some(*v),
                        _ => None,
                    })
            };
            let element = Target::Element(target.clone());
            let mut from = Vec::new();
            let mut to = Vec::new();
            for (on, prop, side, now, before) in [
                (
                    grow_h,
                    Prop::Mb,
                    Property::ClipBottom,
                    size.1,
                    previous.map(|p| p.1),
                ),
                (
                    grow_w,
                    Prop::Mr,
                    Property::ClipRight,
                    size.0,
                    previous.map(|p| p.0),
                ),
            ] {
                if !on {
                    continue;
                }
                let shown = match before {
                    // Entering grows from nothing; a first size otherwise holds.
                    None if enter => 0.,
                    None => continue,
                    Some(before) if before == now => continue,
                    Some(before) => before + current(prop).unwrap_or(0.),
                };
                let margin = (shown - now) as f64;
                from.push((element.clone(), Property::Style(prop), V::Num(margin), None));
                to.push((element.clone(), Property::Style(prop), V::Num(0.)));
                if clip {
                    from.push((element.clone(), side.clone(), V::Num(-margin), None));
                    to.push((element.clone(), side, V::Num(0.)));
                }
            }
            if to.is_empty() {
                return;
            }
            engine.set_now(from);
            let (ease, duration) = layout_ease(&layout);
            engine.tween(vm2.clone(), to, duration, ease, Default::default());
        });
        handle.retain(effect);
    });
    let mut space = column().contents().child(inner);
    // Closed until the content's first size arrives: no frame at full size.
    let closed = |on: bool| if enter && on { 100_000. } else { 0. };
    if enter && grow_h {
        space = space.mb(-100_000.);
    }
    if enter && grow_w {
        space = space.mr(-100_000.);
    }
    if clip {
        space = space.clip_inset(0., closed(grow_w), closed(grow_h), 0.);
    }
    element_view(space, slack, low, |_| {})
}

/// Properties set together: a dynamic one carries the literal rest along.
const TRANSFORM_GROUPS: &[&[Prop]] = &[
    &[Prop::X, Prop::Y],
    &[
        Prop::Scale,
        Prop::ScaleX,
        Prop::ScaleY,
        Prop::Rotate,
        Prop::Skew,
        Prop::SkewX,
        Prop::SkewY,
        Prop::Origin,
    ],
];

fn placeholder() -> Arc<ImageData> {
    thread_local! {
        static EMPTY: Arc<ImageData> = Arc::new(ImageData::new(1, 1, vec![0, 0, 0, 0]).expect("1×1 image"));
    }
    EMPTY.with(Clone::clone)
}
fn resolve_path(vm: &Vm, path: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(path);
    match (&vm.base, path.is_relative()) {
        (Some(base), true) => base.join(path),
        _ => path.to_path_buf(),
    }
}
fn load_image(vm: &Vm, low: &Lowering, src: &str) -> Option<Arc<ImageData>> {
    if let Some(image) = low.assets.images.borrow().get(src) {
        return Some(image.clone());
    }
    let path = resolve_path(vm, src);
    let decoded = low
        .assets
        .read(src, &path)
        .map_err(|e| format!("{}: {e}", path.display()))
        .and_then(|bytes| {
            zgui_gpu::assets::decode_image(&bytes).map_err(|e| format!("{}: {e}", path.display()))
        });
    match decoded {
        Ok(image) => {
            low.assets
                .images
                .borrow_mut()
                .insert(src.to_owned(), image.clone());
            Some(image)
        }
        Err(e) => {
            vm.report(mlua::Error::runtime(e));
            None
        }
    }
}
fn load_svg(vm: &Vm, low: &Lowering, src: &str) -> Arc<SvgData> {
    if let Some(svg) = low.assets.svgs.borrow().get(src) {
        return svg.clone();
    }
    let bytes = match src.strip_prefix("code:") {
        Some(code) => Ok(code.as_bytes().to_vec()),
        None => {
            let path = resolve_path(vm, src);
            low.assets
                .read(src, &path)
                .map_err(|e| format!("{}: {e}", path.display()))
        }
    };
    let parsed = bytes.and_then(|b| SvgData::new(b).map_err(|e| format!("{src}: {e}")));
    match parsed {
        Ok(svg) => {
            let svg = Arc::new(svg);
            low.assets
                .svgs
                .borrow_mut()
                .insert(src.to_owned(), svg.clone());
            svg
        }
        Err(e) => {
            vm.report(mlua::Error::runtime(e));
            Arc::new(
                SvgData::new(&b"<svg xmlns='http://www.w3.org/2000/svg'/>"[..]).expect("empty svg"),
            )
        }
    }
}

fn key_name(key: &Key) -> String {
    match key {
        Key::Character(c) => c.clone(),
        Key::Space => " ".into(),
        Key::Function(n) => format!("F{n}"),
        other => format!("{other:?}"),
    }
}

/// Input handlers: one event listener dispatches to the script's handlers.
fn with_events(
    view: View,
    node: &Node,
    vm: &Rc<Vm>,
    low: &Rc<Lowering>,
    handle: Rc<OnceCell<NodeHandle>>,
) -> View {
    let events = &node.events;
    let (press, release, hover, moved) = (
        events.press.clone(),
        events.release.clone(),
        events.hover.clone(),
        events.pointer_move.clone(),
    );
    let (key, key_up, focus, blur, wheel) = (
        events.key.clone(),
        events.key_up.clone(),
        events.focus.clone(),
        events.blur.clone(),
        events.wheel.clone(),
    );
    // A press that follows the pointer (a divider, a slider of the
    // script's own) keeps it until release, wherever it goes.
    let captures = press.is_some() && (moved.is_some() || release.is_some());
    let (vm, actions) = (vm.clone(), low.actions.clone());
    view.on_event(move |event| {
        let lua = &vm.lua;
        let local = |x: f32, y: f32| {
            let origin = handle
                .get()
                .and_then(|h| h.scene().map(|s| s.borrow().bounds(h.node())))
                .map_or((0., 0.), |b| (b.x, b.y));
            (x - origin.0, y - origin.1)
        };
        let point = |x: f32, y: f32| -> mlua::Result<mlua::Table> {
            let (lx, ly) = local(x, y);
            let t = lua.create_table()?;
            t.set("x", x)?;
            t.set("y", y)?;
            t.set("local_x", lx)?;
            t.set("local_y", ly)?;
            Ok(t)
        };
        if captures
            && event.phase == EventPhase::Target
            && matches!(
                event.event,
                InputEvent::PointerDown {
                    button: PointerButton::Primary,
                    ..
                }
            )
        {
            event.capture_pointer();
        }
        // A child's events reach this node going down and coming back up:
        // the handlers hear them once, as they bubble.
        if event.phase == EventPhase::Capture {
            return;
        }
        match &event.event {
            InputEvent::PointerDown {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                if let (Some(h), Ok(p)) = (&press, point(*x, *y)) {
                    run(h, &vm, &actions, p);
                }
            }
            InputEvent::PointerUp {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                if let (Some(h), Ok(p)) = (&release, point(*x, *y)) {
                    run(h, &vm, &actions, p);
                }
            }
            InputEvent::PointerMove { x, y } => {
                if let (Some(h), Ok(p)) = (&moved, point(*x, *y)) {
                    run(h, &vm, &actions, p);
                }
            }
            InputEvent::PointerEnter => {
                if let Some(h) = &hover {
                    run(h, &vm, &actions, true);
                }
            }
            InputEvent::PointerLeave => {
                if let Some(h) = &hover {
                    run(h, &vm, &actions, false);
                }
            }
            InputEvent::KeyDown {
                key: k,
                modifiers,
                repeat,
            } => {
                if let Some(h) = &key {
                    let table = lua.create_table().and_then(|t| {
                        t.set("key", key_name(k))?;
                        t.set("shift", modifiers.shift)?;
                        t.set("ctrl", modifiers.control)?;
                        t.set("alt", modifiers.alt)?;
                        t.set("meta", modifiers.meta)?;
                        t.set("repeat", *repeat)?;
                        Ok(t)
                    });
                    if let Ok(t) = table
                        && run(h, &vm, &actions, t).is_some_and(|v| v.as_boolean() == Some(true))
                    {
                        event.prevent_default();
                        event.stop_propagation();
                    }
                }
            }
            InputEvent::KeyUp { key: k, .. } => {
                if let Some(h) = &key_up {
                    run(h, &vm, &actions, key_name(k));
                }
            }
            InputEvent::Focus => {
                if let Some(h) = &focus {
                    run(h, &vm, &actions, ());
                }
            }
            InputEvent::Blur => {
                if let Some(h) = &blur {
                    run(h, &vm, &actions, ());
                }
            }
            InputEvent::Scroll {
                delta_x, delta_y, ..
            } => {
                if let Some(h) = &wheel {
                    let table = lua.create_table().and_then(|t| {
                        t.set("dx", *delta_x)?;
                        t.set("dy", *delta_y)?;
                        Ok(t)
                    });
                    if let Ok(t) = table {
                        run(h, &vm, &actions, t);
                    }
                }
            }
            _ => {}
        }
    })
}

/// Drag sources and destinations; payloads are script values.
fn with_drag(
    mut view: View,
    node: &Node,
    vm: &Rc<Vm>,
    low: &Rc<Lowering>,
    handle: Rc<OnceCell<NodeHandle>>,
) -> View {
    use zgui::input::DragPhase;
    let events = &node.events;
    fn call(vm: &Rc<Vm>, f: &mlua::Function, args: impl mlua::IntoLuaMulti) -> Option<mlua::Value> {
        vm.runtime
            .clone()
            .batch(|| match vm.call::<mlua::Value>(f, args) {
                Ok(value) => Some(value),
                Err(error) => {
                    vm.report(error);
                    None
                }
            })
    }
    if let Some(drag) = events.drag.clone() {
        let vm = vm.clone();
        view = view.on_drag(move || -> Value {
            call(&vm, &drag, ())
                .and_then(|v| Value::from_lua(v, &vm.lua).ok())
                .unwrap_or(Value::Nil)
        });
    }
    if let Some(preview) = events.drag_preview.clone() {
        let (vm, low) = (vm.clone(), low.clone());
        view = view.drag_preview(move |payload: &Value| {
            let result = vm.call::<mlua::Value>(&preview, payload.clone());
            built(&vm, result, &low)
        });
    }
    if let Some(end) = events.drag_end.clone() {
        let vm = vm.clone();
        view = view.on_drag_end(move |payload: &Value, accepted| {
            call(&vm, &end, (payload.clone(), accepted));
        });
    }
    let (drop, accept, over, leave) = (
        events.drop.clone(),
        events.drop_when.clone(),
        events.drag_over.clone(),
        events.drag_leave.clone(),
    );
    if drop.is_none() && over.is_none() && leave.is_none() {
        return view;
    }
    let vm = vm.clone();
    view.on_event(move |cx| {
        if cx.phase == EventPhase::Capture {
            return;
        }
        let InputEvent::Drag(event) = cx.event.clone() else {
            return;
        };
        let Some(payload) = event.payload.downcast_ref::<Value>() else {
            return;
        };
        let accepts = || {
            accept.as_ref().is_none_or(|f| {
                call(&vm, f, payload.clone())
                    .is_some_and(|v| !matches!(v, mlua::Value::Nil | mlua::Value::Boolean(false)))
            })
        };
        let point = || -> Option<mlua::Table> {
            let bounds = handle
                .get()
                .and_then(|h| h.scene().map(|s| s.borrow().bounds(h.node())))?;
            let t = vm.lua.create_table().ok()?;
            t.set("x", event.x).ok()?;
            t.set("y", event.y).ok()?;
            t.set("local_x", event.x - bounds.x).ok()?;
            t.set("local_y", event.y - bounds.y).ok()?;
            t.set("w", bounds.width).ok()?;
            t.set("h", bounds.height).ok()?;
            Some(t)
        };
        match event.phase {
            DragPhase::Over if accepts() => {
                cx.accept_drag();
                // The nearest accepting destination is the one that shows it.
                cx.stop_propagation();
                if let (Some(f), Some(p)) = (&over, point()) {
                    call(&vm, f, (payload.clone(), p));
                }
            }
            DragPhase::Leave => {
                if let Some(f) = &leave {
                    call(&vm, f, payload.clone());
                }
            }
            DragPhase::Drop if !cx.default_prevented() && accepts() => {
                if let (Some(f), Some(p)) = (&drop, point()) {
                    call(&vm, f, (payload.clone(), p));
                }
                cx.prevent_default();
                cx.stop_propagation();
            }
            _ => {}
        }
    })
}

/// Runs a handler in one reactive batch; returns what a script returned.
fn run(
    handler: &Handler,
    vm: &Rc<Vm>,
    actions: &Actions,
    args: impl mlua::IntoLuaMulti,
) -> Option<mlua::Value> {
    vm.runtime.clone().batch(|| match handler {
        Handler::Lua(f) => match vm.call::<mlua::Value>(f, args) {
            Ok(v) => Some(v),
            Err(error) => {
                vm.report(error);
                None
            }
        },
        Handler::Action(action) => {
            match action {
                Action::Set(s, v) => {
                    s.set(v.clone());
                }
                Action::Toggle(s) => {
                    s.update(|v| *v = Value::Bool(!v.truthy()));
                }
                Action::Add(s, by) => {
                    s.update(|v| *v = Value::Num(v.as_f64().unwrap_or(0.) + by));
                }
                Action::Host(name, arg) => {
                    let action = actions.borrow().get(&**name).cloned();
                    match action {
                        Some(action) => action(arg.clone()),
                        None => vm.report(mlua::Error::runtime(format!(
                            "no host action named `{name}`"
                        ))),
                    }
                }
            }
            None
        }
    })
}

/// A source with its Lua function or animation detached from the node, so a
/// reactive closure can own it.
enum Owned {
    Lit(Lit),
    State(Signal<Value>),
    Computed(mlua::Function),
    Anim(Option<Value>, Option<Value>),
}
fn resolve_owned(source: &Source) -> Owned {
    match source {
        Source::Lit(l) => Owned::Lit(l.clone()),
        Source::State(s) => Owned::State(s.clone()),
        Source::Computed(f) => Owned::Computed(f.clone()),
        Source::Anim(a) => Owned::Anim(a.from.clone(), a.to.clone()),
    }
}
fn resolve(source: &Owned, ty: Ty, scalar: Option<&Signal<f32>>, vm: &Vm) -> Option<Lit> {
    match source {
        Owned::Lit(l) => Some(l.clone()),
        Owned::State(s) => s.with(|v| lit(v, ty)),
        Owned::Computed(f) => vm
            .call::<Value>(f, ())
            .map_err(|e| vm.report(e))
            .ok()
            .and_then(|v| lit(&v, ty)),
        Owned::Anim(from, to) => {
            let t = scalar?.get();
            match (from, to) {
                (Some(a), Some(b)) => lerp(&lit(a, ty)?, &lit(b, ty)?, t),
                _ => Some(Lit::Num(t)),
            }
        }
    }
}
fn resolve_value(source: &Owned, vm: &Vm) -> Value {
    match source {
        Owned::Lit(Lit::Str(s)) => Value::Str(s.clone()),
        Owned::Lit(Lit::Num(n)) => Value::Num(*n as f64),
        Owned::Lit(Lit::Bool(b)) => Value::Bool(*b),
        Owned::State(s) => s.get(),
        Owned::Computed(f) => vm.call::<Value>(f, ()).unwrap_or_else(|e| {
            vm.report(e);
            Value::Nil
        }),
        _ => Value::Nil,
    }
}
fn lerp(a: &Lit, b: &Lit, t: f32) -> Option<Lit> {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    Some(match (a, b) {
        (Lit::Num(x), Lit::Num(y)) => Lit::Num(mix(*x, *y)),
        (Lit::Len(Length::Px(x)), Lit::Len(Length::Px(y))) => Lit::Len(Length::Px(mix(*x, *y))),
        (Lit::Pair(x0, y0), Lit::Pair(x1, y1)) => Lit::Pair(mix(*x0, *x1), mix(*y0, *y1)),
        (Lit::Color(c0), Lit::Color(c1)) => {
            let ch = |x: u8, y: u8| mix(x as f32, y as f32).round().clamp(0., 255.) as u8;
            Lit::Color(Color(
                ch(c0.0, c1.0),
                ch(c0.1, c1.1),
                ch(c0.2, c1.2),
                ch(c0.3, c1.3),
            ))
        }
        (_, other) if t >= 0.5 => other.clone(),
        (other, _) => other.clone(),
    })
}

fn render_text(text: &Text, vm: &Vm) -> String {
    match text {
        Text::Literal(s) => s.clone(),
        Text::Template(segments) => {
            let mut out = String::new();
            for segment in segments {
                match segment {
                    Segment::Literal(literal) => out.push_str(literal),
                    Segment::State(state) => state.with(|v| {
                        use std::fmt::Write;
                        let _ = write!(out, "{v}");
                    }),
                }
            }
            out
        }
        Text::Computed(f) => match vm.call::<Value>(f, ()) {
            Ok(v) => v.to_string(),
            Err(error) => {
                vm.report(error);
                "⚠".into()
            }
        },
    }
}
fn lower_text(text: &Text, vm: &Rc<Vm>) -> View {
    match text {
        Text::Literal(s) => zgui::compose::text(s.clone()),
        Text::Template(segments) => {
            let segments: Vec<_> = segments
                .iter()
                .map(|s| match s {
                    Segment::Literal(l) => Segment::Literal(l.clone()),
                    Segment::State(s) => Segment::State(s.clone()),
                })
                .collect();
            let template = Text::Template(segments);
            let vm = vm.clone();
            text_signal(move || render_text(&template, &vm))
        }
        Text::Computed(f) => {
            let (f, vm) = (Text::Computed(f.clone()), vm.clone());
            text_signal(move || render_text(&f, &vm))
        }
    }
}
fn lower_rich(spans: &[Span], vm: &Rc<Vm>, low: &Rc<Lowering>) -> View {
    /// A span with its text and handler owned by the closure.
    struct Owned {
        text: Text,
        style: SpanStyle,
        click: Option<Handler>,
    }
    #[derive(Clone, Copy)]
    struct SpanStyle {
        color: Option<zgui::theme::Themed<Color>>,
        background: Option<zgui::theme::Themed<Color>>,
        size: Option<f32>,
        weight: Option<u16>,
        italic: bool,
        underline: bool,
        strike: bool,
        mono: bool,
    }
    let owned: Vec<Owned> = spans
        .iter()
        .map(|s| Owned {
            text: match &s.text {
                Text::Literal(l) => Text::Literal(l.clone()),
                Text::Template(segments) => Text::Template(
                    segments
                        .iter()
                        .map(|seg| match seg {
                            Segment::Literal(l) => Segment::Literal(l.clone()),
                            Segment::State(s) => Segment::State(s.clone()),
                        })
                        .collect(),
                ),
                Text::Computed(f) => Text::Computed(f.clone()),
            },
            style: SpanStyle {
                color: s.color,
                background: s.background,
                size: s.size,
                weight: s.weight,
                italic: s.italic,
                underline: s.underline,
                strike: s.strike,
                mono: s.mono,
            },
            click: s.click.clone(),
        })
        .collect();
    let (vm, actions) = (vm.clone(), low.actions.clone());
    rich_text_signal(move || {
        owned
            .iter()
            .map(|span| {
                let SpanStyle {
                    color,
                    background,
                    size,
                    weight,
                    italic,
                    underline,
                    strike,
                    mono,
                } = span.style;
                let mut out: TextSpan = text_span(render_text(&span.text, &vm));
                if let Some(c) = color {
                    out = out.text_color(c);
                }
                if let Some(c) = background {
                    out = out.background(c);
                }
                if let Some(s) = size {
                    out = out.text_size(s);
                }
                if let Some(w) = weight {
                    out = out.font_weight(w);
                }
                if italic {
                    out = out.italic(true);
                }
                if underline {
                    out = out.underline(Default::default());
                }
                if strike {
                    out = out.strikethrough(Default::default());
                }
                if mono {
                    out = out.font_family(FontFamily::Monospace);
                }
                if let Some(handler) = &span.click {
                    let (handler, vm, actions) = (handler.clone(), vm.clone(), actions.clone());
                    out = out.on_click(move || {
                        run(&handler, &vm, &actions, ());
                    });
                }
                out
            })
            .collect()
    })
}

/// The translation and local transform a node's literals set, if any.
fn literal_transform(
    lits: &[(Prop, Lit)],
) -> (Option<(f32, f32)>, Option<crate::transform::LocalTransform>) {
    let mut translate: Option<(f32, f32)> = None;
    let mut local: Option<crate::transform::LocalTransform> = None;
    for (prop, lit) in lits {
        macro_rules! transform {
            () => {
                local.get_or_insert(crate::transform::LocalTransform::IDENTITY)
            };
        }
        match (prop, lit) {
            (Prop::X, Lit::Num(x)) => translate.get_or_insert((0., 0.)).0 = *x,
            (Prop::Y, Lit::Num(y)) => translate.get_or_insert((0., 0.)).1 = *y,
            (Prop::Scale, Lit::Num(v)) => transform!().scale = (*v, *v),
            (Prop::ScaleX, Lit::Num(v)) => transform!().scale.0 = *v,
            (Prop::ScaleY, Lit::Num(v)) => transform!().scale.1 = *v,
            (Prop::Rotate, Lit::Num(v)) => transform!().rotate = v.to_radians(),
            (Prop::Skew, Lit::Num(v)) => transform!().skew = (v.to_radians(), v.to_radians()),
            (Prop::SkewX, Lit::Num(v)) => transform!().skew.0 = v.to_radians(),
            (Prop::SkewY, Lit::Num(v)) => transform!().skew.1 = v.to_radians(),
            (Prop::Origin, Lit::Pair(x, y)) => transform!().origin = (*x, *y),
            _ => {}
        }
    }
    (translate, local)
}
fn is_corner(prop: Prop) -> bool {
    matches!(
        prop,
        Prop::RoundedTl | Prop::RoundedTr | Prop::RoundedBr | Prop::RoundedBl
    )
}
fn is_transform(prop: Prop) -> bool {
    matches!(
        prop,
        Prop::X
            | Prop::Y
            | Prop::Scale
            | Prop::ScaleX
            | Prop::ScaleY
            | Prop::Rotate
            | Prop::Skew
            | Prop::SkewX
            | Prop::SkewY
            | Prop::Origin
    )
}

/// Applies literal styles; translation and the local transform collect
/// their parts first, so each is set once.
fn apply_all<S: Styled>(mut s: S, lits: &[(Prop, Lit)]) -> S {
    for (prop, lit) in lits {
        if !is_transform(*prop) {
            s = apply(s, *prop, lit);
        }
    }
    // Single corners refine the radius or corners set with them.
    if lits.iter().any(|(p, _)| is_corner(*p)) {
        let mut c = zgui::decoration::Corners::all(0.);
        for (prop, lit) in lits {
            match (prop, lit) {
                (Prop::Rounded, Lit::Num(r)) => c = zgui::decoration::Corners::all(*r),
                (Prop::Corners, Lit::Corners(all)) => c = *all,
                (Prop::RoundedTl, Lit::Num(r)) => c.top_left = *r,
                (Prop::RoundedTr, Lit::Num(r)) => c.top_right = *r,
                (Prop::RoundedBr, Lit::Num(r)) => c.bottom_right = *r,
                (Prop::RoundedBl, Lit::Num(r)) => c.bottom_left = *r,
                _ => {}
            }
        }
        s = s.rounded_corners(c);
    }
    let (translate, local) = literal_transform(lits);
    if let Some((x, y)) = translate {
        s = s.translate(x, y);
    }
    if let Some(local) = local {
        s = crate::transform::style(s, local);
    }
    s
}

fn align(v: &str) -> Align {
    match v {
        "center" => Align::Center,
        "end" => Align::End,
        "baseline" => Align::Baseline,
        "stretch" => Align::Stretch,
        _ => Align::Start,
    }
}

fn apply<S: Styled>(s: S, prop: Prop, lit: &Lit) -> S {
    use Prop::*;
    if let Lit::Token(token) = lit {
        return match token_field(prop) {
            Some(field) => s.token(field, *token),
            None => s,
        };
    }
    // Lengths: points, percentages (of the containing block) or auto.
    let len = |l: &Lit| match l {
        Lit::Len(l) => Some(*l),
        Lit::Num(n) => Some(Length::Px(*n)),
        _ => None,
    };
    if let Some(l) = len(lit) {
        let pct = |p: f32| p * 100.;
        match (prop, l) {
            (W, Length::Px(v)) => return s.w(v),
            (W, Length::Percent(p)) => return s.w_percent(pct(p)),
            (H, Length::Px(v)) => return s.h(v),
            (H, Length::Percent(p)) => return s.h_percent(pct(p)),
            (MinW, Length::Px(v)) => return s.min_w(v),
            (MinW, Length::Percent(p)) => return s.min_w_percent(pct(p)),
            (MaxW, Length::Px(v)) => return s.max_w(v),
            (MaxW, Length::Percent(p)) => return s.max_w_percent(pct(p)),
            (MinH, Length::Px(v)) => return s.min_h(v),
            (MinH, Length::Percent(p)) => return s.min_h_percent(pct(p)),
            (MaxH, Length::Px(v)) => return s.max_h(v),
            (MaxH, Length::Percent(p)) => return s.max_h_percent(pct(p)),
            (P, Length::Px(v)) => return s.p(v),
            (P, Length::Percent(p)) => return s.p_percent(pct(p)),
            (Px, Length::Px(v)) => return s.px(v),
            (Px, Length::Percent(p)) => return s.px_percent(pct(p)),
            (Py, Length::Px(v)) => return s.py(v),
            (Py, Length::Percent(p)) => return s.py_percent(pct(p)),
            (Pt, Length::Px(v)) => return s.pt(v),
            (Pt, Length::Percent(p)) => return s.pt_percent(pct(p)),
            (Pr, Length::Px(v)) => return s.pr(v),
            (Pr, Length::Percent(p)) => return s.pr_percent(pct(p)),
            (Pb, Length::Px(v)) => return s.pb(v),
            (Pb, Length::Percent(p)) => return s.pb_percent(pct(p)),
            (Pl, Length::Px(v)) => return s.pl(v),
            (Pl, Length::Percent(p)) => return s.pl_percent(pct(p)),
            (M, Length::Px(v)) => return s.m(v),
            (M, Length::Percent(p)) => return s.m_percent(pct(p)),
            (M, Length::Auto) => return s.m_auto(),
            (Mx, Length::Px(v)) => return s.mx(v),
            (Mx, Length::Percent(p)) => return s.mx_percent(pct(p)),
            (Mx, Length::Auto) => return s.mx_auto(),
            (My, Length::Px(v)) => return s.my(v),
            (My, Length::Percent(p)) => return s.my_percent(pct(p)),
            (My, Length::Auto) => return s.my_auto(),
            (Mt, Length::Px(v)) => return s.mt(v),
            (Mt, Length::Percent(p)) => return s.mt_percent(pct(p)),
            (Mt, Length::Auto) => return s.mt_auto(),
            (Mr, Length::Px(v)) => return s.mr(v),
            (Mr, Length::Percent(p)) => return s.mr_percent(pct(p)),
            (Mr, Length::Auto) => return s.mr_auto(),
            (Mb, Length::Px(v)) => return s.mb(v),
            (Mb, Length::Percent(p)) => return s.mb_percent(pct(p)),
            (Mb, Length::Auto) => return s.mb_auto(),
            (Ml, Length::Px(v)) => return s.ml(v),
            (Ml, Length::Percent(p)) => return s.ml_percent(pct(p)),
            (Ml, Length::Auto) => return s.ml_auto(),
            (Top, l) => return s.top(l),
            (Left, l) => return s.left(l),
            (Right, l) => return s.right(l),
            (Bottom, l) => return s.bottom(l),
            (Basis, l) => return s.flex_basis(l),
            (GapX, l) => return s.gap_x(l),
            (GapY, l) => return s.gap_y(l),
            _ => {}
        }
    }
    match (prop, lit) {
        (Size, Lit::Pair(w, h)) => s.size(*w, *h),
        (WFull, Lit::Bool(true)) => s.w_full(),
        (HFull, Lit::Bool(true)) => s.h_full(),
        (Aspect, Lit::Num(v)) => s.aspect_ratio(*v),
        (Absolute, Lit::Bool(true)) => s.absolute(),
        (Relative, Lit::Bool(true)) => s.relative(),
        (Gap, Lit::Num(v)) => s.gap(*v),
        (Direction, Lit::Str(v)) => match &**v {
            "row" => s.flex_row(),
            "row_reverse" => s.flex_row_reverse(),
            "column_reverse" => s.flex_col_reverse(),
            _ => s.flex_col(),
        },
        (Grow, Lit::Num(v)) => s.flex_grow(*v),
        (Shrink, Lit::Num(v)) => s.flex_shrink(*v),
        (FlexWrap, Lit::Bool(true)) => s.flex_wrap(),
        (WrapReverse, Lit::Bool(true)) => s.flex_wrap_reverse(),
        (Items, Lit::Str(v)) => match &**v {
            "start" => s.items_start(),
            "center" => s.items_center(),
            "end" => s.items_end(),
            "baseline" => s.items_baseline(),
            _ => s.items_stretch(),
        },
        (Justify, Lit::Str(v)) => match &**v {
            "center" => s.justify_center(),
            "end" => s.justify_end(),
            "between" => s.justify_between(),
            "around" => s.justify_around(),
            "evenly" => s.justify_evenly(),
            "stretch" => s.justify_stretch(),
            _ => s.justify_start(),
        },
        (AlignSelf, Lit::Str(v)) => s.align_self(align(v)),
        (AlignContent, Lit::Str(v)) => s.align_content(match &**v {
            "center" => ContentAlign::Center,
            "end" => ContentAlign::End,
            "stretch" => ContentAlign::Stretch,
            "between" => ContentAlign::SpaceBetween,
            "around" => ContentAlign::SpaceAround,
            "evenly" => ContentAlign::SpaceEvenly,
            _ => ContentAlign::Start,
        }),
        (Grid, Lit::Bool(true)) => s.grid(),
        (Cols, Lit::Num(v)) => s.grid_cols(v.max(1.) as u16),
        (Rows, Lit::Num(v)) => s.grid_rows(v.max(1.) as u16),
        (ColStart, Lit::Num(v)) => s.col_start(*v as i16),
        (RowStart, Lit::Num(v)) => s.row_start(*v as i16),
        (ColSpan, Lit::Num(v)) => s.col_span(v.max(1.) as u16),
        (RowSpan, Lit::Num(v)) => s.row_span(v.max(1.) as u16),
        (ColSpanFull, Lit::Bool(true)) => s.col_span_full(),
        (RowSpanFull, Lit::Bool(true)) => s.row_span_full(),
        (Overflow, Lit::Str(v)) => match &**v {
            "hidden" | "clip" => s.overflow_hidden(),
            _ => s.overflow_visible(),
        },
        (OverflowX, Lit::Str(v)) => match &**v {
            "hidden" | "clip" => s.overflow_x_hidden(),
            _ => s.overflow_x_visible(),
        },
        (OverflowY, Lit::Str(v)) => match &**v {
            "hidden" | "clip" => s.overflow_y_hidden(),
            _ => s.overflow_y_visible(),
        },
        (Hidden, Lit::Bool(true)) => s.hidden(),
        (Hidden, Lit::Bool(false)) => s.visible(),
        (Invisible, Lit::Bool(true)) => s.invisible(),
        (Bg, Lit::Color(c)) => s.bg(*c),
        (Gradient, Lit::Gradient(angle, stops)) => s.bg_gradient(*angle, stops.clone()),
        (TextColor, Lit::Color(c)) => s.text_color(*c),
        (Rounded, Lit::Num(v)) => s.rounded(*v),
        (Corners, Lit::Corners(c)) => s.rounded_corners(*c),
        // The border paints over the node's children (a CSS inset outline).
        (BorderOver, Lit::Bool(v)) => s.border_over(*v),
        // `{ top, right, bottom, left }`, as CSS orders border widths.
        (BorderSides, Lit::Corners(c)) => s.border_edges(zgui::scene::Insets {
            top: c.top_left,
            right: c.top_right,
            bottom: c.bottom_right,
            left: c.bottom_left,
        }),
        (RoundedTl | RoundedTr | RoundedBr | RoundedBl, _) => s,
        (Border, Lit::Num(v)) => s.border(*v),
        (BorderColor, Lit::Color(c)) => s.border_color(*c),
        (BorderDash, Lit::Pair(length, gap)) => {
            s.border_style(zgui::decoration::BorderStyle::Dashed {
                length: *length,
                gap: *gap,
            })
        }
        (Shadow, Lit::Shadows(shadows)) => s.shadows(shadows.clone()),
        (Opacity, Lit::Num(v)) => s.opacity(v.clamp(0., 1.)),
        (Blur, Lit::Num(v)) => s.filter_blur(v.max(0.)),
        (BackdropBlur, Lit::Num(v)) => s.blur(v.max(0.)),
        (Brightness, Lit::Num(v)) => s.brightness(*v),
        (Contrast, Lit::Num(v)) => s.contrast(*v),
        (Saturate, Lit::Num(v)) => s.saturate(*v),
        (Fade, Lit::Pair(top, bottom)) => s.fade_edges(*top, *bottom),
        (FadeX, Lit::Pair(left, right)) => s.fade_edges_x(*left, *right),
        (FadeOverflow, Lit::Bool(v)) => s.fade_overflow(*v),
        (Snap, Lit::Bool(v)) => s.snap(*v),
        // `{ top, right, bottom, left }` in points; negative reaches past the box.
        (ClipInset, Lit::Corners(c)) => {
            s.clip_inset(c.top_left, c.top_right, c.bottom_right, c.bottom_left)
        }
        // `{ top, right, bottom, left }`; 0 keeps an edge sharp.
        (FadeSides, Lit::Corners(c)) => {
            s.fade_sides(c.top_left, c.top_right, c.bottom_right, c.bottom_left)
        }
        (Layer, Lit::Bool(v)) => s.isolated(*v),
        (BgImage | BgFit, _) => s,
        (Fit, Lit::Str(v)) => s.object_fit(match &**v {
            "contain" => ObjectFit::Contain,
            "cover" => ObjectFit::Cover,
            "none" => ObjectFit::None,
            "scale_down" => ObjectFit::ScaleDown,
            _ => ObjectFit::Fill,
        }),
        (Cursor, Lit::Str(v)) => s.cursor(cursor(v)),
        (TextSize, Lit::Num(v)) => s.text_size(*v),
        (Weight, Lit::Num(v)) => s.font_weight(*v as u16),
        (Bold, Lit::Bool(true)) => s.font_bold(),
        (Italic, Lit::Bool(v)) => s.italic(*v),
        (Align, Lit::Str(v)) => s.text_align(match &**v {
            "center" => TextAlign::Center,
            "right" => TextAlign::Right,
            "left" => TextAlign::Left,
            _ => TextAlign::Start,
        }),
        (Font, Lit::Str(v)) => match &**v {
            "mono" | "monospace" => s.font_family(FontFamily::Monospace),
            "sans" | "sans-serif" => s.font_family(FontFamily::SansSerif),
            "serif" => s.font_family(FontFamily::Serif),
            name => s.font_family(FontFamily::Named(name.into())),
        },
        (Wrap, Lit::Bool(v)) => s.text_wrap(*v),
        (Truncate, Lit::Bool(true)) => s.truncate(),
        (Lines, Lit::Num(v)) => s.line_clamp(*v as u32),
        (LineHeight, Lit::Num(v)) => s.line_height(*v),
        (LetterSpacing, Lit::Num(v)) => s.letter_spacing(*v),
        // Every digit one width, so changing numbers hold still.
        (Tabular, Lit::Bool(true)) => {
            s.font_features(zgui::text_layout::FontFeatures::new([(*b"tnum", 1)]))
        }
        _ => s,
    }
}

fn cursor(name: &str) -> zgui::cursor::Cursor {
    use zgui::cursor::Cursor::*;
    match name {
        "pointer" => Pointer,
        "text" => Text,
        "vertical_text" => VerticalText,
        "crosshair" => Crosshair,
        "move" => Move,
        "grab" => Grab,
        "grabbing" => Grabbing,
        "not_allowed" => NotAllowed,
        "wait" => Wait,
        "progress" => Progress,
        "help" => Help,
        "context_menu" => ContextMenu,
        "copy" => Copy,
        "alias" => Alias,
        "no_drop" => NoDrop,
        "all_scroll" => AllScroll,
        "col_resize" => ColResize,
        "row_resize" => RowResize,
        "ew_resize" => EwResize,
        "ns_resize" => NsResize,
        "nesw_resize" => NeswResize,
        "nwse_resize" => NwseResize,
        "zoom_in" => ZoomIn,
        "zoom_out" => ZoomOut,
        _ => Default,
    }
}

/// One piece of content in a presence: its value, and whether it is
/// leaving.
struct Entry {
    id: u64,
    value: Value,
    exiting: bool,
    animate_in: bool,
    /// Holds the presence's place while `mode = "wait"` waits for what
    /// leaves (an empty box: what leaves lies over it).
    placeholder: bool,
}
#[derive(Default)]
struct Present {
    entries: Vec<Entry>,
    next: u64,
    elements: HashMap<u64, (Rc<Element>, NodeHandle)>,
    /// `mode = "wait"`: the value to show once the leaving content is gone.
    waiting: Option<Value>,
}
impl Present {
    fn push(&mut self, value: Value, animate_in: bool, placeholder: bool) {
        let id = self.next;
        self.next += 1;
        self.entries.push(Entry {
            id,
            value,
            exiting: false,
            animate_in,
            placeholder,
        });
    }
}

/// Properties that make a presence a box of its own, which its content
/// fills: without them it lays out as its content, as if in its place.
const BOX: &[Prop] = &[
    Prop::W,
    Prop::H,
    Prop::Size,
    Prop::MinW,
    Prop::MaxW,
    Prop::MinH,
    Prop::MaxH,
    Prop::WFull,
    Prop::HFull,
    Prop::Aspect,
    Prop::Absolute,
    Prop::Top,
    Prop::Left,
    Prop::Right,
    Prop::Bottom,
    Prop::P,
    Prop::Px,
    Prop::Py,
    Prop::Pt,
    Prop::Pr,
    Prop::Pb,
    Prop::Pl,
    Prop::Grow,
    Prop::Shrink,
    Prop::Basis,
    Prop::AlignSelf,
    Prop::ColStart,
    Prop::RowStart,
    Prop::ColSpan,
    Prop::RowSpan,
    Prop::ColSpanFull,
    Prop::RowSpanFull,
];

/// `ui.presence`: content for the current value. When the value changes
/// the new content mounts and plays `enter`; the old leaves the flow
/// (positioned where it was, at the size it had) and plays `exit`, then
/// unmounts. With sizing of its own (`boxed`) it is a box the content
/// fills; otherwise the content lays out as if it stood in its place.
#[allow(clippy::too_many_arguments)]
fn presence(
    cx: &mut Context,
    value: Rc<Owned>,
    builder: mlua::Function,
    enter: Option<mlua::Table>,
    exit: Option<mlua::Table>,
    wait: bool,
    initial: bool,
    boxed: bool,
    vm: &Rc<Vm>,
    low: &Rc<Lowering>,
) -> View {
    let state: Rc<std::cell::RefCell<Present>> = Rc::default();
    let shown = cx.state(Vec::<u64>::new());
    let publish = {
        let (state, shown) = (state.clone(), shown.clone());
        Rc::new(move || {
            let ids: Vec<u64> = state.borrow().entries.iter().map(|e| e.id).collect();
            shown.set(ids);
        })
    };
    // Plays `params` on `element`; `done` runs when it completes.
    let play = {
        let vm = vm.clone();
        Rc::new(
            move |element: &Rc<Element>,
                  params: &mlua::Table,
                  done: Option<Rc<dyn Fn()>>|
                  -> mlua::Result<()> {
                let lua = &vm.lua;
                let own = lua.create_table()?;
                for pair in params.pairs::<mlua::Value, mlua::Value>() {
                    let (k, v) = pair?;
                    own.raw_set(k, v)?;
                }
                if let Some(done) = done {
                    let done = lua.create_function(move |_, _: mlua::MultiValue| {
                        done();
                        Ok(())
                    })?;
                    own.raw_set("onComplete", done)?;
                }
                let target =
                    mlua::Value::UserData(lua.create_userdata(ElementUd(element.clone()))?);
                crate::anime::api::animate(lua, target, own)?;
                Ok(())
            },
        )
    };
    let remove: Rc<dyn Fn(u64)> = {
        let (state, publish) = (state.clone(), publish.clone());
        Rc::new(move |id: u64| {
            {
                let mut st = state.borrow_mut();
                st.entries.retain(|e| e.id != id);
                st.elements.remove(&id);
                if let Some(value) = st
                    .waiting
                    .take()
                    .filter(|_| st.entries.iter().all(|e| !e.exiting))
                {
                    st.entries.retain(|e| !e.placeholder);
                    st.push(value, true, false);
                }
            }
            publish();
        })
    };
    let change = {
        let (state, publish, play, remove, vm, engine) = (
            state.clone(),
            publish.clone(),
            play.clone(),
            remove.clone(),
            vm.clone(),
            vm.lua
                .app_data_ref::<crate::node::Env>()
                .map(|env| env.engine.clone()),
        );
        move |value: Value| {
            let leaving = {
                let mut st = state.borrow_mut();
                let first = st.entries.is_empty() && st.waiting.is_none();
                if st
                    .entries
                    .iter()
                    .any(|e| !e.exiting && !e.placeholder && e.value == value)
                {
                    return;
                }
                let mut leaving = Vec::new();
                for e in st
                    .entries
                    .iter_mut()
                    .filter(|e| !e.exiting && !e.placeholder)
                {
                    e.exiting = true;
                    leaving.push(e.id);
                }
                if wait && st.entries.iter().any(|e| e.exiting) {
                    st.waiting = Some(value);
                    if !st.entries.iter().any(|e| e.placeholder) {
                        st.push(Value::Nil, false, true);
                    }
                } else {
                    st.entries.retain(|e| !e.placeholder);
                    st.push(value, !first || initial, false);
                }
                leaving
            };
            for id in leaving {
                let mounted = state.borrow().elements.get(&id).cloned();
                match (mounted, &exit) {
                    (Some((element, handle)), Some(exit)) => {
                        // Out of the flow, where it was and as large: what
                        // replaces it decides the size from now on.
                        let scene = handle.scene().filter(|_| handle.is_mounted());
                        let size = scene.as_ref().map_or((0., 0.), |scene| {
                            let b = scene.borrow().bounds(handle.node());
                            (b.width, b.height)
                        });
                        if let Some(scene) = &scene {
                            scene.borrow_mut().set_contents(handle.node(), false);
                        }
                        // A one-cell grid holds the content at that size,
                        // flex-sized content included.
                        let px = |v: f32| Lit::Len(zgui::layout::Length::Px(v));
                        for (prop, lit) in [
                            (Prop::Absolute, Lit::Bool(true)),
                            (Prop::Top, px(0.)),
                            (Prop::Left, px(0.)),
                            (Prop::W, px(size.0)),
                            (Prop::H, px(size.1)),
                            (Prop::Grid, Lit::Bool(true)),
                            (Prop::Cols, Lit::Num(1.)),
                            (Prop::Rows, Lit::Num(1.)),
                            (Prop::Items, Lit::Str("stretch".into())),
                        ] {
                            element.set_style(prop, lit);
                        }
                        if let Some(engine) = &engine {
                            engine.set_now(Vec::new());
                        }
                        let then = remove.clone();
                        if let Err(e) = play(&element, exit, Some(Rc::new(move || then(id)))) {
                            vm.report(e);
                            remove(id);
                        }
                    }
                    _ => remove(id),
                }
            }
            publish();
        }
    };
    let runtime = cx.runtime();
    let (value_vm, change_runtime) = (vm.clone(), runtime.clone());
    cx.retain(runtime.effect(move || {
        let current = resolve_value(&value, &value_vm);
        change_runtime.untracked(|| change(current));
    }));
    let (vm, low) = (vm.clone(), low.clone());
    // The content lays out as if it stood where the presence does (or in
    // the presence's box): the region and each entry's wrapper are contents
    // nodes, and what leaves lies over the current content.
    let region = keyed(
        move || shown.get(),
        move |id, cx| {
            let (value, animate_in) = state
                .borrow()
                .entries
                .iter()
                .find(|e| e.id == id)
                .map_or((Value::Nil, false), |e| (e.value.clone(), e.animate_in));
            // Nothing shows for nil: `presence(cond and "x" or nil, …)`
            // enters and exits one piece of content.
            let content = if value == Value::Nil {
                div()
            } else {
                build_scoped(&vm, cx, &low, &builder, value)
            };
            let element = Element::new(&low.runtime, None, Rc::new([]), "presence");
            let (state, enter, play, vm) = (state.clone(), enter.clone(), play.clone(), vm.clone());
            let mounted = element.clone();
            element_view(
                // Until it leaves (see `change`).
                column().contents().child(content),
                element,
                &low,
                move |handle| {
                    state
                        .borrow_mut()
                        .elements
                        .insert(id, (mounted.clone(), handle));
                    if let (true, Some(enter)) = (animate_in, &enter)
                        && let Err(e) = play(&mounted, enter, None)
                    {
                        vm.report(e);
                    }
                },
            )
        },
    )
    .contents();
    if boxed {
        // Its content fills it, in both directions, unless it sizes itself.
        cell().child(region)
    } else {
        region
    }
}

/// A box whose one in-flow child fills it in both directions unless it
/// sizes itself: a one-cell grid, whose items stretch.
fn cell() -> View {
    column().grid().grid_cols(1).grid_rows(1).items_stretch()
}
