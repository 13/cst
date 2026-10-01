//! nushell: reedline's default keybindings plus $env.config.keybindings,
//! read through `nu -c` as tab-separated lines with NUON fields.
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{combo, parse_prefixed, Combo};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/nushell.txt");
pub const NU_SCRIPT: &str = "let d = (keybindings default | each {|r| [\"d\" ($r.mode | to nuon) ($r.modifier | into string) ($r.code | into string) ($r.event | to nuon)] | str join (char tab) }); let u = ($env.config.keybindings | each {|r| [\"u\" ($r.mode | to nuon) ($r.modifier | into string) ($r.keycode | into string) ($r.event | to nuon)] | str join (char tab) }); $d | append $u | str join (char nl)";

pub struct Nushell;

impl Source for Nushell {
    fn app(&self) -> &'static str { "nushell" }
    fn binary(&self) -> &'static str { "nu" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_prefixed(k, '+')], &|g| g.to_lowercase().replace(' ', "_"));
        let live = match env.run("nu", &["-c", NU_SCRIPT]) {
            None => Err("nu keybindings failed".to_string()),
            Some(out) => {
                let changes: Vec<Change> = out.lines().flat_map(line_changes).collect();
                if changes.is_empty() { Err("nu printed no keybindings".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("nushell")
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).map(|x| x.replace("\\\"", "\"")).unwrap_or_else(|| s.to_string())
}

fn modes(nuon: &str) -> Vec<String> {
    nuon.trim().trim_start_matches('[').trim_end_matches(']').split(',')
        .map(unquote).filter(|m| !m.is_empty()).collect()
}

fn key(modifier: &str, code: &str) -> Combo {
    let mods: Vec<&str> = modifier.split(|c: char| !c.is_ascii_alphanumeric()).filter(|m| !m.is_empty() && !m.eq_ignore_ascii_case("none")).collect();
    let code = code.trim();
    let k = if let Some(c) = code.strip_prefix("Char('").and_then(|c| c.strip_suffix("')")) { c.to_string() }
        else if let Some(c) = code.strip_prefix("char_") { c.to_string() }
        else if let Some(n) = code.strip_prefix("F(").and_then(|c| c.strip_suffix(')')) { format!("F{n}") }
        else { code.to_string() };
    combo(&mods, if k == " " { "space" } else { &k })
}

const WRAPPERS: &[&str] = &["Edit", "UntilFound", "Multiple", "Menu"];

/// None = unbind (`null`).
fn event_desc(nuon: &str) -> Option<String> {
    let s = nuon.trim();
    if s.is_empty() || s == "null" { return None; }
    if s.starts_with('"') {
        let dbg = unquote(s);
        if let Some(rest) = dbg.split("Menu(\"").nth(1) {
            return Some(humanize(rest.split('"').next().unwrap_or(rest)));
        }
        return dbg.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).find(|w| !w.is_empty() && !WRAPPERS.contains(w)).map(humanize);
    }
    let field = |name: &str| s.split(&format!("{name}:")).nth(1)
        .map(|v| unquote(v.trim_start().split([',', '}', ']']).next().unwrap_or("")));
    match field("send") {
        Some(v) if v.eq_ignore_ascii_case("menu") => field("name").map(|n| humanize(&n)),
        Some(v) => Some(humanize(&v)),
        None => field("edit").map(|v| humanize(&v)),
    }
}

fn line_changes(line: &str) -> Vec<Change> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() != 5 || !(f[0] == "d" || f[0] == "u") { return vec![]; }
    let seq = vec![key(f[2], f[3])];
    let desc = event_desc(f[4]);
    modes(f[1]).into_iter().map(|mode| match &desc {
        Some(d) => Change::Bind(Binding { scope: mode.clone(), group: humanize(&mode), seq: seq.clone(), desc: d.clone(), id: String::new() }),
        None => Change::Unbind { scope: mode, seq: seq.clone() },
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn load(out: Option<String>) -> Loaded {
        let runs = out.map(|o| HashMap::from([(format!("nu -c {NU_SCRIPT}"), o)])).unwrap_or_default();
        Nushell.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn defaults_and_user_bindings() {
        let out = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nushell/keys.tsv")).unwrap();
        let l = load(Some(out));
        assert_eq!(l.origin, Origin::Live);
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["NUSHELL · EMACS", "NUSHELL · VI INSERT"]);
        let e = "NUSHELL · EMACS";
        assert_eq!(keys_of(&l, e, "Move to line start"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, e, "History menu"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, e, "Completion menu"), vec!["Tab"]);
        assert_eq!(keys_of(&l, e, "Fzf menu"), vec!["Ctrl+T"]);
        assert_eq!(keys_of(&l, "NUSHELL · VI INSERT", "Fzf menu"), vec!["Ctrl+T"]);
        assert_eq!(keys_of(&l, e, "Insertnewline"), vec!["Alt+Enter"]);
        assert!(keys_of(&l, e, "Cut word left").is_empty());                    // user null unbinds the default
        assert_eq!(keys_of(&l, "NUSHELL · VI INSERT", "Ctrl c"), vec!["Ctrl+C"]);
    }

    #[test]
    fn failure_falls_back() {
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: nu keybindings failed"));
        assert_eq!(keys_of(&l, "NUSHELL · EMACS", "Open editor"), vec!["Ctrl+O"]);
    }
}
