// SPDX-License-Identifier: AGPL-3.0-or-later

use std::f64::consts::PI;
use std::fmt::Write;
use std::path::PathBuf;
use std::{env, fs};

use graphql_client_codegen::{CodegenMode, GraphQLClientCodegenOptions, generate_module_token_stream_from_string};
use graphql_parser::query::Definition;

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    logo(&output);
    icons(&output);

    let schema = "../../packages/shared/schema.graphql";
    let fragments = "../../packages/shared/fragments.graphql";
    let queries = ["graphql/session.graphql", "graphql/player.graphql"];
    for path in [schema, fragments].iter().chain(&queries) {
        println!("cargo:rerun-if-changed={path}");
    }

    let shared = fs::read_to_string(fragments).expect("shared fragments");
    let mut shared = graphql_parser::parse_query::<String>(&shared).expect("valid shared fragments");
    shared.definitions.retain(|definition| {
        matches!(definition, Definition::Fragment(fragment) if matches!(fragment.name.as_str(), "Person" | "NotificationFields" | "Playback" | "TranscodingFields" | "SeriesEpisodeFields"))
    });
    let operations = queries.map(|query| fs::read_to_string(query).expect("operations")).join("\n");
    let document = format!("{operations}\n{shared}");
    let mut options = GraphQLClientCodegenOptions::new(CodegenMode::Cli);
    options.set_module_visibility(syn::parse_str("pub").expect("public visibility"));
    options.set_response_derives("Clone, Serialize".into());
    options.set_fragments_other_variant(true);
    let generated = generate_module_token_stream_from_string(&document, &PathBuf::from(schema), options)
        .expect("operations match the shared schema");
    fs::write(output.join("graphql.rs"), generated.to_string()).expect("write generated API types");
}

/// The logo's shapes from the shared `logo.ts`, as steps like the icons'.
fn logo(output: &std::path::Path) {
    let logo = "../../packages/shared/src/logo.ts";
    println!("cargo:rerun-if-changed={logo}");
    let source = fs::read_to_string(logo).expect("shared logo");
    let mut code = String::new();
    for name in ["PLATE", "GLYPH", "CUT"] {
        let d = source
            .split(&format!("export const {name} = '"))
            .nth(1)
            .and_then(|rest| rest.split('\'').next())
            .unwrap_or_else(|| panic!("{name} in the shared logo"));
        let _ = write!(code, "pub(super) const {name}: &[Step] = &[");
        write_steps(&mut code, &path(d));
        code.push_str("];\n");
    }
    fs::write(output.join("logo.rs"), code).expect("write logo");
}

fn write_steps(code: &mut String, steps: &[Step]) {
    for step in steps {
        let _ = match *step {
            Step::Move(x, y) => write!(code, "Step::Move({x:?}, {y:?}),"),
            Step::Line(x, y) => write!(code, "Step::Line({x:?}, {y:?}),"),
            Step::Cubic(a, b, c, d, x, y) => write!(code, "Step::Cubic({a:?}, {b:?}, {c:?}, {d:?}, {x:?}, {y:?}),"),
            Step::Close => write!(code, "Step::Close,"),
        };
    }
}

/// The Lucide icons in `assets/lucide` as Rust: each outline in absolute moves, lines and cubics.
fn icons(output: &std::path::Path) {
    let dir = "assets/lucide";
    println!("cargo:rerun-if-changed={dir}");
    let mut names = fs::read_dir(dir)
        .expect("icons")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok()?.strip_suffix(".svg").map(str::to_owned))
        .collect::<Vec<_>>();
    names.sort();

    let mut code = String::from(
        "/// A Lucide icon, from `assets/lucide` (the lucide-react version the web locks).\n\
         #[allow(dead_code)]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum Icon {\n",
    );
    for name in &names {
        let _ = writeln!(code, "    {},", pascal(name));
    }
    code.push_str(
        "}\n\nimpl Icon {\n    fn shapes(self) -> &'static [(bool, &'static [Step])] {\n        match self {\n",
    );
    for name in &names {
        let svg = fs::read_to_string(format!("{dir}/{name}.svg")).expect("an icon");
        let _ = write!(code, "            Icon::{} => &[", pascal(name));
        for element in svg.split('<').skip(1) {
            let tag = element.split_whitespace().next().unwrap_or_default();
            let filled = attr(element, "fill") == Some("currentColor");
            let steps = match tag {
                "path" => path(attr(element, "d").expect("a path")),
                "circle" => {
                    let [cx, cy, r] = ["cx", "cy", "r"].map(|n| number(element, n));
                    ellipse(cx, cy, r, r)
                },
                "rect" => {
                    let [x, y, w, h] = ["x", "y", "width", "height"].map(|n| number(element, n));
                    let rx = attr(element, "rx").map_or(0.0, |v| v.parse().expect("rx"));
                    let ry = attr(element, "ry").map_or(rx, |v| v.parse().expect("ry"));
                    rect(x, y, w, h, rx, ry)
                },
                "line" => {
                    let [x1, y1, x2, y2] = ["x1", "y1", "x2", "y2"].map(|n| number(element, n));
                    vec![Step::Move(x1, y1), Step::Line(x2, y2)]
                },
                "polyline" | "polygon" => {
                    let points = numbers(attr(element, "points").expect("points"));
                    let mut steps = points
                        .chunks(2)
                        .enumerate()
                        .map(|(i, p)| if i == 0 { Step::Move(p[0], p[1]) } else { Step::Line(p[0], p[1]) })
                        .collect::<Vec<_>>();
                    if tag == "polygon" {
                        steps.push(Step::Close);
                    }
                    steps
                },
                "svg" => continue,
                _ if tag.starts_with('/') => continue,
                _ => panic!("{name}: no support for <{tag}>"),
            };
            let _ = write!(code, "({filled}, &[");
            write_steps(&mut code, &steps);
            code.push_str("]),");
        }
        code.push_str("],\n");
    }
    code.push_str("        }\n    }\n}\n");
    fs::write(output.join("icons.rs"), code).expect("write icons");
}

