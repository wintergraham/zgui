//! Mounted nodes scripts can reach by `id`, `class` or `ref`: what selectors
//! find and what animations write. Each element carries the values its
//! animations hold. Compositor values (translation, opacity, scale, rotation,
//! skew) are written straight to the scene, once per frame and element; the
//! rest go through the node's reactive style. Both are also resolved over
//! the node's own styles whenever those re-run (hover, state), so an
//! animated value never snaps back.
use crate::node::{Lit, Prop};
use crate::transform::{LocalTransform, local_transform, set_local_transform};
use mlua::{UserData, UserDataFields, UserDataMethods};
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};
use zgui::{
    compose::NodeHandle,
    reactive::{Runtime, Signal},
    scene::{NodeId, Scene, Transform},
};

/// An input listener added to an element after it mounted.
pub(crate) type Listener = Rc<dyn Fn(&mut zgui::input::EventContext)>;

/// Animated values over the node's own styles. `None`: not animated.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Animated {
    pub translate: Option<(f32, f32)>,
    pub opacity: Option<f32>,
    /// Gaussian blur radius of the node's content, in points.
    pub blur: Option<f32>,
    /// Color filters: `[brightness, contrast, saturate]`.
    pub color: Option<[f32; 3]>,
    /// Scale, rotation and skew; the origin stays the node's own.
    pub local: Option<LocalTransform>,
    /// Where the node's mask sits, from its origin.
    pub mask: Option<(f32, f32)>,
    /// The clip inset, `[top, right, bottom, left]` in points.
    pub clip: Option<[f32; 4]>,
    /// A shader fill's time and parameters.
    pub shader: Option<zgui::decoration::ShaderUniforms>,
    pub styles: Vec<(Prop, Lit)>,
}

/// A node's literal translation and local transform.
type BaseTransform = (Option<(f32, f32)>, Option<LocalTransform>);

