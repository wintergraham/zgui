//! Retained scene graph. Mutations invalidate only their layout/paint/compositor dependencies.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    index: u32,
    generation: u32,
}
/// Separate damage rectangles a flush keeps before marking damage on a grid
/// of `DAMAGE_CELL` cells instead (see `DamageGrid`).
const DAMAGE_RECTS: usize = 64;
/// The side of a damage grid cell, in viewport units.
const DAMAGE_CELL: f32 = 8.;
/// Damage past `DAMAGE_RECTS` separate places (a busy dashboard changes
/// hundreds a frame): marking cells costs the same however many there are,
/// where merging into a list scans it each time. Rectangles again at flush.
struct DamageGrid {
    columns: usize,
    rows: usize,
    cells: Vec<bool>,
    marked: usize,
}
impl DamageGrid {
    fn new(viewport: Rect) -> Self {
        let columns = (viewport.width / DAMAGE_CELL).ceil().max(1.) as usize;
        let rows = (viewport.height / DAMAGE_CELL).ceil().max(1.) as usize;
        Self {
            columns,
            rows,
            cells: vec![false; columns * rows],
            marked: 0,
        }
    }
    /// The cells `rect` touches: columns and rows, end exclusive.
    fn span(&self, rect: Rect) -> (usize, usize, usize, usize) {
        let first = |value: f32| (value / DAMAGE_CELL).floor().max(0.) as usize;
        let end =
            |value: f32, count: usize| ((value / DAMAGE_CELL).ceil().max(0.) as usize).min(count);
        (
            first(rect.x).min(self.columns),
            first(rect.y).min(self.rows),
            end(rect.x + rect.width, self.columns),
            end(rect.y + rect.height, self.rows),
        )
    }
    fn mark(&mut self, rect: Rect) {
        let (left, top, right, bottom) = self.span(rect);
        for row in top..bottom {
            for cell in &mut self.cells[row * self.columns + left..row * self.columns + right] {
                self.marked += usize::from(!*cell);
                *cell = true;
            }
        }
    }
    fn touches(&self, rect: Rect) -> bool {
        let (left, top, right, bottom) = self.span(rect);
        (top..bottom).any(|row| {
            self.cells[row * self.columns + left..row * self.columns + right].contains(&true)
        })
    }
    fn full(&self) -> bool {
        self.marked == self.cells.len()
    }
    /// Each row's runs of marked cells, a run repeated straight down the
    /// next rows joining them as one rectangle.
    fn rects(&self, viewport: Rect) -> Vec<Rect> {
        let mut done = Vec::new();
        // Runs still growing down: first and end column, first row.
        let (mut open, mut next) = (Vec::<(usize, usize, usize)>::new(), Vec::new());
        for row in 0..=self.rows {
            let cells = if row < self.rows {
                &self.cells[row * self.columns..(row + 1) * self.columns]
            } else {
                &[][..]
            };
            let mut column = 0;
            while column < cells.len() {
                if !cells[column] {
                    column += 1;
                    continue;
                }
                let start = column;
                while column < cells.len() && cells[column] {
                    column += 1;
                }
                let top = open
                    .iter()
                    .find(|(left, right, _)| (*left, *right) == (start, column))
                    .map_or(row, |run| run.2);
                next.push((start, column, top));
            }
            for &(left, right, top) in &open {
                if !next.iter().any(|run| (run.0, run.1) == (left, right)) {
                    let cell = |index: usize| index as f32 * DAMAGE_CELL;
                    let rect =
                        Rect::new(cell(left), cell(top), cell(right - left), cell(row - top));
                    done.extend(rect.intersection(viewport));
                }
            }
            std::mem::swap(&mut open, &mut next);
            next.clear();
        }
        done
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = (self.x + self.width).min(other.x + other.width) - x;
        let h = (self.y + self.height).min(other.y + other.height) - y;
        (w > 0.0 && h > 0.0).then(|| Self::new(x, y, w, h))
    }
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self::new(
            x,
            y,
            (self.x + self.width).max(other.x + other.width) - x,
            (self.y + self.height).max(other.y + other.height) - y,
        )
    }
    pub fn expand(self, by: f32) -> Self {
        Self::new(
            self.x - by,
            self.y - by,
            self.width + 2.0 * by,
            self.height + 2.0 * by,
        )
    }
    fn translated(self, t: Transform) -> Self {
        Self::new(self.x + t.x, self.y + t.y, self.width, self.height)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
}
/// Scale, rotation and skew about an origin, applied after layout and on
/// top of the node's translation (`Transform`). Identity by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalTransform {
    pub scale: (f32, f32),
    /// Radians, clockwise on screen (positive Y down).
    pub rotate: f32,
    /// Radians, along x then y, as CSS `skew(x, y)`.
    pub skew: (f32, f32),
    /// The pivot as a fraction of the node's own laid-out size; (0.5, 0.5) is its centre (CSS default).
    pub origin: (f32, f32),
}
impl Default for LocalTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl LocalTransform {
    pub const IDENTITY: Self = Self {
        scale: (1., 1.),
        rotate: 0.,
        skew: (0., 0.),
        origin: (0.5, 0.5),
    };
    /// Whether it moves nothing, whatever its origin.
    pub fn is_identity(&self) -> bool {
        self.scale == (1., 1.) && self.rotate == 0. && self.skew == (0., 0.)
    }
    /// The matrix for a node at `bounds` (layout rect, untranslated): translate(origin) · rotate · skew · scale · translate(-origin)
    pub fn matrix(&self, bounds: Rect) -> crate::affine::Affine {
        use crate::affine::Affine;
        let skew = Affine {
            b: self.skew.1.tan(),
            c: self.skew.0.tan(),
            ..Affine::IDENTITY
        };
        Affine::scale(self.scale.0, self.scale.1)
            .then(skew)
            .then(Affine::rotation(self.rotate))
            .around(
                bounds.x + bounds.width * self.origin.0,
                bounds.y + bounds.height * self.origin.1,
            )
    }
    /// Each nonfinite component takes its identity value, so equal writes
    /// compare equal (NaN never would).
    fn normalized(self) -> Self {
        let finite = |value: f32, default: f32| if value.is_finite() { value } else { default };
        Self {
            scale: (finite(self.scale.0, 1.), finite(self.scale.1, 1.)),
            rotate: finite(self.rotate, 0.),
            skew: (finite(self.skew.0, 0.), finite(self.skew.1, 0.)),
            origin: (finite(self.origin.0, 0.5), finite(self.origin.1, 0.5)),
        }
    }
    /// The larger axis scale: how much bigger than laid out its content shows.
    pub fn magnification(&self) -> f32 {
        self.scale.0.abs().max(self.scale.1.abs())
    }
}
/// A transformed or filtered ancestor's mapping of its layer's content onto
/// the viewport, how far a content blur spreads it there, and the viewport
/// clip around that layer (see `Scene::descend`).
#[derive(Clone, Copy)]
struct Placement {
    matrix: crate::affine::Affine,
    /// Viewport units: what a layer's content paints shows this much past
    /// its mapped box (nested blurs, each stretched by the matrices after it).
    spread: f32,
    clip: Rect,
}
impl Placement {
    /// Where pixels `rect` paints inside the layer can change on screen.
    fn shows(&self, rect: Rect) -> Option<Rect> {
        let mapped = self.matrix.bounds(rect);
        // Resampled (scaled, rotated off the right angles, skewed), its
        // filtering reaches up to a pixel past the mapped box.
        let m = self.matrix;
        let zero = |v: f32| v.abs() < 1e-4;
        let unit = |v: f32| (v.abs() - 1.).abs() < 1e-4;
        let resampled = !((unit(m.a) && zero(m.b) && zero(m.c) && unit(m.d))
            || (zero(m.a) && unit(m.b) && unit(m.c) && zero(m.d)));
        let spread = self.spread.max(0.) + if resampled { 1. } else { 0. };
        let mapped = if spread > 0. {
            mapped.expand(spread)
        } else {
            mapped
        };
        mapped.intersection(self.clip)
    }
}
/// How far a content blur of `sigma` spreads pixels: three standard
/// deviations, past which the Gaussian leaves under half an 8-bit step.
pub fn filter_spread(sigma: f32) -> f32 {
    3. * sigma
}
/// How far `matrix` moves a point that is one unit away, at most along an
/// axis: a spread inside a layer times this is its spread outside.
fn stretch(matrix: crate::affine::Affine) -> f32 {
    let linear = crate::affine::Affine {
        tx: 0.,
        ty: 0.,
        ..matrix
    };
    let unit = linear.bounds(Rect::new(-1., -1., 2., 2.));
    (unit.width.max(unit.height) / 2.).max(0.)
}
/// A clip inset (CSS `clip-path: inset(top right bottom left)`): the node
/// and its descendants paint only inside its box inset by these lengths,
/// pixels or fractions of its own laid-out size (`Length::Percent(1.)` is
/// 100%: top and bottom of its height, left and right of its width,
/// following it as it resizes). Negative insets reach past the box, so a
/// shadow or ring isn't cut. `Auto` is 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClipInset {
    pub top: crate::layout::Length,
    pub right: crate::layout::Length,
    pub bottom: crate::layout::Length,
    pub left: crate::layout::Length,
}
impl ClipInset {
    /// All four sides in pixels.
    pub fn px(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        use crate::layout::Length::Px;
        Self {
            top: Px(top),
            right: Px(right),
            bottom: Px(bottom),
            left: Px(left),
        }
    }
    /// The box `bounds` inset by these lengths; empty when they cross.
    pub fn apply(&self, bounds: Rect) -> Rect {
        let length = |length: crate::layout::Length, basis: f32| match length {
            crate::layout::Length::Px(value) if value.is_finite() => value,
            crate::layout::Length::Percent(fraction) if fraction.is_finite() => fraction * basis,
            _ => 0.,
        };
        let (top, bottom) = (
            length(self.top, bounds.height),
            length(self.bottom, bounds.height),
        );
        let (left, right) = (
            length(self.left, bounds.width),
            length(self.right, bounds.width),
        );
        Rect::new(
            bounds.x + left,
            bounds.y + top,
            (bounds.width - left - right).max(0.),
            (bounds.height - top - bottom).max(0.),
        )
    }
}
/// An alpha mask over a node (CSS `mask-image` with `mask-position` and
/// `mask-size`, no repeat): its subtree, rendered as a layer and after any
/// `Effects::filter_blur`, shows by the image's alpha, sampled nearest. The
/// image is drawn `size` large with its top-left `offset` from the node's
/// laid-out origin, in the node's own (untransformed) space; outside it
/// nothing shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageMask {
    pub image: Arc<crate::image::ImageData>,
    pub offset: (f32, f32),
    pub size: (f32, f32),
}
impl ImageMask {
    /// Nonfinite components zeroed, so equal writes compare equal.
    fn normalized(mut self) -> Self {
        let finite = |value: f32| if value.is_finite() { value } else { 0. };
        self.offset = (finite(self.offset.0), finite(self.offset.1));
        self.size = (finite(self.size.0).max(0.), finite(self.size.1).max(0.));
        self
    }
}
/// A clip that cuts nothing, as `clipped_bounds` starts from.
const UNCLIPPED: Rect = Rect {
    x: -f32::MAX / 4.,
    y: -f32::MAX / 4.,
    width: f32::MAX / 2.,
    height: f32::MAX / 2.,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    pub opacity: f32,
    /// Backdrop blur (CSS `backdrop-filter: blur()`), the Gaussian sigma in
    /// logical pixels: what lies behind the node, inside its bounds, blurs.
    /// Its own content stays sharp.
    pub blur_radius: f32,
    pub edge_fade: f32,
    /// Content blur (CSS `filter: blur()`), the Gaussian sigma in logical
    /// pixels: the node's own subtree, painted once as a cached layer,
    /// composites blurred, spreading `filter_spread` past its bounds. Opacity
    /// applies to the blurred result. Changing it composites again without
    /// repainting the subtree; at 0 the node paints as any other.
    pub filter_blur: f32,
    /// CSS `filter: brightness()`: the subtree's colours scaled (1 leaves
    /// them, 0 is black, past 1 brighter), as a group, under its opacity.
    pub brightness: f32,
    /// CSS `filter: contrast()`: colours pushed from (or towards) mid grey.
    pub contrast: f32,
    /// CSS `filter: saturate()`: 0 is greyscale, past 1 more vivid.
    pub saturate: f32,
}
impl Default for Effects {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            blur_radius: 0.0,
            edge_fade: 0.0,
            filter_blur: 0.0,
            brightness: 1.0,
            contrast: 1.0,
            saturate: 1.0,
        }
    }
}
/// An affine map of straight-alpha RGB: each row the weights of R, G, B
/// and an offset. Brightness, contrast and saturate filters compose into
/// one; being affine, it can be applied to each primitive of a subtree
/// before blending with the same result as to the subtree as a group.
pub type ColorMatrix = [[f32; 4]; 3];
impl Effects {
    /// Its colour filters as one matrix (brightness, then contrast, then
    /// saturate, as CSS lists them), or `None` when they change nothing.
    pub fn color_matrix(&self) -> Option<ColorMatrix> {
        if (self.brightness, self.contrast, self.saturate) == (1., 1., 1.) {
            return None;
        }
        let (b, k, s) = (self.brightness, self.contrast, self.saturate);
        // Contrast after brightness: (b·c - 0.5)·k + 0.5.
        let (scale, offset) = (b * k, 0.5 - 0.5 * k);
        // CSS's saturate matrix (Rec. 709 luma weights).
        let saturate = [
            [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
        ];
        Some(saturate.map(|row| {
            [
                row[0] * scale,
                row[1] * scale,
                row[2] * scale,
                (row[0] + row[1] + row[2]) * offset,
            ]
        }))
    }
}
/// `outer` applied after `inner`: a filtered subtree inside a filtered one.
pub fn compose_color(
    outer: Option<ColorMatrix>,
    inner: Option<ColorMatrix>,
) -> Option<ColorMatrix> {
    let (outer, inner) = match (outer, inner) {
        (None, inner) => return inner,
        (outer, None) => return outer,
        (Some(outer), Some(inner)) => (outer, inner),
    };
    Some(std::array::from_fn(|row| {
        let o = outer[row];
        let column = |c: usize| o[0] * inner[0][c] + o[1] * inner[1][c] + o[2] * inner[2][c];
        [column(0), column(1), column(2), column(3) + o[3]]
    }))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Row,
    Column,
    Overlay,
}
/// A rectangular outer shadow. Blur radius is the Gaussian sigma in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShadow {
    pub color: Color,
    pub offset: Transform,
    pub blur_radius: f32,
    pub spread: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct QuadStyle {
    pub fill: Color,
    pub radius: f32,
    pub border_color: Color,
    pub border_width: f32,
    pub shadow: Option<BoxShadow>,
    pub decoration: Option<Arc<crate::decoration::Decoration>>,
    /// Paint the border over the node's descendants (an inset stroke, as a
    /// CSS outline drawn inside): the fill paints first, the border after
    /// everything inside (`PaintItem::border_pass`).
    pub border_over: bool,
    /// An outline around the box (CSS `outline` and `outline-offset`).
    pub outline: Option<Outline>,
}
/// An outline (CSS `outline`, `outline-offset`): a ring `width` wide drawn
/// `offset` outside the box (inside it when negative), its corners the
/// box's radii grown by as much, over the node's descendants. It takes no
/// room in layout. A design tool's stroke aligned outside is offset 0;
/// centred, offset `-width / 2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub width: f32,
    pub color: Color,
    pub offset: f32,
}
impl Outline {
    /// Whether it draws anything.
    pub fn visible(&self) -> bool {
        self.width.is_finite() && self.width > 0. && self.offset.is_finite() && self.color.3 > 0
    }
    /// The box it rings for a node at `bounds`: its outer edge.
    pub fn outer(&self, bounds: Rect) -> Rect {
        bounds.expand(self.offset + self.width)
    }
}
impl Default for QuadStyle {
    fn default() -> Self {
        Self {
            fill: Color(0, 0, 0, 0),
            radius: 0.,
            border_color: Color(0, 0, 0, 0),
            border_width: 0.,
            shadow: None,
            decoration: None,
            border_over: false,
            outline: None,
        }
    }
}
impl QuadStyle {
    /// Its border's widths (top, right, bottom, left): per side from the
    /// decoration, or `border_width` all round.
    pub fn border_sides(&self) -> Insets {
        self.decoration
            .as_ref()
            .and_then(|detail| detail.border_widths)
            .unwrap_or(Insets::all(self.border_width))
    }
    /// Whether it has a visible border.
    pub fn has_border(&self) -> bool {
        let sides = self.border_sides();
        self.border_color.3 > 0
            && [sides.top, sides.right, sides.bottom, sides.left]
                .into_iter()
                .any(|width| width.is_finite() && width > 0.)
    }
    pub fn shadows(&self) -> &[BoxShadow] {
        self.decoration
            .as_ref()
            .and_then(|d| d.shadows.as_deref())
            .unwrap_or(self.shadow.as_slice())
    }
    pub fn paint_bounds(&self, bounds: Rect) -> Rect {
        let painted = self.shadows().iter().fold(bounds, |paint, shadow| {
            paint.union(
                bounds
                    .expand(shadow.spread.max(0.) + shadow.blur_radius.max(0.) * 3.)
                    .translated(shadow.offset),
            )
        });
        match self.outline.filter(Outline::visible) {
            Some(outline) => painted.union(outline.outer(bounds)),
            None => painted,
        }
    }
    /// Whether it paints a second time after its descendants (its border
    /// over them, its outline).
    pub fn paints_over(&self) -> bool {
        (self.border_over && self.has_border()) || self.outline.is_some_and(|o| o.visible())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Container(Layout),
    /// A decorated container; its background does not participate in child layout.
    Panel {
        layout: Layout,
        quad: QuadStyle,
    },
    Rect(Color),
    Quad(QuadStyle),
    Image(Arc<crate::image::ImageData>),
    Svg(Arc<crate::svg::SvgData>),
    Canvas(Arc<crate::canvas::Canvas>),
    #[cfg(target_os = "macos")]
    NativeSurface(std::rc::Rc<crate::native_surface::NativeSurface>),
    RichText {
        text: Arc<crate::rich_text::RichText>,
    },
    Text {
        text: Arc<str>,
        color: Color,
        font_size: f32,
    },
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
impl Insets {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Baseline,
    #[default]
    Start,
    Center,
    End,
    Stretch,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}
/// A platform text shaper can provide accurate intrinsic and wrapped text metrics.
pub trait TextMeasurer {
    fn measure(&self, text: &str, font_size: f32, max_width: Option<f32>) -> (f32, f32);
}
impl<F: Fn(&str, f32, Option<f32>) -> (f32, f32)> TextMeasurer for F {
    fn measure(&self, text: &str, font_size: f32, max_width: Option<f32>) -> (f32, f32) {
        self(text, font_size, max_width)
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    /// Opt-in grid, wrapping, richer lengths and positioning.
    pub layout_options: Option<Arc<crate::layout::LayoutOptions>>,
    /// Remove this child from normal flow and position it at its parent content
    /// origin plus margins. It does not contribute to intrinsic parent size.
    pub absolute: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Fraction of the definite parent content width (1.0 means 100%).
    /// Falls back to intrinsic sizing when that axis is indefinite. Pixel width wins.
    pub width_percent: Option<f32>,
    /// Fraction of the definite parent content height. Absolute children use the
    /// final parent box. Invalid/negative fractions resolve to zero; values over 1 are allowed.
    pub height_percent: Option<f32>,
    pub padding: f32,
    /// Per-edge padding, overriding `padding` when present.
    pub padding_edges: Option<Insets>,
    pub gap: f32,
    pub clip: bool,
    /// Fade descendants out over these bands (top, bottom) at the edges of
    /// this node's clip, in the fragment shader: a scroll view whose content
    /// dissolves under a titlebar or into a composer, without a layer.
    pub fade_edges: [f32; 2],
    /// Where `fade_edges` bands end, this far in from the clip's edges (top,
    /// bottom): content past them is hidden, so a scroll view running on
    /// under a bar can be faded out by the bar's edge.
    pub fade_insets: [f32; 2],
    /// `fade_edges` at the left and right edges: a horizontally scrolling
    /// row fading its overflow.
    pub fade_edges_x: [f32; 2],
    /// `fade_insets` at the left and right edges.
    pub fade_insets_x: [f32; 2],
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub margin: Insets,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub align: Align,
    pub justify: Justify,
    /// Request wrapping at the available content width from the text measurer.
    pub text_wrap: bool,
    /// Plain-text display overflow; ignored by editor document shaping.
    /// `NodeKind::RichText` instead uses its immutable `RichText::options()` so
    /// measurement, rendering and actionable span geometry share one source.
    pub text_options: crate::text_layout::TextOptions,
}

const LAYOUT: u8 = 1;
const PAINT: u8 = 2;
const COMPOSITE: u8 = 4;
/// Repainted where its damage is already recorded: layout damaged a node it
/// moved where it was and where it lands (see `arrange`).
const PAINTED: u8 = 8;
struct Node {
    cursor: Option<crate::cursor::Cursor>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    kind: NodeKind,
    font: crate::text_layout::FontStyle,
    style: Style,
    bounds: Rect,
    transform: Transform,
    effects: Effects,
    dirty: u8,
    queued: bool,
    /// The measurement `arrange` places children from: the most recent one used.
    measurement: Option<Measurement>,
    /// Other recent results under different constraints, most recent first.
    alternates: Vec<Measurement>,
    /// Sizes of older results, without their children's places: enough for
    /// an ancestor's probe (see `Scene::measure`), most recent first.
    sizes: Vec<ProbeSize>,
    /// Everything this subtree can paint, including its own translation but not
    /// its ancestors'. Rendering skips subtrees whose ink misses the damage.
    /// A valid visible node has only valid descendants, so invalidation stops
    /// at the first invalid ancestor. A hidden node is cached as painting
    /// nothing without visiting its children; it can only become visible
    /// through its own change, which invalidates it and its ancestors.
    ink: std::cell::Cell<Option<Rect>>,
    ink_valid: std::cell::Cell<bool>,
    /// Intrinsic text or rich text size by wrap limit and shaper revision. Flex layout
    /// measures a leaf under several constraint sets per pass, which defeats
    /// `measurement`; this keeps an unchanged label from being reshaped each
    /// time an ancestor relayouts. Cleared by the node's own layout invalidation.
    /// Damage fills it too (see `text_ink`), for text whose box is fixed.
    text_size: std::cell::Cell<TextSizes>,
    advanced_layout: Option<Box<advanced_layout::AdvancedLayoutCache>>,
    arrange_dirty: bool,
    /// Whether its size can depend on the available width and height it is
    /// measured under (`Scene::reads_available`): `READS_KNOWN` once worked
    /// out, cleared with its layout.
    reads: std::cell::Cell<u8>,
    isolated: bool,
    /// Scale, rotation and skew; a node with one paints as a layer. Boxed:
    /// few nodes have one, and every node's size costs each tree walk.
    local: Option<Box<LocalTransform>>,
    /// An alpha mask over the subtree (`Scene::set_mask`): a layer too.
    image_mask: Option<Box<ImageMask>>,
    /// Its clip inset (`Scene::set_clip_inset`).
    clip_inset: Option<Box<ClipInset>>,
    /// Its shader fill's uniforms (`Scene::set_shader_uniforms`).
    shader_uniforms: Option<Box<crate::decoration::ShaderUniforms>>,
    /// Has had a local transform, content blur or mask: its layer revision
    /// keeps counting while it has none, so a renderer can keep its texture
    /// (see `invalidate`).
    promoted: bool,
    layer_revision: u64,
    /// Scroll content: a translation is reported as a `ScrollMove` the
    /// renderer may apply by copying pixels (see `set_scroll_copy`).
    scroll_copy: bool,
}
impl Node {
    fn local(&self) -> LocalTransform {
        self.local.as_deref().copied().unwrap_or_default()
    }
    /// Whether it has a local transform other than the identity.
    fn transformed(&self) -> bool {
        self.local
            .as_deref()
            .is_some_and(|local| !local.is_identity())
    }
    /// Whether it composites through a content blur.
    fn filtered(&self) -> bool {
        self.effects.filter_blur > 0.
    }
    /// Whether it paints as a layer for an effect on its content: a content
    /// blur or a mask.
    fn effect_layer(&self) -> bool {
        self.filtered() || self.image_mask.is_some()
    }
    fn set_local(&mut self, transform: LocalTransform) {
        match &mut self.local {
            _ if transform == LocalTransform::IDENTITY => self.local = None,
            Some(local) => **local = transform,
            None => self.local = Some(Box::new(transform)),
        }
    }
}
/// A text node's measured sizes for its most recent wrap limits. Layout
/// measures a paragraph at more than one width per pass (intrinsic, then
/// final); one entry would evict the other and reshape the text each time.
#[derive(Clone, Copy, Default)]
struct TextSizes([Option<TextSize>; 3]);
/// A wrap limit, the shaper revision it was measured under, and the size.
type TextSize = (Option<f32>, u64, (f32, f32));
impl TextSizes {
    fn find(&self, limit: Option<f32>, revision: u64) -> Option<(f32, f32)> {
        self.0
            .iter()
            .flatten()
            .find(|(cached, cached_revision, _)| *cached == limit && *cached_revision == revision)
            .map(|(_, _, size)| *size)
    }
    fn insert(&mut self, limit: Option<f32>, revision: u64, size: (f32, f32)) {
        self.0.rotate_right(1);
        self.0[0] = Some((limit, revision, size));
    }
    fn remember(cell: &std::cell::Cell<Self>, limit: Option<f32>, revision: u64, size: (f32, f32)) {
        let mut sizes = cell.get();
        sizes.insert(limit, revision, size);
        cell.set(sizes);
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Constraints {
    width: Option<f32>,
    height: Option<f32>,
    percent_width: Option<f32>,
    percent_height: Option<f32>,
    force_width: Option<f32>,
    force_height: Option<f32>,
}
struct Measurement {
    constraints: Constraints,
    size: (f32, f32),
    children: Vec<(NodeId, Rect)>,
    /// The constraints each child was last measured with for this result. A
    /// later probe (a flex basis, an intrinsic size) can leave a child holding
    /// another result; `arrange` restores this one before placing it.
    child_constraints: Vec<Option<Constraints>>,
    /// Layout pass that produced this result. A dirty node may reuse only
    /// results from the pass in progress.
    pass: u64,
}
/// Flex layout measures a child with its natural, flexed and stretched
/// constraints in one pass, and taffy adds min- and max-content probes, each
/// ancestor on a changed path asking again. Every result of the pass in
/// progress is kept (up to `PASS_RESULTS`), so each probe of a subtree is laid
/// out once per pass however many ancestors repeat it; with too few, each
/// level of nesting multiplied the work. Afterwards a node keeps this many,
/// most recently used first, for the next pass's clean probes.
const MEASUREMENT_ALTERNATES: usize = 7;
/// A bound on one pass's results per node, against pathological probing.
const PASS_RESULTS: usize = 64;
/// Sizes a node keeps of results it no longer holds whole (`Node::sizes`):
/// the probes a clean subtree answers every frame of an animation around it.
const PROBE_SIZES: usize = 32;
/// A result's size alone (see `Node::sizes`).
#[derive(Clone, Copy)]
struct ProbeSize {
    constraints: Constraints,
    size: (f32, f32),
    pass: u64,
}
impl From<&Measurement> for ProbeSize {
    fn from(m: &Measurement) -> Self {
        Self {
            constraints: m.constraints,
            size: m.size,
            pass: m.pass,
        }
    }
}
/// Put `evicted` (most recent first) ahead of `sizes`' older entries, each
/// constraint kept once, at most `PROBE_SIZES`.
fn keep_sizes(sizes: &mut Vec<ProbeSize>, evicted: Vec<ProbeSize>) {
    if evicted.is_empty() {
        return;
    }
    sizes.retain(|kept| !evicted.iter().any(|m| m.constraints == kept.constraints));
    sizes.splice(0..0, evicted);
    sizes.truncate(PROBE_SIZES);
}
fn dimension(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    value
        .max(0.0)
        .min(max.unwrap_or(f32::INFINITY).max(0.0))
        .max(min.unwrap_or(0.0).max(0.0))
}
fn percentage(fraction: Option<f32>, basis: Option<f32>) -> Option<f32> {
    fraction.zip(basis).map(|(fraction, basis)| {
        let fraction = if fraction.is_finite() {
            fraction.max(0.0)
        } else {
            0.0
        };
        ((fraction as f64 * basis.max(0.0) as f64).min(f32::MAX as f64)) as f32
    })
}
struct Slot {
    generation: u32,
    node: Option<Node>,
}
#[derive(Default, Debug)]
pub struct FrameReport {
    pub damage: Vec<Rect>,
    pub layout_nodes: usize,
    pub paint_nodes: usize,
    pub composite_nodes: usize,
}
impl FrameReport {
    pub fn is_idle(&self) -> bool {
        self.damage.is_empty()
            && self.layout_nodes == 0
            && self.paint_nodes == 0
            && self.composite_nodes == 0
    }
}
/// The parts of `clip` that content shifted by `(dx, dy)` leaves uncovered.
fn exposed(clip: Rect, dx: f32, dy: f32) -> Vec<Rect> {
    let mut strips = Vec::new();
    if dy > 0. {
        strips.push(Rect::new(clip.x, clip.y, clip.width, dy));
    } else if dy < 0. {
        strips.push(Rect::new(
            clip.x,
            clip.y + clip.height + dy,
            clip.width,
            -dy,
        ));
    }
    if dx > 0. {
        strips.push(Rect::new(clip.x, clip.y, dx, clip.height));
    } else if dx < 0. {
        strips.push(Rect::new(
            clip.x + clip.width + dx,
            clip.y,
            -dx,
            clip.height,
        ));
    }
    strips
}
/// A `fade_edges` ancestor's clip (less its `fade_insets`) and its bands
/// (top, right, bottom, left).
pub type FadeMask = (Rect, [f32; 4]);
/// A box with rounded corners: its radii top-left, top-right, bottom-right,
/// bottom-left, each at most half its shorter side.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoundRect {
    pub rect: Rect,
    pub radii: [f32; 4],
}
/// The rounded clips around a paint item, innermost first: those of its
/// nearest ancestors that clip (`overflow: hidden`) with rounded corners
/// (`QuadStyle::radius` or per-corner decoration corners). Rectangular
/// clipping stays `PaintItem::clip`; these cut the corners, in the item's
/// layer's space. Two cover a rounded thumbnail inside a rounded card;
/// further out, the corners are those of the outer two.
pub type RoundedClip = [Option<RoundRect>; 2];
#[derive(Clone)]
pub struct PaintItem<'a> {
    /// A layer (an isolated or transformed subtree) represented by its cached image.
    pub isolated: bool,
    /// A transformed layer: where its content, laid out in `bounds`' space,
    /// lands in this item's (see `LocalTransform::matrix`). Its `clip` is
    /// then its parent's; its own clips inside the layer. Always `None`
    /// from `paint_items`.
    pub matrix: Option<crate::affine::Affine>,
    pub id: NodeId,
    pub bounds: Rect,
    pub kind: &'a NodeKind,
    pub font: &'a crate::text_layout::FontStyle,
    pub text_options: crate::text_layout::TextOptions,
    pub effects: Effects,
    pub clip: Option<Rect>,
    /// The nearest `fade_edges` ancestor's clip and bands (top, bottom).
    pub mask: Option<FadeMask>,
    /// Rounded clips around it (see `RoundedClip`).
    pub rounded: RoundedClip,
    /// This item paints `id`'s border alone, after its descendants
    /// (`QuadStyle::border_over`); the node's first item painted the rest.
    pub border_pass: bool,
    /// Its and its ancestors' colour filters as one matrix (see
    /// `Effects::color_matrix`); a layer's own, at its composite.
    pub color: Option<ColorMatrix>,
}
type FontMeasurer =
    dyn Fn(&std::sync::Arc<str>, f32, Option<f32>, &crate::text_layout::FontStyle) -> (f32, f32);
pub struct Scene {
    identity: u64,
    slots: Vec<Slot>,
    free: Vec<u32>,
    root: NodeId,
    dirty: Vec<NodeId>,
    damage: Vec<Rect>,
    /// The last rectangles added to `damage` (each is held by one of its
    /// rectangles, which only grow): a change damages the same places again
    /// and again (before and after, then where layout moves it).
    recent_damage: [Rect; 4],
    damage_grid: Option<DamageGrid>,
    viewport: Rect,
    blur_nodes: Vec<NodeId>,
    /// Nodes the platform paints (native views): their box is their ink.
    platform_nodes: Vec<NodeId>,
    isolation_count: usize,
    /// Nodes with a local transform: none keeps every geometry query on
    /// the translation-only paths.
    transformed: usize,
    /// Nodes with a content blur: with `transformed`, whether damage takes
    /// the placed paths (`descend`).
    filtered: usize,
    /// Nodes that ever had one, a content blur or a mask (`Node::promoted`).
    promoted_count: usize,
    text_measurer: Option<Box<dyn TextMeasurer>>,
    text_shaper: Option<Box<dyn crate::text_layout::TextShaper>>,
    font_shaper: Option<Box<dyn crate::text_layout::FontTextShaper>>,
    font_measurer: Option<Box<FontMeasurer>>,
    rich_measurer: crate::rich_text::TextMeasure,
    rich_shaper: Option<Box<crate::rich_text::RichShaper>>,
    text_shaper_revision: u64,
    pending_layout_nodes: usize,
    layout_revision: u64,
    layout_pass: u64,
    content_revision: u64,
    /// The deepest stack and the item count of the last paint walk
    /// (`layer_items_within`): the next one starts at those sizes.
    walk_hints: std::cell::Cell<(usize, usize)>,
    /// Dirty fixed-size containers whose ancestors stayed clean.
    layout_roots: Vec<NodeId>,
    /// Nodes holding more than `MEASUREMENT_ALTERNATES` results of this pass,
    /// trimmed when it ends.
    overfull: Vec<NodeId>,
    /// `measure` calls, for tests of how much work layout does.
    #[cfg(test)]
    measure_calls: u64,
    geometry_revision: u64,
    cursor_styles: usize,
    /// Scroll content translated since the last flush: its transform before
    /// the first move, and whether it moved again (see `resolve_moves`).
    pending_moves: Vec<(NodeId, Transform, bool)>,
    /// The moves reported by the last flush, and that flush's serial.
    moves: Vec<ScrollMove>,
    flush_serial: u64,
}
/// Scroll content that moved rigidly by `(dx, dy)` within `clip`. The flush
/// damaged only what copying the old pixels cannot provide (`exposed`, plus
/// every other change inside the clip at both positions). A renderer that
/// cannot copy must repaint `clip` as well.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollMove {
    pub node: NodeId,
    pub clip: Rect,
    pub dx: f32,
    pub dy: f32,
    /// `flush_serial` of the flush that reported it: the copy is valid only
    /// over the frame rendered from the previous flush.
    pub serial: u64,
}
impl Scene {
    pub fn new(width: f32, height: f32) -> Self {
        let root = NodeId {
            index: 0,
            generation: 0,
        };
        Self {
            identity: NEXT_SCENE_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .expect("scene identity exhausted"),
            slots: vec![Slot {
                generation: 0,
                node: Some(Node {
                    cursor: None,
                    parent: None,
                    children: Vec::new(),
                    kind: NodeKind::Container(Layout::Column),
                    font: Default::default(),
                    style: Style {
                        width: Some(width),
                        height: Some(height),
                        clip: true,
                        ..Style::default()
                    },
                    bounds: Rect::default(),
                    transform: Transform::default(),
                    effects: Effects::default(),
                    dirty: LAYOUT | PAINT,
                    queued: true,
                    measurement: None,
                    alternates: Vec::new(),
                    sizes: Vec::new(),
                    ink: std::cell::Cell::new(None),
                    ink_valid: std::cell::Cell::new(false),
                    text_size: Default::default(),
                    advanced_layout: None,
                    arrange_dirty: true,
                    reads: std::cell::Cell::new(0),
                    isolated: false,
                    local: None,
                    image_mask: None,
                    clip_inset: None,
                    shader_uniforms: None,
                    promoted: false,
                    scroll_copy: false,
                    layer_revision: 0,
                }),
            }],
            free: Vec::new(),
            root,
            dirty: vec![root],
            damage: Vec::new(),
            recent_damage: [Rect::default(); 4],
            damage_grid: None,
            viewport: Rect::new(0.0, 0.0, width, height),
            blur_nodes: Vec::new(),
            platform_nodes: Vec::new(),
            pending_moves: Vec::new(),
            moves: Vec::new(),
            flush_serial: 0,
            isolation_count: 0,
            transformed: 0,
            filtered: 0,
            promoted_count: 0,
            text_measurer: None,
            text_shaper: None,
            font_shaper: None,
            font_measurer: None,
            rich_measurer: Default::default(),
            rich_shaper: None,
            text_shaper_revision: 0,
            pending_layout_nodes: 0,
            layout_revision: 0,
            layout_pass: 0,
            content_revision: 0,
            walk_hints: Default::default(),
            layout_roots: Vec::new(),
            overfull: Vec::new(),
            #[cfg(test)]
            measure_calls: 0,
            geometry_revision: 0,
            cursor_styles: 0,
        }
    }
    /// Stable identity separates renderer caches when a whole scene is replaced.
    pub fn identity(&self) -> u64 {
        self.identity
    }
    pub fn root(&self) -> NodeId {
        self.root
    }
    /// Mark `id` as painted by the platform (a native view over its box):
    /// its box counts as its ink, so damage culling and paint walks within
    /// regions reach it though zgui draws nothing there.
    pub fn set_platform_painted(&mut self, id: NodeId, painted: bool) {
        if !self.contains(id) {
            return;
        }
        let present = self.platform_nodes.contains(&id);
        if present == painted {
            return;
        }
        let slots = &self.slots;
        self.platform_nodes.retain(|node| {
            *node != id
                && slots
                    .get(node.index as usize)
                    .is_some_and(|s| s.generation == node.generation && s.node.is_some())
        });
        if painted {
            self.platform_nodes.push(id);
        }
        self.invalidate(id, COMPOSITE);
    }
    pub fn is_platform_painted(&self, id: NodeId) -> bool {
        self.platform_nodes.contains(&id)
    }
    pub fn contains(&self, id: NodeId) -> bool {
        self.slots
            .get(id.index as usize)
            .is_some_and(|s| s.generation == id.generation && s.node.is_some())
    }
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }
    pub fn is_empty(&self) -> bool {
        false
    } // A scene always retains its root.
    fn node(&self, id: NodeId) -> &Node {
        assert!(self.contains(id), "stale scene node");
        self.slots[id.index as usize].node.as_ref().unwrap()
    }
    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        assert!(self.contains(id), "stale scene node");
        self.slots[id.index as usize].node.as_mut().unwrap()
    }
    pub fn bounds(&self, id: NodeId) -> Rect {
        self.world(id).0
    }
    /// Visible allocation after ancestor opacity, clipping and viewport intersection.
    /// This does not attempt occlusion testing against overlapping siblings.
    /// Layout visibility excludes hidden/display-none nodes and their descendants.
    /// Mounted ownership is retained; opacity remains an independent paint effect.
    pub fn layout_visible(&self, id: NodeId) -> bool {
        self.contains(id)
            && std::iter::once(id)
                .chain(self.ancestors(id))
                .all(|id| own_visible(&self.node(id).style))
    }
    pub fn visible_bounds(&self, id: NodeId) -> Option<Rect> {
        let (bounds, effects, clip) = self.world(id);
        if effects.opacity <= 0. {
            return None;
        }
        if self.transformed > 0 {
            let (bounds, clip, placement) = self.placed(id);
            let visible = bounds.intersection(clip.unwrap_or(UNCLIPPED))?;
            return match placement {
                Some(placement) => placement
                    .matrix
                    .bounds(visible)
                    .intersection(placement.clip),
                None => Some(visible),
            };
        }
        bounds.intersection(clip.unwrap_or(self.viewport))
    }
    /// Where `id`'s box shows: the viewport-aligned box around its corners
    /// after its own and its ancestors' local transforms, unclipped. `bounds`
    /// is the box as laid out and translated.
    pub fn transformed_bounds(&self, id: NodeId) -> Rect {
        if self.transformed == 0 {
            return self.bounds(id);
        }
        let (bounds, _, placement) = self.placed(id);
        placement.map_or(bounds, |placement| placement.matrix.bounds(bounds))
    }
    /// `id`'s translated box, the clip it paints within (in its layer's
    /// space; `None` cuts nothing) and its layer's placement, if transformed.
    fn placed(&self, id: NodeId) -> (Rect, Option<Rect>, Option<Placement>) {
        let (transform, clip, placement) = match self.node(id).parent {
            Some(parent) => self.context(parent),
            None => (Transform::default(), Some(self.viewport), None),
        };
        let (_, bounds, clip, placement) = self.descend(self.node(id), transform, clip, placement);
        (bounds, clip, placement)
    }
    pub fn local_transform(&self, id: NodeId) -> LocalTransform {
        self.node(id).local()
    }
    /// Set scale, rotation, skew and origin, normalizing each nonfinite
    /// component to its identity value. Changes composite only: the subtree
    /// is drawn as a cached layer, damaged where it was and where it lands.
    pub fn set_local_transform(&mut self, id: NodeId, transform: LocalTransform) {
        let transform = transform.normalized();
        let old = self.node(id).local();
        if old == transform {
            return;
        }
        if old.is_identity() && transform.is_identity() {
            // Only the origin of nothing moved.
            self.node_mut(id).set_local(transform);
            return;
        }
        self.damage_subtree(id);
        if old.is_identity() != transform.is_identity() {
            if transform.is_identity() {
                self.transformed -= 1;
            } else {
                self.transformed += 1;
            }
        }
        let node = self.node_mut(id);
        node.set_local(transform);
        if !std::mem::replace(&mut node.promoted, true) {
            self.promoted_count += 1;
        }
        self.geometry_revision = self.geometry_revision.wrapping_add(1);
        self.damage_subtree(id);
        self.invalidate(id, COMPOSITE);
    }
    /// Whether `id` paints as a cached layer: isolated, transformed, blurred
    /// (`Effects::filter_blur`) or masked.
    pub fn is_layer(&self, id: NodeId) -> bool {
        let node = self.node(id);
        node.isolated || node.transformed() || node.effect_layer()
    }
    /// `id`'s content blur (`Effects::filter_blur`).
    pub fn filter_blur(&self, id: NodeId) -> f32 {
        self.node(id).effects.filter_blur
    }
    /// Set `id`'s content blur (`Effects::filter_blur`), keeping its other
    /// effects. Composites only: damaged where the blur reached and reaches.
    pub fn set_filter_blur(&mut self, id: NodeId, sigma: f32) {
        let effects = Effects {
            filter_blur: sigma,
            ..self.node(id).effects
        };
        self.set_effects(id, effects);
    }
    pub fn mask(&self, id: NodeId) -> Option<&ImageMask> {
        self.node(id).image_mask.as_deref()
    }
    /// Mask `id`'s subtree (see `ImageMask`), or with `None` show it whole.
    /// Nonfinite offsets and sizes are zeroed. A masked node paints as a
    /// cached layer: changing the mask composites again, damaging the
    /// subtree, without repainting it.
    pub fn set_mask(&mut self, id: NodeId, mask: Option<ImageMask>) {
        let mask = mask.map(ImageMask::normalized);
        let old = self.node(id).image_mask.as_deref();
        if old == mask.as_ref() {
            return;
        }
        // A renderer keeps mask textures for the images nodes show.
        if old.map(|old| old.image.id()) != mask.as_ref().map(|mask| mask.image.id()) {
            self.content_revision = self.content_revision.wrapping_add(1);
        }
        // A mask only hides: what it reveals or hides lies within what the
        // subtree paints anyway.
        self.damage_subtree(id);
        let node = self.node_mut(id);
        node.image_mask = mask.map(Box::new);
        if node.image_mask.is_some() && !std::mem::replace(&mut node.promoted, true) {
            self.promoted_count += 1;
        }
        self.invalidate(id, COMPOSITE);
    }
    /// `id`'s shader fill uniforms (see `crate::decoration::FillShader`).
    pub fn shader_uniforms(&self, id: NodeId) -> crate::decoration::ShaderUniforms {
        self.node(id)
            .shader_uniforms
            .as_deref()
            .copied()
            .unwrap_or_default()
    }
    /// Set the uniforms `id`'s shader fill draws with: its time (quantized
    /// to the shader's frame rate, so a capped shader repaints only when
    /// it steps) and user parameters. Repaints that box alone; no layout,
    /// nothing else. Nonfinite values are zeroed.
    pub fn set_shader_uniforms(
        &mut self,
        id: NodeId,
        mut uniforms: crate::decoration::ShaderUniforms,
    ) {
        fn finite(value: f32) -> f32 {
            if value.is_finite() { value } else { 0. }
        }
        uniforms.time = finite(uniforms.time);
        uniforms.params = uniforms.params.map(|row| row.map(finite));
        let fps = match &self.node(id).kind {
            NodeKind::Quad(quad) | NodeKind::Panel { quad, .. } => quad
                .decoration
                .as_deref()
                .and_then(|detail| match &detail.background {
                    Some(crate::decoration::Background::Shader(shader)) => shader.fps(),
                    _ => None,
                }),
            _ => None,
        };
        if let Some(fps) = fps {
            uniforms.time = (uniforms.time * fps).floor() / fps;
        }
        if self.shader_uniforms(id) == uniforms {
            return;
        }
        self.node_mut(id).shader_uniforms = Some(Box::new(uniforms));
        self.invalidate(id, PAINT);
    }
    pub fn clip_inset(&self, id: NodeId) -> Option<ClipInset> {
        self.node(id).clip_inset.as_deref().copied()
    }
    /// Clip `id` and its descendants to its box inset by `inset` (see
    /// `ClipInset`), or with `None` stop. Animating it composites again,
    /// damaging the subtree, without layout or repainting cached layers
    /// (a transformed node clips inside its layer, which repaints).
    pub fn set_clip_inset(&mut self, id: NodeId, inset: Option<ClipInset>) {
        if self.clip_inset(id) == inset {
            return;
        }
        // Clipping more or less shows nothing the subtree doesn't paint.
        self.damage_subtree(id);
        let node = self.node_mut(id);
        node.clip_inset = inset.map(Box::new);
        let flags = if node.transformed() { PAINT } else { COMPOSITE };
        self.geometry_revision = self.geometry_revision.wrapping_add(1);
        self.invalidate(id, flags);
    }
    /// Move `id`'s mask (a shimmer's band, a progress ramp): composites only.
    /// Without a mask, nothing.
    pub fn set_mask_offset(&mut self, id: NodeId, offset: (f32, f32)) {
        if let Some(mask) = self.mask(id) {
            let mask = ImageMask {
                offset,
                ..mask.clone()
            };
            self.set_mask(id, Some(mask));
        }
    }
    /// An explicit cursor inherits through visual ancestors, including disabled regions.
    pub fn cursor_for(&self, mut id: NodeId) -> Option<crate::cursor::Cursor> {
        loop {
            let node = self.node(id);
            if node.cursor.is_some() {
                return node.cursor;
            }
            id = node.parent?;
        }
    }
    pub fn set_cursor(&mut self, id: NodeId, cursor: Option<crate::cursor::Cursor>) {
        let old = self.node(id).cursor;
        if old != cursor {
            self.cursor_styles =
                self.cursor_styles - usize::from(old.is_some()) + usize::from(cursor.is_some());
            self.node_mut(id).cursor = cursor;
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
    }
    pub fn interaction_revision(&self) -> u64 {
        self.geometry_revision
    }
    pub fn cursor_at(&self, x: f32, y: f32) -> Option<crate::cursor::Cursor> {
        if self.cursor_styles == 0 {
            return None;
        }
        self.hit_test_all(x, y)
            .first()
            .and_then(|id| self.cursor_for(*id))
    }
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent
    }
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.node(id).children
    }
    pub fn kind(&self, id: NodeId) -> &NodeKind {
        &self.node(id).kind
    }
    pub fn style(&self, id: NodeId) -> Style {
        self.node(id).style.clone()
    }
    pub fn effects(&self, id: NodeId) -> Effects {
        self.node(id).effects
    }
    pub fn transform(&self, id: NodeId) -> Transform {
        self.node(id).transform
    }
    /// Ancestors from the immediate parent to the root.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(self.parent(id), |id| self.parent(*id))
    }
    /// Front-to-back hit targets, including containers; call after flushing layout.
    /// The point maps through each transformed ancestor's inverse, so nodes
    /// are hit where they are drawn; a singular transform hits nothing.
    pub fn hit_test_all(&self, x: f32, y: f32) -> Vec<NodeId> {
        let inside = |r: Rect, (x, y): (f32, f32)| {
            x >= r.x && y >= r.y && x < r.x + r.width && y < r.y + r.height
        };
        let mut hits = Vec::new();
        // A node, its parent's translation, opacity and clip, and the point
        // in its parent's layer space.
        let mut stack = vec![(self.root, Transform::default(), 1.0, self.viewport, (x, y))];
        while let Some((id, parent_transform, parent_opacity, parent_clip, point)) = stack.pop() {
            let node = self.node(id);
            let transform = Transform {
                x: parent_transform.x + node.transform.x,
                y: parent_transform.y + node.transform.y,
            };
            let bounds = node.bounds.translated(transform);
            let opacity = if own_visible(&node.style) {
                node.effects.opacity * parent_opacity
            } else {
                0.
            };
            let (point, parent_clip) = if !node.transformed() {
                (point, parent_clip)
            } else {
                // Clipped around it, or flattened to nothing, nothing in it is hit.
                let Some(inverse) = node.local().matrix(bounds).inverse() else {
                    continue;
                };
                if !inside(parent_clip, point) {
                    continue;
                }
                (inverse.point(point.0, point.1), UNCLIPPED)
            };
            let clip =
                clipped_bounds(Some(parent_clip), bounds, &node.style).unwrap_or(parent_clip);
            // Clipped by its inset too, in its own space.
            let clip = match node.clip_inset.as_deref() {
                Some(inset) => clip.intersection(inset.apply(bounds)).unwrap_or_default(),
                None => clip,
            };
            stack.extend(
                node.children
                    .iter()
                    .rev()
                    .map(|child| (*child, transform, opacity, clip, point)),
            );
            let painted = |transform: crate::affine::Affine| {
                transform
                    .inverse()
                    .is_some_and(|inverse| inside(bounds, inverse.point(point.0, point.1)))
            };
            let hit = opacity > 0.0
                && match &node.kind {
                    NodeKind::Image(image) => painted(image.paint_transform(bounds)),
                    NodeKind::Svg(svg) => painted(svg.paint_transform(bounds)),
                    _ => inside(bounds, point),
                }
                && inside(clip, point);
            if hit {
                hits.push(id);
            }
        }
        hits.reverse();
        hits
    }
    pub fn hit_test(&self, x: f32, y: f32) -> Option<NodeId> {
        self.hit_test_all(x, y).into_iter().next()
    }
    /// Uses the same installed font measurer as layout, useful for caret geometry.
    pub fn measure_text(&self, text: &str, font_size: f32, max_width: Option<f32>) -> (f32, f32) {
        if let Some(shaper) = &self.font_shaper {
            return shaper
                .shape(text, font_size, max_width, &Default::default())
                .size();
        }
        if let Some(shaper) = &self.text_shaper {
            return shaper.shape(text, font_size, max_width).size();
        }
        if let Some(measurer) = &self.text_measurer {
            return measurer.measure(text, font_size, max_width);
        }
        crate::text_layout::FallbackTextLayout::measure(
            text,
            font_size,
            max_width,
            crate::text_layout::LineHeight::NORMAL,
        )
    }
    /// Reorders an exact permutation of a parent's children, retaining every ID.
    /// Invalid parents, foreign nodes, omissions, and duplicates return `false`
    /// without modifying the scene. An unchanged order is a successful no-op.
    pub fn reorder_children(&mut self, parent: NodeId, children: &[NodeId]) -> bool {
        if !self.contains(parent) {
            return false;
        }
        let current = &self.node(parent).children;
        if current == children {
            return true;
        }
        if current.len() != children.len() {
            return false;
        }
        let unique: std::collections::HashSet<_> = children.iter().copied().collect();
        if unique.len() != children.len() || !current.iter().all(|id| unique.contains(id)) {
            return false;
        }
        self.damage_subtree(parent);
        self.node_mut(parent).children.copy_from_slice(children);
        self.invalidate_layout(parent);
        true
    }
    /// Installs the native font engine for editor geometry and intrinsic measurement.
    pub fn set_text_shaper(&mut self, shaper: impl crate::text_layout::TextShaper + 'static) {
        self.text_shaper_revision = self.text_shaper_revision.wrapping_add(1);
        self.text_shaper = Some(Box::new(shaper));
        let ids: Vec<_> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.node.as_ref().map(|_| NodeId {
                    index: index as u32,
                    generation: slot.generation,
                })
            })
            .collect();
        for id in ids {
            self.invalidate(id, LAYOUT | PAINT);
        }
    }
    pub fn shape_text(
        &self,
        text: &str,
        font_size: f32,
        max_width: Option<f32>,
    ) -> Box<dyn crate::text_layout::TextLayout> {
        if let Some(shaper) = &self.font_shaper {
            shaper.shape(text, font_size, max_width, &Default::default())
        } else if let Some(shaper) = &self.text_shaper {
            shaper.shape(text, font_size, max_width)
        } else {
            Box::new(crate::text_layout::FallbackTextLayout::new(
                text, font_size, max_width,
            ))
        }
    }
    pub fn set_text_measurer(&mut self, measurer: impl TextMeasurer + 'static) {
        self.text_measurer = Some(Box::new(measurer));
        self.text_shaper_revision = self.text_shaper_revision.wrapping_add(1);
        let ids: Vec<_> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.node.as_ref().map(|_| NodeId {
                    index: index as u32,
                    generation: slot.generation,
                })
            })
            .collect();
        for id in ids {
            self.invalidate(id, LAYOUT | PAINT);
        }
    }
    pub fn font(&self, id: NodeId) -> &crate::text_layout::FontStyle {
        &self.node(id).font
    }
    /// Font changes affect shaping, layout, paint and retained layer contents.
    pub fn set_font(&mut self, id: NodeId, mut font: crate::text_layout::FontStyle) {
        font.weight = font.weight.clamp(1, 1000);
        if self.node(id).font == font {
            return;
        }
        self.damage_subtree(id);
        self.node_mut(id).font = font;
        self.invalidate(id, PAINT);
        self.invalidate_layout(id);
    }
    pub fn set_font_text_shaper(
        &mut self,
        shaper: impl crate::text_layout::FontTextShaper + 'static,
    ) {
        self.text_shaper_revision = self.text_shaper_revision.wrapping_add(1);
        self.font_shaper = Some(Box::new(shaper));
        let ids: Vec<_> = self
            .paint_items()
            .filter(|item| matches!(item.kind, NodeKind::Text { .. }))
            .map(|item| item.id)
            .collect();
        for id in ids {
            self.invalidate(id, PAINT);
            self.invalidate_layout(id);
        }
    }
    /// Install rich paragraph geometry for inline selection and interactive spans.
    pub fn set_rich_text_shaper(
        &mut self,
        shaper: impl Fn(
            &crate::rich_text::RichText,
            Option<f32>,
        ) -> Box<dyn crate::text_layout::TextLayout>
        + 'static,
    ) {
        self.rich_shaper = Some(Box::new(shaper));
        self.text_shaper_revision = self.text_shaper_revision.wrapping_add(1);
    }
    pub fn shape_rich_text(
        &self,
        rich: &crate::rich_text::RichText,
        width: Option<f32>,
    ) -> Box<dyn crate::text_layout::TextLayout> {
        self.rich_shaper.as_ref().map_or_else(
            || {
                Box::new(crate::text_layout::FallbackTextLayout::with_rich(
                    rich, width,
                )) as Box<dyn crate::text_layout::TextLayout>
            },
            |shaper| shaper(rich, width),
        )
    }
    /// Install measurement for plain text nodes, sized as the installed font
    /// shaper lays them out. Hosts measure from the text they draw from.
    pub fn set_font_text_measurer(
        &mut self,
        measure: impl Fn(
            &std::sync::Arc<str>,
            f32,
            Option<f32>,
            &crate::text_layout::FontStyle,
        ) -> (f32, f32)
        + 'static,
    ) {
        self.font_measurer = Some(Box::new(measure));
        self.text_shaper_revision = self.text_shaper_revision.wrapping_add(1);
        let ids: Vec<_> = self
            .paint_items()
            .filter(|item| matches!(item.kind, NodeKind::Text { .. }))
            .map(|item| item.id)
            .collect();
        for id in ids {
            self.invalidate(id, PAINT);
            self.invalidate_layout(id);
        }
    }
    /// Install native continuous mixed-run measurement for rich display text.
    pub fn set_rich_text_measurer(
        &mut self,
        measure: impl Fn(&crate::rich_text::RichText, Option<f32>) -> (f32, f32) + 'static,
    ) {
        self.rich_measurer.install(Box::new(measure));
        self.text_shaper_revision = self.text_shaper_revision.wrapping_add(1);
        let ids: Vec<_> = self
            .paint_items()
            .filter(|item| matches!(item.kind, NodeKind::RichText { .. }))
            .map(|item| item.id)
            .collect();
        for id in ids {
            self.invalidate(id, PAINT);
            self.invalidate_layout(id);
        }
    }
    /// Install measurement for rich text that is not mounted (`TextMeasure`),
    /// sized exactly as `set_rich_text_measurer` sizes it but without keeping
    /// what drawing it would need.
    pub fn set_detached_rich_text_measurer(
        &mut self,
        measure: impl Fn(&crate::rich_text::RichText, Option<f32>) -> (f32, f32) + 'static,
    ) {
        self.rich_measurer.install_detached(Box::new(measure));
    }
    /// The rich text measurer layout uses, including one installed later.
    pub fn text_measure(&self) -> crate::rich_text::TextMeasure {
        self.rich_measurer.clone()
    }
    /// Changes whenever either installed shaping callback is replaced.
    pub(crate) fn text_shaper_revision(&self) -> u64 {
        self.text_shaper_revision
    }
    pub fn shape_text_with_font(
        &self,
        text: &str,
        size: f32,
        width: Option<f32>,
        font: &crate::text_layout::FontStyle,
    ) -> Box<dyn crate::text_layout::TextLayout> {
        if let Some(shaper) = &self.font_shaper {
            shaper.shape(text, size, width, font)
        } else if font.line_height != crate::text_layout::LineHeight::NORMAL
            || font.letter_spacing != crate::text_layout::LetterSpacing::ZERO
        {
            // Legacy callbacks cannot receive font metrics. Use the same
            // grapheme-cell fallback for measuring and editing explicit metrics.
            Box::new(crate::text_layout::FallbackTextLayout::with_font(
                text, size, width, font,
            ))
        } else {
            self.shape_text(text, size, width)
        }
    }
    pub fn measure_text_with_font(
        &self,
        text: &str,
        size: f32,
        width: Option<f32>,
        font: &crate::text_layout::FontStyle,
    ) -> (f32, f32) {
        if let Some(shaper) = &self.font_shaper {
            shaper.shape(text, size, width, font).size()
        } else if font.line_height != crate::text_layout::LineHeight::NORMAL
            || font.letter_spacing != crate::text_layout::LetterSpacing::ZERO
        {
            crate::text_layout::FallbackTextLayout::measure_with_font(text, size, width, font)
        } else {
            self.measure_text(text, size, width)
        }
    }
    pub fn set_kind(&mut self, id: NodeId, kind: NodeKind) {
        if self.node(id).kind == kind {
            return;
        }
        // Renderer caches are keyed by node and kind; only a different kind or
        // a replaced texture-backed resource can leave one stale. Text and
        // colour changes, which animate every frame, do not.
        let resource = |kind: &NodeKind| {
            #[cfg(target_os = "macos")]
            if matches!(kind, NodeKind::NativeSurface(_)) {
                return true;
            }
            match kind {
                NodeKind::Image(_) | NodeKind::Svg(_) | NodeKind::Canvas(_) => true,
                NodeKind::Quad(quad) | NodeKind::Panel { quad, .. } => quad.decoration.is_some(),
                _ => false,
            }
        };
        if std::mem::discriminant(&self.node(id).kind) != std::mem::discriminant(&kind)
            || resource(&self.node(id).kind)
        {
            self.content_revision = self.content_revision.wrapping_add(1);
        }
        let container_layout = |kind: &NodeKind| match kind {
            NodeKind::Container(layout) | NodeKind::Panel { layout, .. } => Some(*layout),
            _ => None,
        };
        let old_layout = container_layout(&self.node(id).kind);
        let layout_unchanged = matches!(
            (&self.node(id).kind, &kind),
            (
                NodeKind::Rect(_) | NodeKind::Quad(_),
                NodeKind::Rect(_) | NodeKind::Quad(_)
            )
        ) || (old_layout.is_some() && old_layout == container_layout(&kind))
            || matches!((&self.node(id).kind, &kind),
                (NodeKind::Text { text: old_text, font_size: old_size, .. },
                 NodeKind::Text { text: new_text, font_size: new_size, .. })
                 if old_text == new_text && old_size == new_size)
            || matches!((&self.node(id).kind, &kind),
                (NodeKind::RichText { text: old }, NodeKind::RichText { text: new }) if old.same_metrics(new))
            || matches!((&self.node(id).kind, &kind),
                (NodeKind::Image(old), NodeKind::Image(new))
                if (old.width() == new.width() && old.height() == new.height())
                    // Both concrete axes make intrinsic source dimensions
                    // irrelevant, including when min/max constraints clamp them.
                    || (self.node(id).style.width.is_some() && self.node(id).style.height.is_some()));
        let layout_unchanged = layout_unchanged
            || matches!(
                (&self.node(id).kind, &kind),
                (NodeKind::Canvas(_), NodeKind::Canvas(_)) | (NodeKind::Svg(_), NodeKind::Svg(_))
            );
        #[cfg(target_os = "macos")]
        let layout_unchanged = layout_unchanged
            || matches!((&self.node(id).kind,&kind),(NodeKind::NativeSurface(old),NodeKind::NativeSurface(new)) if (old.width(),old.height())==(new.width(),new.height()) || (self.node(id).style.width.is_some() && self.node(id).style.height.is_some()));
        self.damage_subtree(id);
        if matches!((&self.node(id).kind, &kind), (NodeKind::Image(old), NodeKind::Image(new))
            if old.transform() != new.transform())
        {
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
        if matches!((&self.node(id).kind, &kind), (NodeKind::Svg(old), NodeKind::Svg(new)) if old.transform()!=new.transform())
        {
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
        self.node_mut(id).kind = kind;
        self.invalidate(id, PAINT);
        if !layout_unchanged {
            self.invalidate_layout(id);
        }
    }
    pub fn append(&mut self, parent: NodeId, kind: NodeKind, mut style: Style) -> NodeId {
        self.content_revision = self.content_revision.wrapping_add(1);
        if let Some(options) = &style.layout_options {
            let normalized = (**options).normalized();
            if normalized != **options {
                style.layout_options = Some(Arc::new(normalized));
            }
        }
        self.node(parent);
        let node = Node {
            cursor: None,
            parent: Some(parent),
            children: Vec::new(),
            kind,
            font: Default::default(),
            style,
            bounds: Rect::default(),
            transform: Transform::default(),
            effects: Effects::default(),
            dirty: LAYOUT | PAINT,
            queued: true,
            measurement: None,
            alternates: Vec::new(),
            sizes: Vec::new(),
            ink: std::cell::Cell::new(None),
            ink_valid: std::cell::Cell::new(false),
            text_size: Default::default(),
            advanced_layout: None,
            arrange_dirty: true,
            reads: std::cell::Cell::new(0),
            isolated: false,
            local: None,
            image_mask: None,
            clip_inset: None,
            shader_uniforms: None,
            promoted: false,
            scroll_copy: false,
            layer_revision: 0,
        };
        let id = if let Some(index) = self.free.pop() {
            self.slots[index as usize].node = Some(node);
            NodeId {
                index,
                generation: self.slots[index as usize].generation,
            }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 0,
                node: Some(node),
            });
            NodeId {
                index,
                generation: 0,
            }
        };
        self.node_mut(parent).children.push(id);
        self.dirty.push(id);
        self.invalidate_layout(parent);
        id
    }
    pub fn remove(&mut self, id: NodeId) {
        self.content_revision = self.content_revision.wrapping_add(1);
        assert_ne!(id, self.root, "cannot remove scene root");
        self.damage_subtree(id);
        let parent = self.node(id).parent.unwrap();
        self.node_mut(parent).children.retain(|child| *child != id);
        self.free_subtree(id);
        self.invalidate_layout(parent);
        // Hidden windows may retain damage for an arbitrarily long time while
        // virtual rows are replaced. Amortize stale-generation cleanup so that
        // the queue stays proportional to arena capacity, not update count.
        if self.dirty.len() > self.slots.len().saturating_mul(2).max(64) {
            let slots = &self.slots;
            self.dirty.retain(|id| {
                slots
                    .get(id.index as usize)
                    .is_some_and(|slot| slot.generation == id.generation && slot.node.is_some())
            });
        }
    }
    fn free_subtree(&mut self, id: NodeId) {
        let node = self.slots[id.index as usize].node.take().unwrap();
        self.cursor_styles -= usize::from(node.cursor.is_some());
        if node.isolated {
            self.isolation_count -= 1;
        }
        self.transformed -= usize::from(node.transformed());
        self.filtered -= usize::from(node.filtered());
        self.promoted_count -= usize::from(node.promoted);
        if node.effects.blur_radius > 0.0 {
            self.blur_nodes.retain(|entry| *entry != id);
        }
        for child in node.children {
            self.free_subtree(child);
        }
        self.slots[id.index as usize].generation = self.slots[id.index as usize]
            .generation
            .checked_add(1)
            .expect("node generation exhausted");
        self.free.push(id.index);
    }
    pub fn set_text(&mut self, id: NodeId, value: impl Into<Arc<str>>) {
        let value = value.into();
        let node = self.node(id);
        let NodeKind::Text { text, .. } = &node.kind else {
            panic!("set_text requires a text node")
        };
        if *text == value {
            return;
        }
        let fixed = node.style.width.is_some() && node.style.height.is_some();
        self.damage_subtree(id);
        let node = self.node_mut(id);
        if let NodeKind::Text { text, .. } = &mut node.kind {
            *text = value;
        }
        // Sizes of the old text; a fixed box skips relayout, not these.
        node.text_size.set(TextSizes::default());
        self.invalidate(id, PAINT);
        if !fixed {
            self.invalidate_layout(id);
        }
    }
    pub fn set_style(&mut self, id: NodeId, mut style: Style) {
        if let Some(options) = &style.layout_options
            && !self
                .node(id)
                .style
                .layout_options
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(old, options))
        {
            let normalized = (**options).normalized();
            if normalized != **options {
                style.layout_options = Some(Arc::new(normalized));
            }
        }
        let old = &self.node(id).style;
        if *old == style {
            return;
        }
        // Fields that only place and size nodes paint nothing themselves:
        // layout repaints what they move (`arrange` damages a node whose
        // bounds change, where it was and where it lands).
        let paints = old.clip != style.clip
            || old.fade_edges != style.fade_edges
            || old.fade_insets != style.fade_insets
            || old.fade_edges_x != style.fade_edges_x
            || old.fade_insets_x != style.fade_insets_x
            || old.padding != style.padding
            || old.padding_edges != style.padding_edges
            || old.text_wrap != style.text_wrap
            || old.text_options != style.text_options
            || old.layout_options != style.layout_options;
        if paints {
            self.damage_subtree(id);
        }
        if own_visible(&self.node(id).style) != own_visible(&style) {
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
        self.node_mut(id).style = style;
        if paints {
            self.damage_subtree(id);
            self.invalidate(id, PAINT);
        }
        self.invalidate_layout(id);
    }
    pub fn resize(&mut self, width: f32, height: f32) {
        if self.viewport.width == width && self.viewport.height == height {
            return;
        }
        self.viewport = Rect::new(0.0, 0.0, width, height);
        let mut style = self.node(self.root).style.clone();
        style.width = Some(width);
        style.height = Some(height);
        self.set_style(self.root, style);
        // Everything repaints; a grid would be the old size.
        self.damage_grid = None;
        self.damage.clear();
        self.add_damage(self.viewport);
    }
    /// Set a translation, normalizing each non-finite coordinate to zero.
    pub fn set_transform(&mut self, id: NodeId, mut transform: Transform) {
        let coordinate = |value: f32| {
            if value.is_finite() && value != 0.0 {
                value
            } else {
                0.0
            }
        };
        transform.x = coordinate(transform.x);
        transform.y = coordinate(transform.y);
        if self.node(id).transform == transform {
            return;
        }
        if self.node(id).scroll_copy {
            // Resolved at flush: a copy plus the exposed strip, or full damage.
            match self.pending_moves.iter_mut().find(|(node, ..)| *node == id) {
                Some((.., again)) => *again = true,
                None => {
                    let from = self.node(id).transform;
                    self.pending_moves.push((id, from, false));
                }
            }
            self.node_mut(id).transform = transform;
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
            self.invalidate(id, COMPOSITE);
            return;
        }
        self.damage_subtree(id);
        self.node_mut(id).transform = transform;
        self.geometry_revision = self.geometry_revision.wrapping_add(1);
        self.damage_subtree(id);
        self.invalidate(id, COMPOSITE);
    }
    /// Apply finite render effects. NaN opacity uses the opaque default;
    /// infinite opacity clamps to its endpoint. Non-finite filter radii disable
    /// their filter, and negative radii clamp to zero.
    pub fn set_effects(&mut self, id: NodeId, mut effects: Effects) {
        effects.opacity = if effects.opacity.is_nan() {
            1.0
        } else {
            effects.opacity.clamp(0.0, 1.0).max(0.0)
        };
        let radius = |value: f32| {
            if value.is_finite() && value > 0.0 {
                value
            } else {
                0.0
            }
        };
        effects.blur_radius = radius(effects.blur_radius);
        effects.edge_fade = radius(effects.edge_fade);
        effects.filter_blur = radius(effects.filter_blur);
        // Colour filters: nonfinite values change nothing; none is negative.
        let factor = |value: f32| if value.is_finite() { value.max(0.) } else { 1. };
        effects.brightness = factor(effects.brightness);
        effects.contrast = factor(effects.contrast);
        effects.saturate = factor(effects.saturate);
        if self.node(id).effects == effects {
            return;
        }
        if (self.node(id).effects.opacity <= 0.) != (effects.opacity <= 0.) {
            self.geometry_revision = self.geometry_revision.wrapping_add(1);
        }
        let old_blur = self.node(id).effects.blur_radius > 0.0;
        if !old_blur && effects.blur_radius > 0.0 {
            self.blur_nodes.push(id);
        }
        if old_blur && effects.blur_radius == 0.0 {
            self.blur_nodes.retain(|entry| *entry != id);
        }
        // A content blur's spread damages under the old effects, then the new.
        self.damage_subtree(id);
        let old_filter = self.node(id).filtered();
        if old_filter != (effects.filter_blur > 0.) {
            if old_filter {
                self.filtered -= 1;
            } else {
                // A layer from now on: its texture outlives a blur back at 0.
                self.filtered += 1;
                if !std::mem::replace(&mut self.node_mut(id).promoted, true) {
                    self.promoted_count += 1;
                }
            }
        }
        self.node_mut(id).effects = effects;
        self.damage_subtree(id);
        self.invalidate(id, COMPOSITE);
    }
    fn invalidate(&mut self, id: NodeId, flags: u8) {
        if self.isolation_count + self.promoted_count > 0 {
            let mut current = Some(id);
            while let Some(ancestor) = current {
                let node = self.node_mut(ancestor);
                if (node.isolated || node.promoted)
                    && (ancestor != id || flags & (LAYOUT | PAINT | PAINTED) != 0)
                {
                    node.layer_revision = node.layer_revision.wrapping_add(1);
                }
                current = node.parent;
            }
        }
        let mut current = Some(id);
        while let Some(ancestor) = current {
            let node = self.node(ancestor);
            if !node.ink_valid.replace(false) {
                break;
            }
            current = node.parent;
        }
        let node = self.node_mut(id);
        node.dirty |= flags;
        if flags & LAYOUT != 0 {
            node.text_size.set(TextSizes::default());
            node.reads.set(0);
        }
        if !node.queued {
            node.queued = true;
            self.dirty.push(id);
        }
    }
    /// Mark `id` and the ancestors whose geometry can depend on it. The walk
    /// stops at a layout boundary: its size cannot depend on its content, so
    /// its parent keeps its layout and only the boundary's subtree is redone.
    fn invalidate_layout(&mut self, id: NodeId) {
        self.invalidate(id, LAYOUT);
        let mut child = id;
        while let Some(parent) = self.node(child).parent {
            self.invalidate(parent, LAYOUT);
            if self.is_layout_boundary(parent) {
                self.layout_roots.push(parent);
                return;
            }
            child = parent;
        }
    }
    /// A laid-out container with an explicit width and height: parents measure
    /// it from those (or from their own flex allocation), never from its children.
    fn is_layout_boundary(&self, id: NodeId) -> bool {
        let node = self.node(id);
        matches!(node.kind, NodeKind::Container(_) | NodeKind::Panel { .. })
            && node.parent.is_some()
            && node.style.width.is_some()
            && node.style.height.is_some()
            && node.style.layout_options.is_none()
            && node.measurement.is_some()
    }
    /// Mark every ancestor up to the root, ignoring boundaries.
    fn invalidate_layout_to_root(&mut self, mut id: NodeId) {
        loop {
            self.invalidate(id, LAYOUT);
            match self.node(id).parent {
                Some(parent) => id = parent,
                None => break,
            }
        }
    }
    fn depth(&self, mut id: NodeId) -> usize {
        let mut depth = 0;
        while let Some(parent) = self.node(id).parent {
            depth += 1;
            id = parent;
        }
        depth
    }
    /// Whether this frame already repaints the whole viewport: more damage
    /// changes nothing, so callers skip working out where it would be.
    fn damage_is_full(&self) -> bool {
        self.damage.first() == Some(&self.viewport)
            || self.damage_grid.as_ref().is_some_and(DamageGrid::full)
    }
    fn add_damage(&mut self, rect: Rect) {
        // Raster coverage rounds fractional edges outwards. Quantize before merging
        // so adjacent subpixel rectangles cannot repaint the same alpha pixel twice,
        // and every primitive covering a touched pixel participates in its repaint.
        let rect = Rect::new(
            rect.x.floor(),
            rect.y.floor(),
            (rect.x + rect.width).ceil() - rect.x.floor(),
            (rect.y + rect.height).ceil() - rect.y.floor(),
        );
        let Some(mut rect) = rect.intersection(self.viewport) else {
            return;
        };
        let holds = |outer: &Rect| {
            outer.x <= rect.x
                && outer.y <= rect.y
                && outer.x + outer.width >= rect.x + rect.width
                && outer.y + outer.height >= rect.y + rect.height
        };
        if self.recent_damage.iter().any(holds) {
            return;
        }
        self.recent_damage.rotate_right(1);
        self.recent_damage[0] = rect;
        if let Some(grid) = &mut self.damage_grid {
            grid.mark(rect);
            return;
        }
        let mut index = 0;
        while index < self.damage.len() {
            if self.damage[index].intersects(rect) {
                rect = self.damage.swap_remove(index).union(rect);
                index = 0;
            } else {
                index += 1;
            }
        }
        self.damage.push(rect);
        if self.damage.len() > DAMAGE_RECTS {
            let mut grid = DamageGrid::new(self.viewport);
            for rect in self.damage.drain(..) {
                grid.mark(rect);
            }
            self.damage_grid = Some(grid);
        }
    }
    fn world(&self, id: NodeId) -> (Rect, Effects, Option<Rect>) {
        let node = self.node(id);
        let mut transform = node.transform;
        let mut effects = node.effects;
        if !own_visible(&node.style) {
            effects.opacity = 0.;
        }
        let mut parent = node.parent;
        while let Some(id) = parent {
            let ancestor = self.node(id);
            transform.x += ancestor.transform.x;
            transform.y += ancestor.transform.y;
            effects.opacity *= if own_visible(&ancestor.style) {
                ancestor.effects.opacity
            } else {
                0.
            };
            parent = ancestor.parent;
        }
        let bounds = node.bounds.translated(transform);
        let mut clip = Some(self.viewport);
        let mut current = Some(id);
        while let Some(id) = current {
            let ancestor = self.node(id);
            if clip_axes(&ancestor.style) != (false, false) {
                let mut offset = ancestor.transform;
                let mut parent = ancestor.parent;
                while let Some(parent_id) = parent {
                    let p = self.node(parent_id);
                    offset.x += p.transform.x;
                    offset.y += p.transform.y;
                    parent = p.parent;
                }
                clip = clipped_bounds(clip, ancestor.bounds.translated(offset), &ancestor.style);
            }
            current = ancestor.parent;
        }
        (bounds, effects, clip)
    }
    fn damage_subtree(&mut self, id: NodeId) {
        if self.damage_is_full() || !self.shown(id) {
            return;
        }
        // Resolve the ancestors once, then carry translation and clipping down
        // instead of walking back up from every descendant.
        if self.transformed + self.filtered > 0 {
            let (transform, clip, placement) = match self.node(id).parent {
                Some(parent) => self.context(parent),
                None => (Transform::default(), Some(self.viewport), None),
            };
            self.damage_subtree_placed(id, transform, clip, placement);
            return;
        }
        let (transform, clip) = match self.node(id).parent {
            Some(parent) => self.world_context(parent),
            None => (Transform::default(), Some(self.viewport)),
        };
        self.damage_subtree_in(id, transform, clip);
    }
    fn damage_subtree_in(&mut self, id: NodeId, parent: Transform, parent_clip: Option<Rect>) {
        let node = self.node(id);
        // Not laid out since it was appended, so never painted: nothing to
        // repaint yet (a mounting row styles each node before its first
        // layout, which damages where it lands). Nor are its descendants.
        let unplaced = node.measurement.is_none() && node.bounds == Rect::default();
        if !own_visible(&node.style) || unplaced || self.damage_is_full() {
            return;
        }
        let transform = Transform {
            x: parent.x + node.transform.x,
            y: parent.y + node.transform.y,
        };
        let bounds = node.bounds.translated(transform);
        let clip = if clip_axes(&node.style) != (false, false) {
            clipped_bounds(parent_clip, bounds, &node.style)
        } else {
            parent_clip
        };
        let own = match self.paints_past_clip(id) {
            true => parent_clip,
            false => clip,
        };
        self.damage_painted(id, bounds, node.effects.blur_radius, own);
        for index in 0..self.node(id).children.len() {
            let child = self.node(id).children[index];
            self.damage_subtree_in(child, transform, clip);
        }
    }
    /// `damage_subtree_in` where local transforms exist (see `descend`).
    fn damage_subtree_placed(
        &mut self,
        id: NodeId,
        parent: Transform,
        parent_clip: Option<Rect>,
        placement: Option<Placement>,
    ) {
        let node = self.node(id);
        let unplaced = node.measurement.is_none() && node.bounds == Rect::default();
        if !own_visible(&node.style) || unplaced || self.damage_is_full() {
            return;
        }
        let (transform, bounds, clip, placement) =
            self.descend(node, parent, parent_clip, placement);
        // Inside a layer it starts, nothing cuts its own paint.
        let own = match self.paints_past_clip(id) {
            true if node.transformed() || node.filtered() => None,
            true => parent_clip,
            false => clip,
        };
        self.damage_painted_placed(id, bounds, node.effects.blur_radius, own, placement);
        for index in 0..self.node(id).children.len() {
            let child = self.node(id).children[index];
            self.damage_subtree_placed(child, transform, clip, placement);
        }
    }
    /// What `node` paints under, given what its parent passes on: its
    /// translation, translated box, clip (in its layer's space; `None` cuts
    /// nothing, only inside a transformed layer) and its layer's placement.
    /// A local transform starts a layer: the clip around it moves into the
    /// placement, leaving only the node's own clip inside.
    fn descend(
        &self,
        node: &Node,
        parent: Transform,
        parent_clip: Option<Rect>,
        placement: Option<Placement>,
    ) -> (Transform, Rect, Option<Rect>, Option<Placement>) {
        let transform = Transform {
            x: parent.x + node.transform.x,
            y: parent.y + node.transform.y,
        };
        let bounds = node.bounds.translated(transform);
        if !node.transformed() && !node.filtered() {
            let clip = if clip_axes(&node.style) != (false, false) {
                clipped_bounds(parent_clip, bounds, &node.style)
            } else {
                parent_clip
            };
            return (transform, bounds, clip, placement);
        }
        // A content blur starts a layer as a transform does: it spreads
        // what the layer paints, in the layer's space, and its own clip
        // applies inside, before the blur.
        let matrix = node.local().matrix(bounds);
        let spread = match node.filtered() {
            true => filter_spread(node.effects.filter_blur) * stretch(matrix),
            false => 0.,
        };
        let placement = match placement {
            None => Placement {
                matrix,
                spread,
                clip: parent_clip.unwrap_or(self.viewport),
            },
            Some(outer) => Placement {
                matrix: matrix.then(outer.matrix),
                spread: outer.spread + spread * stretch(outer.matrix),
                // The outer layer's blur reaches past clips inside it.
                clip: match parent_clip {
                    Some(clip) => outer.shows(clip).unwrap_or_default(),
                    None => outer.clip,
                },
            },
        };
        let clip = clip_axes(&node.style)
            .ne(&(false, false))
            .then(|| clipped_bounds(None, bounds, &node.style))
            .flatten();
        (transform, bounds, clip, Some(placement))
    }
    /// What `id`'s children paint under (see `descend`), from the root down.
    fn context(&self, id: NodeId) -> (Transform, Option<Rect>, Option<Placement>) {
        let node = self.node(id);
        let (transform, clip, placement) = match node.parent {
            Some(parent) => self.context(parent),
            None => (Transform::default(), Some(self.viewport), None),
        };
        let (transform, _, clip, placement) = self.descend(node, transform, clip, placement);
        (transform, clip, placement)
    }
    /// World translation of `id` including its own, and the clip its children
    /// paint within.
    fn world_context(&self, id: NodeId) -> (Transform, Option<Rect>) {
        let (_, _, clip) = self.world(id);
        let mut transform = Transform::default();
        let mut current = Some(id);
        while let Some(id) = current {
            let node = self.node(id);
            transform.x += node.transform.x;
            transform.y += node.transform.y;
            current = node.parent;
        }
        (transform, clip)
    }
    /// See `Node::ink`. Computed on demand and cached until invalidated.
    fn ink(&self, id: NodeId) -> Option<Rect> {
        let node = self.node(id);
        if node.ink_valid.get() {
            return node.ink.get();
        }
        if node.effects.opacity <= 0.0 || !own_visible(&node.style) {
            node.ink.set(None);
            node.ink_valid.set(true);
            return None;
        }
        // Its overflow clip cuts what it holds; a box's own paint (its
        // shadows) reaches past it, as the paint walks draw it.
        let own = self.own_ink(id);
        let mut held: Option<Rect> = None;
        for child in &node.children {
            if let Some(child) = self.ink(*child) {
                held = Some(held.map_or(child, |ink| ink.union(child)));
            }
        }
        let held = held.and_then(|ink| clipped_bounds(Some(ink), node.bounds, &node.style));
        let own = match &node.kind {
            NodeKind::Quad(_) | NodeKind::Panel { .. } => own,
            _ => own.and_then(|ink| clipped_bounds(Some(ink), node.bounds, &node.style)),
        };
        let ink = match (own, held) {
            (Some(own), Some(held)) => Some(own.union(held)),
            (own, held) => own.or(held),
        };
        let ink = ink
            .map(|ink| match node.filtered() {
                false => ink,
                // Blurred in its layer's space, before the transform.
                true => ink.expand(filter_spread(node.effects.filter_blur)),
            })
            .map(|ink| match node.transformed() {
                false => ink,
                // About its untranslated box: translating after commutes.
                true => node.local().matrix(node.bounds).bounds(ink),
            })
            .map(|ink| ink.translated(node.transform));
        node.ink.set(ink);
        node.ink_valid.set(true);
        ink
    }
    /// Pixels this node itself can touch, matching what damage covers.
    fn own_ink(&self, id: NodeId) -> Option<Rect> {
        let node = self.node(id);
        let blur = node.effects.blur_radius;
        if matches!(node.kind, NodeKind::Container(_))
            && blur <= 0.0
            && !self.platform_nodes.contains(&id)
        {
            return None;
        }
        let bounds = node.bounds;
        Some(match &node.kind {
            NodeKind::Quad(quad) | NodeKind::Panel { quad, .. } => quad.paint_bounds(bounds),
            NodeKind::Image(image) => image.paint_bounds(bounds),
            NodeKind::Svg(svg) => svg.paint_bounds(bounds),
            NodeKind::Text { .. } | NodeKind::RichText { .. } => self.text_ink(id, bounds),
            _ => bounds,
        })
    }
    /// Where a text node's glyphs can land: its lines as laid out at its
    /// width, padded for glyph overhang, rather than its whole box. Text that
    /// changes every frame (a stream in a larger box) repaints only that. Text
    /// laid out otherwise (line clamps, ellipses, padding) or right to left
    /// keeps its box.
    fn text_ink(&self, id: NodeId, bounds: Rect) -> Rect {
        let node = self.node(id);
        let style = &node.style;
        let padded = style.padding != 0.
            || style
                .padding_edges
                .is_some_and(|edges| edges != Insets::default());
        if style.text_options != Default::default() || padded {
            return bounds;
        }
        // Renderers lay text out at its box's width, wrapping or not.
        let limit = Some(bounds.width);
        // What layout measured serves when it breaks lines as `limit` would:
        // a size measured wider (or unwrapped) whose widest line fits.
        let revision = self.text_shaper_revision;
        let cached = node
            .text_size
            .get()
            .0
            .iter()
            .flatten()
            .find_map(|(measured, at, size)| {
                let same = *at == revision
                    && match (limit, measured) {
                        (None, None) => true,
                        (Some(width), None) => size.0 <= width,
                        (Some(width), Some(measured)) => {
                            *measured == width || (*measured >= width && size.0 <= width)
                        }
                        (None, Some(_)) => false,
                    };
                same.then_some(*size)
            });
        // Measured here, the size serves this node's next damage too: a box
        // of fixed size is not measured by layout.
        let remember = |size: (f32, f32)| {
            if cached.is_none() {
                TextSizes::remember(&node.text_size, limit, revision, size);
            }
            size
        };
        let rtl = |text: &str| {
            text.chars().any(|c| {
                matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
            })
        };
        let (size, font_size, line, align) = match &node.kind {
            NodeKind::Text {
                text, font_size, ..
            } => {
                // Headless approximations are not what a renderer draws.
                if rtl(text) || (self.font_measurer.is_none() && self.font_shaper.is_none()) {
                    return bounds;
                }
                let size = remember(cached.unwrap_or_else(|| match &self.font_measurer {
                    Some(measure) => measure(text, *font_size, limit, &node.font),
                    None => self.measure_text_with_font(text, *font_size, limit, &node.font),
                }));
                let line = node.font.line_height.resolve(*font_size);
                (size, *font_size, line, node.font.align)
            }
            NodeKind::RichText { text } => {
                if text.options() != Default::default()
                    || rtl(text.text())
                    || !self.rich_measurer.is_native()
                {
                    return bounds;
                }
                let size = remember(
                    cached.unwrap_or_else(|| self.rich_measurer.measure_mounted(text, limit)),
                );
                let font_size = text
                    .runs()
                    .iter()
                    .map(|run| run.font_size)
                    .fold(0., f32::max);
                let line = text
                    .runs()
                    .iter()
                    .map(|run| run.font.line_height.resolve(run.font_size))
                    .fold(0., f32::max);
                let align = text
                    .runs()
                    .first()
                    .map_or_else(Default::default, |run| run.font.align);
                (size, font_size, line, align)
            }
            _ => return bounds,
        };
        // Lines starting inside the box are drawn whole.
        let (width, height) = (size.0, size.1.min(bounds.height + line));
        if !(width.is_finite() && height.is_finite()) {
            return bounds;
        }
        let x = match align {
            crate::text_layout::TextAlign::Start | crate::text_layout::TextAlign::Left => bounds.x,
            crate::text_layout::TextAlign::Center => bounds.x + (bounds.width - width) / 2.,
            crate::text_layout::TextAlign::Right => bounds.x + bounds.width - width,
        };
        // Italic and antialiased edges reach past advances and line boxes.
        let pad = (font_size * 0.3).max(2.);
        Rect::new(x - pad, bounds.y - pad, width + 2. * pad, height + 2. * pad)
    }
    /// Whether `id` and its ancestors are displayed: a hidden node paints
    /// nothing, so its changes damage nothing. Hiding one damages what it
    /// painted before the change.
    fn shown(&self, id: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(node) = current {
            let node = self.node(node);
            if !own_visible(&node.style) {
                return false;
            }
            current = node.parent;
        }
        true
    }
    fn damage_node(&mut self, id: NodeId) {
        if self.damage_is_full() || !self.shown(id) {
            return;
        }
        let past_clip = self.paints_past_clip(id);
        if self.transformed + self.filtered > 0 {
            let (bounds, clip, placement) = self.placed(id);
            let node = self.node(id);
            let blur_radius = node.effects.blur_radius;
            let clip = match (past_clip, node.parent) {
                (false, _) => clip,
                _ if node.transformed() || node.filtered() => None,
                (true, Some(parent)) => self.context(parent).1,
                (true, None) => Some(self.viewport),
            };
            self.damage_painted_placed(id, bounds, blur_radius, clip, placement);
            return;
        }
        let (bounds, effects, clip) = self.world(id);
        let clip = match (past_clip, self.node(id).parent) {
            (false, _) => clip,
            (true, Some(parent)) => self.world(parent).2,
            (true, None) => Some(self.viewport),
        };
        self.damage_painted(id, bounds, effects.blur_radius, clip);
    }
    fn damage_painted(&mut self, id: NodeId, bounds: Rect, blur_radius: f32, clip: Option<Rect>) {
        if let Some(bounds) = self.painted(id, bounds, blur_radius)
            // A backdrop filter samples only inside its bounds.
            && let Some(bounds) = clip.and_then(|clip| bounds.intersection(clip))
        {
            self.add_damage(bounds);
        }
    }
    /// Damage what `id` itself paints at `bounds` within `clip`, mapped onto
    /// the viewport by its layer's `placement`. Inside a transformed layer, no
    /// `clip` cuts nothing; outside one, everything.
    fn damage_painted_placed(
        &mut self,
        id: NodeId,
        bounds: Rect,
        blur_radius: f32,
        clip: Option<Rect>,
        placement: Option<Placement>,
    ) {
        let Some(placement) = placement else {
            return self.damage_painted(id, bounds, blur_radius, clip);
        };
        if let Some(bounds) = self
            .painted(id, bounds, blur_radius)
            .and_then(|bounds| bounds.intersection(clip.unwrap_or(UNCLIPPED)))
            .and_then(|visible| placement.shows(visible))
        {
            self.add_damage(bounds);
        }
    }
    /// Whether `id` clips what it holds but not its own paint: a box, whose
    /// shadows and outline reach past its overflow clip as the paint walks
    /// draw them. Its own paint is damaged within its parent's clip then.
    fn paints_past_clip(&self, id: NodeId) -> bool {
        let node = self.node(id);
        matches!(node.kind, NodeKind::Quad(_) | NodeKind::Panel { .. })
            && clip_axes(&node.style) != (false, false)
    }
    /// What `id` itself paints when laid out at `bounds`, if anything.
    #[inline]
    fn painted(&self, id: NodeId, bounds: Rect, blur_radius: f32) -> Option<Rect> {
        if matches!(self.node(id).kind, NodeKind::Container(_)) && blur_radius <= 0.0 {
            return None;
        }
        Some(match &self.node(id).kind {
            NodeKind::Quad(quad) | NodeKind::Panel { quad, .. } => quad.paint_bounds(bounds),
            NodeKind::Image(image) => image.paint_bounds(bounds),
            NodeKind::Svg(svg) => svg.paint_bounds(bounds),
            NodeKind::Text { .. } | NodeKind::RichText { .. } if blur_radius <= 0.0 => {
                self.text_ink(id, bounds)
            }
            _ => bounds,
        })
    }
    /// `constraints` with what `id`'s result cannot depend on dropped, so
    /// probes that differ only there share one result: an available size
    /// a forced one overrides, percentage bases it has no percentages
    /// against, and available sizes a leaf never reads (text lays out at
    /// its available width only when it wraps).
    fn relevant(&self, id: NodeId, mut constraints: Constraints) -> Constraints {
        let (widths, heights) = self.percent_bases(id);
        if !widths {
            constraints.percent_width = None;
        }
        if !heights {
            constraints.percent_height = None;
        }
        let (reads_width, reads_height) = self.reads_available(id);
        if constraints.force_width.is_some() || !reads_width {
            constraints.width = constraints.force_width;
        }
        if constraints.force_height.is_some() || !reads_height {
            constraints.height = constraints.force_height;
        }
        constraints
    }
    /// Whether `id` resolves percentages against the (width, height) basis
    /// its parent passes (`Constraints::percent_width`, `percent_height`): its
    /// own percentage sizes, limits and padding. Its children's bases come
    /// from its own size.
    pub(super) fn percent_bases(&self, id: NodeId) -> (bool, bool) {
        let node = self.node(id);
        let style = &node.style;
        let options = style.layout_options.as_deref();
        let percent = |length: Option<crate::layout::Length>| {
            matches!(length, Some(crate::layout::Length::Percent(_)))
        };
        let widths = node.parent.is_some()
            && (style.width_percent.is_some()
                || options.is_some_and(|o| {
                    percent(o.min_width)
                        || percent(o.max_width)
                        || [
                            o.padding.left,
                            o.padding.right,
                            o.padding.top,
                            o.padding.bottom,
                        ]
                        .into_iter()
                        .any(percent)
                }));
        let heights = style.height_percent.is_some()
            || options.is_some_and(|o| percent(o.min_height) || percent(o.max_height));
        (widths, heights)
    }
    /// Whether `id`'s size can depend on the available (width, height) it is
    /// measured under, unless forced. Text reads its width when it wraps;
    /// other leaves size themselves. A container passes what it is given to
    /// its children unless its own size fixes it, so it reads what they do;
    /// a taffy-laid-out one also fits its auto width to the room, and wraps
    /// a column or sizes grid tracks within the height. Most subtrees read
    /// no height: probes that differ only in it share one result. Cached
    /// until its layout is invalidated, which reaches every ancestor up to a
    /// fixed-size boundary (which reads neither).
    fn reads_available(&self, id: NodeId) -> (bool, bool) {
        const KNOWN: u8 = 1;
        const WIDTH: u8 = 2;
        const HEIGHT: u8 = 4;
        let node = self.node(id);
        let cached = node.reads.get();
        if cached & KNOWN != 0 {
            return (cached & WIDTH != 0, cached & HEIGHT != 0);
        }
        let style = &node.style;
        let (width, height) = match &node.kind {
            NodeKind::Text { .. } => (
                style.text_wrap || style.text_options != Default::default(),
                false,
            ),
            NodeKind::RichText { text } => (
                style.text_wrap || text.options() != Default::default(),
                false,
            ),
            NodeKind::Container(layout) | NodeKind::Panel { layout, .. } => {
                let mut children = (false, false);
                for child in &node.children {
                    if self.node(*child).style.absolute {
                        continue;
                    }
                    let (w, h) = self.reads_available(*child);
                    children = (children.0 || w, children.1 || h);
                }
                // A row fits its shrinkable children into the room it has.
                if *layout == Layout::Row
                    && node.children.iter().any(|child| {
                        let s = &self.node(*child).style;
                        !s.absolute && s.flex_shrink > 0.
                    })
                {
                    children.0 = true;
                }
                let (width, height) = if self.uses_advanced_layout(id) {
                    let options = style.layout_options.as_deref().copied().unwrap_or_default();
                    let wraps_column = *layout != Layout::Row && options.wrap.is_some();
                    let grid = options.display == Some(crate::layout::Display::Grid);
                    (true, children.1 || wraps_column || grid)
                } else {
                    children
                };
                (
                    width && style.width.is_none(),
                    height && style.height.is_none(),
                )
            }
            _ => (false, false),
        };
        node.reads
            .set(KNOWN | if width { WIDTH } else { 0 } | if height { HEIGHT } else { 0 });
        (width, height)
    }
    /// `id`'s size under `constraints`, for a probe: a kept size serves,
    /// leaving whichever result `id` holds whole as it was.
    fn measure(&mut self, id: NodeId, constraints: Constraints) -> (f32, f32) {
        self.measured(id, constraints, false)
    }
    /// `id` laid out under `constraints`, the result it then holds (with its
    /// children's places) for `arrange`.
    fn measure_full(&mut self, id: NodeId, constraints: Constraints) -> (f32, f32) {
        self.measured(id, constraints, true)
    }
    fn measured(&mut self, id: NodeId, constraints: Constraints, full: bool) -> (f32, f32) {
        #[cfg(test)]
        {
            self.measure_calls += 1;
        }
        let constraints = self.relevant(id, constraints);
        let node = self.node(id);
        let pass = self.layout_pass;
        let clean = node.dirty & LAYOUT == 0;
        let usable = |m: &Measurement| m.constraints == constraints && (clean || m.pass == pass);
        if let Some(current) = &node.measurement
            && usable(current)
        {
            return current.size;
        }
        if let Some(index) = node.alternates.iter().position(usable) {
            // Promote it: `arrange` places children from the current result.
            let node = self.node_mut(id);
            let hit = node.alternates.remove(index);
            let size = hit.size;
            if let Some(previous) = node.measurement.replace(hit) {
                node.alternates.insert(0, previous);
            }
            node.arrange_dirty = true;
            return size;
        }
        if !full
            && let Some(kept) = node
                .sizes
                .iter()
                .find(|m| m.constraints == constraints && (clean || m.pass == pass))
        {
            return kept.size;
        }
        let node = self.node(id);
        if let NodeKind::Container(layout) | NodeKind::Panel { layout, .. } = node.kind
            && self.uses_advanced_layout(id)
        {
            return self.measure_advanced(id, layout, constraints);
        }
        self.node_mut(id).advanced_layout = None;
        let style = self.resolved_leaf_style(id, constraints);
        let padding = style.padding_edges.unwrap_or(Insets::all(style.padding));
        let padding = Insets {
            top: padding.top.max(0.0),
            right: padding.right.max(0.0),
            bottom: padding.bottom.max(0.0),
            left: padding.left.max(0.0),
        };
        let horizontal_padding = padding.left + padding.right;
        let vertical_padding = padding.top + padding.bottom;
        let explicit_w = constraints
            .force_width
            .or(style.width)
            .or_else(|| percentage(style.width_percent, constraints.percent_width))
            .map(|w| dimension(w, style.min_width, style.max_width));
        let explicit_h = constraints
            .force_height
            .or(style.height)
            .or_else(|| percentage(style.height_percent, constraints.percent_height))
            .map(|h| dimension(h, style.min_height, style.max_height));
        let available_w = explicit_w
            .or_else(|| {
                constraints
                    .width
                    .map(|w| w.min(style.max_width.unwrap_or(f32::INFINITY)))
            })
            .or(style.max_width)
            .map(|w| (w - horizontal_padding).max(0.0));
        let available_h = explicit_h
            .or_else(|| {
                constraints
                    .height
                    .map(|h| h.min(style.max_height.unwrap_or(f32::INFINITY)))
            })
            .or(style.max_height)
            .map(|h| (h - vertical_padding).max(0.0));
        let percent_width = explicit_w.map(|w| (w - horizontal_padding).max(0.0));
        let percent_height = explicit_h.map(|h| (h - vertical_padding).max(0.0));
        let kind = self.node(id).kind.clone();
        let mut children = Vec::new();
        let mut child_constraints = Vec::new();
        let (mut width, mut height) = match kind {
            NodeKind::Text {
                text, font_size, ..
            } => {
                let limit = (style.text_wrap || style.text_options != Default::default())
                    .then_some(available_w)
                    .flatten();
                let revision = self.text_shaper_revision;
                if let Some(size) = self.node(id).text_size.get().find(limit, revision) {
                    size
                } else {
                    let size = if style.text_options != Default::default() {
                        let runs = if text.is_empty() {
                            Vec::new()
                        } else {
                            vec![crate::rich_text::TextRun {
                                range: 0..text.len(),
                                font: self.node(id).font.clone(),
                                font_size,
                                ..Default::default()
                            }]
                        };
                        let rich = crate::rich_text::RichText::new(text, runs)
                            .unwrap()
                            .with_options(style.text_options);
                        self.rich_measurer.measure_mounted(&rich, limit)
                    } else {
                        let font = &self.node(id).font;
                        match &self.font_measurer {
                            Some(measure) => measure(&text, font_size, limit, font),
                            None => self.measure_text_with_font(&text, font_size, limit, font),
                        }
                    };
                    TextSizes::remember(&self.node(id).text_size, limit, revision, size);
                    size
                }
            }
            NodeKind::RichText { text } => {
                let limit = (style.text_wrap || text.options() != Default::default())
                    .then_some(available_w)
                    .flatten();
                // Same cache as plain text: colour-only changes keep layout
                // (see `set_kind`), so they keep this size too.
                let revision = self.text_shaper_revision;
                if let Some(size) = self.node(id).text_size.get().find(limit, revision) {
                    size
                } else {
                    let size = self.rich_measurer.measure_mounted(&text, limit);
                    TextSizes::remember(&self.node(id).text_size, limit, revision, size);
                    size
                }
            }
            NodeKind::Rect(_) | NodeKind::Quad(_) | NodeKind::Canvas(_) => (0.0, 0.0),
            NodeKind::Svg(_) => (24., 24.),
            NodeKind::Image(image) => (image.width() as f32, image.height() as f32),
            #[cfg(target_os = "macos")]
            NodeKind::NativeSurface(frame) => (frame.width() as f32, frame.height() as f32),
            NodeKind::Container(layout) | NodeKind::Panel { layout, .. } => {
                let (absolute, ids): (Vec<_>, Vec<_>) = self
                    .node(id)
                    .children
                    .iter()
                    .copied()
                    .partition(|child| self.node(*child).style.absolute);
                let horizontal = layout == Layout::Row;
                // The axes a child fills (stretch): the cross axis, both in an
                // overlay, unless it sizes itself there.
                let stretches = |s: &Style| {
                    let fills = style.align == Align::Stretch;
                    (
                        fills
                            && layout != Layout::Row
                            && s.width.is_none()
                            && percentage(s.width_percent, percent_width).is_none(),
                        fills
                            && layout != Layout::Column
                            && s.height.is_none()
                            && percentage(s.height_percent, percent_height).is_none(),
                    )
                };
                // Where this container's own size fixes a stretched axis, its
                // children are sized at the stretched size from the start, as
                // CSS sizes a flex item's content at its stretched cross size:
                // wrapping text then takes the lines it will show, and its
                // height is the one flex distributes.
                let filled = |scene: &Self, child: NodeId| {
                    let s = &scene.node(child).style;
                    let (w, h) = stretches(s);
                    let m = s.margin;
                    (
                        explicit_w
                            .filter(|_| w)
                            .map(|outer| (outer - horizontal_padding - m.left - m.right).max(0.)),
                        explicit_h
                            .filter(|_| h)
                            .map(|outer| (outer - vertical_padding - m.top - m.bottom).max(0.)),
                    )
                };
                let natural = Constraints {
                    width: available_w,
                    height: available_h,
                    percent_width,
                    percent_height,
                    ..Constraints::default()
                };
                // What each child's size was last asked under: its place is
                // that result's (see `store_measurement`).
                let mut asked: Vec<_> = ids
                    .iter()
                    .map(|child| {
                        let (width, height) = filled(self, *child);
                        Constraints {
                            force_width: width,
                            force_height: height,
                            ..natural
                        }
                    })
                    .collect();
                let mut sizes: Vec<_> = (0..ids.len())
                    .map(|i| self.measure(ids[i], asked[i]))
                    .collect();
                // Children whose main size flex distribution decided.
                let mut allocated = vec![false; ids.len()];
                let main_margin = |s: &Style| {
                    if horizontal {
                        s.margin.left + s.margin.right
                    } else {
                        s.margin.top + s.margin.bottom
                    }
                };
                let main = |size: (f32, f32)| if horizontal { size.0 } else { size.1 };
                // `asked` with the main size set to `size`, the cross kept.
                let along = |asked: Constraints, size: f32| Constraints {
                    force_width: if horizontal {
                        Some(size)
                    } else {
                        asked.force_width
                    },
                    force_height: if horizontal {
                        asked.force_height
                    } else {
                        Some(size)
                    },
                    ..asked
                };
                let gaps = style.gap * ids.len().saturating_sub(1) as f32;
                let natural_main = sizes
                    .iter()
                    .enumerate()
                    .map(|(i, size)| main(*size) + main_margin(&self.node(ids[i]).style))
                    .sum::<f32>()
                    + gaps;
                let main_limit = if horizontal {
                    // A row sized by its content, given less room than that,
                    // fits it (CSS fit-content): shrinkable children shrink,
                    // and what cannot shrink still overflows.
                    let content = explicit_w.unwrap_or(match available_w {
                        Some(room) => natural_main.min(room) + horizontal_padding,
                        None => natural_main + horizontal_padding,
                    });
                    Some(dimension(content, style.min_width, style.max_width) - horizontal_padding)
                } else {
                    Some(
                        dimension(
                            explicit_h.unwrap_or(natural_main + vertical_padding),
                            style.min_height,
                            style.max_height,
                        ) - vertical_padding,
                    )
                };
                if layout != Layout::Overlay
                    && let Some(limit) = main_limit
                {
                    let bases: Vec<_> = sizes.iter().copied().map(main).collect();
                    let mut frozen = vec![false; ids.len()];
                    for _ in 0..=ids.len() {
                        let used: f32 = sizes
                            .iter()
                            .enumerate()
                            .map(|(i, size)| main(*size) + main_margin(&self.node(ids[i]).style))
                            .sum::<f32>()
                            + gaps;
                        let remaining = limit - used;
                        if remaining.abs() < 0.001 {
                            break;
                        }
                        let weights: Vec<_> = ids
                            .iter()
                            .enumerate()
                            .map(|(i, child)| {
                                let s = &self.node(*child).style;
                                if frozen[i] {
                                    0.0
                                } else if remaining > 0.0 {
                                    s.flex_grow.max(0.0)
                                } else {
                                    s.flex_shrink.max(0.0) * bases[i]
                                }
                            })
                            .collect();
                        let weight: f32 = weights.iter().sum();
                        if weight <= 0.0 {
                            break;
                        }
                        let mut clamped = false;
                        for (i, child) in ids.iter().enumerate() {
                            if weights[i] == 0.0 {
                                continue;
                            }
                            let s = &self.node(*child).style;
                            let wanted = main(sizes[i]) + remaining * weights[i] / weight;
                            let size = if horizontal {
                                dimension(wanted, s.min_width, s.max_width)
                            } else {
                                dimension(wanted, s.min_height, s.max_height)
                            };
                            if (size - wanted).abs() > 0.001 {
                                frozen[i] = true;
                                clamped = true;
                            }
                            asked[i] = along(asked[i], size);
                            allocated[i] = true;
                            sizes[i] = self.measure(*child, asked[i]);
                        }
                        if !clamped {
                            break;
                        }
                    }
                }
                // A flex item receives a definite main allocation even when
                // its natural size already consumes exactly the available space.
                // Its percentage descendants must see that allocation as a basis.
                if layout != Layout::Overlay
                    && if horizontal {
                        explicit_w.is_some()
                    } else {
                        explicit_h.is_some()
                    }
                {
                    for (i, child) in ids.iter().enumerate() {
                        let child_style = &self.node(*child).style;
                        if child_style.flex_grow > 0.0 || child_style.flex_shrink > 0.0 {
                            asked[i] = along(asked[i], main(sizes[i]));
                            allocated[i] = true;
                            sizes[i] = self.measure(*child, asked[i]);
                        }
                    }
                }
                // The children's extent: across rows (or columns) and gaps,
                // and the widest (or tallest), with their margins.
                let margins: Vec<Insets> = ids.iter().map(|c| self.node(*c).style.margin).collect();
                let extent = |sizes: &[(f32, f32)]| {
                    let (mut natural_w, mut natural_h) = (0.0_f32, 0.0_f32);
                    for (i, m) in margins.iter().enumerate() {
                        let (w, h) = (sizes[i].0 + m.left + m.right, sizes[i].1 + m.top + m.bottom);
                        match layout {
                            Layout::Row => {
                                natural_w += w;
                                natural_h = natural_h.max(h);
                            }
                            Layout::Column => {
                                natural_h += h;
                                natural_w = natural_w.max(w);
                            }
                            Layout::Overlay => {
                                natural_w = natural_w.max(w);
                                natural_h = natural_h.max(h);
                            }
                        }
                    }
                    match layout {
                        Layout::Row => (natural_w + gaps, natural_h),
                        Layout::Column => (natural_w, natural_h + gaps),
                        Layout::Overlay => (natural_w, natural_h),
                    }
                };
                let inner = |(natural_w, natural_h): (f32, f32)| {
                    (
                        dimension(
                            explicit_w.unwrap_or(natural_w + horizontal_padding),
                            style.min_width,
                            style.max_width,
                        ) - horizontal_padding,
                        dimension(
                            explicit_h.unwrap_or(natural_h + vertical_padding),
                            style.min_height,
                            style.max_height,
                        ) - vertical_padding,
                    )
                };
                // Stretched along an axis the container sizes to its widest
                // (or tallest) child: there, now that it is known. The main
                // size stays forced only where flex decided it; otherwise the
                // child takes its natural main size at the stretched size
                // (wrapped text grows taller), and the extent follows.
                let (inner_w, inner_h) = inner(extent(&sizes));
                for (i, child) in ids.iter().enumerate() {
                    let s = &self.node(*child).style;
                    let (w, h) = stretches(s);
                    let m = s.margin;
                    let stretch_w = w && asked[i].force_width.is_none();
                    let stretch_h = h && asked[i].force_height.is_none();
                    if !stretch_w && !stretch_h {
                        continue;
                    }
                    asked[i] = Constraints {
                        force_width: match stretch_w {
                            true => Some((inner_w - m.left - m.right).max(0.0)),
                            false => asked[i].force_width,
                        },
                        force_height: match stretch_h {
                            true => Some((inner_h - m.top - m.bottom).max(0.0)),
                            false => asked[i].force_height,
                        },
                        ..asked[i]
                    };
                    if horizontal && allocated[i] {
                        asked[i].force_width = Some(sizes[i].0);
                    }
                    if !horizontal && layout != Layout::Overlay && allocated[i] {
                        asked[i].force_height = Some(sizes[i].1);
                    }
                    sizes[i] = self.measure(*child, asked[i]);
                }
                let (natural_w, natural_h) = extent(&sizes);
                let (inner_w, inner_h) = inner((natural_w, natural_h));
                let extra = if horizontal {
                    inner_w - natural_w
                } else {
                    inner_h - natural_h
                };
                let (mut cursor, extra_gap) = match style.justify {
                    Justify::Start => (0.0, 0.0),
                    Justify::Center => (extra / 2.0, 0.0),
                    Justify::End => (extra, 0.0),
                    Justify::SpaceBetween if ids.len() > 1 => {
                        (0.0, extra.max(0.0) / (ids.len() - 1) as f32)
                    }
                    Justify::SpaceAround if !ids.is_empty() => (
                        extra.max(0.0) / ids.len() as f32 / 2.0,
                        extra.max(0.0) / ids.len() as f32,
                    ),
                    Justify::SpaceEvenly => {
                        let gap = extra.max(0.0) / (ids.len() + 1) as f32;
                        (gap, gap)
                    }
                    _ => (0.0, 0.0),
                };
                for (i, child) in ids.iter().enumerate() {
                    let s = self.node(*child).style.clone();
                    let m = s.margin;
                    let (w, h) = sizes[i];
                    let cross_free = if horizontal {
                        inner_h - h - m.top - m.bottom
                    } else {
                        inner_w - w - m.left - m.right
                    };
                    let cross = match style.align {
                        Align::Center => cross_free / 2.0,
                        Align::End => cross_free,
                        _ => 0.0,
                    };
                    let (cx, cy) = match layout {
                        Layout::Row => (cursor + m.left, cross + m.top),
                        Layout::Column => (cross + m.left, cursor + m.top),
                        Layout::Overlay => (cross + m.left, m.top),
                    };
                    children.push((*child, Rect::new(cx + padding.left, cy + padding.top, w, h)));
                    child_constraints.push(Some(asked[i]));
                    cursor += main(sizes[i]) + main_margin(&s) + style.gap + extra_gap;
                }
                for child in absolute {
                    let margin = self.node(child).style.margin;
                    let placed = Constraints {
                        width: Some((inner_w - margin.left - margin.right).max(0.0)),
                        height: Some((inner_h - margin.top - margin.bottom).max(0.0)),
                        percent_width: Some(inner_w.max(0.0)),
                        percent_height: Some(inner_h.max(0.0)),
                        ..Constraints::default()
                    };
                    let (w, h) = self.measure(child, placed);
                    child_constraints.push(Some(placed));
                    children.push((
                        child,
                        Rect::new(padding.left + margin.left, padding.top + margin.top, w, h),
                    ));
                }
                (natural_w, natural_h)
            }
        };
        width = dimension(
            explicit_w.unwrap_or(width + horizontal_padding),
            style.min_width,
            style.max_width,
        );
        height = dimension(
            explicit_h.unwrap_or(height + vertical_padding),
            style.min_height,
            style.max_height,
        );
        self.store_measurement(
            id,
            constraints,
            (width, height),
            children,
            child_constraints,
        );
        (width, height)
    }
    /// Make a new result current, keeping earlier ones that are still valid.
    /// `child_constraints`: what each child's place was measured under,
    /// which `arrange` lays it out by again.
    fn store_measurement(
        &mut self,
        id: NodeId,
        constraints: Constraints,
        size: (f32, f32),
        children: Vec<(NodeId, Rect)>,
        child_constraints: Vec<Option<Constraints>>,
    ) {
        // As `measure` looks results up (`measure_hidden` stores directly).
        let constraints = self.relevant(id, constraints);
        let pass = self.layout_pass;
        let node = self.node_mut(id);
        let dirty = node.dirty & LAYOUT != 0;
        if let Some(previous) = node.measurement.take() {
            node.alternates.insert(0, previous);
        }
        // Results from before a node changed are stale; of the rest, all of
        // this pass's and the most recent few of earlier ones.
        // Earlier results past the few kept whole keep their sizes.
        let mut earlier = 0;
        let mut evicted = Vec::new();
        node.alternates.retain(|m| {
            let valid = m.constraints != constraints && (m.pass == pass || !dirty);
            let whole = m.pass == pass || earlier < MEASUREMENT_ALTERNATES;
            earlier += usize::from(valid && whole && m.pass != pass);
            if valid && !whole {
                evicted.push(ProbeSize::from(m));
            }
            valid && whole
        });
        evicted.extend(
            node.alternates
                .drain(PASS_RESULTS.min(node.alternates.len())..)
                .map(|m| ProbeSize::from(&m)),
        );
        node.sizes
            .retain(|m| m.constraints != constraints && (m.pass == pass || !dirty));
        keep_sizes(&mut node.sizes, evicted);
        let overfull = node.alternates.len() == MEASUREMENT_ALTERNATES + 1;
        node.arrange_dirty = true;
        node.measurement = Some(Measurement {
            constraints,
            size,
            children,
            child_constraints,
            pass,
        });
        if overfull {
            self.overfull.push(id);
        }
    }
    /// `covered`: an ancestor moved and already damaged this whole subtree at
    /// its old and new positions.
    fn arrange(&mut self, id: NodeId, x: f32, y: f32, report: &mut FrameReport, covered: bool) {
        let cache = self.node(id).measurement.as_ref().unwrap();
        let bounds = Rect::new(x, y, cache.size.0, cache.size.1);
        if !self.node(id).arrange_dirty && self.node(id).bounds == bounds {
            return;
        }
        let children = cache.children.clone();
        let child_constraints = cache.child_constraints.clone();
        let changed = self.node(id).bounds != bounds;
        if changed || self.node(id).arrange_dirty {
            report.layout_nodes += 1;
        }
        let damage = changed && !covered;
        if damage {
            self.damage_subtree(id);
        }
        if changed {
            self.node_mut(id).bounds = bounds;
            self.invalidate(id, PAINTED);
        }
        for ((child, rect), constraints) in children.into_iter().zip(child_constraints) {
            // Cached: promotes the matching result; evicted: re-measures.
            if let Some(constraints) = constraints {
                self.measure_full(child, constraints);
            }
            self.arrange(child, x + rect.x, y + rect.y, report, covered || changed);
        }
        if damage {
            self.damage_subtree(id);
        }
        self.node_mut(id).dirty &= !LAYOUT;
        self.node_mut(id).arrange_dirty = false;
    }
    pub(crate) fn geometry_revision(&self) -> u64 {
        self.geometry_revision
    }
    pub(crate) fn layout_revision(&self) -> u64 {
        self.layout_revision
    }
    /// Updates geometry for hit testing without consuming pending paint damage.
    /// The next `flush` reports this work and all damage accumulated by layout.
    pub fn prepare_layout(&mut self) {
        if self.node(self.root).dirty & LAYOUT == 0 && self.layout_roots.is_empty() {
            return;
        }
        self.layout_pass = self.layout_pass.wrapping_add(1);
        let mut report = FrameReport::default();
        if self.node(self.root).dirty & LAYOUT != 0 {
            self.measure_full(self.root, Constraints::default());
            self.arrange(self.root, 0.0, 0.0, &mut report, false);
        }
        // Boundaries under clean ancestors keep their place and size; lay out
        // just their subtrees, outermost first.
        let mut roots = std::mem::take(&mut self.layout_roots);
        roots.retain(|id| self.contains(*id));
        roots.sort_by_key(|id| (self.depth(*id), id.index, id.generation));
        roots.dedup();
        let mut relayout_all = false;
        for id in roots {
            if !self.contains(id) || self.node(id).dirty & LAYOUT == 0 {
                continue;
            }
            let Some((constraints, size)) = self
                .node(id)
                .measurement
                .as_ref()
                .map(|m| (m.constraints, m.size))
            else {
                relayout_all = true;
                self.invalidate_layout_to_root(id);
                continue;
            };
            if self.measure_full(id, constraints) != size {
                // Not actually independent of its content: lay out normally.
                relayout_all = true;
                self.invalidate_layout_to_root(id);
                continue;
            }
            let bounds = self.node(id).bounds;
            self.arrange(id, bounds.x, bounds.y, &mut report, false);
        }
        if relayout_all && self.node(self.root).dirty & LAYOUT != 0 {
            self.measure_full(self.root, Constraints::default());
            self.arrange(self.root, 0.0, 0.0, &mut report, false);
        }
        // Probes this pass answered stay only as the next pass's few.
        for id in std::mem::take(&mut self.overfull) {
            if self.contains(id) {
                let node = self.node_mut(id);
                let at = MEASUREMENT_ALTERNATES.min(node.alternates.len());
                let evicted: Vec<_> = node
                    .alternates
                    .drain(at..)
                    .map(|m| ProbeSize::from(&m))
                    .collect();
                keep_sizes(&mut node.sizes, evicted);
            }
        }
        self.pending_layout_nodes += report.layout_nodes;
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.geometry_revision = self.geometry_revision.wrapping_add(1);
    }
    pub fn flush(&mut self) -> FrameReport {
        let mut report = FrameReport::default();
        if self.dirty.is_empty() {
            return report;
        }
        self.prepare_layout();
        report.layout_nodes = std::mem::take(&mut self.pending_layout_nodes);
        let dirty = std::mem::take(&mut self.dirty);
        for id in &dirty {
            if !self.contains(*id) {
                continue;
            }
            let flags = self.node(*id).dirty;
            if flags & (PAINT | PAINTED) != 0 {
                report.paint_nodes += 1;
            }
            if flags & PAINT != 0 {
                self.damage_node(*id);
            }
            if flags & COMPOSITE != 0 {
                report.composite_nodes += 1;
            }
            let node = self.node_mut(*id);
            node.dirty = 0;
            node.queued = false;
        }
        self.dirty = dirty;
        self.dirty.clear();
        // Backdrop-filter output depends on pixels behind it, even when its own node is clean.
        if (!self.damage.is_empty() || self.damage_grid.is_some()) && !self.blur_nodes.is_empty() {
            let mut blur_regions: Vec<(Rect, bool)> = self
                .blur_nodes
                .iter()
                .filter_map(|id| {
                    let (bounds, effects, _) = self.world(*id);
                    // Fully transparent filters cannot change the visible backdrop.
                    // Hide/show still damages both old and new subtree bounds.
                    let bounds = match self.transformed {
                        0 => bounds,
                        _ => self.transformed_bounds(*id),
                    };
                    (effects.opacity > 0.0).then_some((bounds, false))
                })
                .collect();
            loop {
                let mut changed = false;
                for (rect, processed) in &mut blur_regions {
                    let touched = match &self.damage_grid {
                        Some(grid) => grid.touches(*rect),
                        None => self.damage.iter().any(|damage| damage.intersects(*rect)),
                    };
                    if !*processed && touched {
                        self.add_damage(*rect);
                        *processed = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        self.resolve_moves();
        report.damage = match self.damage_grid.take() {
            Some(grid) if grid.full() => vec![self.viewport],
            Some(grid) => grid.rects(self.viewport),
            None => std::mem::take(&mut self.damage),
        };
        self.recent_damage = Default::default();
        if !report.damage.is_empty() || !self.moves.is_empty() {
            self.flush_serial += 1;
            for scroll in &mut self.moves {
                scroll.serial = self.flush_serial;
            }
        }
        report
    }
    /// Mark `id` as scroll content: translating it reports a `ScrollMove`
    /// instead of damaging everything it paints.
    pub fn set_scroll_copy(&mut self, id: NodeId, enabled: bool) {
        self.node_mut(id).scroll_copy = enabled;
    }
    /// Moves reported by the most recent flush.
    pub fn scroll_moves(&self) -> &[ScrollMove] {
        &self.moves
    }
    /// Counts flushes that reported damage or moves.
    pub fn flush_serial(&self) -> u64 {
        self.flush_serial
    }
    /// Whether `id` is `ancestor` or inside it.
    pub fn is_within(&self, mut id: NodeId, ancestor: NodeId) -> bool {
        loop {
            if id == ancestor {
                return true;
            }
            match self.node(id).parent {
                Some(parent) => id = parent,
                None => return false,
            }
        }
    }
    /// Turn this frame's scroll translations into moves: damage what a copy
    /// cannot supply, or everything the content painted where a copy is
    /// unsafe (isolated ancestors, backdrop filters over the clip, nested or
    /// overlapping moves, jumps past the viewport).
    fn resolve_moves(&mut self) {
        self.moves.clear();
        let pending = std::mem::take(&mut self.pending_moves);
        let mut moves = Vec::new();
        for (id, from, again) in pending {
            if !self.contains(id) {
                continue;
            }
            let to = self.node(id).transform;
            let (dx, dy) = (to.x - from.x, to.y - from.y);
            // A layer's pixels are its own: a copy of the window's moves
            // nothing in it.
            let layered = std::iter::once(id)
                .chain(self.ancestors(id))
                .any(|node| self.is_layer(node));
            if again && self.transformed + self.filtered > 0 {
                // Its clip, wherever transformed or blurred ancestors show it.
                let (_, clip, placement) = self.placed(id);
                let shown = match (placement, clip) {
                    (Some(placement), Some(clip)) => placement.shows(clip),
                    (Some(placement), None) => Some(placement.clip),
                    (None, clip) => Some(clip.unwrap_or(self.viewport)),
                };
                if let Some(shown) = shown {
                    self.add_damage(shown);
                }
                continue;
            }
            if again {
                // Descendants changed at an intermediate position (a measured
                // list re-anchoring between layout passes, possibly back to
                // where it started) were damaged neither where they were
                // painted nor where they end: repaint everything it shows.
                match self.world(id).2 {
                    Some(clip) => self.add_damage(clip),
                    None => self.add_damage(self.viewport),
                }
                continue;
            }
            if dx == 0. && dy == 0. {
                continue;
            }
            let (_, _, clip) = self.world(id);
            let isolated = layered;
            let filtered = |scene: &Self, clip: Rect| {
                scene.blur_nodes.iter().any(|blur| {
                    let (bounds, effects, _) = scene.world(*blur);
                    effects.opacity > 0. && bounds.intersects(clip)
                })
            };
            let usable = clip.filter(|clip| {
                !isolated
                    && dx.abs() < clip.width
                    && dy.abs() < clip.height
                    && !filtered(self, *clip)
            });
            match usable {
                Some(clip) => moves.push(ScrollMove {
                    node: id,
                    clip,
                    dx,
                    dy,
                    serial: 0,
                }),
                None => {
                    // Everything the content painted, at both positions.
                    let now = self.node(id).transform;
                    self.node_mut(id).transform = from;
                    self.damage_subtree(id);
                    self.node_mut(id).transform = now;
                    self.damage_subtree(id);
                }
            }
        }
        // Nested or overlapping moves: repaint instead.
        let overlapping: Vec<usize> = (0..moves.len())
            .filter(|&i| {
                (0..moves.len()).any(|j| {
                    i != j
                        && (moves[i].clip.intersects(moves[j].clip)
                            || self.is_within(moves[i].node, moves[j].node))
                })
            })
            .collect();
        for (index, scroll) in moves.into_iter().enumerate() {
            if overlapping.contains(&index) {
                self.add_damage(scroll.clip);
                continue;
            }
            // Other changes inside the clip also land where the copy moves
            // them; the strip the copy cannot fill is exposed.
            let inside: Vec<Rect> = self
                .damage
                .iter()
                .filter_map(|rect| rect.intersection(scroll.clip))
                .collect();
            for rect in inside {
                let shifted = rect.translated(Transform {
                    x: scroll.dx,
                    y: scroll.dy,
                });
                if let Some(rect) = shifted.intersection(scroll.clip) {
                    self.add_damage(rect);
                }
            }
            for rect in exposed(scroll.clip, scroll.dx, scroll.dy) {
                self.add_damage(rect);
            }
            self.moves.push(scroll);
        }
    }
    /// Isolates this subtree into an offscreen group. Opacity then applies once to
    /// overlapping children. Legacy nodes retain per-primitive inherited opacity.
    pub fn set_isolated(&mut self, id: NodeId, isolated: bool) {
        if self.node(id).isolated == isolated {
            return;
        }
        self.damage_subtree(id);
        if isolated {
            self.isolation_count += 1;
        } else {
            self.isolation_count -= 1;
        }
        self.node_mut(id).isolated = isolated;
        self.invalidate(id, PAINT);
    }
    pub fn is_isolated(&self, id: NodeId) -> bool {
        self.node(id).isolated
    }
    pub fn layer_revision(&self, id: NodeId) -> u64 {
        self.node(id).layer_revision
    }
    pub fn isolated_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.node
                .as_ref()
                .filter(|node| node.isolated)
                .map(|_| NodeId {
                    index: index as u32,
                    generation: slot.generation,
                })
        })
    }
    /// World-space content bounds, without ancestor clipping, for offscreen
    /// allocation. A transformed layer's are in its own untransformed space,
    /// where its content is laid out.
    pub fn layer_bounds(&self, id: NodeId) -> Rect {
        let mut bounds = self.bounds(id);
        for item in self.layer_items(Some(id)) {
            let r = if item.isolated {
                // A nested blur's output, spread past its own content.
                let nested = self
                    .layer_bounds(item.id)
                    .expand(filter_spread(item.effects.filter_blur));
                item.matrix.map_or(nested, |matrix| matrix.bounds(nested))
            } else if let NodeKind::Quad(quad) | NodeKind::Panel { quad, .. } = item.kind {
                quad.paint_bounds(item.bounds)
            } else if let NodeKind::Svg(svg) = item.kind {
                svg.paint_bounds(item.bounds)
            } else if let NodeKind::Image(image) = item.kind {
                image.paint_bounds(item.bounds)
            } else {
                item.bounds
            };
            let r = item.clip.map_or(Some(r), |clip| r.intersection(clip));
            if let Some(r) = r {
                bounds = bounds.union(r);
            }
        }
        bounds
    }
    /// Paints a single compositor layer, replacing nested isolation boundaries with
    /// one item. A layer root excludes its own opacity/filter from its cached pixels.
    /// Changes whenever nodes are added, removed or change kind. Renderers can
    /// prune per-node caches only when it moves.
    pub fn content_revision(&self) -> u64 {
        self.content_revision
    }
    /// True when any node applies a backdrop blur, whose output depends on
    /// pixels outside its own subtree.
    pub fn has_backdrop_blur(&self) -> bool {
        !self.blur_nodes.is_empty()
    }
    pub fn layer_items(&self, root: Option<NodeId>) -> Vec<PaintItem<'_>> {
        self.layer_items_within(root, None)
    }
    /// Like [`Scene::layer_items`], but skips whole subtrees that paint nothing
    /// inside `damage`. Items that remain may still lie partly outside it.
    pub fn layer_items_within(
        &self,
        root: Option<NodeId>,
        damage: Option<&[Rect]>,
    ) -> Vec<PaintItem<'_>> {
        let reach = damage.map(|damage| {
            damage
                .iter()
                .copied()
                .reduce(Rect::union)
                .unwrap_or_default()
        });
        let touches = |ink: Option<Rect>, transform: Transform| -> bool {
            let (Some(damage), Some(reach)) = (damage, reach) else {
                return true;
            };
            let Some(ink) = ink else {
                return false;
            };
            let ink = ink.translated(transform);
            ink.intersects(reach) && damage.iter().any(|d| d.intersects(ink))
        };
        let start = root.unwrap_or(self.root);
        if !self.layout_visible(start) {
            return Vec::new();
        }
        let mut transform = Transform::default();
        let mut parent = self.node(start).parent;
        while let Some(id) = parent {
            let n = self.node(id);
            transform.x += n.transform.x;
            transform.y += n.transform.y;
            parent = n.parent;
        }
        let clip = if root.is_some() {
            None
        } else {
            Some(self.viewport)
        };
        // Sized as the last walk was, rather than grown from nothing each frame.
        let (stack_hint, items_hint) = self.walk_hints.get();
        let mut stack = Vec::with_capacity(stack_hint.max(1));
        stack.push((
            start,
            transform,
            1.,
            clip,
            None::<FadeMask>,
            RoundedClip::default(),
            false,
            None::<ColorMatrix>,
        ));
        let mut result = Vec::with_capacity(items_hint);
        let mut deepest = 0;
        while let Some((
            id,
            parent_transform,
            parent_opacity,
            parent_clip,
            mask,
            rounded,
            border,
            parent_color,
        )) = stack.pop()
        {
            let node = self.node(id);
            let transform = Transform {
                x: parent_transform.x + node.transform.x,
                y: parent_transform.y + node.transform.y,
            };
            let bounds = node.bounds.translated(transform);
            let is_root = root == Some(id);
            let transformed = node.transformed();
            let effect = node.effect_layer();
            let isolated = (node.isolated || transformed || effect) && !is_root;
            let mut effects = if is_root {
                Effects::default()
            } else {
                node.effects
            };
            effects.opacity *= parent_opacity;
            // Its colour filters after its ancestors' (a layer root's own
            // apply at its composite).
            let own_color = if is_root {
                None
            } else {
                node.effects.color_matrix()
            };
            let color = compose_color(parent_color, own_color);
            // Fully transparent subtrees contribute no cached pixels or layer
            // bounds. An explicit layer root keeps opacity external above.
            if effects.opacity <= 0.0 || !own_visible(&node.style) {
                continue;
            }
            // A transformed layer clips itself, inside, to its rotated box; a
            // blurred or masked one clips inside, before its effect.
            let matrix = (isolated && transformed).then(|| node.local().matrix(bounds));
            // A border pass (its border and outline over what it holds)
            // isn't cut by its own clip, as CSS doesn't cut an outline.
            let clip = match (isolated && (transformed || effect)) || border {
                true => parent_clip,
                false => clipped_bounds(parent_clip, bounds, &node.style),
            };
            let child_mask = fade_mask(mask, clip, &node.style);
            let mut child_rounded = rounded_clip(rounded, node, bounds);
            // Its clip inset: on its item and descendants here, on the
            // composite of an untransformed layer (after its effect), and
            // inside a transformed one, where its own space is.
            let inset = node
                .clip_inset
                .as_deref()
                .filter(|_| match (is_root, isolated) {
                    (true, _) => transformed,
                    (false, true) => !transformed,
                    (false, false) => true,
                })
                .map(|inset| inset.apply(bounds));
            let clip = match inset {
                None => clip,
                Some(inset) => {
                    Some(clip.map_or(inset, |clip| clip.intersection(inset).unwrap_or_default()))
                }
            };
            // Its rounded clip rounds the inset box instead.
            let mut item_rounded = rounded;
            if let Some(inset) = inset
                && child_rounded != rounded
                && let Some(own) = &mut child_rounded[0]
            {
                own.rect = inset;
                item_rounded = child_rounded;
            }
            // Its border over what it holds: a second item once they paint.
            let border_over = matches!(&node.kind,
                NodeKind::Quad(quad) | NodeKind::Panel { quad, .. } if quad.paints_over());
            if !isolated && !border && border_over {
                let opacity = parent_opacity;
                stack.push((
                    id,
                    parent_transform,
                    opacity,
                    parent_clip,
                    mask,
                    rounded,
                    true,
                    parent_color,
                ));
            }
            if !isolated && !border {
                stack.extend(
                    node.children
                        .iter()
                        .rev()
                        .filter(|child| touches(self.ink(**child), transform))
                        .map(|child| {
                            let opacity = effects.opacity;
                            (
                                *child,
                                transform,
                                opacity,
                                clip,
                                child_mask,
                                child_rounded,
                                false,
                                color,
                            )
                        }),
                );
                deepest = deepest.max(stack.len());
            }
            // A box's own paint (fill, border, shadows) isn't cut by its own
            // overflow clip: CSS clips what an element holds, not its
            // box-shadow. Its clip inset (clip-path) still cuts it.
            let own_clip = match &node.kind {
                NodeKind::Quad(_) | NodeKind::Panel { .. } => match inset {
                    None => parent_clip,
                    Some(inset) => Some(
                        parent_clip
                            .map_or(inset, |clip| clip.intersection(inset).unwrap_or_default()),
                    ),
                },
                _ => clip,
            };
            result.push(PaintItem {
                isolated,
                matrix,
                id,
                bounds,
                kind: &node.kind,
                font: &node.font,
                text_options: node.style.text_options,
                effects,
                clip: own_clip,
                mask,
                rounded: item_rounded,
                border_pass: border,
                color,
            });
        }
        self.walk_hints.set((deepest, result.len()));
        result
    }
    pub fn paint_items(&self) -> impl Iterator<Item = PaintItem<'_>> {
        PaintIter {
            scene: self,
            stack: vec![(
                self.root,
                Transform::default(),
                1.0,
                self.viewport,
                None,
                RoundedClip::default(),
                None,
            )],
        }
    }
}
/// The mask a node passes to its children: its own when it fades its edges
/// (over its clip, less `fade_insets`), otherwise the one it inherited.
fn fade_mask(inherited: Option<FadeMask>, clip: Option<Rect>, style: &Style) -> Option<FadeMask> {
    let [top, bottom] = style.fade_edges;
    let [left, right] = style.fade_edges_x;
    let bands =
        [top, right, bottom, left].map(|band| if band.is_finite() { band.max(0.) } else { 0. });
    match clip {
        Some(clip) if bands.iter().any(|band| *band > 0.) => {
            let inset = |inset: f32| if inset.is_finite() { inset.max(0.) } else { 0. };
            let ([top, bottom], [left, right]) =
                (style.fade_insets.map(inset), style.fade_insets_x.map(inset));
            let rect = Rect::new(
                clip.x + left.min(clip.width),
                clip.y + top.min(clip.height),
                (clip.width - left - right).max(0.),
                (clip.height - top - bottom).max(0.),
            );
            Some((rect, bands))
        }
        _ => inherited,
    }
}
/// The rounded clips a node passes to its children: its own box first when
/// it clips both axes with rounded corners, otherwise those it inherited.
fn rounded_clip(inherited: RoundedClip, node: &Node, bounds: Rect) -> RoundedClip {
    let (NodeKind::Quad(quad) | NodeKind::Panel { quad, .. }) = &node.kind else {
        return inherited;
    };
    if clip_axes(&node.style) != (true, true) {
        return inherited;
    }
    let corners = quad
        .decoration
        .as_ref()
        .and_then(|detail| detail.corners)
        .unwrap_or(crate::decoration::Corners::all(quad.radius));
    let most = bounds.width.min(bounds.height).max(0.) / 2.;
    let radius = |value: f32| {
        if value.is_finite() {
            value.clamp(0., most)
        } else {
            0.
        }
    };
    let radii = [
        radius(corners.top_left),
        radius(corners.top_right),
        radius(corners.bottom_right),
        radius(corners.bottom_left),
    ];
    if radii.iter().all(|radius| *radius <= 0.) {
        return inherited;
    }
    [
        Some(RoundRect {
            rect: bounds,
            radii,
        }),
        inherited[0],
    ]
}
fn clip_axes(style: &Style) -> (bool, bool) {
    let options = style.layout_options.as_deref();
    (
        options.and_then(|o| o.clip_x).unwrap_or(style.clip),
        options.and_then(|o| o.clip_y).unwrap_or(style.clip),
    )
}
fn clipped_bounds(parent: Option<Rect>, bounds: Rect, style: &Style) -> Option<Rect> {
    let (x, y) = clip_axes(style);
    if !x && !y {
        return parent;
    }
    let base = parent.unwrap_or(UNCLIPPED);
    let mask = Rect::new(
        if x { bounds.x } else { base.x },
        if y { bounds.y } else { base.y },
        if x { bounds.width } else { base.width },
        if y { bounds.height } else { base.height },
    );
    Some(base.intersection(mask).unwrap_or_default())
}
struct PaintIter<'a> {
    scene: &'a Scene,
    stack: Vec<PaintFrame>,
}
/// A node `PaintIter` will visit, with what its parent passes on: its
/// translation, opacity, clip, fade mask, rounded clips and colour filters.
type PaintFrame = (
    NodeId,
    Transform,
    f32,
    Rect,
    Option<FadeMask>,
    RoundedClip,
    Option<ColorMatrix>,
);
impl<'a> Iterator for PaintIter<'a> {
    type Item = PaintItem<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let (id, parent_transform, parent_opacity, parent_clip, mask, rounded, parent_color) =
            self.stack.pop()?;
        let node = self.scene.node(id);
        let transform = Transform {
            x: parent_transform.x + node.transform.x,
            y: parent_transform.y + node.transform.y,
        };
        let bounds = node.bounds.translated(transform);
        let mut effects = node.effects;
        if !own_visible(&node.style) {
            effects.opacity = 0.;
        }
        effects.opacity *= parent_opacity;
        let clip = clipped_bounds(Some(parent_clip), bounds, &node.style).unwrap_or(parent_clip);
        let child_mask = fade_mask(mask, Some(clip), &node.style);
        let inset = node.clip_inset.as_deref().map(|inset| inset.apply(bounds));
        let clip = match inset {
            Some(inset) => clip.intersection(inset).unwrap_or_default(),
            None => clip,
        };
        // A box's own paint isn't cut by its own overflow clip (as above).
        let own_clip = match &node.kind {
            NodeKind::Quad(_) | NodeKind::Panel { .. } => match inset {
                Some(inset) => parent_clip.intersection(inset).unwrap_or_default(),
                None => parent_clip,
            },
            _ => clip,
        };
        let child_rounded = rounded_clip(rounded, node, bounds);
        let color = compose_color(parent_color, node.effects.color_matrix());
        self.stack.extend(node.children.iter().rev().map(|child| {
            let opacity = effects.opacity;
            (
                *child,
                transform,
                opacity,
                clip,
                child_mask,
                child_rounded,
                color,
            )
        }));
        Some(PaintItem {
            isolated: false,
            matrix: None,
            id,
            bounds,
            kind: &node.kind,
            font: &node.font,
            text_options: node.style.text_options,
            effects,
            clip: Some(own_clip),
            mask,
            rounded,
            border_pass: false,
            color,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> NodeKind {
        NodeKind::Text {
            text: value.into(),
            color: Color(255, 255, 255, 255),
            font_size: 16.0,
        }
    }
    #[test]
    fn flex_distribution_respects_limits_and_resizes() {
        let mut scene = Scene::new(300.0, 80.0);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Row));
        let a = scene.append(
            scene.root(),
            text("a"),
            Style {
                width: Some(50.0),
                max_width: Some(70.0),
                flex_grow: 1.0,
                ..Style::default()
            },
        );
        let b = scene.append(
            scene.root(),
            text("b"),
            Style {
                width: Some(50.0),
                flex_grow: 1.0,
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(a).width, 70.0);
        assert_eq!(scene.bounds(b).width, 230.0);
        scene.resize(400.0, 80.0);
        let report = scene.flush();
        assert_eq!(scene.bounds(b).width, 330.0);
        assert!(!report.damage.is_empty());
        assert!(scene.flush().is_idle());
    }
    #[test]
    fn flex_shrink_and_margins_are_included_in_allocation() {
        let mut scene = Scene::new(140.0, 80.0);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Row));
        let a = scene.append(
            scene.root(),
            text("a"),
            Style {
                width: Some(100.0),
                min_width: Some(80.0),
                flex_shrink: 1.0,
                ..Style::default()
            },
        );
        let b = scene.append(
            scene.root(),
            text("b"),
            Style {
                width: Some(100.0),
                flex_shrink: 1.0,
                margin: Insets::all(5.0),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(a).width, 80.0);
        assert_eq!(scene.bounds(b), Rect::new(85.0, 5.0, 50.0, 23.0));
    }
    #[test]
    fn text_measurement_reflows_when_parent_constraint_changes() {
        let mut scene = Scene::new(100.0, 300.0);
        scene.set_text_measurer(|text: &str, _: f32, limit: Option<f32>| {
            let width = text.len() as f32 * 10.0;
            let limit = limit.unwrap_or(width).max(1.0);
            (width.min(limit), (width / limit).ceil() * 20.0)
        });
        assert_eq!(
            scene.measure_text("abcdefghijabcdefghij", 16.0, Some(50.0)),
            (50.0, 80.0)
        );
        let a = scene.append(
            scene.root(),
            text("abcdefghijabcdefghij"),
            Style {
                text_wrap: true,
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(a).height, 40.0);
        scene.resize(50.0, 300.0);
        scene.flush();
        assert_eq!(scene.bounds(a).height, 80.0);
    }
    #[test]
    fn text_sizes_survive_alternating_wrap_limits() {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let counter = calls.clone();
        let mut scene = Scene::new(300.0, 100.0);
        scene.set_text_measurer(move |text: &str, _: f32, _: Option<f32>| {
            counter.set(counter.get() + 1);
            (text.len() as f32 * 10.0, 20.0)
        });
        let label = scene.append(
            scene.root(),
            text("label"),
            Style {
                text_wrap: true,
                ..Style::default()
            },
        );
        let mut sizes = TextSizes::default();
        for limit in [Some(100.), Some(200.), Some(100.), Some(200.)] {
            if sizes.find(limit, 0).is_none() {
                sizes.insert(limit, 0, (1., 1.));
            }
        }
        assert_eq!(sizes.0.iter().flatten().count(), 2);
        scene.flush();
        let first = calls.get();
        // Relayout of the parent re-measures the clean label at both widths.
        for width in [200.0, 100.0, 200.0, 100.0] {
            scene.set_style(
                scene.root(),
                Style {
                    width: Some(width),
                    height: Some(100.0),
                    ..Style::default()
                },
            );
            scene.flush();
        }
        assert!(
            calls.get() <= first + 2,
            "{} measurements",
            calls.get() - first
        );
        let _ = label;
    }
    #[test]
    fn cached_text_size_follows_text_and_measurer_changes() {
        let mut scene = Scene::new(300.0, 100.0);
        scene.set_text_measurer(|text: &str, _: f32, _: Option<f32>| {
            (text.len() as f32 * 10.0, 20.0)
        });
        let label = scene.append(scene.root(), text("label"), Style::default());
        scene.flush();
        assert_eq!(scene.bounds(label).width, 50.0);
        scene.set_kind(label, text("longer label"));
        scene.flush();
        assert_eq!(scene.bounds(label).width, 120.0);
        scene.set_text_measurer(|text: &str, _: f32, _: Option<f32>| {
            (text.len() as f32 * 5.0, 20.0)
        });
        scene.flush();
        assert_eq!(scene.bounds(label).width, 60.0);
    }
    #[test]
    fn alignment_stretch_and_justification_apply_to_children() {
        let mut scene = Scene::new(100.0, 100.0);
        scene.set_style(
            scene.root(),
            Style {
                width: Some(100.0),
                height: Some(100.0),
                align: Align::Stretch,
                justify: Justify::SpaceBetween,
                ..Style::default()
            },
        );
        let a = scene.append(scene.root(), text("a"), Style::default());
        let b = scene.append(scene.root(), text("b"), Style::default());
        scene.flush();
        assert_eq!(scene.bounds(a), Rect::new(0.0, 0.0, 100.0, 23.0));
        assert_eq!(scene.bounds(b), Rect::new(0.0, 77.0, 100.0, 23.0));
    }
    #[test]
    fn hit_testing_respects_stacking_clip_transform_and_opacity() {
        let mut scene = Scene::new(100.0, 100.0);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let a = scene.append(scene.root(), text("aaaaaaaa"), Style::default());
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            Style {
                width: Some(20.0),
                height: Some(20.0),
                clip: true,
                ..Style::default()
            },
        );
        let child = scene.append(parent, text("bbbbbbbb"), Style::default());
        scene.set_transform(parent, Transform { x: 10.0, y: 0.0 });
        scene.flush();
        assert_eq!(scene.hit_test(15.0, 10.0), Some(child));
        assert_eq!(scene.hit_test(35.0, 10.0), Some(a));
        assert_eq!(
            scene.ancestors(child).collect::<Vec<_>>(),
            vec![parent, scene.root()]
        );
        scene.set_effects(
            parent,
            Effects {
                opacity: 0.0,
                ..Effects::default()
            },
        );
        assert_eq!(scene.hit_test(15.0, 10.0), Some(a));
        assert_eq!(scene.hit_test(110.0, 10.0), None);
    }
    #[test]
    fn preparing_input_geometry_preserves_damage_for_renderer() {
        let mut scene = Scene::new(200.0, 200.0);
        let node = scene.append(scene.root(), text("first"), Style::default());
        scene.prepare_layout();
        assert!(scene.bounds(node).width > 0.0);
        scene.set_text(node, "longer updated text");
        scene.prepare_layout();
        let report = scene.flush();
        assert!(report.layout_nodes >= 4);
        assert!(!report.damage.is_empty());
        assert!(scene.flush().is_idle());
    }
    #[test]
    fn idle_and_equal_writes_do_no_work() {
        let mut scene = Scene::new(800.0, 600.0);
        let id = scene.append(scene.root(), text("hi"), Style::default());
        assert!(!scene.flush().damage.is_empty());
        assert!(scene.flush().is_idle());
        scene.set_text(id, "hi");
        assert!(scene.flush().is_idle());
    }
    #[test]
    fn fixed_text_invalidates_only_its_paint() {
        let mut scene = Scene::new(800.0, 600.0);
        let a = scene.append(
            scene.root(),
            text("a"),
            Style {
                width: Some(200.0),
                height: Some(24.0),
                ..Style::default()
            },
        );
        scene.append(scene.root(), text("stable"), Style::default());
        scene.flush();
        scene.set_text(a, "stream");
        let frame = scene.flush();
        assert_eq!(frame.layout_nodes, 0);
        assert_eq!(frame.paint_nodes, 1);
        // Headless measurement is approximate: the whole box.
        assert_eq!(frame.damage, vec![Rect::new(0.0, 0.0, 200.0, 24.0)]);
    }
    #[test]
    fn transforms_damage_old_and_new_without_layout_or_paint() {
        let mut scene = Scene::new(800.0, 600.0);
        let id = scene.append(
            scene.root(),
            NodeKind::Rect(Color(1, 2, 3, 255)),
            Style {
                width: Some(20.0),
                height: Some(20.0),
                ..Style::default()
            },
        );
        scene.flush();
        scene.set_transform(id, Transform { x: 100.0, y: 0.0 });
        let frame = scene.flush();
        assert_eq!(frame.layout_nodes, 0);
        assert_eq!(frame.paint_nodes, 0);
        assert_eq!(frame.composite_nodes, 1);
        assert!(frame.damage.contains(&Rect::new(0.0, 0.0, 20.0, 20.0)));
        assert!(frame.damage.contains(&Rect::new(100.0, 0.0, 20.0, 20.0)));
    }
    #[test]
    fn reused_slots_do_not_resurrect_handles() {
        let mut scene = Scene::new(100.0, 100.0);
        let old = scene.append(scene.root(), text("old"), Style::default());
        scene.remove(old);
        let new = scene.append(scene.root(), text("new"), Style::default());
        assert!(!scene.contains(old));
        assert!(scene.contains(new));
        assert_ne!(old, new);
        scene.flush();
    }
    #[test]
    fn opacity_inherits_and_clips_follow_transforms() {
        let mut scene = Scene::new(100.0, 100.0);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            Style {
                width: Some(20.0),
                height: Some(20.0),
                clip: true,
                ..Style::default()
            },
        );
        let child = scene.append(parent, text("hello"), Style::default());
        scene.flush();
        scene.set_effects(
            parent,
            Effects {
                opacity: 0.5,
                ..Effects::default()
            },
        );
        scene.set_effects(
            child,
            Effects {
                opacity: 0.5,
                ..Effects::default()
            },
        );
        scene.set_transform(parent, Transform { x: 10.0, y: 10.0 });
        scene.flush();
        let item = scene.paint_items().find(|i| i.id == child).unwrap();
        assert_eq!(item.effects.opacity, 0.25);
        assert_eq!(item.clip, Some(Rect::new(10.0, 10.0, 20.0, 20.0)));
    }
    #[test]
    fn backdrop_damage_expands_to_filter_output() {
        let mut scene = Scene::new(300.0, 300.0);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let a = scene.append(
            scene.root(),
            text("a"),
            Style {
                width: Some(20.0),
                height: Some(20.0),
                ..Style::default()
            },
        );
        let b = scene.append(
            scene.root(),
            NodeKind::Rect(Color(255, 255, 255, 128)),
            Style {
                width: Some(100.0),
                height: Some(100.0),
                ..Style::default()
            },
        );
        scene.set_effects(
            b,
            Effects {
                blur_radius: 4.0,
                ..Effects::default()
            },
        );
        scene.flush();
        scene.set_text(a, "b");
        let frame = scene.flush();
        assert!(
            frame
                .damage
                .iter()
                .any(|r| r.width >= 100.0 && r.height >= 100.0)
        );
    }
    #[test]
    fn natural_text_layout_preserves_unaffected_subtrees() {
        let mut scene = Scene::new(500.0, 500.0);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let changed = scene.append(scene.root(), text("a"), Style::default());
        let stable = scene.append(scene.root(), text("stable"), Style::default());
        scene.set_transform(stable, Transform { x: 200.0, y: 200.0 });
        scene.flush();
        scene.set_text(changed, "longer");
        let frame = scene.flush();
        assert_eq!(frame.layout_nodes, 2); // Root and the changed leaf; stable is cached.
        assert_eq!(frame.paint_nodes, 1);
        assert!(
            frame
                .damage
                .iter()
                .all(|rect| !rect.intersects(scene.bounds(stable)))
        );
    }
    #[test]
    fn intrinsic_height_change_moves_and_damages_following_sibling() {
        let mut scene = Scene::new(500.0, 500.0);
        let first = scene.append(scene.root(), text("a"), Style::default());
        let second = scene.append(scene.root(), text("b"), Style::default());
        scene.flush();
        let old = scene.bounds(second);
        scene.set_text(first, "a\na");
        let frame = scene.flush();
        let new = scene.bounds(second);
        assert_eq!(old.y, 23.0);
        assert_eq!(new.y, 46.0);
        assert!(frame.damage.iter().any(|rect| rect.intersects(old)));
        assert!(frame.damage.iter().any(|rect| rect.intersects(new)));
        assert!(scene.flush().is_idle());
    }
    #[test]
    fn removing_a_subtree_clears_its_pixels_and_reclaims_every_node() {
        let mut scene = Scene::new(500.0, 500.0);
        let group = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Column),
            Style::default(),
        );
        let child = scene.append(group, text("removed"), Style::default());
        scene.flush();
        let old = scene.bounds(child);
        scene.remove(group);
        let frame = scene.flush();
        assert_eq!(scene.len(), 1);
        assert!(!scene.contains(child));
        assert!(frame.damage.iter().any(|rect| rect.intersects(old)));
        assert!(scene.flush().is_idle());
    }
    #[test]
    fn parent_transform_damages_descendant_at_both_locations() {
        let mut scene = Scene::new(500.0, 500.0);
        let group = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Column),
            Style::default(),
        );
        let child = scene.append(group, text("moving"), Style::default());
        scene.flush();
        let old = scene.bounds(child);
        scene.set_transform(group, Transform { x: 200.0, y: 200.0 });
        let new = scene.bounds(child);
        let frame = scene.flush();
        assert_eq!(frame.layout_nodes, 0);
        assert_eq!(frame.paint_nodes, 0);
        assert!(frame.damage.iter().any(|rect| rect.intersects(old)));
        assert!(frame.damage.iter().any(|rect| rect.intersects(new)));
    }
    #[test]
    fn expanding_clip_reveals_and_damages_previously_hidden_content() {
        let mut scene = Scene::new(500.0, 500.0);
        let mut style = Style {
            width: Some(20.0),
            height: Some(20.0),
            clip: true,
            ..Style::default()
        };
        let group = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            style.clone(),
        );
        scene.append(
            group,
            NodeKind::Rect(Color(255, 255, 255, 255)),
            Style {
                width: Some(100.0),
                height: Some(20.0),
                ..Style::default()
            },
        );
        scene.flush();
        style.clip = false;
        scene.set_style(group, style);
        let frame = scene.flush();
        assert!(frame.damage.iter().any(|rect| rect.x + rect.width >= 100.0));
    }
    #[test]
    fn resizing_clip_damages_newly_visible_pixels() {
        let mut scene = Scene::new(500.0, 500.0);
        let mut style = Style {
            width: Some(20.0),
            height: Some(20.0),
            clip: true,
            ..Style::default()
        };
        let group = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            style.clone(),
        );
        scene.append(
            group,
            NodeKind::Rect(Color(255, 255, 255, 255)),
            Style {
                width: Some(100.0),
                height: Some(20.0),
                ..Style::default()
            },
        );
        scene.flush();
        style.width = Some(100.0);
        scene.set_style(group, style);
        let frame = scene.flush();
        assert!(frame.damage.iter().any(|rect| rect.x + rect.width >= 100.0));
    }
    #[test]
    fn fractional_damage_is_rounded_to_cover_complete_pixels() {
        let mut scene = Scene::new(100.0, 100.0);
        let node = scene.append(
            scene.root(),
            NodeKind::Rect(Color(100, 100, 100, 100)),
            Style {
                width: Some(10.0),
                height: Some(10.0),
                ..Style::default()
            },
        );
        scene.flush();
        scene.set_transform(node, Transform { x: 20.25, y: 20.75 });
        let frame = scene.flush();
        assert!(frame.damage.contains(&Rect::new(20.0, 20.0, 11.0, 11.0)));
        assert!(frame.damage.iter().all(|r| {
            [r.x, r.y, r.width, r.height]
                .into_iter()
                .all(|v| v.fract() == 0.0)
        }));
    }
}

