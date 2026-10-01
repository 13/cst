//! lazygit: the keybinding: block of config.yml (two levels) over defaults.
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/lazygit.txt");

pub struct Lazygit;

impl Source for Lazygit {
    fn app(&self) -> &'static str { "lazygit" }
    fn binary(&self) -> &'static str { "lazygit" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new());
        let path = env.config("lazygit/config.yml");
        let live = if !path.exists() { Ok(None) } else {
            env.read(&path).map(|t| Some(parse(&t))).ok_or_else(|| "config.yml: unreadable".to_string())
        };
        layer(defaults, live, true).into_loaded("lazygit")
    }
}

/// A # starts a comment unless inside quotes.
fn strip_comment(s: &str) -> &str {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (c, quote) {
            ('\'' | '"', None) => quote = Some(c),
            (q, Some(open)) if q == open => quote = None,
            ('#', None) if i == 0 || s[..i].ends_with(' ') => return &s[..i],
            _ => {}
        }
    }
    s
}

fn parse(text: &str) -> Vec<Change> {
    let mut out = Vec::new();
    let (mut inside, mut section) = (false, String::new());
    for raw in text.lines() {
        let line = strip_comment(raw).trim_end();
        if line.trim().is_empty() { continue; }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 { inside = line.trim() == "keybinding:"; continue; }
        if !inside { continue; }
        let Some((k, v)) = line.trim().split_once(':') else { continue };
        let v = v.trim().trim_matches('\'').trim_matches('"');
        if v.is_empty() { section = k.trim().to_string(); continue; }
        if section.is_empty() { continue; }
        let id = format!("{section}.{}", k.trim());
        if v == "<disabled>" { out.push(Change::Remove(id)); continue; }
        out.push(Change::Bind(Binding { scope: String::new(), group: humanize(&section), seq: parse_vim(v), desc: humanize(k.trim()), id }));
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

    #[test]
    fn keybinding_block_over_defaults() {
        let cfg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lazygit");
        let mut env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new()));
        env.vars.insert("XDG_CONFIG_HOME".into(), cfg.display().to_string());
        let l = Lazygit.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "LAZYGIT · UNIVERSAL", "Quit"), vec!["Q"]);           // replaced by id
        assert_eq!(keys_of(&l, "LAZYGIT · UNIVERSAL", "Next tab"), vec!["]"]);
        assert!(keys_of(&l, "LAZYGIT · UNIVERSAL", "Undo").is_empty());             // <disabled>
        assert_eq!(keys_of(&l, "LAZYGIT · FILES", "Commit changes"), vec!["C"]);
        assert_eq!(keys_of(&l, "LAZYGIT · UNIVERSAL", "Push files"), vec!["P"]);     // untouched default
        assert_eq!(keys_of(&l, "LAZYGIT · UNIVERSAL", "Redo"), vec!["Ctrl+Z"]);
    }
}
