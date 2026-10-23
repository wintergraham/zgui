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
}

pub(crate) struct Lowering {
    pub slots: HashMap<String, Vec<Rc<Node>>>,
    pub actions: Actions,
    pub elements: Rc<crate::element::Elements>,
    pub runtime: zgui::reactive::Runtime,
    pub assets: Rc<Assets>,
    pub natives: Rc<crate::Natives>,
}

pub(crate) fn lower(node: &Rc<Node>, low: &Rc<Lowering>) -> View {
    let Some(vm) = node.owner.upgrade() else {
        return div();
    };
    if node.needs_context() {
        let (node, low) = (node.clone(), low.clone());
        return component(move |cx| build(&node, &vm, &low, Some(cx)));
    }
    build(node, &vm, low, None)
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
fn build_scoped(
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
    let element = node.targetable().then(|| {
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
                    _ => data,
                }
            })
        }
        Kind::WebView(spec) => web_view_of(spec, vm, low),
        Kind::Terminal(term) => zgui::terminal::terminal(term),
        Kind::Native(key) => {
            let (key, vm, natives) = (resolve_owned(key), vm.clone(), low.natives.clone());
            zgui::web_view::native_view_when(move || {
                let key = match resolve(&key, Ty::Str, None, &vm) {
                    Some(Lit::Str(key)) => key.to_string(),
                    _ => String::new(),
                };
                natives
                    .get(&key)
                    .ok_or_else(|| format!("Native view · {key} (not provided)"))
            })
        }
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
        } => {
            let value = bridge(
                need(&mut cx),
                value,
                |v| v.to_string(),
                |s| Value::Str(s.into()),
            );
            if *multiline {
                text_area(label.clone(), value)
            } else {
                text_input(label.clone(), value)
            }
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
        }
        Kind::Each {
            items,
            build: builder,
            key,
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
                move |_, index, _| built(&vm, vm.call::<mlua::Value>(&builder, index + 1), &low),
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
            if child.kind.is_region() {
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
    // With `layout`, where the node sits belongs to the box that animates.
    let mut placement = Vec::new();
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
    if let Some(layout) = &node.layout {
        let (moved, kept): (Vec<_>, Vec<_>) = statics
            .into_iter()
            .partition(|(p, _)| PLACEMENT.contains(p));
        statics = kept;
        placement = moved;
        // The animated axis's size is set on the outer box; the content
        // fills its other axis.
        let sizing: &[Prop] = if layout.height {
            &[Prop::W, Prop::MinW, Prop::MaxW, Prop::WFull]
        } else {
            &[Prop::H, Prop::MinH, Prop::MaxH, Prop::HFull]
        };
        let sized: Vec<_> = statics
            .iter()
            .filter(|(p, _)| sizing.contains(p))
            .cloned()
            .collect();
        if !sized.is_empty() {
            statics.retain(|(p, _)| !sizing.contains(p));
            placement.extend(sized);
            statics.push((
                if layout.height {
                    Prop::WFull
                } else {
                    Prop::HFull
                },
                Lit::Bool(true),
            ));
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
    if node.keep_focus {
        view = view.keeps_focus(true);
    }
    if let Some(on) = &node.pointer_events {
        let (on, vm) = (resolve_owned(on), vm.clone());
        view = view.pointer_events_when(move || match resolve(&on, Ty::Bool, None, &vm) {
            Some(Lit::Bool(on)) => Some(on),
            _ => None,
        });
    }
    if let Some(color) = node.selection_color {
        view = view.selection_color(color);
    }
    if let Some(focusable) = node.focusable {
        view = view.focusable(focusable);
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
        let (mount, resize, vm) = (
            node.events.mount.clone(),
            node.events.resize.clone(),
            vm.clone(),
        );
        view = view.on_mount(move |mounted| {
            let _ = handle.set(mounted.clone());
            let Some(element) = element else { return };
            elements.register(&element, mounted.clone());
            for r in &refs {
                r.attach(&element);
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
    if let Some(layout) = &node.layout {
        view = layout_box(view, placement, layout, vm, low);
    }
    view
}

/// How a container lays out its children, for the regions among them
/// (`each`, `show`, `switch`, `presence`, `slot`): a region is a box of its
/// own, and lays its children out as its parent would, stretched across the
/// parent's cross axis in a column. CSS would say `display: contents`.
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

/// Where a node sits in its parent: with `layout`, these move to the box
/// whose size animates.
const PLACEMENT: &[Prop] = &[
    Prop::M,
    Prop::Mx,
    Prop::My,
    Prop::Mt,
    Prop::Mr,
    Prop::Mb,
    Prop::Ml,
    Prop::Absolute,
    Prop::Relative,
    Prop::Top,
    Prop::Left,
    Prop::Right,
    Prop::Bottom,
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
    Prop::X,
    Prop::Y,
];

/// Resolves an element's animated values over `lits`: what an animation
/// writes wins, as the inline styles anime.js writes do.
fn with_animated(element: &Element, mut lits: Vec<(Prop, Lit)>) -> Styles {
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
    if let Some((x, y)) = animated.translate {
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

/// The springy default for size changes: anime.js `spring({ bounce = .1,
/// duration = 400 })`, as the motion lab tunes it.
fn layout_ease(layout: &LayoutAnim) -> (crate::anime::ease::Ease, f64) {
    use crate::anime::ease::{Ease, Spring};
    let ease = layout
        .ease
        .clone()
        .unwrap_or_else(|| Ease::Spring(Rc::new(Spring::perceptual(0.1, 0.4))));
    let duration = layout
        .duration
        .or_else(|| ease.spring_duration_ms())
        .unwrap_or(350.);
    (ease, duration)
}

/// Wraps `inner` in a box whose size follows the inner content's, animated.
/// The content keeps its own size; what animates is the space it takes, as
/// a margin (the lab's technique): a negative bottom (or right) margin holds
/// the box at its last size and eases to zero. The content's constraints
/// never change, so a frame of the animation lays out only the box and its
/// ancestors, and nothing is clipped unless `layout_clip` asks for it.
fn layout_box(
    inner: View,
    placement: Vec<(Prop, Lit)>,
    layout: &LayoutAnim,
    vm: &Rc<Vm>,
    low: &Rc<Lowering>,
) -> View {
    let outer = Element::new(&low.runtime, None, Rc::new([]), "layout");
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
            let mut snap = Vec::new();
            // A height-only box whose width changed reflowed (its container
            // resized: a panel opening, a sidebar dragged); that isn't a
            // content change to animate, so it takes its new height at once.
            let reflowed = grow_h
                && !grow_w
                && previous.is_some_and(|(width, _)| (width - size.0).abs() > 0.5);
            for (on, prop, now, before) in [
                (grow_h, Prop::Mb, size.1, previous.map(|p| p.1)),
                (grow_w, Prop::Mr, size.0, previous.map(|p| p.0)),
            ] {
                if !on {
                    continue;
                }
                if reflowed && prop == Prop::Mb {
                    snap.push((element.clone(), Property::Style(prop), V::Num(0.), None));
                    continue;
                }
                let shown = match before {
                    // Entering grows from nothing; a first size otherwise holds.
                    None if enter => 0.,
                    None => continue,
                    Some(before) if before == now => continue,
                    Some(before) => before + current(prop).unwrap_or(0.),
                };
                from.push((
                    element.clone(),
                    Property::Style(prop),
                    V::Num((shown - now) as f64),
                    None,
                ));
                to.push((element.clone(), Property::Style(prop), V::Num(0.)));
            }
            if !snap.is_empty() {
                engine.remove_targets(std::slice::from_ref(&element));
                engine.set_now(snap);
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
    let mut middle = column().items_stretch().child(inner);
    if enter {
        // Closed until the content's first size arrives: no frame at full size.
        if grow_h {
            middle = middle.mb(-100_000.);
        }
        if grow_w {
            middle = middle.mr(-100_000.);
        }
    }
    let middle = element_view(middle, slack, low, |_| {});
    let mut outer_view = column().items_stretch().child(middle);
    if clip {
        outer_view = outer_view.overflow_hidden();
    }
    outer_view = apply_all(outer_view, &placement);
    element_view(outer_view, outer, low, |_| {})
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
    let decoded = std::fs::read(&path)
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
            std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
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
        // As DOM listeners: at the target and while bubbling. An ancestor is
        // routed both ways, so taking capture too ran its handlers twice per
        // event, and before the target's (a bar saw a press before the button
        // in it did).
        if event.phase == EventPhase::Capture {
            return;
        }
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
            && matches!(
                event.event,
                InputEvent::PointerDown {
                    button: PointerButton::Primary,
                    ..
                }
            )
        {
            event.capture_pointer_unless_taken();
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
        color: Option<Color>,
        background: Option<Color>,
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
/// 0 for shorthands, 1 for axis pairs, 2 for single sides and the rest.
fn specificity(prop: Prop) -> u8 {
    match prop {
        Prop::P
        | Prop::M
        | Prop::Gap
        | Prop::Border
        | Prop::Overflow
        | Prop::Size
        | Prop::Rounded => 0,
        Prop::Px | Prop::Py | Prop::Mx | Prop::My => 1,
        _ => 2,
    }
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
    // Shorthands reset their parts (`border` clears `border_sides`, `p` sets
    // every side), so they go first and the specific forms refine them, as
    // in CSS. Script tables have no order of their own: applied as listed,
    // `{ border = 1, border_sides = { 1, 0, 0, 0 } }` drew either.
    for level in 0..3 {
        for (prop, lit) in lits {
            if !is_transform(*prop) && specificity(*prop) == level {
                s = apply(s, *prop, lit);
            }
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
        "end" | "flex-end" => Align::End,
        "baseline" => Align::Baseline,
        "stretch" => Align::Stretch,
        _ => Align::Start,
    }
}

fn apply<S: Styled>(s: S, prop: Prop, lit: &Lit) -> S {
    use Prop::*;
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
            // CSS spellings too: scripts are often ported from stylesheets.
            "start" | "flex-start" => s.items_start(),
            "center" => s.items_center(),
            "end" | "flex-end" => s.items_end(),
            "baseline" => s.items_baseline(),
            _ => s.items_stretch(),
        },
        (Justify, Lit::Str(v)) => match &**v {
            "center" => s.justify_center(),
            "end" | "flex-end" => s.justify_end(),
            "between" | "space-between" => s.justify_between(),
            "around" | "space-around" => s.justify_around(),
            "evenly" | "space-evenly" => s.justify_evenly(),
            _ => s.justify_start(),
        },
        (AlignSelf, Lit::Str(v)) => s.align_self(align(v)),
        (AlignContent, Lit::Str(v)) => s.align_content(match &**v {
            "center" => ContentAlign::Center,
            "end" | "flex-end" => ContentAlign::End,
            "stretch" => ContentAlign::Stretch,
            "between" | "space-between" => ContentAlign::SpaceBetween,
            "around" | "space-around" => ContentAlign::SpaceAround,
            "evenly" | "space-evenly" => ContentAlign::SpaceEvenly,
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
}
#[derive(Default)]
struct Present {
    entries: Vec<Entry>,
    next: u64,
    elements: HashMap<u64, Rc<Element>>,
    /// `mode = "wait"`: the value to show once the leaving content is gone.
    waiting: Option<Value>,
}

/// `ui.presence`: content for the current value. When the value changes
/// the new content mounts and plays `enter`; the old leaves the flow
/// (positioned where it was) and plays `exit`, then unmounts.
#[allow(clippy::too_many_arguments)]
fn presence(
    cx: &mut Context,
    value: Rc<Owned>,
    builder: mlua::Function,
    enter: Option<mlua::Table>,
    exit: Option<mlua::Table>,
    wait: bool,
    initial: bool,
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
                    let id = st.next;
                    st.next += 1;
                    st.entries.push(Entry {
                        id,
                        value,
                        exiting: false,
                        animate_in: true,
                    });
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
                if st.entries.iter().any(|e| !e.exiting && e.value == value) {
                    return;
                }
                let mut leaving = Vec::new();
                for e in st.entries.iter_mut().filter(|e| !e.exiting) {
                    e.exiting = true;
                    leaving.push(e.id);
                }
                if wait && st.entries.iter().any(|e| e.exiting) {
                    st.waiting = Some(value);
                } else {
                    let id = st.next;
                    st.next += 1;
                    st.entries.push(Entry {
                        id,
                        value,
                        exiting: false,
                        animate_in: !first || initial,
                    });
                }
                leaving
            };
            for id in leaving {
                let element = state.borrow().elements.get(&id).cloned();
                match (element, &exit) {
                    (Some(element), Some(exit)) => {
                        // Out of the flow, where it was: what replaces it
                        // decides the size from now on.
                        for (prop, lit) in [
                            (Prop::Absolute, Lit::Bool(true)),
                            (Prop::Top, Lit::Len(zgui::layout::Length::Px(0.))),
                            (Prop::Left, Lit::Len(zgui::layout::Length::Px(0.))),
                            (Prop::Right, Lit::Len(zgui::layout::Length::Px(0.))),
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
    overlay().w_full().child(
        keyed(
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
                let (state, enter, play, vm) =
                    (state.clone(), enter.clone(), play.clone(), vm.clone());
                let mounted = element.clone();
                // Content fills the presence's box: what sizes the presence
                // (its own props) sizes it.
                element_view(
                    column().items_stretch().w_full().h_full().child(content),
                    element,
                    &low,
                    move |_| {
                        state.borrow_mut().elements.insert(id, mounted.clone());
                        if let (true, Some(enter)) = (animate_in, &enter)
                            && let Err(e) = play(&mounted, enter, None)
                        {
                            vm.report(e);
                        }
                    },
                )
            },
        )
        .w_full()
        .h_full(),
    )
}