#[cfg(test)]
mod measure_work_tests {
    use super::*;
    use crate::layout::{LayoutOptions, Length};
    /// A stack `depth` deep of layout boxes as zgui-script lowers them: an
    /// outer column and a middle one stretching their items, laid out by
    /// taffy (their children set layout options), each holding wrapping
    /// text, a fixed-width row and the next box. The middle columns of a
    /// few levels animate a negative bottom margin every frame (entering
    /// boxes growing open). Returns the scene's measure calls per frame.
    fn measures_per_frame(depth: usize) -> u64 {
        let mut scene = Scene::new(600., 4000.);
        let shrinkable = Some(Arc::new(LayoutOptions {
            min_width: Some(Length::Px(0.)),
            ..Default::default()
        }));
        let column = |scene: &mut Scene, parent| {
            scene.append(
                parent,
                NodeKind::Container(Layout::Column),
                Style {
                    align: Align::Stretch,
                    layout_options: shrinkable.clone(),
                    ..Default::default()
                },
            )
        };
        let mut parent = scene.root();
        let mut middles = Vec::new();
        for level in 0..depth {
            let outer = column(&mut scene, parent);
            let middle = column(&mut scene, outer);
            scene.append(
                middle,
                NodeKind::Text {
                    text: format!("level {level} says something long enough to wrap").into(),
                    color: Color(255, 255, 255, 255),
                    font_size: 14.,
                },
                Style {
                    text_wrap: true,
                    layout_options: shrinkable.clone(),
                    ..Default::default()
                },
            );
            let row = scene.append(middle, NodeKind::Container(Layout::Row), Style::default());
            for _ in 0..3 {
                scene.append(
                    row,
                    NodeKind::Rect(Color(1, 2, 3, 255)),
                    Style {
                        width: Some(40.),
                        height: Some(10.),
                        ..Default::default()
                    },
                );
            }
            middles.push(middle);
            parent = middle;
        }
        scene.flush();
        let animated: Vec<_> = middles.iter().copied().step_by(4).collect();
        let before = scene.measure_calls;
        let frames = 12;
        for frame in 0..frames {
            for (i, id) in animated.iter().enumerate() {
                let mut style = scene.style(*id);
                style.margin.bottom = -((frames - frame) as f32) * 7. - i as f32;
                scene.set_style(*id, style);
            }
            scene.flush();
        }
        (scene.measure_calls - before) / frames as u64
    }
    #[test]
    fn nested_animated_margins_measure_their_paths_not_every_probe_combination() {
        let shallow = measures_per_frame(8);
        let deep = measures_per_frame(32);
        // Four times as deep, with four times the animated boxes: about
        // sixteen times the work on their paths at most, never the
        // exponential growth of each level re-probing the next.
        assert!(
            deep <= 20 * shallow,
            "{shallow} calls at depth 8, {deep} at 32"
        );
        // A handful of probes per node on a changed path and its children.
        let nodes = 32 * 7;
        assert!(deep < 40 * nodes as u64, "{deep} calls for {nodes} nodes");
    }
}
#[cfg(test)]
mod paint_geometry_tests {
    use super::*;
    #[test]
    fn rect_and_quad_paint_changes_preserve_layout() {
        let mut scene = Scene::new(200., 100.);
        let node = scene.append(
            scene.root(),
            NodeKind::Rect(Color(255, 0, 0, 255)),
            Style {
                width: Some(100.),
                height: Some(40.),
                ..Style::default()
            },
        );
        scene.flush();
        let bounds = scene.bounds(node);
        for kind in [
            NodeKind::Rect(Color(0, 255, 0, 255)),
            NodeKind::Quad(QuadStyle {
                fill: Color(0, 0, 255, 255),
                radius: 8.,
                ..QuadStyle::default()
            }),
        ] {
            scene.set_kind(node, kind);
            let report = scene.flush();
            assert_eq!(report.layout_nodes, 0);
            assert!(!report.damage.is_empty());
            assert_eq!(scene.bounds(node), bounds);
        }
        assert!(scene.flush().is_idle());
    }
}

