//! Optional detailed paint styles; ordinary quads keep their existing fast path.
use crate::{
    canvas::{Brush, GradientStop},
    scene::{BoxShadow, Insets},
};
use std::sync::Arc;
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
    /// Shadows inside the box (CSS's `inset`), over its fill and under its
    /// border and children: the box less itself moved by the offset and
    /// shrunk by the spread, cut to its corners (a 1-point top highlight is
    /// `inset 0 1px`). Drawn sharp: their blur is not applied.
    pub inset_shadows: Option<Arc<[BoxShadow]>>,
}
