-- Ask's Bayer dither (src/design/dither.js, a port of bayer-dither.glsl) as
-- zgui WGSL fills. Cells are `pixel` points square, counted from the box's
-- top left, so a fill dithers in the same grid the canvas did.
local common = [=[
fn d_bayer2(a0: vec2<f32>) -> f32 {
    let a = floor(a0);
    return fract(a.x * 0.5 + a.y * a.y * 0.75);
}
fn d_bayer4(a: vec2<f32>) -> f32 { return d_bayer2(0.5 * a) * 0.25 + d_bayer2(a); }
fn d_bayer8(a: vec2<f32>) -> f32 { return d_bayer4(0.5 * a) * 0.25 + d_bayer2(a); }
fn d_hash(p0: vec2<f32>) -> f32 {
    var x = fract(p0.x * 123.34);
    var y = fract(p0.y * 456.21);
    let d = x * (x + 45.32) + y * (y + 45.32);
    x += d;
    y += d;
    return fract(x * y);
}
fn d_vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = d_hash(i);
    let b = d_hash(i + vec2<f32>(1.0, 0.0));
    let c = d_hash(i + vec2<f32>(0.0, 1.0));
    let d = d_hash(i + vec2<f32>(1.0, 1.0));
    return (a + (b - a) * u.x) + ((c + (d - c) * u.x) - (a + (b - a) * u.x)) * u.y;
}
fn d_fbm(p0: vec2<f32>) -> f32 {
    var p = p0;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < 5; i++) {
        v += a * d_vnoise(p);
        p = p * 2.03 + vec2<f32>(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}
fn d_smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
// The density at a cell, as dither.js computes it. `seed` already carries
// the clock; `light` is the sphere's light angle in degrees.
fn d_density(input: FillInput, mode: f32, pixel: f32, angle: f32, light: f32, c: vec2<f32>, radius: f32,
        v0: f32, v1: f32, curve: f32, noise: f32, scale: f32, seed: f32, fade: f32) -> vec2<f32> {
    let cell = max(pixel, 0.25);
    let id = floor(input.p / cell);
    let size = max(input.size, vec2<f32>(1.0));
    let uv = (id + 0.5) * cell / size;
    let aspect = size.x / size.y;
    var v = 0.0;
    if mode < 0.5 {
        let a = radians(angle);
        let t = clamp((uv.x - 0.5) * sin(a) + (uv.y - 0.5) * cos(a) + 0.5, 0.0, 1.0);
        v = v0 + (v1 - v0) * pow(t, curve);
    } else if mode < 1.5 {
        let d = length(vec2<f32>((uv.x - c.x) * aspect, uv.y - c.y)) / radius;
        v = v0 + (v1 - v0) * pow(min(1.0, d), curve);
    } else if mode < 2.5 {
        let p = vec2<f32>((uv.x - c.x) * aspect, uv.y - c.y) / radius;
        let r2 = dot(p, p);
        if r2 < 1.0 {
            let a = radians(light);
            let l = vec3<f32>(cos(a), sin(a), 0.9);
            let lam = max(0.0, dot(vec3<f32>(p, sqrt(1.0 - r2)), l) / length(l));
            v = 0.03 + (v1 - 0.03) * pow(lam, curve);
        } else {
            v = v0;
        }
    } else {
        let n = d_fbm(vec2<f32>(uv.x * aspect * scale + seed, uv.y * scale + seed));
        v = v0 + (v1 - v0) * pow(d_smooth(0.25, 0.75, n), curve);
    }
    if noise > 0.0 && mode < 2.5 {
        v += (d_fbm(vec2<f32>(uv.x * aspect * scale + seed, uv.y * scale + seed)) - 0.5) * noise * d_smooth(0.0, 0.25, v);
    }
    if fade > 0.0 { v *= 1.0 - d_smooth(1.0 - fade, 1.0, uv.y); }
    return vec2<f32>(clamp(v, 0.0, 1.0), d_bayer8(id) + 0.5 / 64.0);
}
]=]

local M = {}

-- params: ink; (mode, pixel, angle, cx); (cy, radius, from, to);
-- (curve, noise or, in noise mode, fade, scale, seed). `time` adds to the
-- seed: a surface's clock animates shaderTime in seed units.
M.bayer = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let a1 = input.params[1];
    let a2 = input.params[2];
    let a3 = input.params[3];
    let noise_mode = a1.x > 2.5;
    let r = d_density(input, a1.x, a1.y, a1.z, a1.z, vec2<f32>(a1.w, a2.x), a2.y, a2.z, a2.w,
        a3.x, select(a3.y, 0.0, noise_mode), a3.z, a3.w + input.time, select(0.0, a3.y, noise_mode));
    let ink = input.params[0];
    return vec4<f32>(ink.rgb, ink.a * step(r.y, r.x));
}
]=], { fps = 12 })

-- The orb: a lit sphere. Active orbs drift their light 225° to 250° on a
-- six-second sine and churn their noise with `time` in seconds.
-- params: ink; (pixel, to, curve, noise); (drift, scale, seed, 0).
M.orb = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let a1 = input.params[1];
    let a2 = input.params[2];
    let t = input.time;
    let light = 225.0 + a2.x * 25.0 * (0.5 - 0.5 * cos(6.2831853 * t / 6.0));
    let r = d_density(input, 2.0, a1.x, 0.0, light, vec2<f32>(0.5, 0.5), 0.5, 0.0, a1.y,
        a1.z, a1.w, a2.y, a2.z + t * 0.7 * a2.x, 0.0);
    let ink = input.params[0];
    return vec4<f32>(ink.rgb, ink.a * step(r.y, r.x));
}
]=], { fps = 12 })

return M