#[cfg(test)]
mod deferred_frame_tests {
    use super::*;
    #[test]
    fn unflushed_node_replacement_bounds_stale_dirty_generations() {
        let mut scene = Scene::new(100., 100.);
        scene.flush();
        for i in 0..100_000 {
            let node = scene.append(
                scene.root(),
                NodeKind::Rect(Color((i % 255) as u8, 0, 0, 255)),
                Style {
                    width: Some(10.),
                    height: Some(10.),
                    ..Style::default()
                },
            );
            scene.remove(node);
            assert!(scene.dirty.len() <= 64);
        }
        let node = scene.append(
            scene.root(),
            NodeKind::Rect(Color(0, 255, 0, 255)),
            Style {
                width: Some(10.),
                height: Some(10.),
                ..Style::default()
            },
        );
        let report = scene.flush();
        assert!(!report.damage.is_empty());
        assert_eq!(scene.bounds(node).width, 10.);
        assert!(scene.flush().is_idle());
        scene.set_transform(node, Transform { x: 20., y: 0. });
        assert!(!scene.flush().damage.is_empty());
    }
}

#[cfg(test)]
mod absolute_layout_tests {
    use super::*;
    #[test]
    fn absolute_children_do_not_change_flow_intrinsic_size_or_gaps() {
        for layout in [Layout::Row, Layout::Column, Layout::Overlay] {
            let mut scene = Scene::new(500., 500.);
            let parent = scene.append(
                scene.root(),
                NodeKind::Container(layout),
                Style {
                    padding: 5.,
                    gap: 7.,
                    ..Style::default()
                },
            );
            let first = scene.append(
                parent,
                NodeKind::Rect(Color(255, 0, 0, 255)),
                Style {
                    width: Some(20.),
                    height: Some(30.),
                    ..Style::default()
                },
            );
            let floating = scene.append(
                parent,
                NodeKind::Rect(Color(0, 255, 0, 255)),
                Style {
                    absolute: true,
                    width: Some(200.),
                    height: Some(300.),
                    margin: Insets {
                        left: 3.,
                        top: 4.,
                        ..Insets::default()
                    },
                    flex_grow: 100.,
                    ..Style::default()
                },
            );
            let second = scene.append(
                parent,
                NodeKind::Rect(Color(0, 0, 255, 255)),
                Style {
                    width: Some(20.),
                    height: Some(30.),
                    ..Style::default()
                },
            );
            scene.flush();
            let expected = match layout {
                Layout::Row => (57., 40.),
                Layout::Column => (30., 77.),
                Layout::Overlay => (30., 40.),
            };
            assert_eq!(
                (scene.bounds(parent).width, scene.bounds(parent).height),
                expected
            );
            assert_eq!(scene.bounds(first), Rect::new(5., 5., 20., 30.));
            assert_eq!(scene.bounds(floating), Rect::new(8., 9., 200., 300.));
            let before = scene.bounds(second);
            scene.remove(floating);
            scene.flush();
            assert_eq!(scene.bounds(second), before);
            assert_eq!(
                (scene.bounds(parent).width, scene.bounds(parent).height),
                expected
            );
        }
    }
    #[test]
    fn absolute_text_remeasures_on_parent_resize_without_taking_flex_space() {
        let mut scene = Scene::new(200., 100.);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Row));
        let sibling = scene.append(
            scene.root(),
            NodeKind::Rect(Color(0, 0, 0, 255)),
            Style {
                width: Some(20.),
                height: Some(10.),
                flex_grow: 1.,
                ..Style::default()
            },
        );
        let text = scene.append(
            scene.root(),
            NodeKind::Text {
                text: "one two three four five six seven eight nine ten".into(),
                color: Color(255, 255, 255, 255),
                font_size: 16.,
            },
            Style {
                absolute: true,
                text_wrap: true,
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(sibling).width, 200.);
        let height = scene.bounds(text).height;
        scene.resize(80., 100.);
        scene.flush();
        assert_eq!(scene.bounds(sibling).width, 80.);
        assert!(scene.bounds(text).height > height);
    }
    #[test]
    fn geometry_revision_tracks_layout_and_transforms_but_not_equal_transforms() {
        let mut scene = Scene::new(100., 100.);
        let before = scene.geometry_revision();
        scene.prepare_layout();
        assert_ne!(scene.geometry_revision(), before);
        let before = scene.geometry_revision();
        scene.prepare_layout();
        assert_eq!(scene.geometry_revision(), before);
        scene.set_transform(scene.root(), Transform { x: 1., y: 2. });
        assert_ne!(scene.geometry_revision(), before);
        let before = scene.geometry_revision();
        scene.set_transform(scene.root(), Transform { x: 1., y: 2. });
        assert_eq!(scene.geometry_revision(), before);
    }
}

