// SPDX-License-Identifier: AGPL-3.0-or-later
// A squircle filled with one colour, its edge antialiased by the mesh's fringe.

struct Params {
    // The primitive's size in frame pixels, which iced's viewport maps it to.
    size: vec2<f32>,
    color: vec4<f32>,
    style: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) coverage: f32,
    @location(1) along: f32,
    @location(2) y: f32,
}

@vertex
fn vs(@location(0) local: vec2<f32>, @location(1) coverage: f32, @location(2) along: f32) -> Out {
    let clip = local / params.size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return Out(vec4<f32>(clip, 0.0, 1.0), coverage, along, local.y / params.size.y);
}

@fragment
fn fs(input: Out) -> @location(0) vec4<f32> {
    let y = clamp(input.y, 0.0, 1.0);
    let alpha = select(mix(params.style.y, params.style.z, (y - 0.35) / 0.65),
        mix(params.style.x, params.style.y, y / 0.35), y < 0.35);
    var coverage = input.coverage * alpha;
    if params.style.w > 0.0 {
        let period = params.style.w;
        let dash = abs(fract(input.along / period - 0.25) - 0.5) * period - period * 0.25;
        coverage *= clamp(dash / max(fwidth(input.along), 0.001) + 0.5, 0.0, 1.0);
    }
    return vec4<f32>(params.color.rgb * params.color.a, params.color.a) * coverage;
}
