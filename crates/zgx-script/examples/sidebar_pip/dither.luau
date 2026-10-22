-- The board's GLSL fills, ported to zgui's WGSL fill functions. Cells are
-- counted in device pixels from the box's top left, as the originals count
-- them from gl_FragCoord with `u_width` keeping the size per point.
local common = [=[
fn d_bayer2(a0: vec2<f32>) -> f32 {
    let a = floor(a0);
    return fract(dot(a, vec2<f32>(0.5, a.y * 0.75)));
}
fn d_bayer4(a: vec2<f32>) -> f32 { return d_bayer2(0.5 * a) * 0.25 + d_bayer2(a); }
fn d_bayer8(a: vec2<f32>) -> f32 { return d_bayer4(0.5 * a) * 0.25 + d_bayer2(a); }
fn d_hash(p0: vec2<f32>) -> f32 {
    var p = fract(p0 * vec2<f32>(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}
fn d_vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(d_hash(i), d_hash(i + vec2<f32>(1.0, 0.0)), u.x),
               mix(d_hash(i + vec2<f32>(0.0, 1.0)), d_hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}
fn d_fbm(p0: vec2<f32>, octaves: i32) -> f32 {
    var p = p0;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < octaves; i++) {
        v += a * d_vnoise(p);
        p = p * 2.03 + vec2<f32>(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}
// The cell grid: `pixel` points per cell, at least one device pixel.
struct Cells { id: vec2<f32>, uv: vec2<f32>, aspect: f32 }
fn d_cells(input: FillInput, pixel: f32) -> Cells {
    let res = max(input.size * input.scale, vec2<f32>(1.0));
    let cell = max(1.0, floor(pixel * input.scale + 0.5));
    let id = floor(input.p * input.scale / cell);
    return Cells(id, (id + 0.5) * cell / res, res.x / res.y);
}
]=]

local M = {}

-- bayer-dither.glsl. params: ink; (mode, pixel, angle, cx); (cy, radius,
-- from, to); (curve, noise, scale, seed). Time drifts the noise and swings
-- the sphere's light, so an orb can breathe; at time 0 it is the board's.
M.bayer = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let ink = input.params[0];
    let a1 = input.params[1];
    let a2 = input.params[2];
    let a3 = input.params[3];
    let g = d_cells(input, a1.y);
    let c = vec2<f32>(a1.w, a2.x);
    let ang = radians(a1.z);
    let t = input.time;
    let drift = vec2<f32>(t * 0.35, -t * 0.2);
    var v = 0.0;
    if a1.x < 0.5 {
        let dir = vec2<f32>(sin(ang), cos(ang));
        let k = clamp(dot(g.uv - 0.5, dir) + 0.5, 0.0, 1.0);
        v = mix(a2.z, a2.w, pow(k, a3.x));
    } else if a1.x < 1.5 {
        let r = a2.y * (1.0 + 0.08 * sin(t * 2.4));
        let d = length((g.uv - c) * vec2<f32>(g.aspect, 1.0)) / r;
        v = mix(a2.z, a2.w, pow(clamp(d, 0.0, 1.0), a3.x));
    } else if a1.x < 2.5 {
        let p = (g.uv - c) * vec2<f32>(g.aspect, 1.0) / a2.y;
        let r2 = dot(p, p);
        if r2 < 1.0 {
            let n = vec3<f32>(p, sqrt(1.0 - r2));
            let la = ang + 0.5 * sin(t * 1.1);
            let l = normalize(vec3<f32>(cos(la), sin(la), 0.9));
            v = mix(0.03, a2.w, pow(max(dot(n, l), 0.0), a3.x));
        } else {
            v = a2.z;
        }
    } else {
        let n = d_fbm(g.uv * vec2<f32>(g.aspect, 1.0) * a3.z + a3.w + drift, 5);
        v = mix(a2.z, a2.w, pow(smoothstep(0.25, 0.75, n), a3.x));
    }
    if a3.y > 0.0 && a1.x < 2.5 {
        v += (d_fbm(g.uv * vec2<f32>(g.aspect, 1.0) * a3.z + a3.w + drift, 5) - 0.5) * a3.y * smoothstep(0.0, 0.25, v);
    }
    let on = step(d_bayer8(g.id) + 0.5 / 64.0, clamp(v, 0.0, 1.0));
    return vec4<f32>(ink.rgb, ink.a * on);
}
]=], { fps = 12 })

