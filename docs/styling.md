# Styling retained views

zgui's Rust API combines GPUI/QuickGUI-style fluent methods with owned component children. Import `Styled` to use the methods on a `View` or reusable `Styles` patch. Values are logical pixels except explicit percentage dimensions; spacing methods take pixel values directly, not a numbered spacing scale.

```rust
use zgui::compose::{button, column, text};
use zgui::style::{rgb, Styled, Styles};

let card = column()
    .w(320.)
    .p(16.)
    .gap(12.)
    .bg(rgb(0x202838))
    .rounded(10.)
    .border_1()
    .border_color(rgb(0x465268))
    .text_color(rgb(0xf0f4ff))
    .text_size(16.)
    .child(text("Connection settings").text_size(22.))
    .child(button()
        .px(14.).py(8.)
        .bg(rgb(0x2854a0))
        .hover(|s| s.bg(rgb(0x3266bd)))
        .active(|s| s.bg(rgb(0x1d407e)))
        .focus(|s| s.border_1().border_color(rgb(0x9abfff)))
        .disabled_style(|s| s.opacity(0.45))
        .child(text("Connect"))
        .on_click(|| println!("connect")));
```

Mount with `Ui::mount(card)` or use the native window's view rendering entry point. A background, rounded border, or shadow decorates the container itself; it does not add a layout child. Children remain retained when a visual style changes.

## Properties and patches

| Area | Methods |
| --- | --- |
| Dimensions | `w`, `h`, `size`, `w_percent`, `h_percent`, `w_full`, `h_full`, `min_w`, `max_w`, `min_h`, `max_h` |
| Padding | `p`, `px`, `py`, `pt`, `pr`, `pb`, `pl` |
| Margins | `m`, `mx`, `my`, `mt`, `mr`, `mb`, `ml` |
| Layout | `flex_row`, `flex_col`, `gap`, `flex_grow`, `flex_shrink`, `grow`, `shrink_0` |
| Alignment | `items_start`, `items_center`, `items_end`, `items_stretch`, `justify_start`, `justify_center`, `justify_end`, `justify_between`, `justify_around` |
| Surface | `bg`, `rounded`, `border`, `border_1`, `border_color`, `shadow`, `shadow_none` |
| Text | `text_color`, `text_size`, `line_height`, `line_height_normal`, `letter_spacing`, `text_wrap`, `font_family`, `font_weight`, `font_bold`, `italic` |
| Effects | `opacity`, `blur`, `filter_blur`, `mask`, `mask_offset`, `edge_fade`, `translate`, `scale`, `scale_xy`, `rotate`, `skew`, `transform_origin`, `isolated` |
| Clipping | `overflow_hidden`, `overflow_visible` |

`Styles::new()` is an empty patch. `.style(patch)` changes only fields supplied by that patch. Later writes win, including individual edges: `.p(8.).px(12.).pt(3.)` produces top 3, right 12, bottom 8, left 12. A later `.pl(20.)` preserves the other three edges. Per-edge padding participates in intrinsic sizing, flex allocation, and resizing.

```rust
use zgui::style::{rgb, Styled, Styles};
let surface = Styles::new().p(12.).bg(rgb(0x202838)).rounded(8.);
let compact = surface.clone().py(6.); // retains horizontal padding and surface
```

Use `.apply(...)` for reusable style functions, `.when(...)` for a conditional transformation, and `.when_some(...)` for an optional value. These helpers work on both views and sparse `Styles` patches; skipped closures are never called.

```rust
use zgui::compose::{column, text};
use zgui::style::{rgb, Styled, Styles};

fn card_surface<S: Styled>(style: S) -> S {
    style.p(16.).bg(rgb(0x202838)).rounded(10.)
}

let compact = true;
let width = Some(320.);
let card = column()
    .apply(card_surface)
    .when(compact, |view| view.py(8.))
    .when_some(width, |view, width| view.w(width))
    .child(text("Connection settings"));
let reusable_patch = Styles::new().apply(card_surface);
```