#[derive(Clone, Copy)]
enum Step {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

fn pascal(name: &str) -> String {
    name.split('-').map(|part| part[..1].to_uppercase() + &part[1..]).collect()
}

fn attr<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let start = element.find(&format!(" {name}=\""))? + name.len() + 3;
    element[start..].split('"').next()
}

fn number(element: &str, name: &str) -> f32 {
    attr(element, name).and_then(|v| v.parse().ok()).unwrap_or_else(|| panic!("{name} in {element}"))
}

fn numbers(list: &str) -> Vec<f32> {
    list.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().expect("a number"))
        .collect()
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<Step> {
    let k = 0.552_284_8;
    let (ox, oy) = (rx * k, ry * k);
    vec![
        Step::Move(cx + rx, cy),
        Step::Cubic(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry),
        Step::Cubic(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy),
        Step::Cubic(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry),
        Step::Cubic(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy),
        Step::Close,
    ]
}

fn rect(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> Vec<Step> {
    let (rx, ry) = (rx.min(w / 2.0), ry.min(h / 2.0));
    let (ox, oy) = (rx * 0.552_284_8, ry * 0.552_284_8);
    vec![
        Step::Move(x + rx, y),
        Step::Line(x + w - rx, y),
        Step::Cubic(x + w - rx + ox, y, x + w, y + ry - oy, x + w, y + ry),
        Step::Line(x + w, y + h - ry),
        Step::Cubic(x + w, y + h - ry + oy, x + w - rx + ox, y + h, x + w - rx, y + h),
        Step::Line(x + rx, y + h),
        Step::Cubic(x + rx - ox, y + h, x, y + h - ry + oy, x, y + h - ry),
        Step::Line(x, y + ry),
        Step::Cubic(x, y + ry - oy, x + rx - ox, y, x + rx, y),
        Step::Close,
    ]
}

/// An SVG path's data, everything made absolute and every curve a cubic.
fn path(d: &str) -> Vec<Step> {
    let bytes = d.as_bytes();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
            *i += 1;
        }
    };
    let read = |i: &mut usize| -> f32 {
        skip(i);
        let start = *i;
        if *i < bytes.len() && matches!(bytes[*i], b'-' | b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < bytes.len() && (bytes[*i].is_ascii_digit() || (bytes[*i] == b'.' && !dot)) {
            dot |= bytes[*i] == b'.';
            *i += 1;
        }
        if *i < bytes.len() && matches!(bytes[*i], b'e' | b'E') {
            *i += 1;
            if matches!(bytes[*i], b'-' | b'+') {
                *i += 1;
            }
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        d[start..*i].parse().unwrap_or_else(|_| panic!("a number at {start} in {d}"))
    };
    let flag = |i: &mut usize| -> bool {
        skip(i);
        *i += 1;
        bytes[*i - 1] == b'1'
    };

    let mut steps = Vec::new();
    let (mut x, mut y, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut last_control: Option<(f32, f32)> = None;
    let mut command = b'M';
    loop {
        skip(&mut i);
        if i >= bytes.len() {
            break;
        }
        if bytes[i].is_ascii_alphabetic() {
            command = bytes[i];
            i += 1;
        }
        let relative = command.is_ascii_lowercase();
        let (ox, oy) = if relative { (x, y) } else { (0.0, 0.0) };
        let mut control = None;
        match command.to_ascii_uppercase() {
            b'M' => {
                (x, y) = (ox + read(&mut i), oy + read(&mut i));
                (sx, sy) = (x, y);
                steps.push(Step::Move(x, y));
                command = if relative { b'l' } else { b'L' };
            },
            b'L' => {
                (x, y) = (ox + read(&mut i), oy + read(&mut i));
                steps.push(Step::Line(x, y));
            },
            b'H' => {
                x = ox + read(&mut i);
                steps.push(Step::Line(x, y));
            },
            b'V' => {
                y = (if relative { y } else { 0.0 }) + read(&mut i);
                steps.push(Step::Line(x, y));
            },
            b'C' | b'S' => {
                let (ax, ay) = if command.eq_ignore_ascii_case(&b'S') {
                    last_control.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy))
                } else {
                    (ox + read(&mut i), oy + read(&mut i))
                };
                let (bx, by) = (ox + read(&mut i), oy + read(&mut i));
                (x, y) = (ox + read(&mut i), oy + read(&mut i));
                steps.push(Step::Cubic(ax, ay, bx, by, x, y));
                control = Some((bx, by));
            },
            b'Q' => {
                let (qx, qy) = (ox + read(&mut i), oy + read(&mut i));
                let (ex, ey) = (ox + read(&mut i), oy + read(&mut i));
                steps.push(Step::Cubic(
                    x + 2.0 / 3.0 * (qx - x),
                    y + 2.0 / 3.0 * (qy - y),
                    ex + 2.0 / 3.0 * (qx - ex),
                    ey + 2.0 / 3.0 * (qy - ey),
                    ex,
                    ey,
                ));
                (x, y) = (ex, ey);
            },
            b'A' => {
                let (rx, ry, rotation) = (read(&mut i), read(&mut i), read(&mut i));
                let (large, sweep) = (flag(&mut i), flag(&mut i));
                let (ex, ey) = (ox + read(&mut i), oy + read(&mut i));
                steps.extend(arc((x, y), (ex, ey), rx, ry, rotation, large, sweep));
                (x, y) = (ex, ey);
            },
            b'Z' => {
                steps.push(Step::Close);
                (x, y) = (sx, sy);
            },
            other => panic!("path command {}", other as char),
        }
        last_control = control;
    }
    steps
}