#[cfg(test)]
mod line_height_tests {
    use super::*;
    use crate::text_layout::{FallbackTextLayout, FontStyle, LineHeight, TextLayout};

    #[test]
    fn explicit_pitch_relayouts_once_and_native_font_callback_receives_metrics() {
        let mut scene = Scene::new(200., 200.);
        scene.set_font_text_shaper(
            |text: &str, size, width, font: &FontStyle| -> Box<dyn TextLayout> {
                Box::new(FallbackTextLayout::with_line_height(
                    text,
                    size,
                    width,
                    font.line_height,
                ))
            },
        );
        let node = scene.append(
            scene.root(),
            NodeKind::Text {
                text: "one\ntwo\n".into(),
                font_size: 10.,
                color: Color(255, 255, 255, 255),
            },
            Style::default(),
        );
        scene.flush();
        assert_eq!(scene.bounds(node).height, 42.);
        let font = FontStyle {
            line_height: LineHeight::px(24.),
            ..FontStyle::default()
        };
        scene.set_font(node, font.clone());
        let changed = scene.flush();
        assert!(changed.layout_nodes > 0 && !changed.damage.is_empty());
        assert_eq!(scene.bounds(node).height, 72.);
        assert_eq!(
            scene
                .shape_text_with_font("a\nb", 10., None, &font)
                .caret(2)
                .y,
            24.
        );
        scene.set_font(node, font);
        assert!(scene.flush().is_idle());
    }

