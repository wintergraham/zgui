// zgx entry points: fs=fs fs_basic=fs_basic fs_basic_both=fs_basic_both fs_both=fs_both vs=vs vs_basic=vs_basic
// language: metal2.4
#include <metal_stdlib>
#include <simd/simd.h>

using metal::uint;

struct _mslBufferSizes {
    uint size2;
};

struct Vertex {
    metal::float4 rect;
    metal::float4 uv;
    metal::float4 color;
    metal::float4 fade;
    metal::float4 options;
    metal::float4 shape;
    metal::float4 border;
    metal::float4 mask;
};
struct Out {
    metal::float4 position;
    metal::float2 uv;
    char _pad2[8];
    metal::float4 color;
    metal::float2 point;
    char _pad4[8];
    metal::float4 fade;
    metal::float4 options;
    metal::float4 shape;
    metal::float4 border;
    metal::float4 mask;
};
struct Region {
    metal::float4 round0_;
    metal::float4 radii0_;
    metal::float4 round1_;
    metal::float4 radii1_;
    metal::float4 fade_box;
    metal::float4 fade;
    metal::float4 color0_;
    metal::float4 color1_;
    metal::float4 color2_;
    metal::float4 tint;
};
typedef Region type_4[1];
struct Both {
    metal::float4 retained;
    metal::float4 drawable;
};
struct Basic {
    metal::float4 position;
    metal::float2 uv;
    char _pad2[8];
    metal::float4 color;
    float texture;
    char _pad4[12];
};

float rounded(
    metal::float4 b,
    metal::float4 r,
    float feather,
    metal::float2 p,
    constant metal::float4& viewport
) {
    if (b.z < 0.0) {
        return 1.0;
    }
    metal::float2 half_1 = b.zw * 0.5;
    metal::float2 q = (p - b.xy) - half_1;
    float radius = (q.y < 0.0) ? ((q.x > 0.0) ? r.y : r.x) : ((q.x > 0.0) ? r.z : r.w);
    metal::float2 d = (metal::abs(q) - half_1) + metal::float2(radius);
    float sdf = (metal::length(metal::max(d, metal::float2(0.0))) + metal::min(metal::max(d.x, d.y), 0.0)) - radius;
    if (feather > 0.0) {
        float t = metal::clamp(-(sdf) / feather, 0.0, 1.0);
        return (t * t) * (3.0 - (2.0 * t));
    }
    float _e60 = viewport.z;
    return metal::clamp(0.5 - (sdf * _e60), 0.0, 1.0);
}

uint naga_f2u32(float value) {
    return static_cast<uint>(metal::clamp(value, 0.0, 4294967000.0));
}

metal::float4 tinted(
    float index,
    metal::float4 premultiplied,
    device type_4 const& regions,
    constant _mslBufferSizes& _buffer_sizes
) {
    bool local = {};
    if (index < 0.5) {
        return premultiplied;
    }
    Region g = regions[naga_f2u32(index) - 1u];
    if (!((g.tint.x < 0.5))) {
        local = premultiplied.w <= 0.0;
    } else {
        local = true;
    }
    bool _e21 = local;
    if (_e21) {
        return premultiplied;
    }
    metal::float4 c = metal::float4(premultiplied.xyz / metal::float3(premultiplied.w), 1.0);
    metal::float3 rgb_1 = metal::clamp(metal::float3(metal::dot(g.color0_, c), metal::dot(g.color1_, c), metal::dot(g.color2_, c)), metal::float3(0.0), metal::float3(1.0));
    return metal::float4(rgb_1 * premultiplied.w, premultiplied.w);
}

