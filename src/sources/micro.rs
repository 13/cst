//! micro: ~/.config/micro/bindings.json over micro's default keys.
use super::{default_group, humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_micro;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/micro.txt");

#[derive(Clone, Debug, PartialEq)]
pub enum Json { Str(String), Obj(Vec<(String, Json)>), Other }

struct P<'a> { s: &'a [u8], i: usize, depth: usize }

impl P<'_> {
    /// Whitespace and json5 comments (`//…`, `/*…*/`).
    fn ws(&mut self) {
        loop {
            while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) { self.i += 1; }
            match (self.s.get(self.i), self.s.get(self.i + 1)) {
                (Some(b'/'), Some(b'/')) => while self.s.get(self.i).is_some_and(|c| *c != b'\n') { self.i += 1; },
                (Some(b'/'), Some(b'*')) => {
                    self.i += 2;
                    while self.i < self.s.len() && !(self.s[self.i] == b'*' && self.s.get(self.i + 1) == Some(&b'/')) { self.i += 1; }
                    self.i = (self.i + 2).min(self.s.len());
                }
                _ => return,
            }
        }
    }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.s.get(self.i) == Some(&c) { self.i += 1; Ok(()) } else { Err(format!("expected '{}' at byte {}", c as char, self.i)) }
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.s.get(self.i) {
                None => return Err("unterminated string".into()),
                Some(b'"') => { self.i += 1; break; }
                Some(b'\\') => {
                    let e = *self.s.get(self.i + 1).ok_or("unterminated string")?;
                    if e == b'u' {
                        let hex = self.s.get(self.i + 2..self.i + 6).and_then(|h| std::str::from_utf8(h).ok()).ok_or("bad \\u escape")?;
                        let c = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).unwrap_or('\u{fffd}');
                        out.extend_from_slice(c.to_string().as_bytes());
                        self.i += 6;
                        continue;
                    }
                    out.push(match e { b'n' => b'\n', b't' => b'\t', other => other });
                    self.i += 2;
                }
                Some(&c) => { out.push(c); self.i += 1; }
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        if self.depth > 64 { return Err("nested too deeply".into()); }
        match self.s.get(self.i) {
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'{') => { self.depth += 1; let o = self.object(); self.depth -= 1; Ok(Json::Obj(o?)) }
            Some(b'[') => { self.depth += 1; let a = self.array(); self.depth -= 1; a?; Ok(Json::Other) }
            Some(c) if c.is_ascii_alphanumeric() || *c == b'-' => {
                while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || b"-+.".contains(c)) { self.i += 1; }
                Ok(Json::Other)
            }
            _ => Err(format!("unexpected input at byte {}", self.i)),
        }
    }
    fn array(&mut self) -> Result<(), String> {
        self.eat(b'[')?;
        self.ws();
        if self.s.get(self.i) == Some(&b']') { self.i += 1; return Ok(()); }
        loop {
            self.value()?;
            self.ws();
            match self.s.get(self.i) { Some(b',') => self.i += 1, Some(b']') => { self.i += 1; return Ok(()); } _ => return Err("bad array".into()) }
            self.ws();
            if self.s.get(self.i) == Some(&b']') { self.i += 1; return Ok(()); }   // json5 trailing comma
        }
    }
    fn object(&mut self) -> Result<Vec<(String, Json)>, String> {
        self.eat(b'{')?;
        let mut out = Vec::new();
        self.ws();
        if self.s.get(self.i) == Some(&b'}') { self.i += 1; return Ok(out); }
        loop {
            self.ws();
            let k = self.string()?;
            self.eat(b':')?;
            out.push((k, self.value()?));
            self.ws();
            match self.s.get(self.i) { Some(b',') => self.i += 1, Some(b'}') => { self.i += 1; return Ok(out); } _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)) }
            self.ws();
            if self.s.get(self.i) == Some(&b'}') { self.i += 1; return Ok(out); }   // json5 trailing comma
        }
    }
}

/// A JSON object, json5-tolerant like micro (comments, trailing commas).
pub fn json_object(s: &str) -> Result<Vec<(String, Json)>, String> {
    let mut p = P { s: s.as_bytes(), i: 0, depth: 0 };
    let o = p.object()?;
    p.ws();
    if p.i != p.s.len() { return Err("trailing data".into()); }
    Ok(o)
}

pub struct Micro;