These decisions run once when constructing the description. They do not subscribe to signals or reapply after mounting. For styles that change with state, return a patch from `reactive_style`; the same helpers can be used inside that tracked callback.

Text color, size, wrapping, font selection, line height and letter spacing inherit through children and components. A child can override each field independently. Updating an inherited value updates the affected text properties; it does not rerun component construction. Explicit overrides shield descendants from unchanged effective typography. Other style properties are local rather than inherited. Padded or decorated text mounts a retained box around its text node; plain text stays a single node. Static and reactive wrapping propagate through boxed text and reflow its retained text node when the available width changes.

## Percentage dimensions

`.w_percent(50.)` takes half of the parent's definite content width; `.h_percent(50.)`
does the same for height. `.w_full()` and `.h_full()` mean 100 percent. These
methods take percentages (50 means 50%, not 0.5), while `.w()` and `.h()` retain
their logical-pixel units. The parent's padding is excluded from the reference
size; the resulting child size includes the child's own padding. Margins and gaps
are separate, so two 50-percent children plus a gap can overflow unless flex
shrink is enabled.

```rust
use zgui::compose::prelude::*;
let content = row().w_full().h(240.).p(16.)
    .child(column().w_percent(40.).h_full().child("Navigation"))
    .child(column().w_percent(60.).h_full().child("Content"));
```

Pixel and percentage values replace one another per axis, including sparse style
patches and interaction variants. `.w(200.).w_percent(50.)` uses 50 percent;
a subsequent `.w(80.)` restores a pixel width. Removing a reactive override restores
the base style. Resizing a parent recomputes dependent child allocation without
reconstructing components or losing editor state. Images, ordinary scrollers and
virtual-list viewports accept the same dimension methods.

A percentage reference is definite when its parent dimension is explicit,
resolved from a percentage, or allocated by flex/stretch (the root uses the
window viewport). A mere available-space constraint or min/max bound does not
make an intrinsic axis definite. On indefinite axes, percentages fall back to
automatic intrinsic sizing, including ordinary cross-axis stretch; layout does
not iterate to solve cyclic percentage sizes. Absolute children use their
parent's final content box because they do not contribute to its intrinsic size.
For example, percentage height inside the content of a vertical scroller usually
has an indefinite reference, while its viewport can have a definite height.

Flex grow/shrink allocation and logical-pixel min/max constraints still apply.
Values above 100 percent are allowed; negative and non-finite values normalize to
zero. Only width and height currently expose percentage units; percentage
padding, margins and min/max dimensions are not provided.

## Image fitting

Image views accept `.object_fit(ObjectFit::...)`, with centered placement inside
the content box after padding:

| Mode | Behavior |
| --- | --- |
| `Fill` (default) | Stretch independently to content width and height |
| `Contain` | Preserve aspect ratio and show the full image |
| `Cover` | Preserve aspect ratio, fill the box and clip excess pixels |
| `None` | Keep source pixel dimensions, centered and clipped |
| `ScaleDown` | Contain without scaling above the source dimensions |

`.object_contain()` and `.object_cover()` are shortcuts. This property is local
to the image and is not inherited by children. Sparse reactive and interaction
patches follow the same precedence as other style properties; removing an
override restores the underlying mode. Uncovered content shows the view's
background. Clipping is rectangular, including when the outer background is
rounded. These modes change placement, not the image's intrinsic size contribution.

```rust
use std::sync::Arc;
use zgui::{compose::prelude::*, image::ImageData};
let pixels = Arc::new(ImageData::new(2, 1, vec![255; 8]).unwrap());
let thumbnail = image("Preview", pixels)
    .size(160., 120.).p(8.).bg(rgb(0x202838)).object_contain();
```

## Line height

`.line_height(24.)` sets the distance between visual text rows to 24 logical pixels.
It inherits through ordinary children, components and providers, including boxed
labels and editors. Font size still controls glyph size. `.line_height_normal()`
explicitly resets an inherited pixel value to normal spacing for the child's own
font size. Omitting the field from a `Styles` patch preserves inheritance; removing
a dynamic override restores the underlying base or inherited value.