float region(
    float index_1,
    metal::float2 position_1,
    constant metal::float4& viewport,
    device type_4 const& regions,
    constant _mslBufferSizes& _buffer_sizes
) {
    float k = {};
    Region g_1 = regions[naga_f2u32(index_1) - 1u];
    float _e10 = viewport.z;
    metal::float2 p_1 = position_1 / metal::float2(_e10);
    float _e17 = rounded(g_1.round0_, g_1.radii0_, g_1.tint.y, p_1, viewport);
    float _e22 = rounded(g_1.round1_, g_1.radii1_, g_1.tint.z, p_1, viewport);
    k = _e17 * _e22;
    if (g_1.fade.x > 0.0) {
        float t_1 = metal::clamp((p_1.y - g_1.fade_box.y) / g_1.fade.x, 0.0, 1.0);
        float _e39 = k;
        k = _e39 * (t_1 * t_1);
    }
    if (g_1.fade.y > 0.0) {
        float t_2 = metal::clamp((g_1.fade_box.z - p_1.x) / g_1.fade.y, 0.0, 1.0);
        float _e56 = k;
        k = _e56 * (t_2 * t_2);
    }
    if (g_1.fade.z > 0.0) {
        float t_3 = metal::clamp((g_1.fade_box.w - p_1.y) / g_1.fade.z, 0.0, 1.0);
        float _e73 = k;
        k = _e73 * (t_3 * t_3);
    }
    if (g_1.fade.w > 0.0) {
        float t_4 = metal::clamp((p_1.x - g_1.fade_box.x) / g_1.fade.w, 0.0, 1.0);
        float _e90 = k;
        k = _e90 * (t_4 * t_4);
    }
    float _e93 = k;
    return _e93;
}

metal::float2 zgx_erf2_(
    metal::float2 x
) {
    metal::float2 y = {};
    metal::float2 s = metal::sign(x);
    metal::float2 a_1 = metal::abs(x);
    y = metal::float2(1.0) + ((metal::float2(0.278393) + ((metal::float2(0.230389) + (0.078108 * (a_1 * a_1))) * a_1)) * a_1);
    metal::float2 _e18 = y;
    metal::float2 _e19 = y;
    y = _e18 * _e19;
    metal::float2 _e21 = y;
    metal::float2 _e22 = y;
    return s - (s / (_e21 * _e22));
}

float zgx_box_shadow_x(
    float x_1,
    float y_1,
    float sigma,
    float corner,
    metal::float2 half_
) {
    float delta = metal::min((half_.y - corner) - metal::abs(y_1), 0.0);
    float curved = (half_.x - corner) + metal::sqrt(metal::max(0.0, (corner * corner) - (delta * delta)));
    metal::float2 _e27 = zgx_erf2_((metal::float2(x_1) + metal::float2(-(curved), curved)) * (0.70710677 / sigma));
    metal::float2 i_3 = metal::float2(0.5) + (0.5 * _e27);
    return i_3.y - i_3.x;
}

float zgx_box_shadow(
    metal::float2 lower,
    metal::float2 upper,
    metal::float2 point,
    float sigma_1,
    float corner_1
) {
    float y_2 = {};
    float value = 0.0;
    int i_2 = 0;
    metal::float2 center = (lower + upper) * 0.5;
    metal::float2 half_2 = (upper - lower) * 0.5;
    metal::float2 p_2 = point - center;
    float low = p_2.y - half_2.y;
    float high = p_2.y + half_2.y;
    float start = metal::clamp(-3.0 * sigma_1, low, high);
    float end = metal::clamp(3.0 * sigma_1, low, high);
    float step = (end - start) * 0.25;
    y_2 = start + (step * 0.5);
    uint2 loop_bound = uint2(4294967295u);
    bool loop_init = true;
    while(true) {
        if (metal::all(loop_bound == uint2(0u))) { break; }
        loop_bound -= uint2(loop_bound.y == 0u, 1u);
        if (!loop_init) {
            int _e62 = i_2;
            i_2 = as_type<int>(as_type<uint>(_e62) + as_type<uint>(1));
        }
        loop_init = false;
        int _e35 = i_2;
        if (_e35 < 4) {
        } else {
            break;
        }
        {
            float _e38 = value;
            float _e41 = y_2;
            float _e43 = zgx_box_shadow_x(p_2.x, p_2.y - _e41, sigma_1, corner_1, half_2);
            float _e44 = y_2;
            float _e46 = y_2;
            value = _e38 + (((_e43 * metal::exp((-(_e44) * _e46) / ((2.0 * sigma_1) * sigma_1))) * (0.3989423 / sigma_1)) * step);
            float _e59 = y_2;
            y_2 = _e59 + step;
        }
    }
    float _e64 = value;
    return metal::clamp(_e64, 0.0, 1.0);
}