    #[test]
    fn fallback_measurement_matches_grapheme_shaping_for_normal_and_explicit_pitch() {
        let scene = Scene::new(100., 100.);
        for line_height in [LineHeight::NORMAL, LineHeight::px(3.), LineHeight::px(30.)] {
            let font = FontStyle {
                line_height,
                ..FontStyle::default()
            };
            let text = "e\u{301}👩‍💻\r\nx";
            let measured = scene.measure_text_with_font(text, 10., Some(6.), &font);
            let shaped = scene.shape_text_with_font(text, 10., Some(6.), &font);
            assert_eq!(measured, shaped.size());
            assert_eq!(measured.1, 3. * line_height.resolve(10.));
        }
    }
}

#[cfg(test)]
mod percentage_layout_tests {
    use super::*;

    fn rect() -> NodeKind {
        NodeKind::Rect(Color(255, 0, 0, 255))
    }

    #[test]
    fn percentages_follow_definite_padded_parent_and_resize_without_idle_work() {
        let mut scene = Scene::new(400., 300.);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            Style {
                width_percent: Some(0.5),
                height_percent: Some(1.),
                padding: 10.,
                align: Align::Stretch,
                ..Style::default()
            },
        );
        let child = scene.append(
            parent,
            rect(),
            Style {
                width_percent: Some(0.5),
                height_percent: Some(0.5),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(child), Rect::new(10., 10., 90., 140.));
        scene.resize(600., 400.);
        scene.flush();
        assert_eq!(scene.bounds(child), Rect::new(10., 10., 140., 190.));
        assert!(scene.flush().is_idle());
        let unchanged = scene.style(child);
        scene.set_style(child, unchanged);
        assert!(scene.flush().is_idle());
    }

