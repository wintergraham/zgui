-- Two of ask.pen's shaders, ported from GLSL (designs/shaders/*.glsl) to
-- zgui's WGSL fills. Each takes its uniforms as four vec4 params, so the
-- Appearance step's tint, intensity and grain reach every one of them.
local K = require("./kit")
local S = require("./state")
local D = require("./data")
local M = {}

local COMMON = [[
fn ob_bayer2(a: vec2<f32>) -> f32 {
    let b = floor(a);
    return fract(dot(b, vec2<f32>(0.5, b.y * 0.75)));
}
fn ob_bayer4(a: vec2<f32>) -> f32 { return ob_bayer2(0.5 * a) * 0.25 + ob_bayer2(a); }
fn ob_bayer8(a: vec2<f32>) -> f32 { return ob_bayer4(0.5 * a) * 0.25 + ob_bayer2(a); }
fn ob_hash(q: vec2<f32>) -> f32 {
    var p = fract(q * vec2<f32>(123.34, 456.21));
    p = p + dot(p, p + 45.32);
    return fract(p.x * p.y);
}
fn ob_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(ob_hash(i), ob_hash(i + vec2<f32>(1.0, 0.0)), u.x),
               mix(ob_hash(i + vec2<f32>(0.0, 1.0)), ob_hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}
fn ob_fbm(q: vec2<f32>) -> f32 {
    var p = q;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < 5; i = i + 1) {
        v = v + a * ob_noise(p);
        p = p * 2.03 + vec2<f32>(1.7, 9.2);
        a = a * 0.5;
    }
    return v;
}
// GLSL's smoothstep, also when the edges run backwards.
fn ob_ss(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
fn ob_pow(x: f32, y: f32) -> f32 { return pow(max(x, 0.000001), y); }
]]

-- bayer-dither.glsl. params: ink; mode, angle, cx, cy; radius, from, to,
-- curve; noise, scale, seed, pixel.
M.DITHER_WGSL = COMMON .. [[
fn fill(input: FillInput) -> vec4<f32> {
    let ink = input.params[0];
    let a = input.params[1];
    let b = input.params[2];
    let c = input.params[3];
    let cell = max(0.5, c.w);
    let size = max(input.size, vec2<f32>(1.0));
    let id = floor(input.p / cell);
    let uv = (id + 0.5) * cell / size;
    let aspect = size.x / size.y;
    let mode = a.x;
    let ang = radians(a.y);
    let centre = a.zw;
    var v = 0.0;
    if mode < 0.5 {
        let dir = vec2<f32>(sin(ang), cos(ang));
        let t = clamp(dot(uv - 0.5, dir) + 0.5, 0.0, 1.0);
        v = mix(b.y, b.z, ob_pow(t, b.w));
    } else if mode < 1.5 {
        let d = length((uv - centre) * vec2<f32>(aspect, 1.0)) / b.x;
        v = mix(b.y, b.z, ob_pow(clamp(d, 0.0, 1.0), b.w));
    } else if mode < 2.5 {
        let p = (uv - centre) * vec2<f32>(aspect, 1.0) / b.x;
        let r2 = dot(p, p);
        if r2 < 1.0 {
            let n = vec3<f32>(p, sqrt(1.0 - r2));
            let l = normalize(vec3<f32>(cos(ang), sin(ang), 0.9));
            v = mix(0.03, b.z, ob_pow(max(dot(n, l), 0.0), b.w));
        } else {
            v = b.y;
        }
    } else {
        let n = ob_fbm(uv * vec2<f32>(aspect, 1.0) * c.y + c.z);
        v = mix(b.y, b.z, ob_pow(ob_ss(0.25, 0.75, n), b.w));
    }
    if c.x > 0.0 && mode < 2.5 {
        v = v + (ob_fbm(uv * vec2<f32>(aspect, 1.0) * c.y + c.z) - 0.5) * c.x * ob_ss(0.0, 0.25, v);
    }
    let on = step(ob_bayer8(id) + 0.5 / 64.0, clamp(v, 0.0, 1.0));
    return vec4<f32>(ink.rgb, ink.a * on);
}
]]