impl Source for Micro {
    fn app(&self) -> &'static str { "micro" }
    fn binary(&self) -> &'static str { "micro" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_micro(k)], &|_| String::new());
        let path = env.config("micro/bindings.json");
        let live = if !path.is_file() { Ok(None) } else {
            env.read(&path).ok_or_else(|| "bindings.json: unreadable".to_string())
                .and_then(|t| json_object(&t).map_err(|e| format!("bindings.json: {e}")))
                .map(|o| Some(changes(&o, &defaults)))
        };
        layer(defaults, live, true).into_loaded("micro")
    }
}

fn changes(o: &[(String, Json)], defaults: &[Binding]) -> Vec<Change> {
    let buffer = o.iter().find(|(k, _)| k == "buffer").and_then(|(_, v)| if let Json::Obj(b) = v { Some(b.as_slice()) } else { None }).unwrap_or(&[]);
    o.iter().chain(buffer).filter_map(|(key, v)| {
        let Json::Str(action) = v else { return None };
        let seq = vec![parse_micro(key)];
        let first = action.split([',', '|', '&']).next().unwrap_or("").trim();
        if first == "None" { return Some(Change::Unbind { scope: String::new(), seq }); }
        let desc = if let Some(f) = first.strip_prefix("lua:") { humanize(f.rsplit('.').next().unwrap_or(f)) }
            else if let Some(c) = first.strip_prefix("command:").or_else(|| first.strip_prefix("command-edit:")) { format!("Command: {c}") }
            else { humanize(first) };
        Some(Change::Bind(Binding::new(&default_group(defaults, &desc), seq, &desc)))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, desc: &str) -> Vec<String> {
        l.sections.iter().flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    fn env(cfg: &std::path::Path) -> Env {
        let mut e = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::new()));
        e.vars.insert("XDG_CONFIG_HOME".into(), cfg.display().to_string());
        e
    }

    #[test]
    fn json_reader() {
        let o = json_object(r#"{"a": "x\"y\u00e9", "b": {"c": "d"}, "e": [1, {"f": 2}], "g": true}"#).unwrap();
        assert_eq!(o[0], ("a".into(), Json::Str("x\"yé".into())));
        assert_eq!(o[1], ("b".into(), Json::Obj(vec![("c".into(), Json::Str("d".into()))])));
        assert_eq!((o[2].1.clone(), o[3].1.clone()), (Json::Other, Json::Other));
        // micro reads bindings.json as json5: comments and trailing commas are fine
        assert_eq!(json_object("{ // keys\n \"a\": \"b\", /* x */ }").unwrap(), vec![("a".into(), Json::Str("b".into()))]);
        assert!(json_object(r#"{"a": [1, 2,],}"#).is_ok());
        assert!(json_object(r#"{"a": "#).is_err());
        assert!(json_object(r#"{"a": }"#).is_err());
        let deep = format!("{{\"a\": {}1{}}}", "[".repeat(10_000), "]".repeat(10_000));
        assert!(json_object(&deep).is_err());                                 // depth-capped, no stack overflow
    }

    #[test]
    fn bindings_over_defaults() {
        let l = Micro.load(&env(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/micro")));
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "Undo"), vec!["Ctrl+Z", "Ctrl+Y"]);
        assert!(keys_of(&l, "Redo").is_empty());                            // its Ctrl-y now undoes
        assert_eq!(keys_of(&l, "Comment"), vec!["Alt+/"]);
        assert_eq!(keys_of(&l, "Select up"), vec!["Ctrl+Shift+↑"]);
        assert!(keys_of(&l, "Quit").is_empty());                            // Ctrl-q: None
        assert_eq!(keys_of(&l, "Command: setlocal ruler off"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, "Save as"), vec!["Alt+S"]);                  // from the buffer object
        assert!(keys_of(&l, "Unsplit").is_empty());                         // terminal mode ignored
        assert_eq!(keys_of(&l, "Move lines up"), vec!["Alt+↑"]);           // "AltUp" replaced the default "Alt-Up"
    }

    #[test]
    fn micro_bad_json_notes() {
        let dir = std::env::temp_dir().join("cst-micro-bad");
        std::fs::create_dir_all(dir.join("micro")).unwrap();
        std::fs::write(dir.join("micro/bindings.json"), "{\"Ctrl-y\": }").unwrap();
        let l = Micro.load(&env(&dir));
        assert_eq!(l.origin, Origin::Defaults);
        assert!(l.note.as_deref().unwrap().starts_with("config: bindings.json"));
    }
}
