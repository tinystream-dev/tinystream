// SPDX-License-Identifier: AGPL-3.0-or-later
// mpv's picture, the light it spills around it (`Ambilight`) and the frame receding (`.recede`).

struct Params {
    // Where to draw, in target pixels: x, y, width, height.
    rect: vec4<f32>,
    target_size: vec2<f32>,
    // How far the frame has receded (0 to 1), and its blur then, in target pixels.
    recede: f32,
    blur: f32,
    opacity: f32,
    // 1 when the target stores linear colour (an sRGB format), so encoded colour is decoded first.
    linear: f32,
    // The constant blend of a new sample over the last, for the ambient light.
    blend: f32,
    pad: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var filtering: sampler;

// The size the ambient light is sampled down to (the web's 32 by 18 canvas).
const AMBIENT = vec2<f32>(32.0, 18.0);

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Out {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let p = params.rect.xy + corner * params.rect.zw;
    let clip = p / params.target_size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return Out(vec4<f32>(clip, 0.0, 1.0), corner);
}

fn saturate_by(color: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return max(mix(vec3<f32>(luma), color, amount), vec3<f32>(0.0));
}

fn decode(color: vec3<f32>) -> vec3<f32> {
    if params.linear < 0.5 {
        return color;
    }
    return select(pow((color + 0.055) / 1.055, vec3<f32>(2.4)), color / 12.92, color <= vec3<f32>(0.04045));
}

@fragment
fn video(in: Out) -> @location(0) vec4<f32> {
    var color = textureSampleLevel(picture, filtering, in.uv, 0.0).rgb;
    if params.recede > 0.0 {
        // blur(6px), as a disc of samples around the pixel.
        let size = vec2<f32>(textureDimensions(picture));
        let radius = params.blur * params.recede / params.rect.zw;
        var sum = color;
        for (var i = 1; i < 24; i++) {
            let angle = f32(i) * 2.39996;
            let reach = sqrt(f32(i) / 23.0);
            let offset = vec2<f32>(cos(angle), sin(angle)) * reach * radius;
            sum += textureSampleLevel(picture, filtering, in.uv + offset, 0.0).rgb;
        }
        color = sum / 24.0;
        // brightness(0.55) saturate(0.85).
        color = saturate_by(color * (1.0 - 0.45 * params.recede), 1.0 - 0.15 * params.recede);
    }
    return vec4<f32>(decode(color), 1.0);
}

// A frame sampled down into the ambient light, averaging what each of its texels covers.
@fragment
fn accumulate(in: Out) -> @location(0) vec4<f32> {
    var sum = vec3<f32>(0.0);
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            let offset = (vec2<f32>(f32(x), f32(y)) - 1.5) / 4.0 / AMBIENT;
            sum += textureSampleLevel(picture, filtering, in.uv + offset, 0.0).rgb;
        }
    }
    return vec4<f32>(sum / 16.0, params.blend);
}

// The ambient light blurred at its own size, which is far cheaper than at the window's.
@fragment
fn blur(in: Out) -> @location(0) vec4<f32> {
    let weights = array<f32, 5>(0.0545, 0.2442, 0.4026, 0.2442, 0.0545);
    var sum = vec3<f32>(0.0);
    for (var y = 0; y < 5; y++) {
        for (var x = 0; x < 5; x++) {
            let offset = vec2<f32>(f32(x - 2), f32(y - 2)) / AMBIENT;
            sum += textureSampleLevel(picture, filtering, in.uv + offset, 0.0).rgb * weights[x] * weights[y];
        }
    }
    return vec4<f32>(sum, 1.0);
}

// The blurred light, stretched over the stage with a B-spline so its texels don't show.
@fragment
fn ambient(in: Out) -> @location(0) vec4<f32> {
    let texel = in.uv * AMBIENT - 0.5;
    let base = floor(texel);
    let f = texel - base;
    let w0 = (1.0 - f) * (1.0 - f) * (1.0 - f) / 6.0;
    let w1 = (4.0 - 6.0 * f * f + 3.0 * f * f * f) / 6.0;
    let w2 = (1.0 + 3.0 * f + 3.0 * f * f - 3.0 * f * f * f) / 6.0;
    let w3 = f * f * f / 6.0;
    let g0 = w0 + w1;
    let g1 = w2 + w3;
    let h0 = (base - 1.0 + w1 / g0 + 0.5) / AMBIENT;
    let h1 = (base + 1.0 + w3 / g1 + 0.5) / AMBIENT;
    let color = g0.y * (g0.x * textureSampleLevel(picture, filtering, vec2<f32>(h0.x, h0.y), 0.0).rgb
        + g1.x * textureSampleLevel(picture, filtering, vec2<f32>(h1.x, h0.y), 0.0).rgb)
        + g1.y * (g0.x * textureSampleLevel(picture, filtering, vec2<f32>(h0.x, h1.y), 0.0).rgb
        + g1.x * textureSampleLevel(picture, filtering, vec2<f32>(h1.x, h1.y), 0.0).rgb);
    // saturate(1.5), at 55% opacity.
    return vec4<f32>(decode(saturate_by(color, 1.5)) * params.opacity, params.opacity);
}