pub(crate) struct Element {
    handle: OnceCell<NodeHandle>,
    pub id: Option<Rc<str>>,
    pub classes: Rc<[Rc<str>]>,
    /// The element's kind (`"button"`, `"text"`…), for type selectors.
    pub tag: &'static str,
    pub animated: RefCell<Animated>,
    /// Bumped when animated style values change: the node's reactive style
    /// reads it. Compositor values do not need it.
    pub revision: Signal<u64>,
    styles_changed: Cell<bool>,
    compositor_changed: Cell<bool>,
    /// Queued for this frame's flush.
    queued: Cell<bool>,
    /// The animation that owns each animated property (`replace`
    /// composition): later animations take a property over.
    pub owners: RefCell<Vec<(crate::anime::Key, crate::anime::engine::Owner)>>,
    /// The scene, kept weakly so a frame's flush groups elements by window
    /// without upgrading the UI per element.
    scene: OnceCell<std::rc::Weak<RefCell<Scene>>>,
    /// Pointer and key listeners added after mount (`createDraggable`).
    pub listeners: RefCell<Vec<Listener>>,
    /// A scroll view's offset (points), for scroll observers.
    pub scroll: OnceCell<(Signal<f32>, bool)>,
    /// What the node's literals set: where an animation starts from before
    /// the scene has them.
    base: Cell<BaseTransform>,
    /// The literal opacity and content blur.
    base_effects: Cell<(f32, f32)>,
    /// The literal color filters.
    base_color: Cell<[f32; 3]>,
    /// The literal corner radii.
    base_corners: Cell<Option<zgui::decoration::Corners>>,
    /// The literal clip inset.
    base_clip: Cell<Option<[f32; 4]>>,
    /// The literal shader parameters.
    base_shader: Cell<Option<zgui::decoration::ShaderUniforms>>,
}
impl Element {
    pub fn new(
        runtime: &Runtime,
        id: Option<Rc<str>>,
        classes: Rc<[Rc<str>]>,
        tag: &'static str,
    ) -> Rc<Self> {
        Rc::new(Self {
            handle: OnceCell::new(),
            id,
            classes,
            tag,
            animated: RefCell::default(),
            revision: runtime.signal(0),
            styles_changed: Cell::new(false),
            compositor_changed: Cell::new(false),
            queued: Cell::new(false),
            owners: RefCell::default(),
            scene: OnceCell::new(),
            listeners: RefCell::default(),
            scroll: OnceCell::new(),
            base: Cell::new((None, None)),
            base_effects: Cell::new((1., 0.)),
            base_color: Cell::new([1.; 3]),
            base_corners: Cell::new(None),
            base_clip: Cell::new(None),
            base_shader: Cell::new(None),
        })
    }
    pub fn node(&self) -> Option<NodeId> {
        self.handle.get().map(NodeHandle::node)
    }
    /// Sets an input's selection once the handler's writes have settled, so
    /// text given in the same handler is what gets selected.
    fn select(&self, event: zgui::input::InputEvent) -> bool {
        let Some(handle) = self.handle.get().cloned() else {
            return false;
        };
        let Some(runtime) = handle.runtime() else {
            return false;
        };
        runtime.defer(move || {
            handle.dispatch(event);
        });
        true
    }
    pub fn scene(&self) -> Option<Rc<RefCell<Scene>>> {
        let scene = self.scene.get()?.upgrade()?;
        let contains = scene.borrow().contains(self.node()?);
        contains.then_some(scene)
    }
    pub fn scene_weak(&self) -> Option<&std::rc::Weak<RefCell<Scene>>> {
        self.scene.get()
    }
    pub fn is_mounted(&self) -> bool {
        self.handle.get().is_some_and(NodeHandle::is_mounted)
    }
    /// Mounted once and since removed: it will not come back.
    pub fn is_gone(&self) -> bool {
        self.handle.get().is_some_and(|h| !h.is_mounted())
    }
    /// The current translation: animated, or the node's own.
    pub fn translate(&self) -> (f32, f32) {
        if let Some(t) = self.animated.borrow().translate {
            return t;
        }
        if let Some(t) = self.base.get().0 {
            return t;
        }
        self.read(|scene, id| {
            let t = scene.transform(id);
            (t.x, t.y)
        })
        .unwrap_or_default()
    }
    pub fn opacity(&self) -> f32 {
        if let Some(o) = self.animated.borrow().opacity {
            return o;
        }
        self.read(|scene, id| scene.effects(id).opacity)
            .unwrap_or(1.)
    }
    pub fn blur(&self) -> f32 {
        if let Some(b) = self.animated.borrow().blur {
            return b;
        }
        self.read(|scene, id| scene.effects(id).filter_blur)
            .unwrap_or(0.)
    }
    pub fn set_blur(&self, b: f32) {
        let mut animated = self.animated.borrow_mut();
        if animated.blur != Some(b) {
            animated.blur = Some(b);
            self.compositor_changed.set(true);
        }
    }
    pub fn mask_offset(&self) -> (f32, f32) {
        if let Some(m) = self.animated.borrow().mask {
            return m;
        }
        self.read(|scene, id| scene.mask(id).map_or((0., 0.), |m| m.offset))
            .unwrap_or_default()
    }
    /// The clip inset now, in points (`[top, right, bottom, left]`).
    pub fn clip_inset(&self) -> [f32; 4] {
        if let Some(c) = self.animated.borrow().clip {
            return c;
        }
        if let Some(c) = self.base_clip.get() {
            return c;
        }
        self.read(|scene, id| {
            let b = scene.bounds(id);
            scene.clip_inset(id).map_or([0.; 4], |c| {
                let px = |l: zgui::layout::Length, basis: f32| match l {
                    zgui::layout::Length::Px(v) => v,
                    zgui::layout::Length::Percent(f) => f * basis,
                    _ => 0.,
                };
                [
                    px(c.top, b.height),
                    px(c.right, b.width),
                    px(c.bottom, b.height),
                    px(c.left, b.width),
                ]
            })
        })
        .unwrap_or_default()
    }
    pub fn set_clip_side(&self, side: usize, value: f32) {
        let current = self.animated.borrow().clip;
        // The other sides start from where the clip is now.
        let mut clip = current.unwrap_or_else(|| self.clip_inset());
        if current.is_some() && clip[side] == value {
            return;
        }
        clip[side] = value;
        self.animated.borrow_mut().clip = Some(clip);
        self.compositor_changed.set(true);
    }
    pub fn set_mask_offset(&self, offset: (f32, f32)) {
        let mut animated = self.animated.borrow_mut();
        if animated.mask != Some(offset) {
            animated.mask = Some(offset);
            self.compositor_changed.set(true);
        }
    }
    pub fn local(&self) -> LocalTransform {
        if let Some(l) = self.animated.borrow().local {
            return l;
        }
        if let Some(l) = self.base.get().1 {
            return l;
        }
        self.read(local_transform).unwrap_or_default()
    }
    pub fn base_corners(&self) -> Option<zgui::decoration::Corners> {
        self.base_corners.get()
    }
    pub fn set_base_color(&self, color: [f32; 3]) {
        self.base_color.set(color);
    }
    /// The color filters now, `[brightness, contrast, saturate]`.
    pub fn color(&self) -> [f32; 3] {
        self.animated
            .borrow()
            .color
            .unwrap_or_else(|| self.base_color.get())
    }
    pub fn set_color_part(&self, part: usize, value: f32) {
        let mut color = self.color();
        if self.animated.borrow().color.is_some() && color[part] == value {
            return;
        }
        color[part] = value;
        self.animated.borrow_mut().color = Some(color);
        self.compositor_changed.set(true);
    }
    pub fn set_base_shader(&self, uniforms: Option<zgui::decoration::ShaderUniforms>) {
        self.base_shader.set(uniforms);
    }
    /// The shader fill's uniforms now.
    pub fn shader_uniforms(&self) -> zgui::decoration::ShaderUniforms {
        if let Some(u) = self.animated.borrow().shader {
            return u;
        }
        if let Some(u) = self.base_shader.get() {
            return u;
        }
        self.read(|scene, id| scene.shader_uniforms(id))
            .unwrap_or_default()
    }
    /// Sets the time (`slot` 0) or one of the sixteen parameters (1 to 16).
    pub fn set_shader_slot(&self, slot: usize, value: f32) {
        let current = self.animated.borrow().shader;
        let mut u = current.unwrap_or_else(|| self.shader_uniforms());
        let target = match slot {
            0 => &mut u.time,
            n => &mut u.params[(n - 1) / 4][(n - 1) % 4],
        };
        if current.is_some() && *target == value {
            return;
        }
        *target = value;
        self.animated.borrow_mut().shader = Some(u);
        self.compositor_changed.set(true);
    }
    pub fn set_base_clip(&self, clip: Option<[f32; 4]>) {
        self.base_clip.set(clip);
    }
    pub fn set_base_corners(&self, corners: Option<zgui::decoration::Corners>) {
        self.base_corners.set(corners);
    }
    pub fn set_base(
        &self,
        translate: Option<(f32, f32)>,
        local: Option<LocalTransform>,
        effects: (f32, f32),
    ) {
        self.base.set((translate, local));
        self.base_effects.set(effects);
    }
    pub fn read<R>(&self, f: impl FnOnce(&Scene, NodeId) -> R) -> Option<R> {
        let scene = self.scene()?;
        let scene = scene.borrow();
        Some(f(&scene, self.node()?))
    }
    pub fn set_translate(&self, t: (f32, f32)) {
        let mut animated = self.animated.borrow_mut();
        if animated.translate != Some(t) {
            animated.translate = Some(t);
            self.compositor_changed.set(true);
        }
    }
    pub fn set_opacity(&self, o: f32) {
        let mut animated = self.animated.borrow_mut();
        if animated.opacity != Some(o) {
            animated.opacity = Some(o);
            self.compositor_changed.set(true);
        }
    }
    pub fn set_local(&self, l: LocalTransform) {
        let mut animated = self.animated.borrow_mut();
        if animated.local != Some(l) {
            animated.local = Some(l);
            self.compositor_changed.set(true);
        }
    }
    pub fn set_style(&self, prop: Prop, lit: Lit) {
        let mut animated = self.animated.borrow_mut();
        match animated.styles.iter_mut().find(|(p, _)| *p == prop) {
            Some((_, old)) if *old == lit => return,
            Some((_, old)) => *old = lit,
            None => animated.styles.push((prop, lit)),
        }
        self.styles_changed.set(true);
    }
    /// Forget the animated values of `keys` (all of them for `None`): each
    /// returns to what the node's own literals set, as removing an inline
    /// style lets the stylesheet's value show again.
    pub fn clear_only(&self, keys: Option<&[crate::anime::Key]>) {
        use crate::anime::Key;
        let wants = |k: &Key| keys.is_none_or(|keys| keys.contains(k));
        let any = |ks: &[Key]| ks.iter().any(&wants);
        let cleared = {
            let mut animated = self.animated.borrow_mut();
            let mut cleared = Animated::default();
            if any(&[Key::X, Key::Y]) {
                cleared.translate = animated.translate.take();
            }
            if wants(&Key::Opacity) {
                cleared.opacity = animated.opacity.take();
            }
            if wants(&Key::Blur) {
                cleared.blur = animated.blur.take();
            }
            if any(&[Key::Brightness, Key::Contrast, Key::Saturate]) {
                cleared.color = animated.color.take();
            }
            if any(&[
                Key::Scale,
                Key::ScaleX,
                Key::ScaleY,
                Key::Rotate,
                Key::Skew,
                Key::SkewX,
                Key::SkewY,
            ]) {
                cleared.local = animated.local.take();
            }
            if any(&[Key::MaskX, Key::MaskY]) {
                cleared.mask = animated.mask.take();
            }
            if any(&[Key::ClipTop, Key::ClipRight, Key::ClipBottom, Key::ClipLeft]) {
                cleared.clip = animated.clip.take();
            }
            if keys.is_none_or(|keys| keys.iter().any(|k| matches!(k, Key::Shader(_)))) {
                cleared.shader = animated.shader.take();
            }
            let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut animated.styles)
                .into_iter()
                .partition(|(p, _)| wants(&Key::Style(*p)));
            animated.styles = kept;
            cleared.styles = gone;
            cleared
        };
        // The animations that owned these properties stop writing them.
        self.owners.borrow_mut().retain(|(key, owner)| {
            let clears = match key {
                Key::Style(p) => cleared.styles.iter().any(|(q, _)| q == p),
                Key::X | Key::Y => cleared.translate.is_some(),
                Key::Opacity => cleared.opacity.is_some(),
                Key::Blur => cleared.blur.is_some(),
                Key::Brightness | Key::Contrast | Key::Saturate => cleared.color.is_some(),
                Key::MaskX | Key::MaskY => cleared.mask.is_some(),
                Key::ClipTop | Key::ClipRight | Key::ClipBottom | Key::ClipLeft => {
                    cleared.clip.is_some()
                }
                Key::Field(_) => false,
                _ => cleared.local.is_some(),
            };
            if clears {
                owner.track.set(true);
            }
            !clears
        });
        let (base_translate, base_local) = self.base.get();
        if let (Some(scene), Some(id)) = (self.scene(), self.node()) {
            let mut scene = scene.borrow_mut();
            if cleared.translate.is_some() {
                let (x, y) = base_translate.unwrap_or_default();
                scene.set_transform(id, Transform { x, y });
            }
            if cleared.opacity.is_some() || cleared.blur.is_some() || cleared.color.is_some() {
                let (opacity, blur) = self.base_effects.get();
                let mut effects = scene.effects(id);
                if cleared.color.is_some() {
                    let [brightness, contrast, saturate] = self.base_color.get();
                    effects.brightness = brightness;
                    effects.contrast = contrast;
                    effects.saturate = saturate;
                }
                if cleared.opacity.is_some() {
                    effects.opacity = opacity;
                }
                if cleared.blur.is_some() {
                    effects.filter_blur = blur;
                }
                scene.set_effects(id, effects);
            }
            if cleared.local.is_some() {
                let origin = local_transform(&scene, id).origin;
                let local = base_local.unwrap_or(LocalTransform {
                    origin,
                    ..LocalTransform::IDENTITY
                });
                set_local_transform(&mut scene, id, local);
            }
        }
        // The node's style re-runs and restores what it sets itself.
        self.revision.update(|r| *r += 1);
    }
    /// Queue for `flush`; returns whether it was not already queued.
    pub fn queue(&self) -> bool {
        !self.queued.replace(true)
    }
    /// Drops queued writes for an element no longer in a scene.
    pub fn flush_detached(&self) {
        self.queued.set(false);
        self.compositor_changed.set(false);
        self.styles_changed.set(false);
    }
    /// Writes this frame's compositor values to the scene. Style changes are
    /// returned to bump once the scene is released.
    pub fn flush(&self, scene: &mut Scene) -> bool {
        self.queued.set(false);
        if self.compositor_changed.replace(false)
            && let Some(id) = self.node()
            && scene.contains(id)
        {
            let animated = self.animated.borrow();
            if let Some((x, y)) = animated.translate {
                scene.set_transform(id, Transform { x, y });
            }
            if animated.opacity.is_some() || animated.blur.is_some() || animated.color.is_some() {
                let mut effects = scene.effects(id);
                if let Some([brightness, contrast, saturate]) = animated.color {
                    effects.brightness = brightness;
                    effects.contrast = contrast;
                    effects.saturate = saturate;
                }
                if let Some(opacity) = animated.opacity {
                    effects.opacity = opacity;
                }
                // `blur` is the content's blur; `blur_radius` is the backdrop's.
                if let Some(blur) = animated.blur {
                    effects.filter_blur = blur;
                }
                scene.set_effects(id, effects);
            }
            if let Some(local) = animated.local {
                set_local_transform(scene, id, local);
            }
            if let Some(offset) = animated.mask {
                scene.set_mask_offset(id, offset);
            }
            if let Some([t, r, b, l]) = animated.clip {
                scene.set_clip_inset(id, Some(zgui::scene::ClipInset::px(t, r, b, l)));
            }
            if let Some(uniforms) = animated.shader {
                scene.set_shader_uniforms(id, uniforms);
            }
        }
        self.styles_changed.replace(false)
    }
}