    #[test]
    fn percentages_supply_flex_bases_and_forced_allocation_supplies_descendant_basis() {
        let mut scene = Scene::new(400., 200.);
        let row = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Row),
            Style {
                width: Some(300.),
                height: Some(100.),
                ..Style::default()
            },
        );
        let first = scene.append(
            row,
            NodeKind::Container(Layout::Overlay),
            Style {
                width_percent: Some(0.75),
                height_percent: Some(1.),
                flex_shrink: 1.,
                ..Style::default()
            },
        );
        let second = scene.append(
            row,
            rect(),
            Style {
                width_percent: Some(0.75),
                height_percent: Some(1.),
                flex_shrink: 1.,
                ..Style::default()
            },
        );
        let nested = scene.append(
            first,
            rect(),
            Style {
                width_percent: Some(0.5),
                height_percent: Some(1.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(first).width, 150.);
        assert_eq!(scene.bounds(second).width, 150.);
        assert_eq!(scene.bounds(nested).width, 75.);
        assert_eq!(scene.bounds(nested).height, 100.);
    }

    #[test]
    fn indefinite_percentages_use_intrinsic_size_and_absolute_uses_final_content_box() {
        let mut scene = Scene::new(400., 300.);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Column),
            Style {
                padding: 10.,
                ..Style::default()
            },
        );
        let child = scene.append(
            parent,
            NodeKind::Container(Layout::Overlay),
            Style {
                width_percent: Some(0.5),
                height_percent: Some(1.),
                ..Style::default()
            },
        );
        scene.append(
            child,
            rect(),
            Style {
                width: Some(80.),
                height: Some(40.),
                ..Style::default()
            },
        );
        let absolute = scene.append(
            parent,
            rect(),
            Style {
                absolute: true,
                width_percent: Some(0.5),
                height_percent: Some(0.5),
                margin: Insets::all(3.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(parent).width, 100.);
        assert_eq!(scene.bounds(parent).height, 60.);
        assert_eq!(scene.bounds(child).width, 80.);
        assert_eq!(scene.bounds(child).height, 40.);
        assert_eq!(scene.bounds(absolute), Rect::new(13., 13., 40., 20.));
        assert!(scene.flush().is_idle());
    }

    #[test]
    fn indefinite_percentage_uses_auto_cross_axis_stretch() {
        let mut scene = Scene::new(400., 300.);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Column),
            Style {
                align: Align::Stretch,
                ..Style::default()
            },
        );
        scene.append(
            parent,
            rect(),
            Style {
                width: Some(200.),
                height: Some(20.),
                ..Style::default()
            },
        );
        let child = scene.append(
            parent,
            NodeKind::Container(Layout::Overlay),
            Style {
                width_percent: Some(0.5),
                ..Style::default()
            },
        );
        let nested = scene.append(
            child,
            rect(),
            Style {
                width: Some(80.),
                height: Some(20.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(parent).width, 200.);
        assert_eq!(scene.bounds(child).width, 200.);
        assert_eq!(scene.bounds(nested).width, 80.);
        assert!(scene.flush().is_idle());
    }

    #[test]
    fn growing_row_provides_definite_height_for_percentage_container() {
        let mut scene = Scene::new(640., 420.);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Column),
            Style {
                width_percent: Some(1.),
                height_percent: Some(1.),
                padding: 20.,
                gap: 12.,
                ..Style::default()
            },
        );
        scene.append(
            parent,
            rect(),
            Style {
                width: Some(120.),
                height: Some(24.),
                ..Style::default()
            },
        );
        let row = scene.append(
            parent,
            NodeKind::Container(Layout::Row),
            Style {
                width_percent: Some(1.),
                flex_grow: 1.,
                ..Style::default()
            },
        );
        let right = scene.append(
            row,
            NodeKind::Container(Layout::Column),
            Style {
                width_percent: Some(0.5),
                height_percent: Some(1.),
                padding: 12.,
                ..Style::default()
            },
        );
        scene.append(
            right,
            rect(),
            Style {
                width: Some(80.),
                height: Some(74.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(row).height, 344.);
        assert_eq!(scene.bounds(right).height, 344.);
        for _ in 0..3 {
            scene.invalidate_layout(row);
            scene.prepare_layout();
            assert_eq!(scene.bounds(right).height, 344.);
        }
    }

    #[test]
    fn zero_delta_flex_allocation_is_definite_for_percentage_descendants() {
        let mut scene = Scene::new(200., 100.);
        let row = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Row),
            Style {
                flex_grow: 1.,
                ..Style::default()
            },
        );
        scene.append(
            row,
            rect(),
            Style {
                width: Some(50.),
                height: Some(100.),
                ..Style::default()
            },
        );
        let percent = scene.append(
            row,
            rect(),
            Style {
                width: Some(50.),
                height_percent: Some(1.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(row).height, 100.);
        assert_eq!(scene.bounds(percent).height, 100.);
        assert!(scene.flush().is_idle());
    }

    #[test]
    fn percentage_constraints_precedence_and_invalid_values_are_finite() {
        let mut scene = Scene::new(200., 100.);
        let child = scene.append(
            scene.root(),
            rect(),
            Style {
                width_percent: Some(2.),
                height_percent: Some(f32::INFINITY),
                max_width: Some(120.),
                min_height: Some(5.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(child).width, 120.);
        assert_eq!(scene.bounds(child).height, 5.);
        scene.set_style(
            child,
            Style {
                width: Some(37.),
                width_percent: Some(0.5),
                height_percent: Some(-1.),
                ..Style::default()
            },
        );
        scene.flush();
        assert_eq!(scene.bounds(child).width, 37.);
        assert_eq!(scene.bounds(child).height, 0.);
    }
}

#[cfg(test)]
mod letter_spacing_fallback_tests {
    use super::*;
    use crate::text_layout::{FontStyle, LetterSpacing, TextLayout};
    #[test]
    fn explicit_spacing_bypasses_legacy_callbacks_without_losing_metrics() {
        let mut scene = Scene::new(200., 100.);
        scene.set_text_shaper(|_: &str, _, _| -> Box<dyn TextLayout> {
            panic!("legacy callback cannot receive nonzero letter spacing")
        });
        let font = FontStyle {
            letter_spacing: LetterSpacing::px(4.),
            ..FontStyle::default()
        };
        let measured = scene.measure_text_with_font("abc", 10., Some(20.), &font);
        let shaped = scene.shape_text_with_font("abc", 10., Some(20.), &font);
        assert_eq!(measured, (20., 28.));
        assert_eq!(measured, shaped.size());
    }
}

#[cfg(test)]
mod image_replacement_layout_tests {
    use super::*;
    use crate::image::ImageData;
    use std::sync::Arc;
    fn pixels(w: u32, h: u32) -> NodeKind {
        NodeKind::Image(Arc::new(
            ImageData::new(w, h, [0, 0, 0, 255].repeat((w * h) as usize)).unwrap(),
        ))
    }
    #[test]
    fn fixed_axes_ignore_source_size_even_zero_and_constrained_but_auto_axes_remeasure() {
        for style in [
            Style {
                width: Some(0.),
                height: Some(0.),
                ..Default::default()
            },
            Style {
                width: Some(80.),
                height: Some(70.),
                max_width: Some(30.),
                min_height: Some(100.),
                width_percent: Some(0.5),
                ..Default::default()
            },
        ] {
            let mut scene = Scene::new(200., 200.);
            let image = scene.append(scene.root(), pixels(20, 10), style.clone());
            scene.flush();
            let before = scene.bounds(image);
            scene.set_kind(image, pixels(10, 20));
            let report = scene.flush();
            assert_eq!(report.layout_nodes, 0);
            assert_eq!(scene.bounds(image), before);
        }
        for style in [
            Style::default(),
            Style {
                width: Some(40.),
                ..Default::default()
            },
            Style {
                width_percent: Some(0.5),
                height_percent: Some(0.5),
                ..Default::default()
            },
        ] {
            let mut scene = Scene::new(200., 200.);
            let image = scene.append(scene.root(), pixels(20, 10), style.clone());
            scene.flush();
            scene.set_kind(image, pixels(10, 20));
            assert!(
                scene.flush().layout_nodes > 0,
                "non-concrete axes use existing invalidation"
            );
            if style.height.is_none() && style.height_percent.is_none() {
                assert_eq!(scene.bounds(image).height, 20.);
            }
        }
    }
}

#[cfg(test)]
mod hidden_layer_tests {
    use super::*;
    #[test]
    fn hidden_huge_descendants_do_not_expand_visible_layer_allocation() {
        let mut scene = Scene::new(300., 200.);
        let root = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            Style {
                width: Some(80.),
                height: Some(60.),
                ..Default::default()
            },
        );
        scene.set_isolated(root, true);
        let hidden = scene.append(
            root,
            NodeKind::Container(Layout::Overlay),
            Style {
                absolute: true,
                width: Some(1.),
                height: Some(1.),
                ..Default::default()
            },
        );
        let huge = scene.append(
            hidden,
            NodeKind::Rect(Color(255, 0, 0, 255)),
            Style {
                width: Some(10000.),
                height: Some(12000.),
                ..Default::default()
            },
        );
        scene.set_effects(
            hidden,
            Effects {
                opacity: 0.,
                ..Default::default()
            },
        );
        scene.flush();
        assert_eq!(scene.layer_bounds(root), scene.bounds(root));
        assert_eq!(
            scene
                .layer_items(Some(root))
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            vec![root]
        );
        assert!(
            scene.paint_items().any(|p| p.id == huge),
            "public traversal retains cache liveness"
        );
        scene.set_effects(hidden, Effects::default());
        scene.flush();
        assert_eq!(scene.layer_bounds(root).width, 10000.);
        assert_eq!(scene.layer_bounds(root).height, 12000.);
    }

    #[test]
    fn hidden_nested_layers_prune_but_explicit_layer_root_opacity_stays_external() {
        let mut scene = Scene::new(300., 200.);
        let root = scene.append(
            scene.root(),
            NodeKind::Rect(Color(255, 0, 0, 255)),
            Style {
                width: Some(80.),
                height: Some(60.),
                ..Default::default()
            },
        );
        scene.set_isolated(root, true);
        let nested = scene.append(
            root,
            NodeKind::Rect(Color(0, 255, 0, 255)),
            Style {
                absolute: true,
                width: Some(10000.),
                height: Some(12000.),
                ..Default::default()
            },
        );
        scene.set_isolated(nested, true);
        scene.set_effects(
            nested,
            Effects {
                opacity: 0.,
                ..Default::default()
            },
        );
        scene.flush();
        assert_eq!(scene.layer_bounds(root), scene.bounds(root));
        assert!(!scene.layer_items(Some(root)).iter().any(|p| p.id == nested));
        assert!(scene.isolated_nodes().any(|id| id == nested));
        scene.set_effects(
            root,
            Effects {
                opacity: 0.,
                ..Default::default()
            },
        );
        scene.flush();
        assert!(!scene.layer_items(None).iter().any(|p| p.id == root));
        let cached = scene.layer_items(Some(root));
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].effects.opacity, 1.);
        assert_eq!(scene.layer_bounds(root), scene.bounds(root));
        assert_eq!(scene.layer_items(Some(nested))[0].effects.opacity, 1.);
    }
}

