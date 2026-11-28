// SPDX-License-Identifier: AGPL-3.0-or-later

//! The part of CBOR that CTAP2 uses: integers, byte and text strings, arrays, maps and simple
//! values, written in CTAP2's canonical form.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed CBOR")
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl From<&[u8]> for Value {
    fn from(value: &[u8]) -> Self {
        Self::Bytes(value.to_vec())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// A map from `(key, value)` pairs.
pub fn map<K: Into<Value>, V: Into<Value>>(entries: impl IntoIterator<Item = (K, V)>) -> Value {
    Value::Map(entries.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
}

/// Nesting deeper than any CTAP2 message, so a hostile device can't exhaust the stack.
const DEPTH: usize = 16;

impl Value {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Int(n) if *n >= 0 => head(out, 0, *n as u64),
            Self::Int(n) => head(out, 1, !(*n as u64)),
            Self::Bytes(bytes) => {
                head(out, 2, bytes.len() as u64);
                out.extend_from_slice(bytes);
            },
            Self::Text(text) => {
                head(out, 3, text.len() as u64);
                out.extend_from_slice(text.as_bytes());
            },
            Self::Array(items) => {
                head(out, 4, items.len() as u64);
                for item in items {
                    item.write(out);
                }
            },
            Self::Map(entries) => {
                let mut encoded: Vec<_> = entries.iter().map(|(k, v)| (k.encode(), v.encode())).collect();
                encoded.sort_by(|(a, _), (b, _)| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
                head(out, 5, encoded.len() as u64);
                for (k, v) in encoded {
                    out.extend_from_slice(&k);
                    out.extend_from_slice(&v);
                }
            },
            Self::Bool(false) => out.push(0xF4),
            Self::Bool(true) => out.push(0xF5),
            Self::Null => out.push(0xF6),
        }
    }

    /// Reads exactly one value that fills `bytes`.
    pub fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        let mut reader = Reader { bytes, at: 0 };
        let value = reader.value(0)?;
        if reader.at != bytes.len() {
            return Err(Malformed);
        }
        Ok(value)
    }

    pub fn get(&self, key: impl Into<Value>) -> Option<&Value> {
        let key = key.into();
        match self {
            Self::Map(entries) => entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let major = major << 5;
    match n {
        0..=23 => out.push(major | n as u8),
        24..=0xFF => out.extend_from_slice(&[major | 24, n as u8]),
        0x100..=0xFFFF => {
            out.push(major | 25);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        },
        0x1_0000..=0xFFFF_FFFF => {
            out.push(major | 26);
            out.extend_from_slice(&(n as u32).to_be_bytes());
        },
        _ => {
            out.push(major | 27);
            out.extend_from_slice(&n.to_be_bytes());
        },
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Malformed> {
        let end = self.at.checked_add(n).filter(|end| *end <= self.bytes.len()).ok_or(Malformed)?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn argument(&mut self, info: u8) -> Result<u64, Malformed> {
        Ok(match info {
            0..=23 => info as u64,
            24 => self.take(1)?[0] as u64,
            25 => u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| Malformed)?) as u64,
            26 => u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Malformed)?) as u64,
            27 => u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Malformed)?),
            _ => return Err(Malformed),
        })
    }

    /// A length, which can't be more than what's left to read.
    fn length(&mut self, info: u8) -> Result<usize, Malformed> {
        let n = self.argument(info)?;
        usize::try_from(n).ok().filter(|n| *n <= self.bytes.len() - self.at).ok_or(Malformed)
    }

    fn value(&mut self, depth: usize) -> Result<Value, Malformed> {
        if depth > DEPTH {
            return Err(Malformed);
        }
        let initial = self.take(1)?[0];
        let (major, info) = (initial >> 5, initial & 0x1F);
        Ok(match major {
            0 => Value::Int(i64::try_from(self.argument(info)?).map_err(|_| Malformed)?),
            1 => Value::Int(-1 - i64::try_from(self.argument(info)?).map_err(|_| Malformed)?),
            2 => {
                let n = self.length(info)?;
                Value::Bytes(self.take(n)?.to_vec())
            },
            3 => {
                let n = self.length(info)?;
                Value::Text(String::from_utf8(self.take(n)?.to_vec()).map_err(|_| Malformed)?)
            },
            4 => {
                let n = self.length(info)?;
                (0..n).map(|_| self.value(depth + 1)).collect::<Result<_, _>>().map(Value::Array)?
            },
            5 => {
                let n = self.length(info)?;
                (0..n)
                    .map(|_| Ok((self.value(depth + 1)?, self.value(depth + 1)?)))
                    .collect::<Result<_, _>>()
                    .map(Value::Map)?
            },
            7 => match info {
                20 => Value::Bool(false),
                21 => Value::Bool(true),
                22 => Value::Null,
                _ => return Err(Malformed),
            },
            _ => return Err(Malformed),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_rfc_examples() {
        assert_eq!(Value::Int(0).encode(), [0x00]);
        assert_eq!(Value::Int(23).encode(), [0x17]);
        assert_eq!(Value::Int(24).encode(), [0x18, 0x18]);
        assert_eq!(Value::Int(1000).encode(), [0x19, 0x03, 0xE8]);
        assert_eq!(Value::Int(1_000_000).encode(), [0x1A, 0x00, 0x0F, 0x42, 0x40]);
        assert_eq!(Value::Int(-1).encode(), [0x20]);
        assert_eq!(Value::Int(-100).encode(), [0x38, 0x63]);
        assert_eq!(Value::Int(-1000).encode(), [0x39, 0x03, 0xE7]);
        assert_eq!(Value::from("IETF").encode(), [0x64, 0x49, 0x45, 0x54, 0x46]);
        assert_eq!(Value::Bytes(vec![1, 2, 3, 4]).encode(), [0x44, 1, 2, 3, 4]);
        assert_eq!(Value::Bool(true).encode(), [0xF5]);
        assert_eq!(Value::Null.encode(), [0xF6]);
    }

    #[test]
    fn sorts_map_keys_canonically() {
        let key = map([(-3, Value::Null), (1, Value::Null), (-1, Value::Null), (3, Value::Null)]);
        assert_eq!(key.encode(), [0xA4, 0x01, 0xF6, 0x03, 0xF6, 0x20, 0xF6, 0x22, 0xF6]);
        let named = map([("type", 1), ("alg", 2)]);
        assert_eq!(named.encode()[1..5], [0x63, b'a', b'l', b'g']);
    }

    #[test]
    fn reads_what_it_writes() {
        let value = Value::Map(vec![
            (Value::Int(1), Value::Array(vec![Value::from("FIDO_2_0"), Value::from("FIDO_2_1")])),
            (Value::Int(3), Value::Bytes(vec![0xAA; 16])),
            (Value::Int(4), map([("up", false), ("clientPin", true)])),
            (Value::Int(5), Value::Int(1200)),
            (Value::Int(6), Value::Array(vec![Value::Int(2), Value::Int(1)])),
            (Value::Int(-7), Value::Null),
            (Value::Int(i64::MIN), Value::Int(i64::MAX)),
        ]);
        assert_eq!(Value::decode(&value.encode()).unwrap(), value);
    }

    #[test]
    fn turns_away_malformed_input() {
        assert_eq!(Value::decode(&[]), Err(Malformed));
        assert_eq!(Value::decode(&[0x00, 0x00]), Err(Malformed));
        assert_eq!(Value::decode(&[0x5A, 0xFF, 0xFF, 0xFF, 0xFF]), Err(Malformed));
        assert_eq!(Value::decode(&[0x9B, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), Err(Malformed));
        assert_eq!(Value::decode(&[0x1B, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), Err(Malformed));
        assert_eq!(Value::decode(&[0x62, 0xFF, 0xFE]), Err(Malformed));
        assert_eq!(Value::decode(&[0xC0, 0x00]), Err(Malformed));
        assert_eq!(Value::decode(&[0xF9, 0x00, 0x00]), Err(Malformed));
        assert_eq!(Value::decode(&[0x81; 64]), Err(Malformed));
    }
}
