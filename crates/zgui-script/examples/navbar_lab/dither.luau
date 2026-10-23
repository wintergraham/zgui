-- Ask's Bayer dither (experiments/react/src/design/dither.js), run per pixel on the GPU. Each cell of `pixel`
-- points takes one Bayer threshold, as the source's canvas pixels do when stretched over the element.
-- Parameters (four vec4s):
--   [0] ink (rgba)
--   [1] mode, pixel, angle (degrees), from
--   [2] to, curve, noise, scale
--   [3] seed, soft, radius (mode 2) or band (mode 5), seconds-to-seed speed
-- Modes: 0 linear ramp, 2 lit orb, 3 noise blotches, 5 the agent's edge aura. `input.time` adds
-- `time * speed` to the seed, so an animation of shaderTime drifts the noise on the compositor.
local common = [=[
fn d_bayer2(x: f32, y: f32) -> f32 {
    let fx = floor(x);
    let fy = floor(y);
    return fract(fx * 0.5 + fy * fy * 0.75);
}
fn d_bayer4(x: f32, y: f32) -> f32 { return d_bayer2(0.5 * x, 0.5 * y) * 0.25 + d_bayer2(x, y); }
fn d_bayer8(x: f32, y: f32) -> f32 { return d_bayer4(0.5 * x, 0.5 * y) * 0.25 + d_bayer2(x, y); }
fn d_hash(px: f32, py: f32) -> f32 {
    var x = fract(px * 123.34);
    var y = fract(py * 456.21);
    let d = x * (x + 45.32) + y * (y + 45.32);
    x += d;
    y += d;
    return fract(x * y);
}
fn d_vnoise(x: f32, y: f32) -> f32 {
    let ix = floor(x);
    let iy = floor(y);
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = d_hash(ix, iy);
    let b = d_hash(ix + 1.0, iy);
    let c = d_hash(ix, iy + 1.0);
    let d = d_hash(ix + 1.0, iy + 1.0);
    return (a + (b - a) * ux) + ((c + (d - c) * ux) - (a + (b - a) * ux)) * uy;
}
fn d_fbm(px: f32, py: f32) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var x = px;
    var y = py;
    for (var i = 0; i < 5; i++) {
        v += a * d_vnoise(x, y);
        x = x * 2.03 + 1.7;
        y = y * 2.03 + 9.2;
        a *= 0.5;
    }
    return v;
}
fn d_smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
// dither.js's density for one cell, given the cell's centre (ux, uy) in 0..1 of the box.
fn d_density(input: FillInput, ux: f32, uy: f32) -> f32 {
    let w = input.size.x;
    let h = max(input.size.y, 1.0);
    let aspect = w / h;
    let mode = input.params[1].x;
    let a = radians(input.params[1].z);
    let lo = input.params[1].w;
    let hi = input.params[2].x;
    let curve = input.params[2].y;
    let noise = input.params[2].z;
    let scale = input.params[2].w;
    let seed = input.params[3].x + input.time * input.params[3].w;
    var v = 0.0;
    if mode < 0.5 {
        let t = clamp((ux - 0.5) * sin(a) + (uy - 0.5) * cos(a) + 0.5, 0.0, 1.0);
        v = lo + (hi - lo) * pow(t, curve);
    } else if mode < 2.5 {
        let radius = select(0.5, input.params[3].z, input.params[3].z > 0.0);
        let px = (ux - 0.5) * aspect / radius;
        let py = (uy - 0.5) / radius;
        let r2 = px * px + py * py;
        if r2 < 1.0 {
            let l = vec3<f32>(cos(a), sin(a), 0.9);
            let lam = max(0.0, dot(vec3<f32>(px, py, sqrt(1.0 - r2)), l) / length(l));
            v = 0.03 + (hi - 0.03) * pow(lam, curve);
        } else {
            v = lo;
        }
    } else if mode > 4.5 {
        // Frame: density by a soft distance to the edges, its depth following slow drifting noise.
        let band = input.params[3].z;
        let ex = ux * w;
        let ey = uy * h;
        let k = 18.0;
        let e = -k * log(exp(-ex / k) + exp(-(w - ex) / k) + exp(-ey / k) + exp(-(h - ey) / k));
        if e >= band * 1.8 { return -1.0; }
        let lx = ux * aspect * 1.3 - seed * 0.35;
        let ly = uy * 1.3 + seed * 0.25;
        let reach = 0.2 + 1.6 * d_smooth(0.3, 0.7, d_vnoise(lx, ly) * 0.65 + d_vnoise(lx * 2.1 + 5.2, ly * 2.1 + 1.3) * 0.35);
        let depth = max(0.0, 1.0 - e / (band * reach));
        if depth <= 0.0 { return -1.0; }
        let n = d_fbm(ux * aspect * scale + seed, uy * scale + seed * 0.7);
        v = lo * pow(depth, curve) * d_smooth(0.2, 0.8, n) * 1.6;
    } else {
        let n = d_fbm(ux * aspect * scale + seed, uy * scale + seed);
        v = lo + (hi - lo) * pow(d_smooth(0.25, 0.75, n), curve);
    }
    if noise > 0.0 && (mode < 2.5 || mode > 3.5) {
        v += (d_fbm(ux * aspect * scale + seed, uy * scale + seed) - 0.5) * noise * d_smooth(0.0, 0.25, v);
    }
    return v;
}
// The cell's lit amount: 0 or 1, or a soft ramp around its threshold.
fn d_lit(input: FillInput, cell: f32, soft: f32) -> f32 {
    let i = floor(input.p.x / cell);
    let j = floor(input.p.y / cell);
    let size = max(input.size, vec2<f32>(1.0));
    let v = d_density(input, (i + 0.5) * cell / size.x, (j + 0.5) * cell / size.y);
    if v < 0.0 { return 0.0; }
    let thr = d_bayer8(i, j) + 0.5 / 64.0;
    let vv = clamp(v, 0.0, 1.0);
    if soft > 0.0 { return clamp((vv - thr) / soft + 0.5, 0.0, 1.0); }
    return select(0.0, 1.0, thr <= vv);
}
]=]