```rust
use zgui::compose::prelude::*;
let content = column().text_size(16.).line_height(24.)
    .child(text("First line\nSecond line"))
    .child(text("Compact caption").text_size(12.).line_height_normal());
```

Positive finite values are used exactly, including values smaller than the font
size. Tight spacing can overlap adjacent glyph rows; glyph ink outside the text
node's allocated rectangle is clipped, consistent with other text rendering.
Zero, negative and non-finite values resolve to normal spacing. The normalized
metric supports stable equality and hashing, so repeated equivalent styles do
not invalidate layout or paint.

Reactive line-height changes update retained text geometry and editor caret,
selection, hit testing and IME placement together. They do not reconstruct the
component or modify its text model. Single-line inputs still use the selected
line height for their caret and metrics while remaining unwrapped.

## Letter spacing

`.letter_spacing(2.)` adds two logical pixels of tracking to native glyph
advances. It inherits through children, components and lexical slots, including
editors. `.letter_spacing(0.)` explicitly resets inherited tracking; omitting the
field preserves inheritance. Signed values allow tighter or looser text.

```rust
use zgui::compose::prelude::*;
let heading = column().letter_spacing(2.)
    .child(text("STATUS"))
    .child(text("Normal caption").letter_spacing(0.));
```

Native shaping uses the same pixel-to-em conversion for measurement, editor
geometry, GPU placement and software rasterization. Tracking is part of the font
cache key. Changing it reflows text and updates carets, selections and hit testing
without rebuilding components or changing editor text. Equal normalized values
do not invalidate geometry. Integer tracking changes can reuse glyph bitmaps
while updating their positions.

The native shaper applies tracking per shaped glyph, including the final advance;
ligatures and combining sequences therefore follow its glyph clustering rather
than a per-character CSS spacing algorithm. Core-only fallback measurement uses
approximate grapheme cells and clamps their advances at zero for very tight
spacing. Negative native advances can overlap glyphs; selection rectangles retain
ordered edges, and text ink still clips to its allocated bounds.

Zero and negative zero are equivalent; non-finite values normalize to zero.
Finite values clamp to ±1,000,000 logical pixels to keep extreme tracking from
producing nonfinite geometry. This is a bound on the style value, not a promise
that extreme spacing is readable. Font-aware custom shapers receive the
normalized value through `FontStyle::letter_spacing`.

## Interaction and reactive styles

Styles resolve in this order: base fluent properties, `reactive_style`, hover, active, focus, disabled. Later patches take precedence only for fields they specify. While disabled, hover, active, and focus patches are excluded. Removing a state or a dynamic property restores the underlying style; it does not leave stale values behind.

`.disabled(true)` and `.disabled_when(|| signal.get())` disable interaction and update accessibility. `.disabled_style(...)` controls appearance; supplying that patch alone does not disable a view. Focus is cleared when a focused subtree becomes disabled. Buttons support pointer and keyboard activation.

```rust
use zgui::{compose::column, style::{rgb, Styled, Styles}, widgets::Ui};
let mut ui = Ui::new(640., 480.);
let highlighted = ui.signal(false);
let read = highlighted.clone();
let view = column().p(12.).reactive_style(move || {
    Styles::new().bg(if read.get() { rgb(0x345888) } else { rgb(0x202838) })
});
let mounted = ui.mount(view);
highlighted.set(true);
```

Signal reads inside `reactive_style` are tracked. A background or text-color change damages paint without invalidating layout. Size, spacing, wrapping, or font-size changes invalidate dependent layout. Translations, scale/rotation/skew and opacity use compositor invalidation. Equal resolved properties produce no new scene mutation. The mounted children and component-local state are preserved in each case.

## Transparency and effects

`.translate(x, y)` uses logical pixels. Each nonfinite coordinate (NaN or either
infinity) resolves to zero at the scene boundary; the other coordinate remains
independent. Repeating an equivalent normalized translation leaves geometry
revision, layout and damage unchanged. Removing a sparse override restores the
base translation. This does not clamp otherwise finite coordinates.

