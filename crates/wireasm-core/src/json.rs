//! Tiny dependency-free JSON parser / writer.
//! Enough for graph files: objects, arrays, strings, numbers, bools, null.

use std::collections::BTreeMap;

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
    pub fn parse(src: &str) -> Result<Json, String> {
        let mut p = Parser {
            bytes: src.as_bytes(),
            pos: 0,
        };
        p.skip_ws();
        let v = p.value()?;
        p.skip_ws();
        if p.pos != p.bytes.len() {
            return Err(format!("unexpected trailing data at byte {}", p.pos));
        }
        Ok(v)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) if *n >= 0.0 => Some(*n as u64),
            Json::Str(s) => parse_u64(s).ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn stringify(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// Human-readable output with two-space indentation, suitable for graph
    /// files that people open in an editor or review on GitHub.
    pub fn stringify_pretty(&self) -> String {
        let mut out = String::new();
        self.write_pretty(&mut out, 0);
        out.push('\n');
        out
    }

    fn write_pretty(&self, out: &mut String, indent: usize) {
        let pad = |n: usize| "  ".repeat(n);
        match self {
            Json::Arr(a) if !a.is_empty() => {
                out.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    v.write_pretty(out, indent + 1);
                    if i + 1 < a.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad(indent));
                out.push(']');
            }
            Json::Obj(m) if !m.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in m.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    write_escaped(k, out);
                    out.push_str(": ");
                    v.write_pretty(out, indent + 1);
                    if i + 1 < m.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad(indent));
                out.push('}');
            }
            other => other.write(out),
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 9.0e15 {
                    out.push_str(&format!("{}", *n as i64));
                } else {
                    out.push_str(&format!("{}", n));
                }
            }
            Json::Str(s) => write_escaped(s, out),
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

pub fn write_escaped(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn parse_u64(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|e| format!("bad hex number '{s}': {e}"))
    } else if let Some(bin) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        u64::from_str_radix(bin, 2).map_err(|e| format!("bad binary number '{s}': {e}"))
    } else {
        s.parse::<u64>()
            .map_err(|e| format!("bad number '{s}': {e}"))
    }
}

pub fn esc(s: &str) -> String {
    let mut out = String::new();
    write_escaped(s, &mut out);
    out
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected '{}' at byte {}, found {:?}",
                b as char,
                self.pos,
                self.peek().map(|c| c as char)
            ))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => {
                self.literal("true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(Json::Bool(false))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(Json::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            other => Err(format!(
                "unexpected {:?} at byte {}",
                other.map(|c| c as char),
                self.pos
            )),
        }
    }

    fn literal(&mut self, s: &str) -> Result<(), String> {
        if self.bytes[self.pos..].starts_with(s.as_bytes()) {
            self.pos += s.len();
            Ok(())
        } else {
            Err(format!("expected '{s}' at byte {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let txt = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
        txt.parse::<f64>()
            .map(Json::Num)
            .map_err(|e| format!("bad number '{txt}': {e}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let b = self
                .bump()
                .ok_or_else(|| "unterminated string".to_string())?;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.bump().ok_or_else(|| "bad escape".to_string())?;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut code = 0u32;
                            for _ in 0..4 {
                                let h = self.bump().ok_or_else(|| "bad \\u escape".to_string())?;
                                let d = (h as char)
                                    .to_digit(16)
                                    .ok_or_else(|| "bad \\u escape".to_string())?;
                                code = code * 16 + d;
                            }
                            if (0xD800..=0xDBFF).contains(&code) {
                                let save = self.pos;
                                if self.bump() == Some(b'\\') && self.bump() == Some(b'u') {
                                    let mut low = 0u32;
                                    for _ in 0..4 {
                                        let h = self
                                            .bump()
                                            .ok_or_else(|| "bad surrogate pair".to_string())?;
                                        low = low * 16
                                            + (h as char)
                                                .to_digit(16)
                                                .ok_or_else(|| "bad surrogate pair".to_string())?;
                                    }
                                    code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                } else {
                                    self.pos = save;
                                    code = 0xFFFD;
                                }
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        other => {
                            return Err(format!("unknown escape \\{}", other as char));
                        }
                    }
                }
                b if b < 0x80 => out.push(b as char),
                _ => {
                    // Re-decode UTF-8 from the start byte.
                    self.pos -= 1;
                    let rest = &self.bytes[self.pos..];
                    let mut len = 1;
                    if b & 0xE0 == 0xC0 {
                        len = 2;
                    } else if b & 0xF0 == 0xE0 {
                        len = 3;
                    } else if b & 0xF8 == 0xF0 {
                        len = 4;
                    }
                    let chunk = rest
                        .get(..len)
                        .ok_or_else(|| "truncated utf-8".to_string())?;
                    let s = std::str::from_utf8(chunk)
                        .map_err(|e| format!("bad utf-8 in string: {e}"))?;
                    out.push_str(s);
                    self.pos += len;
                }
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut out = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(out));
        }
        loop {
            out.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(Json::Arr(out)),
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut out = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(out));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let val = self.value()?;
            out.insert(key, val);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(Json::Obj(out)),
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let src = r#"{"a":[1,2,{"b":"x\ny"}],"c":true,"d":null,"e":-3.5}"#;
        let j = Json::parse(src).unwrap();
        assert_eq!(j.get("c").unwrap().as_bool(), Some(true));
        assert_eq!(
            j.get("a").unwrap(),
            &Json::Arr(vec![
                Json::Num(1.0),
                Json::Num(2.0),
                Json::Obj(BTreeMap::from([(
                    "b".to_string(),
                    Json::Str("x\ny".to_string())
                )]))
            ])
        );
        let back = Json::parse(&j.stringify()).unwrap();
        assert_eq!(j, back);
    }

    #[test]
    fn unicode() {
        let j = Json::parse(r#""Привет \u00e9""#).unwrap();
        assert_eq!(j.as_str(), Some("Привет é"));
    }

    #[test]
    fn nums() {
        assert_eq!(parse_u64("0x2a").unwrap(), 42);
        assert_eq!(parse_u64("0b101010").unwrap(), 42);
        assert!(parse_u64("-1").is_err());
    }
}