-- splash-space.glsl, for the window's 880 × 580: a lit planet on the
-- horizon, stars, a ringed planet, a moon and a shooting star. With
-- `punch`, it is the scene only where the "ask." wordmark's own dither is
-- off, so the solid wordmark under it shows through as dots.
-- params: offset x, offset y, punch, text top; pixel, tint, stars, text cell.
M.SPACE_WGSL = COMMON .. [[
const OB_INK = vec3<f32>(0.929, 0.922, 0.898);
const OB_PAPER = vec3<f32>(0.039, 0.039, 0.035);
const OB_TA = vec3<f32>(0.373, 0.416, 0.639);
const OB_TB = vec3<f32>(0.765, 0.808, 0.698);
const OB_TC = vec3<f32>(0.851, 0.635, 0.231);
const OB_TD = vec3<f32>(0.910, 0.659, 0.596);
const OB_W = 880.0;
const OB_H = 580.0;
const OB_PLANET = 860.0;
const OB_HORIZON = 360.0;
fn ob_tint(t: vec3<f32>, k: f32) -> vec3<f32> { return mix(OB_INK, t, clamp(k, 0.0, 1.0)); }
fn ob_sphere(p: vec2<f32>, c: vec2<f32>, r: f32, light: vec2<f32>) -> f32 {
    let q = (p - c) / r;
    let r2 = dot(q, q);
    if r2 >= 1.0 { return -1.0; }
    let n = vec3<f32>(q, sqrt(1.0 - r2));
    return max(dot(n, normalize(vec3<f32>(light, 0.9))), 0.0);
}
// The brightest layer wins: xyz its colour, w its value.
fn ob_layer(acc: vec4<f32>, lv: f32, lc: vec3<f32>) -> vec4<f32> {
    if lv > acc.w { return vec4<f32>(lc, lv); }
    return acc;
}
fn fill(input: FillInput) -> vec4<f32> {
    let o = input.params[0];
    let s = input.params[1];
    let cell = max(1.0, s.x);
    let tint = s.y;
    let id = floor((input.p + o.xy) / cell);
    let p = (id + 0.5) * cell;
    let T = floor(input.time * 12.0) / 12.0;
    var acc = vec4<f32>(OB_INK, 0.0);

    // The main planet.
    let pc = vec2<f32>(OB_W * 0.5, OB_HORIZON + OB_PLANET);
    let rim = length(p - pc) - OB_PLANET;
    let top = vec2<f32>(OB_W * 0.5, OB_HORIZON);
    let breathe = 0.5 + 0.5 * sin(T * 0.6);
    if rim < 0.0 {
        let lit = ob_pow(1.0 - clamp(length(p - top) / (OB_PLANET * 0.72), 0.0, 1.0), 1.1);
        let q = (p - pc) / OB_PLANET;
        let z = sqrt(max(1.0 - dot(q, q), 0.0));
        let sp = vec2<f32>(q.x / (z + 0.35) + T * 0.015, q.y * 3.0);
        let bands = ob_fbm(sp * vec2<f32>(4.0, 6.0));
        let clouds = ob_fbm(sp * 9.0 + vec2<f32>(T * 0.02, 0.0));
        var pv = lit + (bands - 0.5) * 0.35 * ob_ss(0.0, 0.25, lit) + (clouds - 0.5) * 0.18;
        pv = pv * (ob_ss(0.0, 18.0, -rim) * 0.25 + 0.75);
        var sc = ob_tint(OB_TB, tint * ob_ss(0.45, 0.75, bands) * 1.6);
        sc = mix(sc, ob_tint(OB_TA, tint * 1.2), ob_ss(30.0, 0.0, -rim) * 0.7);
        acc = ob_layer(acc, pv, sc);
        if -rim < 2.5 { acc = ob_layer(acc, 0.28 + 0.5 * lit, ob_tint(OB_TA, tint * 1.5)); }
    } else {
        var glow = exp(-rim / (5.0 + 3.0 * breathe)) * (0.16 + 0.06 * breathe);
        let spread = 1.0 - clamp(abs(p.x - OB_W * 0.5) / (OB_W * 0.65), 0.0, 1.0);
        glow = glow * (0.35 + 0.65 * spread);
        acc = ob_layer(acc, glow, ob_tint(OB_TA, tint * 2.2));
    }

    // Stars, kept off the tagline's row.
    let tagline = abs(p.x - OB_W * 0.5) < 200.0 && abs(p.y - 315.0) < 16.0;
    if rim > 4.0 && !tagline {
        for (var k = 0; k < 2; k = k + 1) {
            let fk = f32(k);
            let g = select(23.0, 9.0, fk < 0.5);
            let gid = floor(p / g);
            let h = ob_hash(gid + fk * 31.7);
            if h > 1.0 - s.z * select(0.11, 0.055, fk < 0.5) {
                let sp = (gid + vec2<f32>(ob_hash(gid + 3.1), ob_hash(gid + 8.7))) * g;
                let size = select(1.6, 1.0, fk < 0.5);
                if length(p - sp) < size * cell * 0.5 {
                    let tw = 0.5 + 0.5 * sin(T * (0.8 + h * 2.4) + h * 40.0);
                    var sv = select(0.55, 0.18, fk < 0.5) * (0.45 + 0.55 * tw);
                    sv = sv * ob_ss(0.0, 70.0, rim);
                    let pick = ob_hash(gid + 17.0);
                    var sc = OB_INK;
                    if pick < 0.15 { sc = ob_tint(OB_TA, 0.45); }
                    else if pick < 0.25 { sc = ob_tint(OB_TC, 0.35); }
                    else if pick < 0.32 { sc = ob_tint(OB_TD, 0.35); }
                    acc = ob_layer(acc, sv, sc);
                }
            }
        }
    }

    // The ringed planet, bobbing.
    let rc = vec2<f32>(OB_W * 0.165 + sin(T * 0.07) * 6.0, OB_H * 0.165 + cos(T * 0.09) * 3.0);
    let rp = p - rc;
    let ca = cos(0.21);
    let sa = sin(0.21);
    let rq = vec2<f32>(ca * rp.x - sa * rp.y, sa * rp.x + ca * rp.y);
    let ring_e = length(rq / vec2<f32>(76.0, 16.0));
    let ring = ob_ss(0.80, 0.86, ring_e) * (1.0 - ob_ss(0.96, 1.0, ring_e));
    let sl = ob_sphere(p, rc, 33.0, vec2<f32>(0.7, -0.7));
    let front = rq.y > 0.0;
    if sl >= 0.0 && !(front && ring > 0.0) {
        let bands = ob_fbm(vec2<f32>(rq.x * 0.05 + T * 0.03, rq.y * 0.25));
        acc = ob_layer(acc, (0.14 + 0.7 * ob_pow(sl, 1.3)) * (0.8 + 0.4 * bands), ob_tint(OB_TC, tint * 1.1));
    } else if ring > 0.0 {
        let shimmer = 0.5 + 0.5 * sin(atan2(rq.y, rq.x) * 6.0 - T * 0.9);
        acc = ob_layer(acc, ring * (0.22 + 0.16 * shimmer), ob_tint(OB_TC, tint * 1.2));
    }

    // The moon, and a far one.
    let ma = T * 0.05;
    let mc = vec2<f32>(OB_W * 0.85 + cos(ma) * 26.0, OB_H * 0.24 + sin(ma) * 10.0);
    let ml = ob_sphere(p, mc, 11.0, vec2<f32>(-0.8, -0.4));
    if ml >= 0.0 { acc = ob_layer(acc, 0.05 + 0.75 * ob_pow(ml, 1.2), ob_tint(OB_TD, tint * 1.1)); }
    if length(p - vec2<f32>(OB_W * 0.73 + sin(T * 0.11) * 4.0, OB_H * 0.08)) < 3.5 { acc = ob_layer(acc, 0.32, OB_INK); }

    // A shooting star every nine seconds.
    let cyc = floor(T / 9.0);
    let ph = fract(T / 9.0);
    if ph < 0.12 {
        let kk = ph / 0.12;
        let a0 = vec2<f32>(OB_W * (0.55 + 0.3 * ob_hash(vec2<f32>(cyc, 1.0))), OB_H * (0.02 + 0.12 * ob_hash(vec2<f32>(cyc, 2.0))));
        let dir = normalize(vec2<f32>(-1.0, 0.36));
        let rel = p - (a0 + dir * kk * 220.0);
        let along = dot(rel, -dir);
        let across = abs(dot(rel, vec2<f32>(-dir.y, dir.x)));
        if along > 0.0 && along < 110.0 && across < 1.6 {
            acc = ob_layer(acc, (1.0 - along / 110.0) * sin(kk * 3.14159) * 0.95, ob_tint(OB_TC, 0.45));
        }
    }

    let on = step(ob_bayer8(id) + 0.5 / 64.0, clamp(acc.w, 0.0, 1.0));
    let scene = mix(OB_PAPER, acc.xyz, on);
    if o.z < 0.5 { return vec4<f32>(scene, 1.0); }
    // The wordmark's dither: dense at its top, thinning toward its foot.
    let tc = max(1.0, s.w);
    let tid = floor(input.p / tc);
    let ty = clamp(((tid.y + 0.5) * tc - o.w) / 210.0, 0.0, 1.0);
    let text_on = step(ob_bayer8(tid) + 0.5 / 64.0, mix(1.0, 0.12, ob_pow(ty, 1.1)));
    return vec4<f32>(scene, 1.0 - text_on);
}
]]

