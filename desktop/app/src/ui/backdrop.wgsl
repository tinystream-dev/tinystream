// SPDX-License-Identifier: AGPL-3.0-or-later
// What's behind a region, blurred: `backdrop-filter: blur() saturate()` under a tint, optionally
// seen through a lens whose rounded rim bends it, splits its colours and catches the light.

struct Params {
    // The region read from the frame, in frame pixels: x, y, width, height.
    region: vec4<f32>,
    frame: vec2<f32>,
    // How many frame pixels one scratch texel covers, and how many samples each axis takes.
    down: f32,
    taps: f32,
    // The blur's standard deviation, in scratch texels.
    sigma: f32,
    saturate: f32,
    zoom: f32,
    // 0 for horizontal blur passes, 1 for vertical ones.
    vertical: f32,
    tint: vec4<f32>,
    // The squircle, in frame pixels: x, y, width, height.
    shape: vec4<f32>,
    // How far its corners reach along each side, and the exponent of the rounded box that follows
    // their curve.
    corner: f32,
    exponent: f32,
    // How wide the lens' rim is, in frame pixels (0 for flat glass), and how bright its highlight.
    bezel: f32,
    light: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var linear: sampler;

struct Full {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn full_vs(@builtin(vertex_index) index: u32) -> Full {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Full(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0));
}

// The frame under the region, shrunk by `down` with a box filter (each bilinear tap averages 2×2).
@fragment
fn down_fs(input: Full) -> @location(0) vec4<f32> {
    let center = params.region.xy + params.region.zw * 0.5;
    let at = params.region.xy + input.position.xy * params.down;
    let n = i32(params.taps);
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < n; i++) {
        for (var j = 0; j < n; j++) {
            let offset = (vec2<f32>(f32(i), f32(j)) + 0.5) / params.taps - 0.5;
            let p = center + (at + offset * params.down - center) / params.zoom;
            sum += textureSampleLevel(source, linear, p / params.frame, 0.0);
        }
    }
    return sum / f32(n * n);
}

@fragment
fn blur_fs(input: Full) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source));
    let p = vec2<i32>(input.position.xy);
    let direction = select(vec2<i32>(1, 0), vec2<i32>(0, 1), params.vertical > 0.5);
    if params.sigma < 0.01 {
        return textureLoad(source, p, 0);
    }
    let reach = i32(ceil(params.sigma * 3.0));
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -reach; i <= reach; i++) {
        let weight = exp(-f32(i * i) / (2.0 * params.sigma * params.sigma));
        let q = clamp(p + direction * i, vec2<i32>(0), size - 1);
        sum += textureLoad(source, q, 0) * weight;
        total += weight;
    }
    return sum / total;
}

struct Shape {
    @builtin(position) position: vec4<f32>,
    @location(0) coverage: f32,
}

@vertex
fn shape_vs(@location(0) at: vec2<f32>, @location(1) coverage: f32) -> Shape {
    let clip = at / params.frame * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return Shape(vec4<f32>(clip, 0.0, 1.0), coverage);
}

// How deep inside the squircle a point is (negative outside), and which way is out.
fn edge(p: vec2<f32>) -> vec3<f32> {
    let half = params.shape.zw * 0.5;
    let local = p - params.shape.xy - half;
    let q = abs(local) - half + params.corner;
    let o = max(q, vec2<f32>(1e-4));
    let n = params.exponent;
    var out = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), q.x > q.y);
    var distance = max(q.x, q.y) - params.corner;
    if q.x > 0.0 && q.y > 0.0 {
        out = normalize(pow(o, vec2<f32>(n - 1.0)));
        distance = pow(pow(o.x, n) + pow(o.y, n), 1.0 / n) - params.corner;
    }
    return vec3<f32>(-distance, out * sign(local));
}

fn backdrop(p: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(source, linear, (p - params.region.xy) / params.region.zw, 0.0).rgb;
}

// The blurred backdrop, saturated as CSS's `saturate()` does, with the tint painted over it. Under
// a lens, the rim curves away like a quarter circle and shows what's just past the edge, drawn in
// and squeezed as the curve steepens, with red bent less than blue.
@fragment
fn shape_fs(input: Shape) -> @location(0) vec4<f32> {
    let p = input.position.xy;
    var c: vec3<f32>;
    var shine = 0.0;
    let e = edge(p);
    if params.bezel <= 0.0 || e.x >= params.bezel {
        c = backdrop(p);
    } else {
        let u = 1.0 - max(e.x, 0.0) / params.bezel;
        let incidence = asin(min(u, 0.999));
        let bend = tan(incidence - asin(sin(incidence) / 1.5)) * params.bezel * 1.2;
        let out = e.yz * bend;
        c = vec3<f32>(backdrop(p + out * 0.92).r, backdrop(p + out).g, backdrop(p + out * 1.08).b);
        // Lit from the top left, and again where that light leaves at the bottom right.
        let facing = abs(dot(e.yz, vec2<f32>(-0.7071, -0.7071)));
        let rim = 1.0 - smoothstep(0.0, max(params.bezel * 0.12, 1.5), e.x);
        shine = params.light * (rim * mix(0.25, 1.0, facing * facing) + u * u * u * 0.18 * facing);
    }
    let s = params.saturate;
    let saturated = clamp(vec3<f32>(
        dot(c, vec3<f32>(0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s)),
        dot(c, vec3<f32>(0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s)),
        dot(c, vec3<f32>(0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s)),
    ), vec3<f32>(0.0), vec3<f32>(1.0));
    let color = params.tint.rgb * params.tint.a + saturated * (1.0 - params.tint.a);
    return vec4<f32>(mix(color, vec3<f32>(1.0), clamp(shine, 0.0, 1.0)), 1.0) * input.coverage;
}