metal::float4 shade_texel(
    Out input,
    metal::float4 given,
    bool has_texel,
    constant metal::float4& viewport,
    metal::texture2d<float, metal::access::sample> atlas,
    device type_4 const& regions,
    metal::sampler tex_sampler,
    constant _mslBufferSizes& _buffer_sizes
) {
    Out v_5 = {};
    metal::float4 color_1 = {};
    bool local_1 = {};
    bool local_2 = {};
    float corner_2 = {};
    bool local_3 = {};
    float a = {};
    bool premultiplied_1 = false;
    metal::float3 rgb = metal::float3(0.0);
    float scale = 1.0;
    metal::float4 tex = {};
    v_5 = input;
    float _e6 = v_5.options.z;
    if (_e6 < 0.5) {
        metal::float4 _e11 = v_5.position;
        float _e15 = viewport.z;
        v_5.point = _e11.xy / metal::float2(_e15);
    }
    metal::float4 _e19 = v_5.color;
    color_1 = _e19;
    float _e23 = v_5.options.z;
    if (_e23 < 0.5) {
        float _e30 = v_5.shape.w;
        if (!((_e30 > 0.5))) {
            float _e38 = v_5.shape.z;
            local_2 = _e38 > 0.0;
        } else {
            local_2 = true;
        }
        bool _e42 = local_2;
        local_1 = _e42;
    } else {
        local_1 = false;
    }
    bool _e44 = local_1;
    if (_e44) {
        float _e47 = v_5.shape.x;
        corner_2 = _e47;
        float _e51 = v_5.options.w;
        if (_e51 > 0.5) {
            float _e56 = v_5.point.x;
            float _e59 = v_5.fade.x;
            float _e62 = v_5.fade.z;
            bool right = _e56 > (_e59 + (_e62 * 0.5));
            float _e69 = v_5.point.y;
            float _e72 = v_5.fade.y;
            float _e75 = v_5.fade.w;
            bool bottom = _e69 > (_e72 + (_e75 * 0.5));
            float _e82 = v_5.border.x;
            float _e85 = v_5.border.y;
            float _e89 = v_5.border.w;
            float _e92 = v_5.border.z;
            corner_2 = bottom ? (right ? _e92 : _e89) : (right ? _e85 : _e82);
        }
        float _e95 = corner_2;
        float _e98 = v_5.fade.z;
        float _e101 = v_5.fade.w;
        float radius_1 = metal::clamp(_e95, 0.0, metal::min(_e98, _e101) * 0.5);
        metal::float2 _e108 = v_5.point;
        metal::float4 _e110 = v_5.fade;
        metal::float4 _e114 = v_5.fade;
        metal::float4 _e121 = v_5.fade;
        metal::float2 q_1 = metal::abs((_e108 - _e110.xy) - (_e114.zw * 0.5)) - ((_e121.zw * 0.5) - metal::float2(radius_1));
        float distance = (metal::length(metal::max(q_1, metal::float2(0.0))) + metal::min(metal::max(q_1.x, q_1.y), 0.0)) - radius_1;
        float _e141 = v_5.shape.y;
        if (_e141 < 0.0) {
            float _e146 = v_5.shape.x;
            uint a_2 = naga_f2u32(_e146);
            float _e150 = v_5.shape.z;
            uint b_1 = naga_f2u32(_e150);
            metal::float4 sides = metal::float4(static_cast<float>(a_2 & 4095u), static_cast<float>(a_2 >> 12u), static_cast<float>(b_1 & 4095u), static_cast<float>(b_1 >> 12u)) / metal::float4(16.0);
            float _e170 = v_5.point.x;
            float _e173 = v_5.fade.x;
            float _e176 = v_5.fade.z;
            bool right_1 = _e170 > (_e173 + (_e176 * 0.5));
            float _e183 = v_5.point.y;
            float _e186 = v_5.fade.y;
            float _e189 = v_5.fade.w;
            bool bottom_1 = _e183 > (_e186 + (_e189 * 0.5));
            float across = right_1 ? sides.y : sides.w;
            float down = bottom_1 ? sides.z : sides.x;
            metal::float4 _e201 = v_5.fade;
            metal::float4 _e206 = v_5.fade;
            metal::float4 box = metal::float4(_e201.xy + sides.wx, metal::max((_e206.zw - sides.wx) - sides.yz, metal::float2(0.0)));
            float inner_radius = metal::clamp(radius_1 - ((across + down) * 0.5), 0.0, metal::min(box.z, box.w) * 0.5);
            metal::float2 _e228 = v_5.point;
            metal::float2 iq = metal::abs((_e228 - box.xy) - (box.zw * 0.5)) - ((box.zw * 0.5) - metal::float2(inner_radius));
            float inside = (metal::length(metal::max(iq, metal::float2(0.0))) + metal::min(metal::max(iq.x, iq.y), 0.0)) - inner_radius;
            float _e253 = metal::fwidth(distance);
            float aa = metal::max(_e253, 0.5);
            if (box.z > 0.0) {
                local_3 = box.w > 0.0;
            } else {
                local_3 = false;
            }
            bool _e271 = local_3;
            float hollow = _e271 ? metal::clamp(0.5 - (inside / aa), 0.0, 1.0) : 0.0;
            float _e275 = color_1.w;
            color_1.w = _e275 * metal::max(metal::clamp(0.5 - (distance / aa), 0.0, 1.0) - hollow, 0.0);
        } else {
            float _e288 = v_5.shape.z;
            if (_e288 > 0.0) {
                metal::float4 _e292 = v_5.fade;
                metal::float4 _e295 = v_5.fade;
                metal::float4 _e298 = v_5.fade;
                metal::float2 _e302 = v_5.point;
                float _e305 = v_5.shape.z;
                float _e306 = zgx_box_shadow(_e292.xy, _e295.xy + _e298.zw, _e302, _e305, radius_1);
                a = _e306;
                float spread = v_5.shape.w;
                metal::float4 _e312 = v_5.fade;
                metal::float2 _e315 = v_5.uv;
                metal::float4 _e320 = v_5.fade;
                metal::float4 own = metal::float4((_e312.xy - _e315) + metal::float2(spread), metal::max(_e320.zw - metal::float2(2.0 * spread), metal::float2(0.0)));
                float own_radius = metal::max(radius_1 - spread, 0.0);
                metal::float2 _e334 = v_5.point;
                metal::float2 oq = metal::abs((_e334 - own.xy) - (own.zw * 0.5)) - ((own.zw * 0.5) - metal::float2(own_radius));
                float outside = (metal::length(metal::max(oq, metal::float2(0.0))) + metal::min(metal::max(oq.x, oq.y), 0.0)) - own_radius;
                float _e359 = a;
                float _e360 = metal::fwidth(outside);
                a = _e359 * metal::clamp(0.5 + (outside / metal::max(_e360, 0.5)), 0.0, 1.0);
                float _e371 = color_1.w;
                float _e372 = a;
                color_1.w = _e371 * _e372;
            } else {
                float _e374 = metal::dfdx(distance);
                float dx = metal::abs(_e374);
                float _e376 = metal::dfdy(distance);
                float dy = metal::abs(_e376);
                float aa_1 = metal::max((radius_1 <= 0.0) ? metal::max(dx, dy) : (dx + dy), 0.5);
                float outer = metal::clamp(0.5 - (distance / aa_1), 0.0, 1.0);
                float _e393 = v_5.shape.y;
                float inner = metal::clamp(0.5 - ((distance + _e393) / aa_1), 0.0, 1.0);
                float _e402 = color_1.w;
                float _e406 = v_5.border.w;
                float alpha = (_e402 * inner) + (_e406 * (outer - inner));
                metal::float4 _e410 = color_1;
                float _e413 = color_1.w;
                metal::float4 _e417 = v_5.border;
                float _e421 = v_5.border.w;
                metal::float3 rgb_2 = (alpha > 0.0) ? ((((_e410.xyz * _e413) * inner) + ((_e417.xyz * _e421) * (outer - inner))) / metal::float3(metal::max(alpha, 0.00001))) : metal::float3(0.0);
                color_1 = metal::float4(rgb_2, alpha);
            }
        }
    }
    float _e445 = v_5.options.y;
    if (_e445 > 0.5) {
        tex = given;
        if (!(has_texel)) {
            metal::float2 _e453 = v_5.uv;
            metal::float4 _e454 = atlas.sample(tex_sampler, _e453);
            tex = _e454;
        }
        float _e457 = v_5.options.y;
        if (_e457 > 1.5) {
            premultiplied_1 = true;
            metal::float4 _e461 = color_1;
            metal::float4 _e463 = tex;
            float _e467 = color_1.w;
            rgb = (_e461.xyz * _e463.xyz) * _e467;
            float _e471 = color_1.w;
            float _e473 = tex.w;
            color_1.w = _e471 * _e473;
        } else {
            metal::float4 _e475 = color_1;
            metal::float4 _e477 = tex;
            float _e481 = color_1.w;
            float _e483 = tex.w;
            color_1 = metal::float4(_e475.xyz * _e477.xyz, _e481 * _e483);
        }
    }
    float _e488 = v_5.options.x;
    if (_e488 > 0.0) {
        metal::float2 _e492 = v_5.point;
        metal::float4 _e494 = v_5.fade;
        metal::float4 _e498 = v_5.fade;
        metal::float4 _e501 = v_5.fade;
        metal::float2 _e505 = v_5.point;
        metal::float2 distance_1 = metal::min(_e492 - _e494.xy, (_e498.xy + _e501.zw) - _e505);
        float _e513 = v_5.options.x;
        float f = metal::clamp(metal::min(distance_1.x, distance_1.y) / _e513, 0.0, 1.0);
        float _e519 = color_1.w;
        color_1.w = _e519 * f;
        float _e521 = scale;
        scale = _e521 * f;
    }
    float _e525 = v_5.mask.x;
    if (_e525 > 0.5) {
        float _e530 = v_5.mask.x;
        metal::float4 _e532 = v_5.position;
        float _e534 = region(_e530, _e532.xy, viewport, regions, _buffer_sizes);
        float _e536 = color_1.w;
        color_1.w = _e536 * _e534;
        float _e538 = scale;
        scale = _e538 * _e534;
    }
    float _e542 = v_5.mask.x;
    metal::float4 _e543 = color_1;
    float _e546 = color_1.w;
    float _e549 = color_1.w;
    metal::float3 _e551 = rgb;
    float _e552 = scale;
    float _e555 = color_1.w;
    bool _e557 = premultiplied_1;
    metal::float4 _e559 = tinted(_e542, _e557 ? metal::float4(_e551 * _e552, _e555) : metal::float4(_e543.xyz * _e546, _e549), regions, _buffer_sizes);
    return _e559;
}