M.DITHER = ui.shader(M.DITHER_WGSL)
M.SPACE = ui.shader(M.SPACE_WGSL, { fps = 12 })

----------------------------------------------------------------------------
-- What the Appearance step changes, applied to the shaders' numbers
----------------------------------------------------------------------------

local function rgba(hex: string): { number }
	local h = hex:gsub("#", "")
	local out = {}
	for k = 0, 3 do
		out[k + 1] = (tonumber(h:sub(k * 2 + 1, k * 2 + 2), 16) or 255) / 255
	end
	return out
end
local STONE = rgba("#858379")
local tint_of = {}
for _, t in D.tints do
	tint_of[t.id] = rgba(t.color)
end

-- An ink in the chosen tint: each channel scaled by how far the swatch
-- leans from Stone, so the texture keeps its brightness and takes the hue.
function M.ink(dark: string, light: string?): { number }
	local base = rgba(if K.light() and light then light else dark)
	local t = tint_of[S.tint:get()] or STONE
	local out = {}
	for k = 1, 3 do
		local lean = 1 + (t[k] / STONE[k] - 1) * 2.2
		out[k] = math.clamp(base[k] * lean, 0, 1)
	end
	out[4] = base[4]
	return out
end

local DENSITY = { Off = 0, Subtle = 0.6, Default = 1, Rich = 1.4 }
local GRAIN = { Fine = 0.75, Default = 1, Coarse = 1.5 }
-- How dense textures are drawn, 0 when they are off.
function M.density(): number
	return DENSITY[S.intensity:get()] or 1