Effect values normalize at the scene boundary before equality and damage checks.
Opacity clamps to `[0, 1]`; NaN uses the default `1`, while positive/negative
infinity clamp to `1`/`0`. Blur radius and edge-fade length map nonfinite or
negative values to `0`. An explicit invalid value still overrides an earlier
style: it resolves to this fallback rather than restoring inheritance. Repeating
the same normalized effect does not add scene damage or invalidation.

`rgb(0xRRGGBB)` creates an opaque color; `rgba(0xRRGGBBAA)` includes alpha. `.opacity(value)` multiplies opacity. Without isolation, opacity applies through individual subtree primitives: overlapping translucent children can accumulate alpha. Use `.isolated(true).opacity(0.5)` when the subtree should first render as a group and then composite once. GPU isolated layers retain their pixels across eligible transform/opacity-only updates, subject to cache limits.

`.scale(factor)`, `.scale_xy(x, y)`, `.rotate(radians)` and `.skew(x, y)` (radians, CSS `skew`) transform a view after layout, about `.transform_origin(x, y)` (fractions of its own size, centre by default), on top of its translation: CSS order translate, rotate, skew, scale. Each sets only its component, so a `.hover(|s| s.scale(1.05))` keeps a base rotation. Layout never sees them. A transformed view composites as a cached layer: its subtree is painted once into a texture (group opacity applies once, overflow clips to the rotated box), and a transform change only redraws the texture under the new matrix, damaging the old and new transformed bounds. Magnifying past about 1.3× (or shrinking below about 0.77×) re-renders the layer at a raster scale in √2 steps, with hysteresis, so a zoom repaints a few times rather than looking blurry or repainting every frame. Hit testing maps the pointer through each transform; a zero scale is hit nowhere. Nonfinite components resolve to their identity value. See `Scene::set_local_transform` for the scene API.

