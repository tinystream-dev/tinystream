// SPDX-License-Identifier: AGPL-3.0-or-later

struct Params {
    // Icon size, stroke width, physical pixels per logical pixel, segment count.
    shape: vec4<f32>,
    color: vec4<f32>,
}
struct Segment {
    ends: vec4<f32>,
    // Participates in fill, participates in stroke.
    flags: vec4<f32>,
}
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> segments: array<Segment>;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
}
@vertex
fn vs(@builtin(vertex_index) index: u32) -> Out {
    let points = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let p = points[index];
    return Out(vec4(p * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0), p * (params.shape.x + 2.0) - vec2(1.0));
}
@fragment
fn fs(input: Out) -> @location(0) vec4<f32> {
    let p = input.local;
    var stroke_distance = 1e6;
    var fill_distance = 1e6;
    var winding = 0i;
    for (var i = 0u; i < u32(params.shape.w); i++) {
        let segment = segments[i];
        let a = segment.ends.xy;
        let b = segment.ends.zw;
        let edge = b - a;
        let t = clamp(dot(p - a, edge) / max(dot(edge, edge), 1e-12), 0.0, 1.0);
        let distance = length(p - a - t * edge);
        if segment.flags.y > 0.0 { stroke_distance = min(stroke_distance, distance); }
        if segment.flags.x > 0.0 {
            fill_distance = min(fill_distance, distance);
            let cross = edge.x * (p.y - a.y) - edge.y * (p.x - a.x);
            if a.y <= p.y && b.y > p.y && cross > 0.0 { winding++; }
            if a.y > p.y && b.y <= p.y && cross < 0.0 { winding--; }
        }
    }
    let stroke = stroke_distance - params.shape.y * 0.5;
    let fill = select(fill_distance, -fill_distance, winding != 0i);
    let coverage = clamp(0.5 - min(stroke, fill) * params.shape.z, 0.0, 1.0);
    return vec4(params.color.rgb * params.color.a, params.color.a) * coverage;
}
