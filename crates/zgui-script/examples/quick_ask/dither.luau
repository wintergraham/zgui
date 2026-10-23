-- Static shader, driven only by finite native animations while a demo is active.
-- shortcut: simplified noise; port the full source FBM when exact pixel matching is required.
return ui.shader([=[
fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn bayer2(p: vec2<f32>) -> f32 {
    let q = floor(p);
    return fract(q.x * 0.5 + q.y * q.y * 0.75);
}
fn bayer8(p: vec2<f32>) -> f32 {
    return (bayer2(p * 0.25) * 0.25 + bayer2(p * 0.5)) * 0.25 + bayer2(p);
}
fn fill(input: FillInput) -> vec4<f32> {
    let settings = input.params[1];
    let cell = max(1.0, settings.x);
    let p = floor(input.p / cell);
    let uv = input.p / max(input.size, vec2<f32>(1.0));
    let mode = settings.y;
    var density = 0.0;
    if mode < 0.5 {
        // The original lit, Bayer-dithered model orb.
        let q = (uv - 0.5) * 2.0;
        let r2 = dot(q, q);
        if r2 > 1.0 { return vec4<f32>(0.0); }
        let angle = 3.927 + 0.436 * (0.5 - 0.5 * cos(input.time * 1.047));
        density = pow(max(0.0, dot(vec3<f32>(q, sqrt(1.0 - r2)),
            normalize(vec3<f32>(cos(angle), sin(angle), 0.9)))), 1.1) * settings.z;
    } else if mode < 1.5 {
        density = (0.10 + uv.x * 0.28 + hash2(p * 0.035 + input.time) * 0.28) * settings.z;
    } else if mode < 2.5 {
        density = max(0.0, 0.48 - distance(uv, vec2<f32>(1.0, 0.0)) * 0.65);
    } else if mode < 3.5 {
        density = select(0.0, 0.6, uv.y > 0.91 && uv.x < settings.z);
    } else if mode < 4.5 {
        density = max(0.0, 0.6 - distance(uv * vec2<f32>(2.0, 1.0), vec2<f32>(0.15, 0.5)));
    } else {
        density = 0.15 + uv.x * 0.75;
    }
    let threshold = bayer8(p) + 0.5 / 64.0;
    var alpha = select(0.0, 1.0, density > threshold);
    if settings.w > 0.0 { alpha = clamp((density - threshold) / settings.w + 0.5, 0.0, 1.0); }
    return vec4<f32>(input.params[0].rgb, input.params[0].a * alpha);
}
]=])