/// Every targetable element, by id and class.
#[derive(Default)]
pub(crate) struct Elements {
    /// Set while work waits for elements to mount; a registration then
    /// wakes the engine once to retry it.
    watching: Cell<bool>,
    mounted: Cell<bool>,
    wake: RefCell<Option<Rc<dyn Fn()>>>,
    by_node: RefCell<HashMap<NodeId, Weak<Element>>>,
    ids: RefCell<HashMap<Rc<str>, Vec<Weak<Element>>>>,
    classes: RefCell<HashMap<Rc<str>, Vec<Weak<Element>>>>,
    tags: RefCell<HashMap<&'static str, Vec<Weak<Element>>>>,
}
impl Elements {
    pub fn set_wake(&self, wake: Rc<dyn Fn()>) {
        *self.wake.borrow_mut() = Some(wake);
    }
    pub fn watch(&self, watching: bool) {
        self.watching.set(watching);
        self.mounted.set(false);
    }
    /// Whether an element mounted since work started waiting; clears it.
    pub fn mounted_since_watch(&self) -> bool {
        self.watching.get() && self.mounted.replace(false)
    }
    /// The element mounted at `node`, if it is targetable.
    pub fn at(&self, node: NodeId) -> Option<Rc<Element>> {
        self.by_node.borrow().get(&node).and_then(Weak::upgrade)
    }
    /// Registers a mounted element until `handle`'s node is removed.
    pub fn register(self: &Rc<Self>, element: &Rc<Element>, handle: NodeHandle) {
        self.by_node
            .borrow_mut()
            .insert(handle.node(), Rc::downgrade(element));
        if self.watching.get() && !self.mounted.replace(true) {
            let wake = self.wake.borrow().clone();
            if let Some(wake) = wake {
                wake();
            }
        }
        let _ = element.handle.set(handle.clone());
        if let Some(scene) = handle.scene() {
            let _ = element.scene.set(Rc::downgrade(&scene));
        }
        let weak = Rc::downgrade(element);
        if let Some(id) = &element.id {
            self.ids
                .borrow_mut()
                .entry(id.clone())
                .or_default()
                .push(weak.clone());
        }
        for class in element.classes.iter() {
            self.classes
                .borrow_mut()
                .entry(class.clone())
                .or_default()
                .push(weak.clone());
        }
        self.tags
            .borrow_mut()
            .entry(element.tag)
            .or_default()
            .push(weak.clone());
        handle.retain(Registration {
            elements: Rc::downgrade(self),
            element: element.clone(),
        });
    }
    fn unregister(&self, element: &Rc<Element>) {
        let ptr = Rc::as_ptr(element);
        let remove = |map: &mut HashMap<_, Vec<Weak<Element>>>, key| {
            if let Some(list) = map.get_mut(key) {
                list.retain(|e| e.as_ptr() != ptr && e.strong_count() > 0);
                if list.is_empty() {
                    map.remove(key);
                }
            }
        };
        if let Some(id) = &element.id {
            remove(&mut self.ids.borrow_mut(), id);
        }
        for class in element.classes.iter() {
            remove(&mut self.classes.borrow_mut(), class);
        }
        if let Some(list) = self.tags.borrow_mut().get_mut(element.tag) {
            list.retain(|e| e.as_ptr() != ptr && e.strong_count() > 0);
        }
        if let Some(node) = element.node() {
            self.by_node.borrow_mut().remove(&node);
        }
    }
    /// Elements matching a selector list: `#id`, `.class`, a kind such as
    /// `button`, `*`, comma separated, in document order without repeats.
    pub fn select(&self, selector: &str) -> Vec<Rc<Element>> {
        let mut found: Vec<Rc<Element>> = Vec::new();
        for part in selector.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            // Compound selectors (`button.primary`) match all their parts.
            let (tag, rest) = match part.find(['#', '.']) {
                Some(0) => (None, part),
                Some(i) => (Some(&part[..i]), &part[i..]),
                None => (Some(part), ""),
            };
            let mut ids = Vec::new();
            let mut classes = Vec::new();
            let mut cursor = rest;
            while let Some(c) = cursor.chars().next() {
                let end = cursor[1..].find(['#', '.']).map_or(cursor.len(), |i| i + 1);
                let name = &cursor[1..end];
                if c == '#' {
                    ids.push(name);
                } else {
                    classes.push(name);
                }
                cursor = &cursor[end..];
            }
            let candidates: Vec<Weak<Element>> = if let Some(id) = ids.first() {
                self.ids.borrow().get(*id).cloned().unwrap_or_default()
            } else if let Some(class) = classes.first() {
                self.classes
                    .borrow()
                    .get(*class)
                    .cloned()
                    .unwrap_or_default()
            } else if let Some(tag) = tag.filter(|t| *t != "*") {
                self.tags.borrow().get(tag).cloned().unwrap_or_default()
            } else {
                self.tags.borrow().values().flatten().cloned().collect()
            };
            for element in candidates.iter().filter_map(Weak::upgrade) {
                let matches = element.is_mounted()
                    && tag.is_none_or(|t| t == "*" || t == element.tag)
                    && ids.iter().all(|id| element.id.as_deref() == Some(*id))
                    && classes
                        .iter()
                        .all(|c| element.classes.iter().any(|own| &**own == *c));
                if matches && !found.iter().any(|f| Rc::ptr_eq(f, &element)) {
                    found.push(element);
                }
            }
        }
        sort_by_document_order(&mut found);
        found
    }
}