-- dither-fog-sweep.glsl, as the active pin uses it (smooth blend, no warp).
-- params: (c1, saturation); (c2, opacity); (c3, density); (angle, noise
-- scale, seed, pixel).
M.fog = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let a3 = input.params[3];
    let g = d_cells(input, a3.w);
    let q = g.uv * vec2<f32>(g.aspect, 1.0);
    let seed = a3.z + input.time * 0.15;
    let v = smoothstep(0.3, 0.75, d_fbm(q * a3.y + seed, 5)) * input.params[2].a;
    let ang = radians(a3.x);
    let k = clamp(dot(g.uv - 0.5, vec2<f32>(sin(ang), cos(ang))) + 0.5, 0.0, 1.0);
    let seg = k * 2.0;
    let pick = smoothstep(0.0, 1.0, fract(seg));
    var col = mix(input.params[1].rgb, input.params[2].rgb, pick);
    if seg < 1.0 { col = mix(input.params[0].rgb, input.params[1].rgb, pick); }
    let l = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(l), col, input.params[0].a);
    let on = step(d_bayer8(g.id) + 0.5 / 64.0, v);
    return vec4<f32>(col, on * input.params[1].a);
}
]=], { fps = 12 })

-- tab-loading-bg.v1.glsl: the row's dither, with a band sweeping away from
-- the favicon while it loads, dissolving as the favicon resolves.
-- params: ink; (progress, pixel, from, seed). Time is in seconds.
M.tab_loading = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let a1 = input.params[1];
    let g = d_cells(input, a1.y);
    let p = a1.x;
    let t = input.time;
    var v = mix(a1.z, 0.0, pow(g.uv.x, 0.9));
    v += (d_fbm(g.uv * vec2<f32>(g.aspect, 1.0) * 3.0 + a1.w + vec2<f32>(t * 0.6, 0.0), 4) - 0.5) * 0.2 * smoothstep(0.0, 0.25, v);
    let head = fract(t / 1.2) * 1.5 - 0.25;
    v += exp(-pow((g.uv.x - head) * 5.0, 2.0)) * 0.28 * (1.0 - smoothstep(0.25, 0.4, p));
    let r = clamp((p - 0.35) / 0.5, 0.0, 1.0);
    v *= 1.0 - smoothstep(0.0, 0.8, r);
    let on = step(d_bayer8(g.id) + 0.5 / 64.0, clamp(v, 0.0, 1.0));
    let ink = input.params[0];
    return vec4<f32>(ink.rgb, ink.a * on);
}
]=], { fps = 30 })

-- The loading half of favicon-resolve.v2.glsl: the wobbling dithered blob.
-- The favicon itself focuses over it as an image (blur, scale, saturation),
-- since a fill cannot sample one. params: ink; (progress, seed, 0, 0).
M.favicon_blob = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let p = input.params[1].x;
    let seed = input.params[1].y;
    let t = input.time;
    let uv = input.p / max(input.size, vec2<f32>(1.0));
    let did = floor(input.p * input.scale);
    let c = uv - 0.5;
    let ang = atan2(c.y, c.x);
    let rad = 0.37
        + 0.045 * sin(3.0 * ang + t * 1.3 + seed)
        + 0.03 * sin(5.0 * ang - t * 0.9 + seed * 0.5)
        + 0.06 * (d_fbm(vec2<f32>(cos(ang), sin(ang)) * 1.4 + vec2<f32>(seed, t * 0.25), 4) - 0.5);
    let blob = 1.0 - smoothstep(rad - 0.14, rad + 0.02, length(c));
    let n = d_fbm(uv * 2.6 + vec2<f32>(seed, seed * 0.37) + vec2<f32>(t * 0.35, -t * 0.22), 4);
    let r = clamp((p - 0.35) / 0.5, 0.0, 1.0);
    let v = blob * mix(0.45, 0.95, smoothstep(0.25, 0.75, n)) * (1.0 - smoothstep(0.0, 0.55, r));
    let on = step(d_bayer8(did) + 0.5 / 64.0, clamp(v, 0.0, 1.0));
    let ink = input.params[0];
    return vec4<f32>(ink.rgb, ink.a * on);
}
]=], { fps = 30 })

return M