local M = {}
M.fill = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    let ink = input.params[0];
    return vec4<f32>(ink.rgb, ink.a * d_lit(input, max(0.5, input.params[1].y), input.params[3].y));
}
]=])

-- The address field's load wash (loading.rs): neutral dots on the linear ramp, the same dots shading toward
-- the logo's tint near the moving edge. The box is the whole field; the visible bar is clipped to the
-- progress, whose right edge is where the tint gathers (navbar.css .nb-wash-tint mask: solid for 24pt,
-- gone by 180pt).
--   [0] neutral ink   [1] tint ink (alpha: the tint layer's opacity)
--   [2] from, to, seed, visible width   [3] unused
M.wash = ui.shader(common .. [=[
fn fill(input: FillInput) -> vec4<f32> {
    var u = input;
    let edge = input.params[2].w;
    u.params[1] = vec4<f32>(0.0, 1.5, 90.0, input.params[2].x);
    u.params[2] = vec4<f32>(input.params[2].y, 0.8, 0.25, 3.0);
    u.params[3] = vec4<f32>(input.params[2].z, 0.0, 0.0, 0.0);
    let a = d_lit(u, 1.5, 0.0);
    let reach = edge - input.p.x;
    let mask = clamp((180.0 - reach) / 156.0, 0.0, 1.0);
    let tint = input.params[1];
    let ink = mix(input.params[0].rgb, tint.rgb, tint.a * mask);
    return vec4<f32>(ink, input.params[0].a * a);
}
]=])

-- Parameter lists for M.fill.
function M.params(ink, u)
    return { ink,
        { u.mode or 0, u.pixel or 2, u.angle or 0, u.from or 0 },
        { u.to or 1, u.curve or 1, u.noise or 0, u.scale or 4 },
        { u.seed or 0, u.soft or 0, u.radius or u.band or 0, u.speed or 0 } }
end
-- dither.js ORB.
M.ORB = { mode = 2, pixel = 1, angle = 225, radius = 0.5, to = 1, curve = 1.1 }
return M