#[cfg(test)]
mod nonfinite_effect_tests {
    use super::*;
    #[test]
    fn invalid_effects_normalize_before_equality_and_paint_traversal() {
        let mut scene = Scene::new(100., 100.);
        let node = scene.append(
            scene.root(),
            NodeKind::Rect(Color(255, 0, 0, 255)),
            Style {
                width: Some(50.),
                height: Some(50.),
                ..Default::default()
            },
        );
        scene.flush();
        let invalid = Effects {
            opacity: f32::NAN,
            blur_radius: f32::INFINITY,
            edge_fade: f32::NEG_INFINITY,
            filter_blur: f32::NAN,
            brightness: 1.,
            contrast: 1.,
            saturate: 1.,
        };
        scene.set_effects(node, invalid);
        assert_eq!(scene.effects(node), Effects::default());
        assert!(scene.flush().is_idle());
        for _ in 0..3 {
            scene.set_effects(node, invalid);
            assert!(scene.flush().is_idle());
        }
        assert!(scene.paint_items().all(|p| p.effects.opacity.is_finite()
            && p.effects.blur_radius.is_finite()
            && p.effects.edge_fade.is_finite()
            && p.effects.filter_blur.is_finite()));
        assert!(scene.blur_nodes.is_empty());
        assert_eq!(scene.filtered, 0);
    }
    #[test]
    fn opacity_infinities_keep_clamped_endpoints_and_invalid_filters_remove_blur_tracking() {
        let mut scene = Scene::new(100., 100.);
        let node = scene.append(
            scene.root(),
            NodeKind::Rect(Color(255, 0, 0, 255)),
            Style {
                width: Some(50.),
                height: Some(50.),
                ..Default::default()
            },
        );
        scene.set_effects(
            node,
            Effects {
                opacity: 0.5,
                blur_radius: 4.,
                edge_fade: 3.,
                filter_blur: 0.,
                brightness: 1.,
                contrast: 1.,
                saturate: 1.,
            },
        );
        scene.flush();
        assert_eq!(scene.blur_nodes, vec![node]);
        scene.set_effects(
            node,
            Effects {
                opacity: f32::NEG_INFINITY,
                blur_radius: f32::NAN,
                edge_fade: f32::INFINITY,
                filter_blur: 0.,
                brightness: 1.,
                contrast: 1.,
                saturate: 1.,
            },
        );
        assert_eq!(
            scene.effects(node),
            Effects {
                opacity: 0.,
                ..Default::default()
            }
        );
        assert!(scene.blur_nodes.is_empty());
        assert!(!scene.layer_items(None).iter().any(|p| p.id == node));
        scene.flush();
        scene.set_effects(
            node,
            Effects {
                opacity: f32::INFINITY,
                blur_radius: -2.,
                edge_fade: -0.,
                filter_blur: 0.,
                brightness: 1.,
                contrast: 1.,
                saturate: 1.,
            },
        );
        assert_eq!(scene.effects(node), Effects::default());
        assert_eq!(scene.effects(node).edge_fade.to_bits(), 0);
        assert!(!scene.flush().damage.is_empty());
        scene.set_effects(
            node,
            Effects {
                opacity: 2.,
                blur_radius: f32::NEG_INFINITY,
                edge_fade: f32::NAN,
                filter_blur: 0.,
                brightness: 1.,
                contrast: 1.,
                saturate: 1.,
            },
        );
        assert!(scene.flush().is_idle());
    }
}

