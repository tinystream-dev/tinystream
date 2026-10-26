// SPDX-License-Identifier: AGPL-3.0-or-later
// Artwork leaning toward the pointer (`.tilt`), with a glint where the light catches (`.glare`).

struct Params {
    // The card in frame pixels: x, y, width, height.
    rect: vec4<f32>,
    frame: vec2<f32>,
    // rotateX and rotateY, in radians, and the perspective distance in frame pixels.
    tilt: vec2<f32>,
    perspective: f32,
    glare_opacity: f32,
    // Where the glint is (0 to 1 across the card) and its reach, in frame pixels.
    glare: vec2<f32>,
    glare_radius: f32,
    hover: f32,
    // How the picture covers the card: uv = local * scale + offset.
    cover: vec4<f32>,
    background: vec4<f32>,
    media_ink: vec4<f32>,
    glow: vec4<f32>,
    // The lit edge's opacity at the top, a third of the way down and at the bottom.
    edge: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var linear: sampler;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) coverage: f32,
    @location(2) edge: f32,
}

@vertex
fn vs(@location(0) local: vec2<f32>, @location(1) coverage: f32, @location(2) edge: f32) -> Out {
    let size = params.rect.zw;
    let p = local - size * 0.5;
    // perspective(d) rotateX(rx) rotateY(ry), applied to the point right to left, as CSS does.
    let sx = sin(params.tilt.x);
    let cx = cos(params.tilt.x);
    let sy = sin(params.tilt.y);
    let cy = cos(params.tilt.y);
    let x1 = p.x * cy;
    let z1 = -p.x * sy;
    let y2 = p.y * cx - z1 * sx;
    let z2 = p.y * sx + z1 * cx;
    let w = (params.perspective - z2) / params.perspective;
    let screen = params.rect.xy + size * 0.5 + vec2<f32>(x1, y2) / w;
    let clip = screen / params.frame * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return Out(vec4<f32>(clip * w, 0.0, w), local, coverage, edge);
}

fn soft_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= vec3<f32>(0.25));
    return select(b + (2.0 * s - 1.0) * (d - b), b - (1.0 - 2.0 * s) * b * (1.0 - b), s <= vec3<f32>(0.5));
}

@fragment
fn fs(input: Out) -> @location(0) vec4<f32> {
    let size = params.rect.zw;
    let uv = input.local / size;
    let art = textureSample(picture, linear, uv * params.cover.xy + params.cover.zw);
    var color = params.background.rgb * (1.0 - art.a) + art.rgb * art.a;

    // radial-gradient(circle 65cqmin at gx gy, media-ink 55%, transparent 70%), soft-light.
    let reach = length(input.local - params.glare * size) / params.glare_radius;
    let glint = params.media_ink.a * 0.55 * clamp(1.0 - reach / 0.7, 0.0, 1.0) * params.glare_opacity;
    color = mix(color, soft_light(color, params.media_ink.rgb), glint);

    color = mix(color, params.media_ink.rgb, 0.04 * params.hover);

    let y = uv.y;
    let lit = select(mix(params.edge.y, params.edge.z, (y - 0.35) / 0.65), mix(params.edge.x, params.edge.y, y / 0.35), y < 0.35);
    color = mix(color, params.glow.rgb, input.edge * lit * params.glow.a);
    return vec4<f32>(color, 1.0) * input.coverage;
}
