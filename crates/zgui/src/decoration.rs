//! Optional detailed paint styles; ordinary quads keep their existing fast path.
use crate::{
    canvas::{Brush, GradientStop},
    scene::{BoxShadow, Insets},
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
/// Corner smoothing that reads as iOS's continuous corners (Figma's "iOS"
/// preset): see `corner_shape`.
pub const IOS_CORNER_SMOOTHING: f32 = 0.6;
/// The corner smoothing boxes take when their style sets none (`f32` bits).
static CORNER_SMOOTHING: AtomicU32 = AtomicU32::new(0);
/// Every rounded corner whose style sets no smoothing of its own (fills,
/// borders, outlines, shadows, rounded clips, backdrop blurs, native views)
/// takes `smoothing` from here: 0, the default, keeps circular corners;
/// `IOS_CORNER_SMOOTHING` makes squircles of them all. Set it before the
/// first frame: rasters already cached keep the corners they were drawn with.
pub fn set_default_corner_smoothing(smoothing: f32) {
    CORNER_SMOOTHING.store(clamp_smoothing(smoothing).to_bits(), Ordering::Relaxed);
}
/// The smoothing set by `set_default_corner_smoothing`.
pub fn default_corner_smoothing() -> f32 {
    f32::from_bits(CORNER_SMOOTHING.load(Ordering::Relaxed))
}
/// `smoothing` in 0..=1 (anything else, as NaN, is 0).
pub fn clamp_smoothing(smoothing: f32) -> f32 {
    if smoothing.is_finite() {
        smoothing.clamp(0., 1.)
    } else {
        0.
    }
}
/// A rounded corner of `radius` with `smoothing` (0 to 1) in a box whose
/// corners may reach `limit` (half its shorter side): how far along each
/// edge the curve runs (its extent) and the exponent `n` of the superellipse
/// quadrant it follows, |x|ⁿ + |y|ⁿ = extentⁿ about the point `extent` in
/// from both edges.
///
/// Smoothing 0 is the circle (`radius`, 2). Above, the curve starts
/// `radius · (1 + smoothing)` along the edges, as Figma's smoothed corners
/// do, and `n` keeps its middle where the circle's is (its 45° point
/// `radius · (1 − 1/√2)` in from each edge), so a squircle looks as round as
/// the circle it replaces: its curvature eases in from the straight edges
/// rather than jumping to `1/radius`. Without room for the extent (a pill),
/// the smoothing gives way, back to a circle at `extent == radius`.
pub fn corner_shape(radius: f32, smoothing: f32, limit: f32) -> (f32, f32) {
    let (radius, extent) = corner_extent(radius, smoothing, limit);
    (extent, corner_exponent(radius, extent))
}
/// A corner's radius held to `limit`, and its extent (see `corner_shape`).
fn corner_extent(radius: f32, smoothing: f32, limit: f32) -> (f32, f32) {
    let limit = limit.max(0.);
    let radius = if radius.is_finite() {
        radius.clamp(0., limit)
    } else {
        0.
    };
    (
        radius,
        (radius * (1. + clamp_smoothing(smoothing))).min(limit),
    )
}
/// The exponent of a corner of `radius` running `extent` along its edges.
fn corner_exponent(radius: f32, extent: f32) -> f32 {
    if extent <= radius {
        return 2.;
    }
    let inset = (1. - std::f32::consts::FRAC_1_SQRT_2) * radius / extent;
    -std::f32::consts::LN_2 / (1. - inset).ln()
}
/// Signed distance (negative inside) from a point to a box with a rounded
/// corner of `radius` and `smoothing` (see `corner_shape`) toward the point:
/// `q` is the point's offset from the box's centre, made positive, and
/// `half` the box's half size. The superellipse's distance is its norm less
/// its extent over the norm's gradient, exact on its edge (where coverage
/// is decided) and close within a few pixels of it. corner.wgsl computes
/// the same for the draw and blur shaders.
pub fn corner_distance(q: (f32, f32), half: (f32, f32), radius: f32, smoothing: f32) -> f32 {
    let (radius, extent) = corner_extent(radius, smoothing, half.0.min(half.1));
    let (dx, dy) = (q.0 - half.0 + extent, q.1 - half.1 + extent);
    if extent > radius * 1.0001 && dx > 0. && dy > 0. {
        // With u the smaller part over the larger, the norm is the larger
        // times (1 + uⁿ)^(1/n) and its gradient's length √(1 + u^(2n−2))
        // over (1 + uⁿ)^((n−1)/n): two powers, the exponent only here.
        let n = corner_exponent(radius, extent);
        let (most, u) = (dx.max(dy), dx.min(dy) / dx.max(dy));
        let un1 = u.powf(n - 1.);
        let s = 1. + un1 * u;
        let a = s.powf(1. / n);
        return (most * a - extent) * s / (a * (1. + un1 * un1).sqrt());
    }
    dx.max(0.).hypot(dy.max(0.)) + dx.max(dy).min(0.) - extent
}
/// The handle of a cubic Bézier that draws a corner of exponent `n` (see
/// `corner_shape`), as a fraction of its extent: each control point that
/// far from its end toward the corner, so the curve passes through the
/// superellipse's 45° point. The circle's is the familiar 0.5523.
pub fn corner_handle(n: f32) -> f32 {
    (8. * 2_f32.powf(-1. / n) - 4.) / 3.
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}
impl Corners {
    pub fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed {
        length: f32,
        gap: f32,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub enum Background {
    Brush(Brush),
    /// Angle in clockwise radians; endpoints span the allocated box.
    Linear {
        angle: f32,
        stops: Arc<[GradientStop]>,
    },
    /// An image filling the box as `fit` places it (centred; CSS
    /// `background-size` cover/contain, `object-fit` alike), over the
    /// box's `fill` colour, cut to its rounded corners, under its border
    /// and children.
    Image {
        image: Arc<crate::image::ImageData>,
        fit: crate::style::ObjectFit,
    },
    /// A fragment shader filling the box (see `FillShader`), over its
    /// `fill` colour where it returns transparency, cut to its rounded
    /// corners, under its border and children. Its uniforms are the node's
    /// (`Scene::set_shader_uniforms`).
    Shader(Arc<FillShader>),
}
/// A fill written as a WGSL function the GPU runs for every pixel of a box:
///
/// ```wgsl
/// fn fill(input: FillInput) -> vec4<f32>
/// ```
///
/// returning a straight-alpha colour. `FillInput` holds `p` (the pixel's
/// logical position from the box's top-left), `size` (the box's logical
/// size), `time` (seconds, as the node's uniforms set it), `scale` (device
/// pixels per logical pixel) and `params` (`array<vec4<f32>, 4>`: the
/// node's sixteen user floats, colours or anything else). Its helper
/// functions and constants may sit alongside; names starting `zgui_` are
/// reserved. A shader that fails to compile fills with the box's colour.
#[derive(Debug)]
pub struct FillShader {
    id: u64,
    source: Arc<str>,
    fps: Option<f32>,
}
impl FillShader {
    /// `source`, its time advancing at most `fps` times a second (a dither
    /// drifting at 30 fps redraws at 30 fps): `None` for every frame.
    pub fn new(source: impl Into<Arc<str>>, fps: Option<f32>) -> Arc<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Arc::new(Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            source: source.into(),
            fps: fps.filter(|fps| fps.is_finite() && *fps > 0.),
        })
    }
    /// Stable for this shader's lifetime: renderers cache its pipeline by it.
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn fps(&self) -> Option<f32> {
        self.fps
    }
}
impl PartialEq for FillShader {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
/// A shader fill's per-node uniforms (`Scene::set_shader_uniforms`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShaderUniforms {
    /// Seconds.
    pub time: f32,
    /// Sixteen user values, as four vec4s.
    pub params: [[f32; 4]; 4],
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoration {
    pub background: Option<Background>,
    pub corners: Option<Corners>,
    pub border_widths: Option<Insets>,
    pub border_style: BorderStyle,
    /// Some(empty) explicitly clears all shadows; None inherits the legacy shadow.
    pub shadows: Option<Arc<[BoxShadow]>>,
}

#[cfg(test)]
mod corner_tests {
    use super::*;
    #[test]
    fn smoothing_zero_is_the_circle() {
        assert_eq!(corner_shape(20., 0., 50.), (20., 2.));
        // A point 25 px out along the diagonal from the corner's centre.
        let q = (50. - 20. + 25. * std::f32::consts::FRAC_1_SQRT_2, 0.);
        let q = (q.0, q.0);
        assert!((corner_distance(q, (50., 50.), 20., 0.) - 5.).abs() < 1e-4);
        // Edges and the inside as a box's.
        assert!((corner_distance((10., 52.), (50., 50.), 20., 0.) - 2.).abs() < 1e-5);
        assert!((corner_distance((0., 0.), (50., 50.), 20., 0.) + 50.).abs() < 1e-5);
    }
    #[test]
    fn a_squircle_runs_further_along_its_edges_through_the_circles_middle() {
        let (extent, n) = corner_shape(20., 0.6, 50.);
        assert!((extent - 32.).abs() < 1e-4);
        assert!(n > 3. && n < 4., "{n}");
        // The 45° point stays where the circle's is: 20·(1 − 1/√2) in from
        // each edge of a 100 px box.
        let at = 50. - 20. * (1. - std::f32::consts::FRAC_1_SQRT_2);
        assert!(corner_distance((at, at), (50., 50.), 20., 0.6).abs() < 1e-3);
        assert!(corner_distance((at, at), (50., 50.), 20., 0.).abs() < 1e-3);
        // Where the circle has met the edge, the squircle still curves in.
        let near = (50. - 20., 49.8);
        assert!(corner_distance(near, (50., 50.), 20., 0.) < 0.);
        assert!(corner_distance(near, (50., 50.), 20., 0.6) > 0.);
        // Its distance is a distance near the edge: a pixel out along the
        // normal at the 45° point is about one out.
        let d = corner_distance((at + 0.7071, at + 0.7071), (50., 50.), 20., 0.6);
        assert!((d - 1.).abs() < 0.05, "{d}");
    }
    #[test]
    fn without_room_smoothing_gives_way() {
        // A pill: radius at half the height leaves no room to run further.
        assert_eq!(corner_shape(20., 0.6, 20.), (20., 2.));
        // Partly: the extent stops at the limit, the curve between.
        let (extent, n) = corner_shape(20., 0.6, 26.);
        assert_eq!(extent, 26.);
        assert!(n > 2. && n < corner_shape(20., 0.6, 50.).1);
        // Out of range smoothing clamps; NaN is none.
        assert_eq!(corner_shape(20., 3., 100.), corner_shape(20., 1., 100.));
        assert_eq!(corner_shape(20., f32::NAN, 100.), (20., 2.));
    }
    #[test]
    fn bezier_handles_match_the_circle_and_grow_squarer() {
        assert!((corner_handle(2.) - 0.552_284_8).abs() < 1e-4);
        assert!(corner_handle(4.) > 0.85);
    }
}
