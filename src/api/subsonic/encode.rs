// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::{Map, Value};

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {},
            c => out.push(c),
        }
    }
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// An element: plain values become attributes, `value` its text, and
/// objects and lists its children, one element per list item.
fn element(name: &str, fields: &Map<String, Value>, out: &mut String) {
    out.push('<');
    out.push_str(name);

    for (k, v) in fields {
        if k == "value" {
            continue;
        }
        if let Some(s) = scalar(v) {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            escape(&s, out);
            out.push('"');
        }
    }

    let text = fields.get("value").and_then(scalar);

    let children: Vec<(&String, &Value)> =
        fields.iter().filter(|(k, v)| *k != "value" && matches!(v, Value::Object(_) | Value::Array(_))).collect();

    if text.is_none() && children.is_empty() {
        out.push_str("/>");
        return;
    }

    out.push('>');

    if let Some(t) = text {
        escape(&t, out);
    }

    for (k, v) in children {
        match v {
            Value::Object(m) => element(k, m, out),
            Value::Array(items) => {
                for item in items {
                    match item {
                        Value::Object(m) => element(k, m, out),
                        other => {
                            if let Some(s) = scalar(other) {
                                out.push('<');
                                out.push_str(k);
                                out.push('>');
                                escape(&s, out);
                                out.push_str("</");
                                out.push_str(k);
                                out.push('>');
                            }
                        },
                    }
                }
            },
            _ => {},
        }
    }

    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

pub fn xml(root: &str, fields: &Map<String, Value>) -> String {
    let mut out = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let mut fields = fields.clone();
    fields.insert("xmlns".into(), Value::String("http://subsonic.org/restapi".into()));
    element(root, &fields, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn attributes_children_and_text() {
        let v = json!({
            "status": "ok",
            "genres": { "genre": [{ "value": "Rock & Roll", "songCount": 3 }] },
            "song": { "id": "tr-1", "replayGain": { "trackGain": -6.5 }, "releaseTypes": ["Album"] },
        });

        let out = xml("r", v.as_object().unwrap());
        assert!(out.contains(r#"<genre songCount="3">Rock &amp; Roll</genre>"#), "{out}");
        assert!(out.contains(r#"<replayGain trackGain="-6.5"/>"#), "{out}");
        assert!(out.contains(r#"<releaseTypes>Album</releaseTypes>"#), "{out}");
    }
}
