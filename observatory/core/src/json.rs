//! Minimal JSON reader/writer for scenes, metadata and checkpoints.

use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn obj() -> Json {
        Json::Obj(BTreeMap::new())
    }
    pub fn set(&mut self, k: &str, v: impl Into<Json>) -> &mut Self {
        if let Json::Obj(m) = self {
            m.insert(k.to_string(), v.into());
        }
        self
    }
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(k),
            _ => None,
        }
    }
    pub fn num(&self, k: &str) -> Option<f64> {
        match self.get(k) {
            Some(Json::Num(n)) => Some(*n),
            Some(Json::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
    pub fn str(&self, k: &str) -> Option<&str> {
        match self.get(k) {
            Some(Json::Str(s)) => Some(s),
            _ => None,
        }
    }
    pub fn bool(&self, k: &str) -> Option<bool> {
        match self.get(k) {
            Some(Json::Bool(b)) => Some(*b),
            Some(Json::Num(n)) => Some(*n != 0.0),
            _ => None,
        }
    }
    pub fn arr(&self, k: &str) -> Option<&[Json]> {
        match self.get(k) {
            Some(Json::Arr(a)) => Some(a),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        if let Json::Num(n) = self { Some(*n) } else { None }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }

    pub fn dump(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, 0);
        s
    }

    fn write(&self, s: &mut String, ind: usize) {
        match self {
            Json::Null => s.push_str("null"),
            Json::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => {
                if n.is_finite() {
                    if n.fract() == 0.0 && n.abs() < 1e15 { let _ = write!(s, "{}", *n as i64); } else { let _ = write!(s, "{}", n); }
                } else {
                    s.push_str("null");
                }
            }
            Json::Str(t) => {
                s.push('"');
                for c in t.chars() {
                    match c {
                        '"' => s.push_str("\\\""),
                        '\\' => s.push_str("\\\\"),
                        '\n' => s.push_str("\\n"),
                        c if (c as u32) < 0x20 => { let _ = write!(s, "\\u{:04x}", c as u32); }
                        c => s.push(c),
                    }
                }
                s.push('"');
            }
            Json::Arr(a) => {
                let flat = a.iter().all(|v| matches!(v, Json::Num(_) | Json::Str(_) | Json::Bool(_)));
                s.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 { s.push_str(if flat { ", " } else { "," }); }
                    if !flat { s.push('\n'); s.push_str(&"  ".repeat(ind + 1)); }
                    v.write(s, ind + 1);
                }
                if !flat && !a.is_empty() { s.push('\n'); s.push_str(&"  ".repeat(ind)); }
                s.push(']');
            }
            Json::Obj(m) => {
                s.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 { s.push(','); }
                    s.push('\n');
                    s.push_str(&"  ".repeat(ind + 1));
                    Json::Str(k.clone()).write(s, 0);
                    s.push_str(": ");
                    v.write(s, ind + 1);
                }
                if !m.is_empty() { s.push('\n'); s.push_str(&"  ".repeat(ind)); }
                s.push('}');
            }
        }
    }

    pub fn parse(src: &str) -> Result<Json, String> {
        let mut p = Parser { b: src.as_bytes(), i: 0 };
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() { return Err(format!("trailing data at {}", p.i)); }
        Ok(v)
    }
}

impl From<f64> for Json { fn from(v: f64) -> Json { Json::Num(v) } }
impl From<u64> for Json { fn from(v: u64) -> Json { Json::Num(v as f64) } }
impl From<u32> for Json { fn from(v: u32) -> Json { Json::Num(v as f64) } }
impl From<usize> for Json { fn from(v: usize) -> Json { Json::Num(v as f64) } }
impl From<i64> for Json { fn from(v: i64) -> Json { Json::Num(v as f64) } }
impl From<bool> for Json { fn from(v: bool) -> Json { Json::Bool(v) } }
impl From<&str> for Json { fn from(v: &str) -> Json { Json::Str(v.to_string()) } }
impl From<String> for Json { fn from(v: String) -> Json { Json::Str(v) } }
impl From<Vec<Json>> for Json { fn from(v: Vec<Json>) -> Json { Json::Arr(v) } }

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() { self.i += 1; }
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        let c = *self.b.get(self.i).ok_or("unexpected end")?;
        match c {
            b'{' => {
                self.i += 1;
                let mut m = BTreeMap::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') { self.i += 1; return Ok(Json::Obj(m)); }
                loop {
                    self.ws();
                    let k = match self.value()? { Json::Str(s) => s, _ => return Err("object key".into()) };
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') { return Err(format!("':' expected at {}", self.i)); }
                    self.i += 1;
                    let v = self.value()?;
                    m.insert(k, v);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => { self.i += 1; return Ok(Json::Obj(m)); }
                        _ => return Err(format!("',' or '}}' expected at {}", self.i)),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') { self.i += 1; return Ok(Json::Arr(a)); }
                loop {
                    a.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => { self.i += 1; return Ok(Json::Arr(a)); }
                        _ => return Err(format!("',' or ']' expected at {}", self.i)),
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut s = String::new();
                loop {
                    let c = *self.b.get(self.i).ok_or("unterminated string")?;
                    self.i += 1;
                    match c {
                        b'"' => return Ok(Json::Str(s)),
                        b'\\' => {
                            let e = *self.b.get(self.i).ok_or("bad escape")?;
                            self.i += 1;
                            match e {
                                b'n' => s.push('\n'),
                                b't' => s.push('\t'),
                                b'u' => {
                                    let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|_| "bad \\u")?;
                                    s.push(char::from_u32(u32::from_str_radix(h, 16).map_err(|_| "bad \\u")?).unwrap_or('?'));
                                    self.i += 4;
                                }
                                o => s.push(o as char),
                            }
                        }
                        _ => {
                            // copy UTF-8 continuation bytes verbatim
                            let start = self.i - 1;
                            let mut end = self.i;
                            while end < self.b.len() && (self.b[end] & 0xC0) == 0x80 { end += 1; }
                            s.push_str(std::str::from_utf8(&self.b[start..end]).map_err(|_| "utf8")?);
                            self.i = end;
                        }
                    }
                }
            }
            b't' if self.b[self.i..].starts_with(b"true") => { self.i += 4; Ok(Json::Bool(true)) }
            b'f' if self.b[self.i..].starts_with(b"false") => { self.i += 5; Ok(Json::Bool(false)) }
            b'n' if self.b[self.i..].starts_with(b"null") => { self.i += 4; Ok(Json::Null) }
            _ => {
                let start = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') { self.i += 1; }
                let t = std::str::from_utf8(&self.b[start..self.i]).unwrap();
                t.parse::<f64>().map(Json::Num).map_err(|_| format!("bad number '{t}' at {start}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let src = r#"{"a": 1.5, "b": [1, 2, 3], "c": {"d": "x\"y", "e": true, "f": null}, "g": "β ≤ 3√3"}"#;
        let v = Json::parse(src).unwrap();
        assert_eq!(Json::parse(&v.dump()).unwrap(), v);
        assert_eq!(v.get("c").unwrap().str("d"), Some("x\"y"));
        assert_eq!(v.str("g"), Some("β ≤ 3√3"));
    }
}