metal::float4 shade(
    Out input_1,
    constant metal::float4& viewport,
    metal::texture2d<float, metal::access::sample> atlas,
    device type_4 const& regions,
    metal::sampler tex_sampler,
    constant _mslBufferSizes& _buffer_sizes
) {
    metal::float4 _e4 = shade_texel(input_1, metal::float4(0.0), false, viewport, atlas, regions, tex_sampler, _buffer_sizes);
    return _e4;
}

metal::float4 basic(
    Basic v_6,
    metal::texture2d<float, metal::access::sample> atlas,
    metal::sampler tex_sampler
) {
    if (v_6.texture > 0.5) {
        metal::float4 tex_1 = atlas.sample(tex_sampler, v_6.uv);
        float a_3 = v_6.color.w * tex_1.w;
        return metal::float4((v_6.color.xyz * tex_1.xyz) * ((v_6.texture > 1.5) ? v_6.color.w : a_3), a_3);
    }
    return metal::float4(v_6.color.xyz * v_6.color.w, v_6.color.w);
}

struct vsInput {
    metal::float4 rect [[attribute(0)]];
    metal::float4 uv [[attribute(1)]];
    metal::float4 color [[attribute(2)]];
    metal::float4 fade [[attribute(3)]];
    metal::float4 options [[attribute(4)]];
    metal::float4 shape [[attribute(5)]];
    metal::float4 border [[attribute(6)]];
    metal::float4 mask [[attribute(7)]];
};
struct vsOutput {
    metal::float4 position [[position]];
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), flat]];
    metal::float2 point [[user(loc2), center_perspective]];
    metal::float4 fade [[user(loc3), flat]];
    metal::float4 options [[user(loc4), flat]];
    metal::float4 shape [[user(loc5), flat]];
    metal::float4 border [[user(loc6), flat]];
    metal::float4 mask [[user(loc7), flat]];
};
vertex vsOutput vs(
  vsInput varyings [[stage_in]]
, uint i [[vertex_id]]
, constant metal::float4& viewport [[buffer(0)]]
) {
    const Vertex v = { varyings.rect, varyings.uv, varyings.color, varyings.fade, varyings.options, varyings.shape, varyings.border, varyings.mask };
    metal::float2 position = {};
    float w = 1.0;
    Out o = {};
    metal::float2 corner_3 = metal::float2(static_cast<float>(i & 1u), static_cast<float>(i >> 1u));
    metal::float2 p_3 = v.rect.xy + (corner_3 * v.rect.zw);
    position = p_3;
    if (v.options.z > 0.5) {
        position = metal::float2(metal::dot(v.shape.xy, p_3) + v.border.x, metal::dot(v.shape.zw, p_3) + v.border.y);
    }
    if (v.options.z > 1.5) {
        w = metal::max(metal::dot(v.border.zw, p_3) + 1.0, 0.0001);
    }
    float _e49 = position.x;
    float _e52 = viewport.x;
    float _e56 = w;
    float _e58 = w;
    float _e60 = position.y;
    float _e63 = viewport.y;
    float _e68 = w;
    o.position = metal::float4(((_e49 / _e52) * 2.0) - _e56, _e58 - ((_e60 / _e63) * 2.0), 0.0, _e68);
    o.uv = v.uv.xy + (corner_3 * v.uv.zw);
    o.color = v.color;
    o.point = p_3;
    o.fade = v.fade;
    o.options = v.options;
    o.shape = v.shape;
    o.border = v.border;
    o.mask = v.mask;
    Out _e91 = o;
    const auto _tmp = _e91;
    return vsOutput { _tmp.position, _tmp.uv, _tmp.color, _tmp.point, _tmp.fade, _tmp.options, _tmp.shape, _tmp.border, _tmp.mask };
}


