//! fish: the live `bind` table (fish 4 `ctrl-x` and fish 3 `\cx` notations).
use super::shell::{widget_desc, DROP};
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{combo, key_name, parse_prefixed, parse_term, Seq, Term};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/fish.txt");
/// fish installs its key bindings only during interactive startup, so set
/// them up explicitly (configured preset, then the user's), then list them.
pub const FISH_SCRIPT: &str = "set -q fish_key_bindings; or set fish_key_bindings fish_default_key_bindings; eval $fish_key_bindings 2>/dev/null; functions -q fish_user_key_bindings; and fish_user_key_bindings 2>/dev/null; bind";

pub struct Fish;

impl Source for Fish {
    fn app(&self) -> &'static str { "fish" }
    fn binary(&self) -> &'static str { "fish" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &fish4_key, &|g| g.to_lowercase());
        let live = match env.run("fish", &["-c", FISH_SCRIPT]) {
            None => Err("fish -c bind failed".to_string()),
            Some(out) => {
                let changes: Vec<Change> = out.lines().filter_map(bind_line).collect();
                if changes.is_empty() { Err("fish bind printed nothing".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("fish")
    }
}

/// Whitespace-separated words; '…' and "…" quotes removed; backslashes kept.
pub fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut cur, mut in_word, mut quote) = (String::new(), false, None);
    for c in line.chars() {
        match (c, quote) {
            ('\'' | '"', None) => { quote = Some(c); in_word = true; }
            (q, Some(open)) if q == open => quote = None,
            (c, None) if c.is_whitespace() => if in_word { out.push(std::mem::take(&mut cur)); in_word = false; },
            (c, _) => { cur.push(c); in_word = true; }
        }
    }
    if in_word { out.push(cur); }
    out
}

fn terminfo(name: &str) -> Seq {
    vec![match name {
        "dc" => vec!["Del".into()], "ic" => vec!["Ins".into()],
        "ppage" => vec!["PgUp".into()], "npage" => vec!["PgDn".into()],
        "btab" => vec!["⇧Tab".into()],
        "sleft" => combo(&["shift"], "left"), "sright" => combo(&["shift"], "right"),
        other => vec![key_name(other, false)],
    }]
}

/// fish 4 `ctrl-x,ctrl-e`: a comma separates keys unless it is itself the key
/// (`alt-,`, `ctrl-x,,`); `\X` escapes (`alt-\#`, `ctrl-\\`) are unescaped.
fn fish4_key(key: &str) -> Seq {
    let mut parts: Vec<String> = vec![String::new()];
    let mut chars = key.chars();
    while let Some(c) = chars.next() {
        let cur = parts.last_mut().unwrap();
        match c {
            '\\' => if let Some(n) = chars.next() { cur.push(n) },
            ',' if !cur.is_empty() && !cur.ends_with('-') => parts.push(String::new()),
            c => cur.push(c),
        }
    }
    parts.iter().filter(|p| !p.is_empty()).flat_map(|k| {
        let one = parse_prefixed(k, '-');
        let known = one.len() > 1 || k.chars().count() == 1 || key_name(k, false) != *k;
        if known { vec![one] } else { k.chars().map(|c| vec![key_name(&c.to_string(), false)]).collect() }
    }).collect()
}

/// fish 3 escapes (`\cx`, `\e[A`, `\x7f`, `\b`) vs fish 4 names with `\X` escapes.
fn is_fish3(key: &str) -> bool {
    key.starts_with('\\') || ["\\c", "\\e", "\\x", "\\b"].iter().any(|p| key.contains(p))
}

fn bind_line(line: &str) -> Option<Change> {
    let w = words(line);
    if w.first().map(String::as_str) != Some("bind") { return None; }
    let (mut mode, mut i, mut ti) = ("default".to_string(), 1, false);
    while let Some(f) = w.get(i) {
        match f.as_str() {
            "--preset" | "-p" | "--user" | "-s" | "--silent" => i += 1,
            "-M" | "--mode" => { mode = w.get(i + 1)?.clone(); i += 2; }
            "-m" | "--sets-mode" => i += 2,
            "-k" | "--key" => { ti = true; i += 1; }
            f if f.starts_with("--mode=") => { mode = f["--mode=".len()..].to_string(); i += 1; }
            f if f.starts_with("--sets-mode=") => i += 1,
            _ => break,
        }
    }
    let key = w.get(i)?;
    if key.is_empty() { return None; }
    let seq = if ti { terminfo(key) } else if is_fish3(key) { parse_term(key, Term::Fish3) } else { fish4_key(key) };
    let cmd: Vec<&str> = w[i + 1..].iter().flat_map(|c| c.split_whitespace()).collect();
    let func = match cmd.first() {
        Some(&"commandline") => cmd.iter().position(|c| *c == "-f").and_then(|p| cmd.get(p + 1)).copied().unwrap_or("commandline"),
        Some(f) => *f,
        None => return None,
    };
    if DROP.contains(&func) { return None; }
    Some(Change::Bind(Binding { scope: mode.clone(), group: humanize(&mode), seq, desc: widget_desc(func), id: String::new() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn load(name: Option<&str>) -> Loaded {
        let runs = name.map(|n| HashMap::from([(format!("fish -c {FISH_SCRIPT}"),
            std::fs::read_to_string(format!("{}/tests/fixtures/fish/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap())])).unwrap_or_default();
        Fish.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn words_keep_backslashes() {
        assert_eq!(words(r"bind --preset \e\[A 'commandline -f x' y"), vec!["bind", "--preset", r"\e\[A", "commandline -f x", "y"]);
    }

    #[test]
    fn fish4_notation() {
        let l = load(Some("fish4.txt"));
        assert_eq!(l.origin, Origin::Live);
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["FISH · DEFAULT", "FISH · INSERT", "FISH · VISUAL"]);
        let d = "FISH · DEFAULT";
        assert_eq!(keys_of(&l, d, "Start of line"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, d, "Edit command in $EDITOR"), vec!["Ctrl+X › Ctrl+E"]);
        assert_eq!(keys_of(&l, d, "Insert last argument"), vec!["Alt+."]);
        assert_eq!(keys_of(&l, d, "Prevd or backward word"), vec!["Alt+←"]);
        assert_eq!(keys_of(&l, d, "Cancel"), vec!["Ctrl+G"]);                      // commandline -f cancel
        assert_eq!(keys_of(&l, "FISH · INSERT", "History pager"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, "FISH · VISUAL", "Clipboard copy"), vec!["y"]);
        assert!(!l.sections.iter().flat_map(|s| &s.rows).any(|r| r.desc == "Self insert"));
    }

    fn key_text(line: &str) -> String {
        match bind_line(line) { Some(Change::Bind(b)) => seq_text(&b.seq), other => panic!("{line}: {other:?}") }
    }

    #[test]
    fn fish4_commas_and_escapes() {
        assert_eq!(key_text("bind alt-, history-search-backward"), "Alt+,");
        assert_eq!(key_text("bind ctrl-x,, history-search-backward"), "Ctrl+X › ,");
        assert_eq!(key_text("bind , history-search-backward"), ",");
        assert_eq!(key_text(r"bind alt-\# history-search-backward"), "Alt+#");
        assert_eq!(key_text(r"bind ctrl-\\ history-search-backward"), "Ctrl+\\");
        assert_eq!(key_text(r"bind \cx history-search-backward"), "Ctrl+X");      // fish 3 escapes
        assert_eq!(key_text(r"bind \b history-search-backward"), "Bksp");
    }

    #[test]
    fn fish3_notation() {
        let l = load(Some("fish3.txt"));
        let d = "FISH · DEFAULT";
        assert_eq!(keys_of(&l, d, "Start of line"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, d, "Previous command"), vec!["↑"]);
        assert_eq!(keys_of(&l, d, "Delete char"), vec!["Del"]);
        assert_eq!(keys_of(&l, d, "Edit command in $EDITOR"), vec!["Ctrl+X › Ctrl+E"]);
        assert_eq!(keys_of(&l, d, "Delete line"), vec!["d › d"]);
    }

    #[test]
    fn failure_falls_back() {
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: fish -c bind failed"));
        assert_eq!(keys_of(&l, "FISH · DEFAULT", "Complete and search"), vec!["Shift+Tab"]);
    }
}