/// Orders elements as they appear in the tree (depth first), the order
/// anime.js staggers selector matches in.
pub(crate) fn sort_by_document_order(elements: &mut [Rc<Element>]) {
    if elements.len() < 2 {
        return;
    }
    let Some(scene) = elements.iter().find_map(|e| e.scene()) else {
        return;
    };
    let scene = scene.borrow();
    let path = |element: &Rc<Element>| {
        let mut path = Vec::new();
        let mut node = element.node();
        while let Some(id) = node {
            let parent = scene.parent(id);
            if let Some(parent) = parent {
                path.push(
                    scene
                        .children(parent)
                        .iter()
                        .position(|c| *c == id)
                        .unwrap_or(0),
                );
            }
            node = parent;
        }
        path.reverse();
        path
    };
    let mut keyed: Vec<(Vec<usize>, Rc<Element>)> =
        elements.iter().map(|e| (path(e), e.clone())).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    for (slot, (_, element)) in elements.iter_mut().zip(keyed) {
        *slot = element;
    }
}

struct Registration {
    elements: Weak<Elements>,
    element: Rc<Element>,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(elements) = self.elements.upgrade() {
            elements.unregister(&self.element);
        }
    }
}

/// `ui.ref()`: the elements a node or nodes with `ref = r` mounted.
#[derive(Default)]
pub(crate) struct Ref {
    elements: RefCell<Vec<Weak<Element>>>,
}
impl Ref {
    pub fn attach(&self, element: &Rc<Element>) {
        let mut elements = self.elements.borrow_mut();
        elements.retain(|e| e.upgrade().is_some_and(|e| e.is_mounted()));
        elements.push(Rc::downgrade(element));
    }
    pub fn elements(&self) -> Vec<Rc<Element>> {
        let mut live: Vec<_> = self
            .elements
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|e| e.is_mounted())
            .collect();
        sort_by_document_order(&mut live);
        live
    }
}
#[derive(Clone)]
pub(crate) struct RefUd(pub Rc<Ref>);
impl UserData for RefUd {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("count", |_, r| Ok(r.0.elements().len()));
        fields.add_field_method_get("mounted", |_, r| Ok(!r.0.elements().is_empty()));
    }
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("elements", |lua, r, ()| {
            lua.create_sequence_from(r.0.elements().into_iter().map(ElementUd))
        });
    }
}