struct fsInput {
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), flat]];
    metal::float2 point [[user(loc2), center_perspective]];
    metal::float4 fade [[user(loc3), flat]];
    metal::float4 options [[user(loc4), flat]];
    metal::float4 shape [[user(loc5), flat]];
    metal::float4 border [[user(loc6), flat]];
    metal::float4 mask [[user(loc7), flat]];
};
struct fsOutput {
    metal::float4 member_1 [[color(0)]];
};
fragment fsOutput fs(
  fsInput varyings_1 [[stage_in]]
, metal::float4 position_2 [[position]]
, constant metal::float4& viewport [[buffer(0)]]
, metal::texture2d<float, metal::access::sample> atlas [[texture(0)]]
, device type_4 const& regions [[buffer(2)]]
, constant _mslBufferSizes& _buffer_sizes [[buffer(3)]]
) {
    constexpr metal::sampler tex_sampler(
        metal::s_address::clamp_to_edge,
        metal::t_address::clamp_to_edge,
        metal::r_address::clamp_to_edge,
        metal::mag_filter::linear,
        metal::min_filter::linear,
        metal::coord::normalized
    );
    const Out v_1 = { position_2, varyings_1.uv, {}, varyings_1.color, varyings_1.point, {}, varyings_1.fade, varyings_1.options, varyings_1.shape, varyings_1.border, varyings_1.mask };
    metal::float4 _e1 = shade(v_1, viewport, atlas, regions, tex_sampler, _buffer_sizes);
    return fsOutput { _e1 };
}


