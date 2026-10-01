//! fzf: --bind options in FZF_DEFAULT_OPTS / FZF_DEFAULT_OPTS_FILE.
use super::{default_group, humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_prefixed;
use crate::model::{Binding, Change};
use std::path::Path;

const DEFAULTS: &str = include_str!("../defaults/fzf.txt");
const EVENTS: &[&str] = &["start", "load", "result", "resize", "focus", "one", "zero", "change", "backward-eof", "jump", "jump-cancel", "click-header", "multi"];

pub struct Fzf;

impl Source for Fzf {
    fn app(&self) -> &'static str { "fzf" }
    fn binary(&self) -> &'static str { "fzf" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_prefixed(k, '-')], &|_| String::new());
        let mut opts = env.var("FZF_DEFAULT_OPTS").unwrap_or("").to_string();
        if let Some(t) = env.var("FZF_DEFAULT_OPTS_FILE").and_then(|f| env.read(Path::new(f))) {
            opts.push(' ');
            opts.push_str(&t);
        }
        let binds = binds(&opts);
        let live = if binds.is_empty() { Ok(None) } else {
            Ok(Some(binds.iter().filter_map(|b| {
                let (key, actions) = b.split_once(':')?;
                if EVENTS.contains(&key) { return None; }
                let first = split_top(actions, '+').into_iter().next()?;
                let name = first.split(['(', ':']).next().unwrap_or(&first).to_string();
                let desc = humanize(&name);
                Some(Change::Bind(Binding::new(&default_group(&defaults, &desc), vec![parse_prefixed(key, '-')], &desc)))
            }).collect()))
        };
        layer(defaults, live, true).into_loaded("fzf")
    }
}

fn binds(opts: &str) -> Vec<String> {
    let words = shell_words(opts);
    let mut out = Vec::new();
    let mut it = words.iter();
    while let Some(w) = it.next() {
        let value = if w == "--bind" { it.next().cloned() } else { w.strip_prefix("--bind=").map(String::from) };
        if let Some(v) = value { out.extend(split_top(&v, ',')); }
    }
    out
}

pub fn shell_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => if in_word { out.push(std::mem::take(&mut cur)); in_word = false; },
            '\'' => { in_word = true; for d in chars.by_ref() { if d == '\'' { break; } cur.push(d); } }
            '"' => {
                in_word = true;
                while let Some(d) = chars.next() {
                    match d { '"' => break, '\\' => if let Some(e) = chars.next() { cur.push(e) }, _ => cur.push(d) }
                }
            }
            '\\' => { in_word = true; if let Some(e) = chars.next() { cur.push(e); } }
            _ => { in_word = true; cur.push(c); }
        }
    }
    if in_word { out.push(cur); }
    out
}

/// Split on `sep` outside (), [] and {}.
pub fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            c if c == sep && depth == 0 => { out.push(std::mem::take(&mut cur)); continue; }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn words_and_top_level_split() {
        assert_eq!(shell_words(r#"--height 40% --bind 'a:b(c d),e:f' "x y" z\ w"#), vec!["--height", "40%", "--bind", "a:b(c d),e:f", "x y", "z w"]);
        assert_eq!(split_top("ctrl-y:execute(echo {} , x)+abort,ctrl-/:toggle-preview", ','), vec!["ctrl-y:execute(echo {} , x)+abort", "ctrl-/:toggle-preview"]);
    }

    #[test]
    fn binds_from_env_and_file() {
        let dir = std::env::temp_dir().join("cst-fzf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("opts"), "--bind ctrl-a:select-all\n").unwrap();
        let mut env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new()));
        env.vars.insert("FZF_DEFAULT_OPTS".into(),
            "--height 40% --bind 'ctrl-y:execute-silent(echo {} | wl-copy)+abort,ctrl-/:toggle-preview' --bind=alt-j:down,start:reload(ls)".into());
        env.vars.insert("FZF_DEFAULT_OPTS_FILE".into(), dir.join("opts").display().to_string());
        let l = Fzf.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "FZF · CUSTOM", "Execute silent"), vec!["Ctrl+Y"]);
        assert_eq!(keys_of(&l, "FZF · CUSTOM", "Toggle preview"), vec!["Ctrl+/"]);
        assert_eq!(keys_of(&l, "FZF · CUSTOM", "Select all"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, "FZF · NAVIGATION", "Down"), vec!["↓", "Ctrl+J", "Ctrl+N", "Alt+J"]);
        assert!(!l.sections.iter().flat_map(|s| &s.rows).any(|r| r.desc == "Reload"));    // start event skipped
        assert!(keys_of(&l, "FZF · EDITING", "Beginning of line").is_empty());            // ctrl-a rebound
    }

    #[test]
    fn no_bind_is_defaults() {
        let l = Fzf.load(&Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new())));
        assert_eq!(l.origin, Origin::Defaults);
    }
}