/// An SVG arc as cubics, by the endpoint-to-centre conversion in the SVG spec (F.6.5).
fn arc(from: (f32, f32), to: (f32, f32), rx: f32, ry: f32, rotation: f32, large: bool, sweep: bool) -> Vec<Step> {
    let (x1, y1, x2, y2) = (from.0 as f64, from.1 as f64, to.0 as f64, to.1 as f64);
    let (mut rx, mut ry) = ((rx as f64).abs(), (ry as f64).abs());
    if rx == 0.0 || ry == 0.0 || (x1 == x2 && y1 == y2) {
        return vec![Step::Line(to.0, to.1)];
    }
    let phi = (rotation as f64).to_radians();
    let (sin, cos) = phi.sin_cos();
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let (x1p, y1p) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let (cxp, cyp) = (coef * rx * y1p / ry, -coef * ry * x1p / rx);
    let (cx, cy) = (cos * cxp - sin * cyp + (x1 + x2) / 2.0, sin * cxp + cos * cyp + (y1 + y2) / 2.0);
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let theta = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
    if !sweep && delta > 0.0 {
        delta -= 2.0 * PI;
    } else if sweep && delta < 0.0 {
        delta += 2.0 * PI;
    }

    let pieces = (delta.abs() / (PI / 2.0)).ceil().max(1.0) as usize;
    let step = delta / pieces as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |t: f64| {
        let (s, c) = t.sin_cos();
        (cx + rx * c * cos - ry * s * sin, cy + rx * c * sin + ry * s * cos)
    };
    let derivative = |t: f64| {
        let (s, c) = t.sin_cos();
        (-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
    };
    (0..pieces)
        .map(|n| {
            let (t0, t1) = (theta + step * n as f64, theta + step * (n + 1) as f64);
            let (p0, p1) = (point(t0), point(t1));
            let (d0, d1) = (derivative(t0), derivative(t1));
            Step::Cubic(
                (p0.0 + k * d0.0) as f32,
                (p0.1 + k * d0.1) as f32,
                (p1.0 - k * d1.0) as f32,
                (p1.1 - k * d1.1) as f32,
                if n + 1 == pieces { to.0 } else { p1.0 as f32 },
                if n + 1 == pieces { to.1 } else { p1.1 as f32 },
            )
        })
        .collect()
}