struct fs_bothInput {
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), flat]];
    metal::float2 point [[user(loc2), center_perspective]];
    metal::float4 fade [[user(loc3), flat]];
    metal::float4 options [[user(loc4), flat]];
    metal::float4 shape [[user(loc5), flat]];
    metal::float4 border [[user(loc6), flat]];
    metal::float4 mask [[user(loc7), flat]];
};
struct fs_bothOutput {
    metal::float4 retained [[color(0)]];
    metal::float4 drawable [[color(1)]];
};
fragment fs_bothOutput fs_both(
  fs_bothInput varyings_2 [[stage_in]]
, metal::float4 position_3 [[position]]
, constant metal::float4& viewport [[buffer(0)]]
, metal::texture2d<float, metal::access::sample> atlas [[texture(0)]]
, device type_4 const& regions [[buffer(2)]]
, constant _mslBufferSizes& _buffer_sizes [[buffer(3)]]
) {
    constexpr metal::sampler tex_sampler(
        metal::s_address::clamp_to_edge,
        metal::t_address::clamp_to_edge,
        metal::r_address::clamp_to_edge,
        metal::mag_filter::linear,
        metal::min_filter::linear,
        metal::coord::normalized
    );
    const Out v_2 = { position_3, varyings_2.uv, {}, varyings_2.color, varyings_2.point, {}, varyings_2.fade, varyings_2.options, varyings_2.shape, varyings_2.border, varyings_2.mask };
    metal::float4 _e1 = shade(v_2, viewport, atlas, regions, tex_sampler, _buffer_sizes);
    const auto _tmp = Both {_e1, _e1};
    return fs_bothOutput { _tmp.retained, _tmp.drawable };
}