end
function M.pixel(base: number): number
	return base * (GRAIN[S.grain:get()] or 1)
end

export type Dither = {
	ink: string,
	light: string?, -- the ink in the light theme
	mode: number,
	angle: number?,
	cx: number?,
	cy: number?,
	radius: number?,
	from: number?,
	to: number?,
	curve: number?,
	noise: number?,
	scale: number?,
	seed: number?,
	pixel: number?,
	fixed: boolean?, -- a logo: neither intensity nor grain changes it
}
-- bayer-dither's uniforms, as a fill's four params (tracked).
function M.dither(u: Dither)
	local k = if u.fixed then 1 else M.density()
	return {
		M.ink(u.ink, u.light),
		{ u.mode, u.angle or 0, u.cx or 0.5, u.cy or 0.5 },
		{ u.radius or 0.5, (u.from or 0) * k, (u.to or 1) * k, u.curve or 1 },
		{ u.noise or 0, u.scale or 4, u.seed or 0, if u.fixed then u.pixel or 2 else M.pixel(u.pixel or 2) },
	}
end

-- A shader-filled node's props whose `shader_params` is a tracked function:
-- the numbers are set once, and every change after is written to the
-- node's `shaderParam0` to `shaderParam15`. Call it while building, so the
-- effect goes when the node's part does.
function M.live(props: { [any]: any })
	local params = props.shader_params
	local ref = ui.ref()
	props.ref = ref
	props.shader_params = params()
	ui.effect(function()
		local values, list = {}, params()
		local el = ref:get()
		if not el then
			return
		end
		for i, v in list do
			for k = 1, 4 do
				values["shaderParam" .. ((i - 1) * 4 + k - 1)] = v[k] or 0
			end
		end
		anime.utils.set(el, values)
	end)
	return props
end

----------------------------------------------------------------------------
-- Time: drifting fills run at the board's 12 fps until Reduce motion
----------------------------------------------------------------------------

local live = {} :: { { el: any, anim: any } }
-- Starts a fill's clock once its node mounts.
function M.drift(el)
	local anim = anime.animate(el, {
		shaderTime = { 0, 3600 },
		duration = 3600000,
		ease = "linear",
		loop = true,
		frameRate = 12,
		autoplay = not S.reduce_motion:peek(),
	})
	table.insert(live, { el = el, anim = anim })
end
-- Keeps any looping animation of `el` under Reduce motion and the sweep
-- (a spinner's turn).
function M.keep(el, anim)
	if S.reduce_motion:peek() then
		anim:pause()
	end
	table.insert(live, { el = el, anim = anim })
end
-- Lets go of the clocks of fills that have left.
function M.sweep()
	local kept = {}
	for _, entry in live do
		if entry.el.mounted then
			table.insert(kept, entry)
		else
			entry.anim:cancel()
		end
	end
	live = kept
end
ui.effect(function()
	local still = S.reduce_motion:get()
	for _, entry in live do
		if still then
			entry.anim:pause()
		else
			entry.anim:play()
		end
	end
end)

return M
