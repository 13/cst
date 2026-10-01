//! yazi: [mgr] keymap / prepend_keymap / append_keymap of keymap.toml.
use super::{default_group, humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_vim, Seq};
use crate::model::{Binding, Change};
use toml::{Table, Value};

const DEFAULTS: &str = include_str!("../defaults/yazi.txt");

pub struct Yazi;

fn keys(s: &str) -> Seq { s.split_whitespace().flat_map(parse_vim).collect() }

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

impl Source for Yazi {
    fn app(&self) -> &'static str { "yazi" }
    fn binary(&self) -> &'static str { "yazi" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &keys, &|_| String::new());
        let path = env.config("yazi/keymap.toml");
        let live = if !path.exists() { Ok(None) } else {
            env.read(&path).ok_or_else(|| "keymap.toml: unreadable".to_string())
                .and_then(|t| t.parse::<Table>().map_err(|e| format!("keymap.toml: {}", e.to_string().lines().next().unwrap_or("parse error"))))
                .map(|t| Some(changes(&t, &defaults)))
        };
        layer(defaults, live, true).into_loaded("yazi")
    }
}

fn changes(t: &Table, defaults: &[Binding]) -> Vec<Change> {
    let Some(mgr) = t.get("mgr").or(t.get("manager")).and_then(Value::as_table) else { return vec![] };
    let mut out = Vec::new();
    for list in ["keymap", "prepend_keymap", "append_keymap"] {
        let Some(entries) = mgr.get(list).and_then(Value::as_array) else { continue };
        if list == "keymap" { out.push(Change::Clear(None)); }
        for e in entries.iter().filter_map(Value::as_table) {
            let seq: Seq = strings(e.get("on")).iter().flat_map(|k| parse_vim(k)).collect();
            if seq.is_empty() { continue; }
            let desc = e.get("desc").and_then(Value::as_str).map(String::from).unwrap_or_else(|| {
                humanize(strings(e.get("run")).first().and_then(|r| r.split_whitespace().next()).unwrap_or("?"))
            });
            out.push(Change::Bind(Binding::new(&default_group(defaults, &desc), seq, &desc)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    fn env(cfg: &str) -> Env {
        let mut e = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new()));
        e.vars.insert("XDG_CONFIG_HOME".into(), cfg.into());
        e
    }

    #[test]
    fn prepend_keymap_over_defaults() {
        let cfg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/yazi");
        let l = Yazi.load(&env(&cfg.display().to_string()));
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "YAZI · CUSTOM", "Drag out"), vec!["Ctrl+N"]);
        assert_eq!(keys_of(&l, "YAZI · CUSTOM", "Go to downloads"), vec!["g › d"]);
        assert_eq!(keys_of(&l, "YAZI · CUSTOM", "Plugin"), vec!["T"]);
        assert_eq!(keys_of(&l, "YAZI · OTHER", "Quit"), vec!["q"]);            // same key + desc: no duplicate
        assert_eq!(keys_of(&l, "YAZI · NAVIGATION", "Top"), vec!["g › g"]);
    }

    #[test]
    fn missing_file_is_defaults() {
        let l = Yazi.load(&env("/nonexistent"));
        assert_eq!(l.origin, Origin::Defaults);
        assert_eq!(keys_of(&l, "YAZI · TABS", "New tab"), vec!["t"]);
    }
}