struct vs_basicInput {
    metal::float4 rect [[attribute(0)]];
    metal::float4 uv [[attribute(1)]];
    metal::float4 color [[attribute(2)]];
    metal::float4 options [[attribute(4)]];
};
struct vs_basicOutput {
    metal::float4 position [[position]];
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), flat]];
    float texture [[user(loc2), flat]];
};
vertex vs_basicOutput vs_basic(
  vs_basicInput varyings_3 [[stage_in]]
, uint i_1 [[vertex_id]]
, constant metal::float4& viewport [[buffer(0)]]
) {
    const auto rect = varyings_3.rect;
    const auto uv = varyings_3.uv;
    const auto color = varyings_3.color;
    const auto options = varyings_3.options;
    Basic o_1 = {};
    metal::float2 corner_4 = metal::float2(static_cast<float>(i_1 & 1u), static_cast<float>(i_1 >> 1u));
    metal::float2 p_4 = rect.xy + (corner_4 * rect.zw);
    float _e21 = viewport.x;
    float _e30 = viewport.y;
    o_1.position = metal::float4(((p_4.x / _e21) * 2.0) - 1.0, 1.0 - ((p_4.y / _e30) * 2.0), 0.0, 1.0);
    o_1.uv = uv.xy + (corner_4 * uv.zw);
    o_1.color = color;
    o_1.texture = options.y;
    Basic _e47 = o_1;
    const auto _tmp = _e47;
    return vs_basicOutput { _tmp.position, _tmp.uv, _tmp.color, _tmp.texture };
}


struct fs_basicInput {
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), flat]];
    float texture [[user(loc2), flat]];
};
struct fs_basicOutput {
    metal::float4 member_4 [[color(0)]];
};
fragment fs_basicOutput fs_basic(
  fs_basicInput varyings_4 [[stage_in]]
, metal::float4 position_4 [[position]]
, metal::texture2d<float, metal::access::sample> atlas [[texture(0)]]
) {
    constexpr metal::sampler tex_sampler(
        metal::s_address::clamp_to_edge,
        metal::t_address::clamp_to_edge,
        metal::r_address::clamp_to_edge,
        metal::mag_filter::linear,
        metal::min_filter::linear,
        metal::coord::normalized
    );
    const Basic v_3 = { position_4, varyings_4.uv, {}, varyings_4.color, varyings_4.texture };
    metal::float4 _e1 = basic(v_3, atlas, tex_sampler);
    return fs_basicOutput { _e1 };
}


struct fs_basic_bothInput {
    metal::float2 uv [[user(loc0), center_perspective]];
    metal::float4 color [[user(loc1), flat]];
    float texture [[user(loc2), flat]];
};
struct fs_basic_bothOutput {
    metal::float4 retained [[color(0)]];
    metal::float4 drawable [[color(1)]];
};
fragment fs_basic_bothOutput fs_basic_both(
  fs_basic_bothInput varyings_5 [[stage_in]]
, metal::float4 position_5 [[position]]
, metal::texture2d<float, metal::access::sample> atlas [[texture(0)]]
) {
    constexpr metal::sampler tex_sampler(
        metal::s_address::clamp_to_edge,
        metal::t_address::clamp_to_edge,
        metal::r_address::clamp_to_edge,
        metal::mag_filter::linear,
        metal::min_filter::linear,
        metal::coord::normalized
    );
    const Basic v_4 = { position_5, varyings_5.uv, {}, varyings_5.color, varyings_5.texture };
    metal::float4 _e1 = basic(v_4, atlas, tex_sampler);
    const auto _tmp = Both {_e1, _e1};
    return fs_basic_bothOutput { _tmp.retained, _tmp.drawable };
}