`.blur(radius)` is backdrop blur, not a generic image or text blur. It samples content painted behind the view. GPU backdrop updates reconstruct the affected filter outputs and their sampling halos in paint order. Unrelated damage stays local; connected filters can still require the full target. The software reference backend conservatively repaints the full target when blur is present. The GPU blur implementation caps sigma at 64 physical pixels. For blurring a view's own content (CSS `filter: blur()`), use `.filter_blur(sigma)` instead. `.brightness(x)`, `.contrast(x)` and `.saturate(x)` (CSS `filter`, 1 changes nothing) filter the view and its subtree's colours as a group: being affine maps of colour, they are applied as the GPU writes each colour, exactly as to the group, with no layer and no extra pass, composing with ancestors' filters and with opacity; animating them (a button brightening on hover) repaints only the view's area, and a layer's cached pixels are filtered at its composite. `.edge_fade(length)` fades the top and bottom edges. `.fade_edges(top, bottom)` on a clipping view fades its descendants out over bands at its clip's edges, per pixel and without a layer; `.fade_edges_x(left, right)` does the left and right edges (a horizontally scrolling row fading its overflow), `.fade_sides(top, right, bottom, left)` all four, each band independent, and `.fade_insets(..)`/`.fade_insets_x(..)` end the bands that far inside the clip. `.clip_inset(top, right, bottom, left)` (CSS `clip-path: inset(..)`) clips the view and its descendants to its box inset by those pixels; negative values reach past it so shadows and rings aren't cut, `.clip_inset_percent(..)` takes percentages of its live size (`clip_inset_percent(0., 0., 100., 0.)` closes it to its top edge) and `.clip_inset_lengths(ClipInset { .. })` mixes them. With `overflow_hidden` and rounded corners, the inset box is rounded alike. Animating it composites again without layout; a blurred, masked or isolated view's cached pixels are cut at composite, a transformed view's layer repaints. See `Scene::set_clip_inset`. `.overflow_hidden()` clips descendants to the layout box, and with `.rounded(r)` or `.rounded_corners(..)` to its rounded corners too (CSS `overflow: hidden` with `border-radius`): every descendant (quads, text, images, SVGs, nested and transformed layers, blurred or masked ones) is cut along the curve, antialiased, in the GPU fragment shader, following the box's laid-out size every frame. Nested rounded clips intersect (the innermost two per item; further out, their corners). Radii are held to half the box's shorter side. The clip is the box's outer edge, as the rectangular clip is. Solid borders (`.border(w)`, or per side with `.border_edges(Insets {..})`, under uniform or `.rounded_corners(..)` radii) are drawn analytically by the quad shader, so an animating box keeps them without re-rasterizing; dashed borders and gradient fills rasterize. `.border_over(true)` paints the border after the view's descendants (an inset stroke, a CSS outline drawn inside): a photo filling a framed thumbnail stays under its frame. A dashed border still paints with the fill. `.outline(width, color)` and `.outline_offset(offset)` (CSS `outline`, `outline-offset`) ring the box outside its border edge, `offset` past it (negative reaches inside), its corners the box's radii grown by `offset + width` (square corners stay square). It paints after the view's descendants, is not cut by the view's own `overflow_hidden` (an ancestor's clip still cuts it), takes no layout, and is drawn analytically by the quad shader, so changing its width, colour or offset repaints only where it was and where it lands. `.outline_none()` removes it. `.stroke(width, color, StrokeAlign::Inside | Center | Outside)` is a design tool's stroke alignment: `Inside` is `.border(width).border_color(color).border_over(true)` (inside the box, over its content), `Center` an outline at offset `-width / 2`, `Outside` one at offset 0. `.bg_image(image, fit)` fills the view with an image placed as `fit` has it, centred (`ObjectFit::Fill`, `Cover`, `Contain`, `None`, `ScaleDown`, as CSS `background-size`/`object-fit`), over its `.bg(..)` colour (set `bg` first: it clears the image), cut to its rounded corners, under its border and children. It is drawn as a textured quad following the box's size every frame, with no raster. A dashed border over a background image rasterizes without the image. `.bg_shader(FillShader::new(wgsl, fps))` fills the view with a WGSL function run per pixel on the GPU, `fn fill(input: FillInput) -> vec4<f32>` returning a straight-alpha colour, where `FillInput` has `p` (logical position in the box), `size`, `time`, `scale` (device pixels per logical pixel) and `params` (four `vec4`s of user values). It paints over the `bg` colour, cut to the rounded corners, under the border and children. `.shader_uniforms(ShaderUniforms { time, params })` or `Scene::set_shader_uniforms` sets them; changing them repaints that view alone, and the shader's `fps` quantizes time so a 30 fps effect repaints at 30 fps however often it is set. On direct Metal the same module is translated by naga. A shader that does not compile fills with the `bg` colour. The quad shader's names (`Vertex`, `Out`, `Region`, `shade`, `rounded`, `region`, `viewport`, `regions`, `atlas`, `basic`, `vs`, `fs`) and those starting `zgui_` are reserved.

`.filter_blur(sigma)` blurs the view and its subtree by a Gaussian of `sigma` logical pixels, spreading three sigma past its bounds (it is clipped by its ancestors' clips, not its own), with opacity applied to the blurred result. `.mask(image, (width, height))` shows the view through the image's alpha (CSS `mask-image`, no repeat, nearest sampling), drawn at that size and at `.mask_offset(x, y)` from the view's origin; outside the image nothing shows. Blur applies before the mask, the mask before opacity, all in the view's own space under any scale, rotation or skew. Either makes the view a cached layer: its subtree is painted once, and animating the sigma or the mask offset (a fade in from blurred text, a shimmer band, a progress fill) only blurs and masks the cached pixels again and composites them, without layout or repainting the subtree. On the GPU every layer blurred or masked anew in a frame shares two passes (a wide blur runs on a copy averaged down by a power of two, so its cost stays flat); at sigma 0 and without a mask the view paints as any other. See `Scene::set_filter_blur`, `Scene::set_mask` and `Scene::set_mask_offset`. The software reference backend approximates the blur with three box passes.

The [native effects example](../crates/zgui-desktop/examples/effects.rs) builds its glass card and controls with component children, `.reactive_style(...)`, and these fluent effect methods. Component-owned tasks animate a translation signal; the application does not append or mutate scene nodes. Run it with `cargo run -p zgui-desktop --example effects`.

`.shadow(BoxShadow { ... })` adds an outer shadow without changing layout dimensions. It is a CSS `box-shadow`: the box, grown by its spread and moved by its offset, blurred by a Gaussian of half the blur radius (exact along one axis, sampled along the other), and painted only outside the view's own rounded box, so a translucent surface does not darken over its own shadow. Shadow extents participate in damage tracking. `.shadow_none()` explicitly clears an inherited patch's shadow setting (shadows do not inherit down the view tree).

The native GPU renderer shapes text and provides fallback fonts. Font family, weight and italic inherit through components and slots alongside color, size and wrapping. Use `.font_family("DejaVu Serif").font_weight(700).italic(true)`, or the portable `FontFamily::{SansSerif, Serif, Monospace}` choices. Weight is clamped to 1–1000; named families and unavailable faces use platform fallback. A variable font (one file with a `wght` axis, such as the design system's Geist and Geist Mono) serves every weight from that one file: register it once and `font_weight(500)` measures, shapes and draws the 500 instance, the same advances and outlines as a static 500 file cut from it, on every path. Native measurement, caret shaping and GPU glyph selection share font attributes; changing them invalidates layout and retained glyph geometry. The software reference backend also renders styled fonts, using a lazy platform font database. Letter spacing and custom line height use the shared inherited metrics described above. It does not yet expose the full CSS layout or styling surface of the inspiration projects. GPU cache and effect limits are documented in [the renderer notes](../crates/zgui-gpu/README.md).

The software styled-font cache retains at most 128 entries and 8 MiB of text keys/glyph images; oversized runs render without being retained. Its platform font database is created on first styled text and shared with software layer renderers. Headless core-only scenes use approximate metrics until a `Scene::set_font_text_shaper` backend is installed.

Editable views use the same fluent styles. For example, `text_input("Search", query).w(320.).px(12.).font_family(FontFamily::Monospace).focus(|s| s.border(2.).border_color(rgb(0x5ea5ff)))` styles the actual focusable editor root. Its text and caret inherit color, size, family, weight, and italic; root padding and allocated dimensions determine the clipped editing viewport. Typography and dimension changes preserve selection and the bound model. Use `.text_wrap(true)` on `text_area` to wrap at its allocated content width, including inherited or reactive wrapping styles. Rendering, pointer hit testing, selection, caret movement and IME geometry share the same wrapped layout. Resizing reflows visual lines without adding newlines to the model or losing selection. Wrapping defaults to false, preserving horizontal scrolling for long lines; `text_input` always remains a single unwrapped line. Home/End follow visual lines while wrapping; repeated Up/Down retain the preferred horizontal caret position across shorter lines. Edits, pointer placement, Home/End, and changes to wrapping, font metrics or allocated width reset that preferred position.

Multiline editors accept wheel scrolling without moving the selection; scrolling clamps to the text extent and bubbles to ancestors at an exhausted edge. Single-line editors leave vertical wheel events to their parent. Font/color changes preserve component ownership; color-only changes repaint without relayout, while font changes also update selection, hit testing and caret metrics.

`.absolute()` removes a child from normal layout flow. It starts at its parent's
content origin (inside padding), with `.ml(...)` and `.mt(...)` setting offsets.
It does not consume flex space, add a gap, or enlarge the parent's intrinsic size.
Its wrapping constraints follow the parent's allocated content dimensions.
`.relative()` restores ordinary row, column, or overlay participation, including
when applied by a reactive style patch. Absolute positioning is relative to the
immediate parent; it does not implicitly create a window-level portal or change
paint order. Use `.translate(...)` for a compositor-only positional change when
layout participation should stay the same.


Focused editors reuse bounded shaped geometry for unchanged displayed text and
metrics. Selection, scrolling and navigation do not reshape that text; font,
line-height and wrapped-width changes invalidate the reuse entry. See
[editor layout reuse](performance/editor-shaping/README.md) for admission limits
and measured call counts. This does not change component ownership or the style
inheritance rules above.