/// An element handed to scripts: function-based animation values receive
/// their target as one.
#[derive(Clone)]
pub(crate) struct ElementUd(pub Rc<Element>);
impl UserData for ElementUd {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, e| Ok(e.0.id.as_deref().map(str::to_owned)));
        fields.add_field_method_get("tag", |_, e| Ok(e.0.tag));
        fields.add_field_method_get("mounted", |_, e| Ok(e.0.is_mounted()));
        fields.add_field_method_get("width", |_, e| Ok(e.0.read(|s, id| s.bounds(id).width)));
        fields.add_field_method_get("height", |_, e| Ok(e.0.read(|s, id| s.bounds(id).height)));
        fields.add_field_method_get("x", |_, e| Ok(e.0.read(|s, id| s.bounds(id).x)));
        fields.add_field_method_get("y", |_, e| Ok(e.0.read(|s, id| s.bounds(id).y)));
        fields.add_field_method_get("focused", |_, e| {
            Ok(e.0.handle.get().is_some_and(NodeHandle::is_focused))
        });
    }
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("has_class", |_, e, class: String| {
            Ok(e.0.classes.iter().any(|c| **c == *class))
        });
        // Keyboard focus, as a click or Tab gives it (an input, a button),
        // once the handler's writes have settled: an input the same handler
        // reveals can take it.
        methods.add_method("focus", |_, e, ()| {
            let Some(handle) = e.0.handle.get().cloned() else {
                return Ok(false);
            };
            let Some(runtime) = handle.runtime() else {
                return Ok(false);
            };
            runtime.defer(move || {
                handle.focus();
            });
            Ok(true)
        });
        methods.add_method("blur", |_, e, ()| {
            Ok(e.0.handle.get().is_some_and(NodeHandle::blur))
        });
        // An input's selection, in bytes of its text: `select(0, #text)`, or a
        // caret with one argument. Out-of-range offsets are clamped.
        methods.add_method("select", |_, e, (anchor, focus): (usize, Option<usize>)| {
            let event = zgui::input::InputEvent::SetTextSelection {
                anchor,
                focus: focus.unwrap_or(anchor),
            };
            Ok(e.0.select(event))
        });
        methods.add_method("select_all", |_, e, ()| {
            Ok(e.0.select(zgui::input::InputEvent::SetTextSelection {
                anchor: 0,
                focus: usize::MAX,
            }))
        });
    }
}
