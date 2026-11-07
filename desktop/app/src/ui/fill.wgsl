// SPDX-License-Identifier: AGPL-3.0-or-later
// A squircle filled with one colour, its edge antialiased by the mesh's fringe.

struct Params {
    // The primitive's size in frame pixels, which iced's viewport maps it to.
    size: vec2<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) coverage: f32,
}

@vertex
fn vs(@location(0) local: vec2<f32>, @location(1) coverage: f32) -> Out {
    let clip = local / params.size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return Out(vec4<f32>(clip, 0.0, 1.0), coverage);
}

@fragment
fn fs(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(params.color.rgb * params.color.a, params.color.a) * input.coverage;
}
