//! Minimal JSON builder (no external dependency).

use std::fmt::{self, Write};

/// A JSON value.
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

impl Json {
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }

    /// `Some(v)` -> value, `None` -> `null`.
    pub fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> Json) -> Json {
        v.map(f).unwrap_or(Json::Null)
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) if n.is_finite() => {
                let _ = write!(out, "{}", n);
            }
            Json::Num(_) => out.push_str("null"),
            Json::Str(s) => write_str(out, s),
            Json::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.write(&mut out);
        f.write_str(&out)
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // "<" is escaped so the JSON can never close a tag if it is
            // ever embedded in HTML
            '<' => out.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_strings() {
        let j = Json::Obj(vec![("a", Json::str("x\"y\\z\n<\u{1}é"))]);
        assert_eq!(j.to_string(), r#"{"a":"x\"y\\z\n\u003c\u0001é"}"#);
    }

    #[test]
    fn numbers_and_null() {
        let j = Json::Arr(vec![Json::Num(1.5), Json::Num(f64::NAN), Json::Null, Json::Bool(true)]);
        assert_eq!(j.to_string(), "[1.5,null,null,true]");
    }
}