#[cfg(test)]
mod nonfinite_transform_tests {
    use super::*;
    #[test]
    fn invalid_axes_normalize_independently_before_equality_and_damage() {
        let mut scene = Scene::new(200., 150.);
        let node = scene.append(
            scene.root(),
            NodeKind::Rect(Color(255, 0, 0, 255)),
            Style {
                width: Some(30.),
                height: Some(20.),
                ..Default::default()
            },
        );
        scene.flush();
        scene.set_transform(node, Transform { x: 40., y: 25. });
        scene.flush();
        assert_eq!(scene.hit_test(45., 30.), Some(node));
        scene.set_transform(
            node,
            Transform {
                x: f32::NAN,
                y: 25.,
            },
        );
        assert_eq!(scene.transform(node), Transform { x: 0., y: 25. });
        let report = scene.flush();
        assert_eq!(report.layout_nodes, 0);
        assert!(!report.damage.is_empty());
        assert!(report.damage.iter().all(|r| r.x.is_finite()
            && r.y.is_finite()
            && r.width.is_finite()
            && r.height.is_finite()));
        assert_eq!(scene.hit_test(5., 30.), Some(node));
        assert_ne!(scene.hit_test(45., 30.), Some(node));
        let revision = scene.geometry_revision;
        for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0., 0.] {
            scene.set_transform(node, Transform { x, y: 25. });
            assert_eq!(scene.transform(node).x.to_bits(), 0);
            assert!(scene.flush().is_idle());
            assert_eq!(scene.geometry_revision, revision);
        }
        scene.set_transform(
            node,
            Transform {
                x: 12.,
                y: f32::INFINITY,
            },
        );
        assert_eq!(scene.transform(node), Transform { x: 12., y: 0. });
        assert_eq!(scene.hit_test(15., 5.), Some(node));
        assert!(!scene.flush().damage.is_empty());
        scene.set_transform(node, Transform { x: 40., y: 25. });
        assert_eq!(scene.hit_test(45., 30.), Some(node));
        assert!(!scene.flush().damage.is_empty());
    }
}

#[path = "scene_layout.rs"]
mod advanced_layout;

fn own_visible(style: &Style) -> bool {
    style.layout_options.as_deref().is_none_or(|options| {
        options.visible != Some(false) && options.display != Some(crate::layout::Display::None)
    })
}

#[cfg(test)]
mod cursor_count_tests {
    use super::*;
    #[test]
    fn clearing_and_removing_last_cursor_restores_default_fast_path() {
        let mut scene = Scene::new(100., 100.);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Column),
            Default::default(),
        );
        let child = scene.append(
            parent,
            NodeKind::Container(Layout::Column),
            Default::default(),
        );
        scene.set_cursor(parent, Some(crate::cursor::Cursor::Grab));
        scene.set_cursor(child, Some(crate::cursor::Cursor::Grabbing));
        assert_eq!(scene.cursor_styles, 2);
        scene.set_cursor(child, Some(crate::cursor::Cursor::Pointer));
        assert_eq!(scene.cursor_styles, 2);
        scene.set_cursor(child, None);
        assert_eq!(scene.cursor_styles, 1);
        scene.remove(parent);
        assert_eq!(scene.cursor_styles, 0);
        assert_eq!(scene.cursor_at(10., 10.), None);
    }
}
#[cfg(test)]
mod local_transform_tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    fn fixed(width: f32, height: f32) -> Style {
        Style {
            width: Some(width),
            height: Some(height),
            ..Style::default()
        }
    }
    fn rotated(radians: f32) -> LocalTransform {
        LocalTransform {
            rotate: radians,
            ..LocalTransform::IDENTITY
        }
    }
    /// A scene with an overlay root and one `width` × `height` rect at (100, 100).
    fn placed(width: f32, height: f32) -> (Scene, NodeId) {
        let mut scene = Scene::new(400., 300.);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let id = scene.append(
            scene.root(),
            NodeKind::Rect(Color(1, 2, 3, 255)),
            fixed(width, height),
        );
        scene.set_transform(id, Transform { x: 100., y: 100. });
        scene.flush();
        (scene, id)
    }
    fn union(damage: &[Rect]) -> Rect {
        damage.iter().copied().reduce(Rect::union).unwrap()
    }
    fn near(a: Rect, b: Rect) -> bool {
        (a.x - b.x).abs() <= 1.
            && (a.y - b.y).abs() <= 1.
            && (a.width - b.width).abs() <= 2.
            && (a.height - b.height).abs() <= 2.
    }
    #[test]
    fn matrix_is_css_order_about_the_origin() {
        let bounds = Rect::new(10., 20., 40., 20.);
        let transform = LocalTransform {
            scale: (2., 1.),
            rotate: FRAC_PI_2,
            ..LocalTransform::IDENTITY
        };
        let matrix = transform.matrix(bounds);
        // The centre stays; scale applies first, along x, then rotation.
        let (x, y) = matrix.point(30., 30.);
        assert!((x - 30.).abs() < 1e-4 && (y - 30.).abs() < 1e-4);
        let (x, y) = matrix.point(40., 30.);
        assert!((x - 30.).abs() < 1e-4 && (y - 50.).abs() < 1e-4);
        let corner = LocalTransform {
            origin: (0., 0.),
            ..transform
        };
        let (x, y) = corner.matrix(bounds).point(10., 20.);
        assert!((x - 10.).abs() < 1e-4 && (y - 20.).abs() < 1e-4);
        let skew = LocalTransform {
            skew: (FRAC_PI_4, 0.),
            origin: (0., 0.),
            ..LocalTransform::IDENTITY
        };
        let (x, y) = skew.matrix(bounds).point(10., 30.);
        assert!((x - 20.).abs() < 1e-4 && (y - 30.).abs() < 1e-4);
        assert!(LocalTransform::IDENTITY.matrix(bounds) == crate::affine::Affine::IDENTITY);
    }
    #[test]
    fn equal_and_normalized_writes_do_nothing() {
        let (mut scene, id) = placed(40., 20.);
        scene.set_local_transform(id, rotated(0.3));
        assert!(!scene.flush().is_idle());
        let revision = scene.interaction_revision();
        scene.set_local_transform(id, rotated(0.3));
        assert!(scene.flush().is_idle());
        assert_eq!(scene.interaction_revision(), revision);
        // Nonfinite components normalize before comparing: NaN never equals.
        scene.set_local_transform(
            id,
            LocalTransform {
                scale: (f32::NAN, 1.),
                skew: (0., f32::INFINITY),
                origin: (f32::NAN, 0.5),
                ..rotated(0.3)
            },
        );
        assert!(scene.flush().is_idle());
        assert_eq!(scene.local_transform(id), rotated(0.3));
        // Moving the origin of an identity transform moves nothing.
        scene.set_local_transform(id, LocalTransform::IDENTITY);
        scene.flush();
        scene.set_local_transform(
            id,
            LocalTransform {
                origin: (0., 0.),
                ..LocalTransform::IDENTITY
            },
        );
        assert!(scene.flush().is_idle());
        assert!(!scene.is_layer(id));
    }
    #[test]
    fn transform_changes_damage_old_and_new_transformed_bounds_only() {
        let (mut scene, id) = placed(40., 20.);
        let layout = scene.layout_revision();
        scene.set_local_transform(id, rotated(FRAC_PI_2));
        let frame = scene.flush();
        assert_eq!(frame.layout_nodes, 0);
        assert_eq!(frame.paint_nodes, 0);
        assert_eq!(frame.composite_nodes, 1);
        assert_eq!(scene.layout_revision(), layout);
        // Old box (100, 100, 40, 20) and the upright one about its centre
        // (110, 90, 20, 40), merged as they overlap.
        assert!(near(union(&frame.damage), Rect::new(100., 90., 40., 40.)));
        assert!(near(
            scene.transformed_bounds(id),
            Rect::new(110., 90., 20., 40.)
        ));
        assert_eq!(scene.bounds(id), Rect::new(100., 100., 40., 20.));
        assert!(scene.is_layer(id));
        // The layer's pixels are kept: its revision does not move.
        let revision = scene.layer_revision(id);
        scene.set_local_transform(id, rotated(1.0));
        scene.flush();
        assert_eq!(scene.layer_revision(id), revision);
        // Translating it carries the rotated box along.
        scene.set_local_transform(id, rotated(FRAC_PI_2));
        scene.flush();
        scene.set_transform(id, Transform { x: 200., y: 100. });
        let frame = scene.flush();
        assert!(
            frame
                .damage
                .iter()
                .any(|d| near(*d, Rect::new(110., 90., 20., 40.)))
        );
        assert!(
            frame
                .damage
                .iter()
                .any(|d| near(*d, Rect::new(210., 90., 20., 40.)))
        );
    }
    #[test]
    fn descendants_damage_where_a_transformed_ancestor_shows_them() {
        let mut scene = Scene::new(400., 300.);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            fixed(40., 40.),
        );
        scene.set_transform(parent, Transform { x: 100., y: 100. });
        let child = scene.append(parent, NodeKind::Rect(Color(9, 9, 9, 255)), fixed(10., 10.));
        scene.set_local_transform(
            parent,
            LocalTransform {
                scale: (2., 2.),
                origin: (0., 0.),
                ..LocalTransform::IDENTITY
            },
        );
        scene.flush();
        let revision = scene.layer_revision(parent);
        scene.set_kind(child, NodeKind::Rect(Color(1, 1, 1, 255)));
        let frame = scene.flush();
        // The 10 × 10 child at the parent's origin shows twice the size.
        assert!(near(union(&frame.damage), Rect::new(100., 100., 20., 20.)));
        assert_ne!(scene.layer_revision(parent), revision);
        assert!(near(
            scene.transformed_bounds(child),
            Rect::new(100., 100., 20., 20.)
        ));
        assert!(near(
            scene.visible_bounds(child).unwrap(),
            Rect::new(100., 100., 20., 20.)
        ));
    }
    #[test]
    fn rotated_ink_culls_by_where_it_paints() {
        let (mut scene, id) = placed(40., 20.);
        scene.set_local_transform(id, rotated(FRAC_PI_2));
        scene.flush();
        let visits = |scene: &Scene, damage: Rect| {
            scene
                .layer_items_within(None, Some(&[damage]))
                .iter()
                .any(|item| item.id == id && item.isolated && item.matrix.is_some())
        };
        // Above the laid-out box, inside the rotated one.
        assert!(visits(&scene, Rect::new(118., 92., 4., 4.)));
        // Inside the laid-out box's corner, outside the rotated one.
        assert!(!visits(&scene, Rect::new(101., 101., 4., 4.)));
        // Its own items are drawn in its unrotated space.
        let items = scene.layer_items(Some(id));
        assert_eq!(items[0].bounds, Rect::new(100., 100., 40., 20.));
        assert_eq!(items[0].matrix, None);
    }
    #[test]
    fn hit_tests_map_through_rotation_scale_and_skew() {
        let (mut scene, id) = placed(40., 40.);
        let root = scene.root();
        scene.set_local_transform(id, rotated(FRAC_PI_4));
        scene.flush();
        // The diamond's top point reaches 28 px above the centre (120, 120).
        assert_eq!(scene.hit_test(120., 94.), Some(id));
        assert_eq!(scene.hit_test(102., 102.), Some(root));
        scene.set_local_transform(
            id,
            LocalTransform {
                scale: (1.5, 1.5),
                ..LocalTransform::IDENTITY
            },
        );
        scene.flush();
        assert_eq!(scene.hit_test(92., 92.), Some(id));
        assert_eq!(scene.hit_test(88., 88.), Some(root));
        scene.set_local_transform(
            id,
            LocalTransform {
                scale: (0., 1.),
                ..LocalTransform::IDENTITY
            },
        );
        scene.flush();
        assert_eq!(scene.hit_test(120., 120.), Some(root));
        // Skewed along x by 45°: rows shift right with their distance below
        // the centre.
        scene.set_local_transform(
            id,
            LocalTransform {
                skew: (FRAC_PI_4, 0.),
                ..LocalTransform::IDENTITY
            },
        );
        scene.flush();
        assert_eq!(scene.hit_test(150., 135.), Some(id));
        assert_eq!(scene.hit_test(105., 135.), Some(root));
    }
    #[test]
    fn descendants_and_clips_hit_where_drawn() {
        let mut scene = Scene::new(400., 300.);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let root = scene.root();
        let card = scene.append(
            root,
            NodeKind::Container(Layout::Overlay),
            Style {
                clip: true,
                ..fixed(40., 40.)
            },
        );
        scene.set_transform(card, Transform { x: 100., y: 100. });
        // Overflows the card to the right; the card clips it.
        let wide = scene.append(card, NodeKind::Rect(Color(0, 0, 0, 255)), fixed(80., 10.));
        scene.set_local_transform(card, rotated(FRAC_PI_2));
        scene.flush();
        // Rotated about (120, 120): the child's row runs down the card's
        // right side (x 130..140), inside the clip from y 100 to 140.
        assert_eq!(scene.hit_test(135., 105.), Some(wide));
        assert_eq!(scene.hit_test(135., 135.), Some(wide));
        // Where it would be unrotated: the card, not the child.
        assert_eq!(scene.hit_test(105., 105.), Some(card));
        // Below the card the child is clipped away, rotated or not.
        assert_eq!(scene.hit_test(135., 150.), Some(root));
        assert_eq!(scene.hit_test_all(135., 105.), vec![wide, card, root]);
    }
    #[test]
    fn nested_transforms_compose() {
        let mut scene = Scene::new(400., 300.);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let panel = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            fixed(100., 100.),
        );
        let card = scene.append(panel, NodeKind::Rect(Color(0, 0, 0, 255)), fixed(20., 20.));
        scene.set_transform(card, Transform { x: 40., y: 40. });
        scene.set_local_transform(
            panel,
            LocalTransform {
                scale: (2., 2.),
                origin: (0., 0.),
                ..LocalTransform::IDENTITY
            },
        );
        scene.set_local_transform(card, rotated(FRAC_PI_4));
        scene.flush();
        // The card's centre (50, 50) lands at (100, 100); its diamond spans
        // 2 × 14.1 px either side there.
        assert!(near(
            scene.transformed_bounds(card),
            Rect::new(71.7, 71.7, 56.6, 56.6)
        ));
        assert_eq!(scene.hit_test(100., 74.), Some(card));
        assert_eq!(scene.hit_test(82., 82.), Some(panel));
        let items = scene.layer_items(Some(panel));
        let inner = items.iter().find(|item| item.id == card).unwrap();
        assert!(inner.isolated && inner.matrix.is_some());
    }
    #[test]
    fn removing_transformed_nodes_restores_counts() {
        let (mut scene, id) = placed(10., 10.);
        scene.set_local_transform(id, rotated(1.));
        scene.flush();
        assert_eq!((scene.transformed, scene.promoted_count), (1, 1));
        scene.set_local_transform(id, LocalTransform::IDENTITY);
        assert_eq!((scene.transformed, scene.promoted_count), (0, 1));
        scene.remove(id);
        assert_eq!((scene.transformed, scene.promoted_count), (0, 0));
    }
    #[test]
    fn content_blur_promotes_and_animates_by_compositing_alone() {
        let (mut scene, id) = placed(40., 20.);
        assert!(!scene.is_layer(id));
        scene.set_filter_blur(id, 2.);
        let frame = scene.flush();
        assert!(scene.is_layer(id));
        assert_eq!((frame.layout_nodes, frame.paint_nodes), (0, 0));
        assert_eq!(frame.composite_nodes, 1);
        // The box (100, 100, 40, 20) spread three sigma all round.
        assert!(near(union(&frame.damage), Rect::new(94., 94., 52., 32.)));
        // Its own items paint unblurred inside the layer; the window sees
        // one layer item.
        let items = scene.layer_items(None);
        let item = items.iter().find(|item| item.id == id).unwrap();
        assert!(item.isolated && item.matrix.is_none());
        assert_eq!(item.effects.filter_blur, 2.);
        assert_eq!(scene.layer_items(Some(id))[0].effects, Effects::default());
        // Animating the sigma neither lays out nor repaints the content.
        let (revision, content, layout) = (
            scene.layer_revision(id),
            scene.content_revision(),
            scene.layout_revision(),
        );
        scene.set_effects(
            id,
            Effects {
                opacity: 0.5,
                filter_blur: 4.,
                ..Effects::default()
            },
        );
        let frame = scene.flush();
        assert_eq!((frame.layout_nodes, frame.paint_nodes), (0, 0));
        assert!(near(union(&frame.damage), Rect::new(88., 88., 64., 44.)));
        assert_eq!(scene.layer_revision(id), revision);
        assert_eq!(scene.content_revision(), content);
        assert_eq!(scene.layout_revision(), layout);
        // Culling sees the spread: damage beside the box reaches the layer.
        let visits = |scene: &Scene, damage: Rect| {
            scene
                .layer_items_within(None, Some(&[damage]))
                .iter()
                .any(|item| item.id == id)
        };
        assert!(visits(&scene, Rect::new(90., 90., 4., 4.)));
        assert!(!visits(&scene, Rect::new(80., 80., 4., 4.)));
        // Back at 0, it paints as any node again, damaging the old spread.
        scene.set_filter_blur(id, 0.);
        let frame = scene.flush();
        assert!(!scene.is_layer(id));
        assert!(near(union(&frame.damage), Rect::new(88., 88., 64., 44.)));
        assert!(
            scene
                .layer_items(None)
                .iter()
                .any(|item| item.id == id && !item.isolated)
        );
        assert_eq!((scene.filtered, scene.promoted_count), (0, 1));
        scene.set_filter_blur(id, 1.);
        scene.remove(id);
        assert_eq!((scene.filtered, scene.promoted_count), (0, 0));
    }
    #[test]
    fn descendants_of_a_blurred_node_damage_its_spread() {
        let mut scene = Scene::new(400., 300.);
        scene.set_kind(scene.root(), NodeKind::Container(Layout::Overlay));
        let row = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            Style {
                clip: true,
                ..fixed(40., 40.)
            },
        );
        scene.set_transform(row, Transform { x: 100., y: 100. });
        let word = scene.append(row, NodeKind::Rect(Color(9, 9, 9, 255)), fixed(10., 10.));
        scene.set_filter_blur(row, 2.);
        scene.flush();
        let revision = scene.layer_revision(row);
        scene.set_kind(word, NodeKind::Rect(Color(1, 1, 1, 255)));
        let frame = scene.flush();
        // The 10 × 10 word at the row's origin, blurred out 6 px past it
        // (the row clips before its blur, not after).
        assert!(near(union(&frame.damage), Rect::new(94., 94., 22., 22.)));
        assert_ne!(scene.layer_revision(row), revision);
        // Scaled up twice, the spread doubles with the content.
        scene.set_local_transform(
            row,
            LocalTransform {
                scale: (2., 2.),
                origin: (0., 0.),
                ..LocalTransform::IDENTITY
            },
        );
        scene.flush();
        scene.set_kind(word, NodeKind::Rect(Color(2, 2, 2, 255)));
        let frame = scene.flush();
        assert!(near(union(&frame.damage), Rect::new(88., 88., 44., 44.)));
        // A blurred child spreads its parent layer's bounds.
        let mut scene = Scene::new(400., 300.);
        let parent = scene.append(
            scene.root(),
            NodeKind::Container(Layout::Overlay),
            fixed(40., 40.),
        );
        let child = scene.append(parent, NodeKind::Rect(Color(9, 9, 9, 255)), fixed(10., 10.));
        scene.set_isolated(parent, true);
        scene.set_filter_blur(child, 5.);
        scene.flush();
        assert_eq!(scene.layer_bounds(parent), Rect::new(-15., -15., 55., 55.));
    }
    #[test]
    fn clip_insets_clip_items_follow_size_and_composite_alone() {
        use crate::layout::Length::{Percent, Px};
        let (mut scene, id) = placed(40., 20.);
        let child = scene.append(id, NodeKind::Rect(Color(9, 9, 9, 255)), fixed(40., 20.));
        scene.flush();
        let clip_of = |scene: &Scene, node: NodeId| {
            scene
                .layer_items(None)
                .into_iter()
                .find(|item| item.id == node)
                .and_then(|item| item.clip)
        };
        // Closed to the top half: top 0, bottom 50% of its 20 px height.
        let half = ClipInset {
            top: Px(0.),
            right: Px(-5.),
            bottom: Percent(0.5),
            left: Px(2.),
        };
        let layout = scene.layout_revision();
        scene.set_clip_inset(id, Some(half));
        let frame = scene.flush();
        assert_eq!((frame.layout_nodes, frame.paint_nodes), (0, 0));
        assert_eq!(scene.layout_revision(), layout);
        assert!(near(union(&frame.damage), Rect::new(100., 100., 40., 20.)));
        let inset = Rect::new(102., 100., 43., 10.);
        assert_eq!(clip_of(&scene, id), Some(inset));
        assert_eq!(clip_of(&scene, child), Some(inset));
        // Hits only inside it.
        assert_eq!(scene.hit_test(110., 105.), Some(id));
        assert_eq!(scene.hit_test(110., 115.), Some(scene.root()));
        // The percentage follows the box as it grows.
        let mut style = scene.style(id);
        style.height = Some(40.);
        scene.set_style(id, style);
        scene.flush();
        assert_eq!(clip_of(&scene, id), Some(Rect::new(102., 100., 43., 20.)));
        // A blurred layer's composite is clipped; its pixels keep.
        scene.set_filter_blur(id, 2.);
        scene.flush();
        let revision = scene.layer_revision(id);
        scene.set_clip_inset(id, Some(ClipInset::px(1., 1., 1., 1.)));
        scene.flush();
        assert_eq!(scene.layer_revision(id), revision);
        assert_eq!(clip_of(&scene, id), Some(Rect::new(101., 101., 38., 38.)));
        scene.set_clip_inset(id, None);
        assert!(!scene.flush().damage.is_empty());
        assert_eq!(scene.clip_inset(id), None);
    }
    #[test]
    fn colour_filters_compose_down_the_tree_without_layers() {
        let (mut scene, id) = placed(40., 20.);
        assert_eq!(Effects::default().color_matrix(), None);
        // Nonfinite factors change nothing; negative ones clamp to 0.
        scene.set_effects(
            id,
            Effects {
                brightness: f32::NAN,
                contrast: -1.,
                ..Effects::default()
            },
        );
        assert_eq!(
            (scene.effects(id).brightness, scene.effects(id).contrast),
            (1., 0.)
        );
        let half = Effects {
            brightness: 0.5,
            ..Effects::default()
        };
        scene.set_effects(id, half);
        let frame = scene.flush();
        assert_eq!((frame.layout_nodes, frame.paint_nodes), (0, 0));
        assert!(!scene.is_layer(id));
        let matrix = half.color_matrix().unwrap();
        assert_eq!(matrix[0][0], 0.5);
        assert_eq!(matrix[1][1], 0.5);
        assert_eq!(matrix[0][3], 0.);
        // A filtered node inside a filtered one: both, composed.
        let quarter = compose_color(Some(matrix), Some(matrix)).unwrap();
        assert_eq!(quarter[2][2], 0.25);
        let item = scene
            .layer_items(None)
            .into_iter()
            .find(|item| item.id == id)
            .unwrap();
        assert_eq!(item.color, Some(matrix));
        // Contrast 0 maps everything to mid grey.
        let grey = Effects {
            contrast: 0.,
            ..Effects::default()
        }
        .color_matrix()
        .unwrap();
        assert_eq!(grey[0], [0., 0., 0., 0.5]);
    }
    #[test]
    fn masks_promote_and_move_by_compositing_alone() {
        let image = Arc::new(crate::image::ImageData::new(2, 1, vec![255; 8]).unwrap());
        let (mut scene, id) = placed(40., 20.);
        let content = scene.content_revision();
        scene.set_mask(
            id,
            Some(ImageMask {
                image: image.clone(),
                offset: (0., 0.),
                size: (20., 20.),
            }),
        );
        let frame = scene.flush();
        assert!(scene.is_layer(id));
        assert_eq!((frame.layout_nodes, frame.paint_nodes), (0, 0));
        // A mask only hides: damage stays within the box.
        assert!(near(union(&frame.damage), Rect::new(100., 100., 40., 20.)));
        assert_ne!(scene.content_revision(), content);
        let items = scene.layer_items(None);
        assert!(items.iter().any(|item| item.id == id && item.isolated));
        let (revision, content) = (scene.layer_revision(id), scene.content_revision());
        scene.set_mask_offset(id, (12.5, f32::NAN));
        let frame = scene.flush();
        assert_eq!((frame.layout_nodes, frame.paint_nodes), (0, 0));
        assert_eq!(frame.composite_nodes, 1);
        assert!(near(union(&frame.damage), Rect::new(100., 100., 40., 20.)));
        assert_eq!(scene.mask(id).unwrap().offset, (12.5, 0.));
        assert_eq!(scene.layer_revision(id), revision);
        assert_eq!(scene.content_revision(), content);
        // The same offset again changes nothing.
        scene.set_mask_offset(id, (12.5, 0.));
        assert!(scene.flush().is_idle());
        scene.set_mask(id, None);
        scene.flush();
        assert!(!scene.is_layer(id));
        // Without a mask, moving one does nothing.
        scene.set_mask_offset(id, (1., 1.));
        assert!(scene.flush().is_idle());
    }
}
