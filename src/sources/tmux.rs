//! tmux: the running server's `list-keys`, or ~/.tmux.conf over the
//! built-in defaults. Prefix-table keys are shown after each prefix.
use super::{describe, humanize, layer, parse_defaults, Env, Layered, Loaded, Source};
use crate::keys::{parse_prefixed, Seq};
use crate::model::{Binding, Change};
use std::path::Path;

const DEFAULTS: &str = include_str!("../defaults/tmux.txt");

const ALIASES: &[(&str, &str)] = &[
    ("bind", "bind-key"), ("unbind", "unbind-key"), ("set", "set-option"), ("source", "source-file"),
    ("send", "send-keys"), ("splitw", "split-window"), ("neww", "new-window"), ("new", "new-session"),
    ("killp", "kill-pane"), ("killw", "kill-window"), ("selectp", "select-pane"), ("selectw", "select-window"),
    ("resizep", "resize-pane"), ("swapp", "swap-pane"), ("swapw", "swap-window"), ("detach", "detach-client"),
    ("run", "run-shell"), ("display", "display-message"), ("last", "last-window"), ("next", "next-window"),
    ("prev", "previous-window"), ("lastp", "last-pane"), ("confirm", "confirm-before"), ("if", "if-shell"),
    ("switchc", "switch-client"), ("renamew", "rename-window"), ("rename", "rename-session"),
];

const ACTIONS: &[(&str, &str)] = &[
    ("new-window", "New window"), ("new-session", "New session"), ("split-window -h", "Split right"),
    ("split-window -v", "Split down"), ("split-window", "Split down"), ("kill-pane", "Close pane"),
    ("kill-window", "Close window"), ("kill-session", "Kill session"), ("detach-client", "Detach"),
    ("select-pane -L", "Focus left"), ("select-pane -R", "Focus right"), ("select-pane -U", "Focus up"),
    ("select-pane -D", "Focus down"), ("select-pane", "Select pane"), ("last-pane", "Last pane"),
    ("select-window", "Go to window"), ("next-window", "Next window"), ("previous-window", "Previous window"),
    ("last-window", "Last window"), ("rename-window", "Rename window"), ("rename-session", "Rename session"),
    ("choose-tree -Zs", "Choose session"), ("choose-tree -Zw", "Choose window"), ("choose-tree", "Choose tree"),
    ("choose-buffer", "Choose buffer"), ("list-buffers", "List buffers"), ("paste-buffer", "Paste"),
    ("copy-mode", "Copy mode"), ("command-prompt", "Command prompt"), ("resize-pane -Z", "Zoom pane"),
    ("resize-pane -L", "Resize left"), ("resize-pane -R", "Resize right"), ("resize-pane -U", "Resize up"),
    ("resize-pane -D", "Resize down"), ("swap-pane -U", "Swap pane up"), ("swap-pane -D", "Swap pane down"),
    ("swap-window", "Move window"), ("break-pane", "Break pane to window"), ("display-panes", "Show pane numbers"),
    ("list-keys", "List keys"), ("source-file", "Reload config"), ("refresh-client", "Refresh client"),
    ("send-prefix", "Send prefix"), ("clock-mode", "Clock"), ("switch-client -l", "Last session"),
    ("switch-client -n", "Next session"), ("switch-client -p", "Previous session"), ("rotate-window", "Rotate panes"),
    ("next-layout", "Next layout"), ("select-layout", "Select layout"), ("suspend-client", "Suspend"),
    ("customize-mode", "Customize options"), ("find-window", "Find window"), ("run-shell", "Run shell command"),
    ("set-option", "Set option"), ("display-message", "Show message"), ("send-keys", "Send keys"),
    ("clear-history", "Clear history"),
];

const COPY: &[(&str, &str)] = &[
    ("cancel", "Exit copy mode"), ("copy-selection-and-cancel", "Copy and exit"),
    ("copy-pipe-and-cancel", "Copy and exit"), ("rectangle-toggle", "Rectangle selection"),
];

pub struct Tmux;

impl Source for Tmux {
    fn app(&self) -> &'static str { "tmux" }
    fn binary(&self) -> &'static str { "tmux" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_prefixed(k, '-')], &scope_of);
        let mut st = State { changes: vec![], prefix: "C-b".into(), prefix2: None, copy: vec!["copy-mode-vi", "copy-mode"] };
        // only a running server is asked: list-keys alone would start a throwaway one
        let running = env.run("tmux", &["list-sessions"]).is_some();
        let layered = if let Some(list) = running.then(|| env.run("tmux", &["list-keys"])).flatten() {
            let show = |o: &str| env.run("tmux", &["show", "-gv", o]).map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "None");
            if let Some(p) = show("prefix") { st.prefix = p; }
            st.prefix2 = show("prefix2");
            st.copy = if show("mode-keys").as_deref() == Some("vi") { vec!["copy-mode-vi"] } else { vec!["copy-mode"] };
            parse_text(env, &list, Path::new("/"), 0, &mut st);
            if let Some(notes) = env.run("tmux", &["list-keys", "-N"]) { apply_notes(&mut st, &notes); }
            layer(defaults, Ok(Some(std::mem::take(&mut st.changes))), false)
        } else {
            let conf = env.home.join(".tmux.conf");
            let live = match env.read(&conf) {
                None if !conf.exists() => Ok(None),
                None => Err(".tmux.conf: unreadable".to_string()),
                Some(text) => {
                    parse_text(env, &text, conf.parent().unwrap_or(Path::new("/")), 0, &mut st);
                    Ok(Some(std::mem::take(&mut st.changes)))
                }
            };
            layer(defaults, live, true)
        };
        expand_prefix(layered, &st).into_loaded("tmux")
    }
}

/// `list-keys` prints no notes; `list-keys -N` prints `<prefix> <key>  <note>`
/// (prefix table) or `<key>  <note>` (root). Notes become the descriptions.
fn apply_notes(st: &mut State, notes: &str) {
    for line in notes.lines() {
        let mut words = line.split_whitespace();
        let (Some(first), Some(second)) = (words.next(), words.next()) else { continue };
        let (table, key, note) = if first == st.prefix || Some(first) == st.prefix2.as_deref() {
            ("prefix", second, words.collect::<Vec<_>>().join(" "))
        } else {
            ("root", first, std::iter::once(second).chain(words).collect::<Vec<_>>().join(" "))
        };
        let seq = vec![parse_prefixed(key, '-')];
        for c in &mut st.changes {
            if let Change::Bind(b) = c && b.scope == table && b.seq == seq && !note.is_empty() {
                b.desc = note.clone();
            }
        }
    }
}

struct State { changes: Vec<Change>, prefix: String, prefix2: Option<String>, copy: Vec<&'static str> }

fn scope_of(group: &str) -> String {
    match group { "Prefix" => "prefix", "No prefix" => "root", "Copy mode" => "copy-mode-vi", g => g }.to_string()
}

fn group_of(table: &str) -> &'static str {
    match table { "prefix" => "Prefix", "root" => "No prefix", _ => "Copy mode" }
}

/// Each prefix-table binding becomes one sequence per prefix.
fn expand_prefix(mut l: Layered, st: &State) -> Layered {
    let prefixes: Vec<Seq> = std::iter::once(&st.prefix).chain(st.prefix2.as_ref())
        .map(|p| vec![parse_prefixed(p, '-')]).collect();
    l.bindings = l.bindings.into_iter().flat_map(|b| {
        if b.scope != "prefix" { return vec![b]; }
        prefixes.iter().map(|p| Binding { seq: p.iter().cloned().chain(b.seq.iter().cloned()).collect(), ..b.clone() }).collect()
    }).collect();
    l
}

/// Shell-like words: '…' literal, "…" with \ escapes, \x outside quotes,
/// # at a word start ends the line; an unquoted \; or ; is a separator (None).
pub fn words(line: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => if in_word { out.push(Some(std::mem::take(&mut cur))); in_word = false; },
            '#' if !in_word => break,
            '\'' => { in_word = true; for d in chars.by_ref() { if d == '\'' { break; } cur.push(d); } }
            '"' => {
                in_word = true;
                while let Some(d) = chars.next() {
                    match d { '"' => break, '\\' => if let Some(e) = chars.next() { cur.push(e) }, _ => cur.push(d) }
                }
            }
            '\\' => match chars.next() {
                Some(';') if !in_word && matches!(chars.peek(), None | Some(' ') | Some('\t')) => out.push(None),
                Some(e) => { in_word = true; cur.push(e); }
                None => {}
            },
            ';' if !in_word && matches!(chars.peek(), None | Some(' ') | Some('\t')) => out.push(None),
            _ => { in_word = true; cur.push(c); }
        }
    }
    if in_word { out.push(Some(cur)); }
    out
}

fn alias(cmd: &str) -> &str {
    ALIASES.iter().find(|(a, _)| *a == cmd).map(|(_, c)| *c).unwrap_or(cmd)
}

fn describe_cmd(cmd: &[String]) -> String {
    let Some(first) = cmd.first() else { return String::new() };
    let name = alias(first);
    let rest = &cmd[1..];
    if name == "confirm-before" {
        let mut i = 0;
        while i < rest.len() && rest[i].starts_with('-') { i += if rest[i] == "-p" || rest[i] == "-c" { 2 } else { 1 }; }
        return describe_cmd(&rest[i.min(rest.len())..]);
    }
    if name == "send-keys" && rest.first().map(String::as_str) == Some("-X")
        && let Some(x) = rest.get(1) {
            return COPY.iter().find(|(k, _)| k == x).map(|(_, d)| d.to_string()).unwrap_or_else(|| humanize(x));
        }
    let text = std::iter::once(name.to_string()).chain(rest.iter().cloned()).collect::<Vec<_>>().join(" ");
    describe(ACTIONS, &text).unwrap_or(text)
}

/// Lines ending in an unescaped `\` continue on the next line.
fn logical_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        match line.strip_suffix('\\') {
            Some(head) if !head.ends_with('\\') => { cur.push_str(head); cur.push(' '); }
            _ => { cur.push_str(line); out.push(std::mem::take(&mut cur)); }
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn shown(table: &str, key: &str, st: &State) -> bool {
    match table {
        "prefix" => true,
        "root" => !(key.contains("Mouse") || ["Wheel", "DoubleClick", "TripleClick", "SecondClick"].iter().any(|p| key.starts_with(p))),
        t => st.copy.contains(&t),
    }
}

fn parse_text(env: &Env, text: &str, dir: &Path, depth: u8, st: &mut State) {
    for line in logical_lines(text) {
        let toks = words(&line);
        let Some(Some(first)) = toks.first() else { continue };
        let first = alias(first);
        let mut i = 1;
        match first {
            "bind-key" | "unbind-key" => {
                let (mut table, mut note, mut all) = ("prefix".to_string(), None, false);
                while let Some(Some(f)) = toks.get(i) {
                    if !(f.starts_with('-') && f.len() > 1) { break; }
                    match f.as_str() {
                        "-n" => table = "root".into(),
                        "-T" => { i += 1; if let Some(Some(t)) = toks.get(i) { table = t.clone(); } }
                        "-N" => { i += 1; note = toks.get(i).cloned().flatten(); }
                        "-a" => all = true,
                        _ => {}
                    }
                    i += 1;
                }
                if first == "unbind-key" && all {
                    st.changes.push(Change::Clear(Some(table)));
                    continue;
                }
                // a separator token in key position is the ";" key
                let Some(key) = toks.get(i).map(|t| t.clone().unwrap_or_else(|| ";".into())) else { continue };
                if !shown(&table, &key, st) { continue; }
                let seq = vec![parse_prefixed(&key, '-')];
                if first == "unbind-key" {
                    st.changes.push(Change::Unbind { scope: table, seq });
                    continue;
                }
                let cmd: Vec<String> = toks[i + 1..].iter().map_while(|t| t.clone()).collect();
                let desc = note.unwrap_or_else(|| describe_cmd(&cmd));
                st.changes.push(Change::Bind(Binding { group: group_of(&table).into(), scope: table, seq, desc, id: String::new() }));
            }
            "set-option" => {
                let args: Vec<&String> = toks[1..].iter().map_while(|t| t.as_ref()).filter(|a| !a.starts_with('-')).collect();
                match (args.first().map(|s| s.as_str()), args.get(1)) {
                    (Some("prefix"), Some(v)) => st.prefix = v.to_string(),
                    (Some("prefix2"), Some(v)) => st.prefix2 = (v.as_str() != "None").then(|| v.to_string()),
                    _ => {}
                }
            }
            "source-file" if depth < 8 => {
                for p in toks[1..].iter().map_while(|t| t.as_ref()).filter(|a| !a.starts_with('-')) {
                    if p.contains("#{") || p.contains('$') || p.contains('*') { continue; }
                    let path = env.expand(p, dir);
                    if let Some(t) = env.read(&path) {
                        parse_text(env, &t, path.parent().unwrap_or(dir), depth + 1, st);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tmux") }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    fn all_keys(l: &Loaded) -> Vec<String> {
        l.sections.iter().flat_map(|s| &s.rows).flat_map(|r| r.alts.iter().map(seq_text)).collect()
    }

    #[test]
    fn words_quotes_escapes_comments_separators() {
        let w = words(r#"bind -N "a \"b\"" r run 'x y' \; display hi # tail"#);
        assert_eq!(w, vec![Some("bind".into()), Some("-N".into()), Some("a \"b\"".into()), Some("r".into()),
            Some("run".into()), Some("x y".into()), None, Some("display".into()), Some("hi".into())]);
    }

    #[test]
    fn config_file_over_defaults() {
        let env = Env::test(&fixtures().join("home"), FakeRunner(HashMap::new()));
        let l = Tmux.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        let p = "TMUX · PREFIX";
        assert_eq!(keys_of(&l, p, "Split down"), vec!["Ctrl+B › \"", "Ctrl+A › \"", "Ctrl+B › -", "Ctrl+A › -"]);
        assert_eq!(keys_of(&l, p, "Split right"), vec!["Ctrl+B › %", "Ctrl+A › %", "Ctrl+B › _", "Ctrl+A › _"]);
        assert!(keys_of(&l, p, "Next window").is_empty());                       // unbind n
        assert_eq!(keys_of(&l, p, "Move window"), vec!["Ctrl+B › Ctrl+Shift+H", "Ctrl+A › Ctrl+Shift+H"]);
        assert_eq!(keys_of(&l, p, "Reload the config"), vec!["Ctrl+B › r", "Ctrl+A › r"]); // -N note, replaces default r
        assert!(keys_of(&l, p, "Refresh client").is_empty());
        assert_eq!(keys_of(&l, p, "Last pane"), vec!["Ctrl+B › ;", "Ctrl+A › ;"]);     // \; as key; same desc as default ;
        assert_eq!(keys_of(&l, p, "Set option"), vec!["Ctrl+B › m", "Ctrl+A › m"]);   // continued line
        assert_eq!(keys_of(&l, p, "Kill session"), vec!["Ctrl+B › X", "Ctrl+A › X"]); // source-file
        assert_eq!(keys_of(&l, "TMUX · NO PREFIX", "Send keys"), vec!["Ctrl+L"]);
        assert_eq!(keys_of(&l, "TMUX · COPY MODE", "Begin selection"), vec!["v"]);
        assert_eq!(keys_of(&l, "TMUX · COPY MODE", "Copy and exit"), vec!["y"]);
        assert!(!all_keys(&l).iter().any(|k| k.contains("Mouse") || k.ends_with("› y")));
    }

    #[test]
    fn running_server_is_the_live_table() {
        let list = std::fs::read_to_string(fixtures().join("list-keys.txt")).unwrap();
        let env = Env::test(&fixtures().join("home"), FakeRunner(HashMap::from([
            ("tmux list-sessions".into(), "0: 1 windows (created Thu Oct  1 10:00:00 2026)\n".into()),
            ("tmux list-keys".into(), list),
            ("tmux list-keys -N".into(), std::fs::read_to_string(fixtures().join("list-keys-N.txt")).unwrap()),
            ("tmux show -gv prefix".into(), "C-b\n".into()),
            ("tmux show -gv prefix2".into(), "None\n".into()),
            ("tmux show -gv mode-keys".into(), "vi\n".into()),
        ])));
        let l = Tmux.load(&env);
        assert_eq!(l.origin, Origin::Live);
        let p = "TMUX · PREFIX";
        // -N notes are the descriptions: distinct keys don't collapse into "Command prompt"
        assert_eq!(keys_of(&l, p, "Rename current session"), vec!["Ctrl+B › $"]);
        assert_eq!(keys_of(&l, p, "Rename current window"), vec!["Ctrl+B › ,"]);
        assert_eq!(keys_of(&l, p, "Create a new window"), vec!["Ctrl+B › c"]);
        assert!(keys_of(&l, p, "Command prompt").is_empty());
        assert_eq!(keys_of(&l, "TMUX · NO PREFIX", "Clear the screen"), vec!["Ctrl+L"]);
        assert_eq!(keys_of(&l, p, "Close pane"), vec!["Ctrl+B › x"]);
        assert_eq!(keys_of(&l, p, "Zoom pane"), vec!["Ctrl+B › z"]);
        assert_eq!(keys_of(&l, p, "Focus up"), vec!["Ctrl+B › ↑"]);
        assert!(keys_of(&l, p, "Split right").is_empty());                        // no defaults merged
        assert_eq!(keys_of(&l, "TMUX · COPY MODE", "Begin selection"), vec!["v"]);
        assert!(keys_of(&l, "TMUX · COPY MODE", "Exit copy mode").is_empty());   // emacs table hidden in vi mode
        assert!(!all_keys(&l).iter().any(|k| k.contains("Mouse")));
    }

    #[test]
    fn list_keys_unused_without_a_running_server() {
        // `tmux list-keys` would start a throwaway server (slow): only ask a running one
        let list = std::fs::read_to_string(fixtures().join("list-keys.txt")).unwrap();
        let env = Env::test(&fixtures().join("home"), FakeRunner(HashMap::from([("tmux list-keys".into(), list)])));
        let l = Tmux.load(&env);
        assert_eq!(l.origin, Origin::Mixed);   // from ~/.tmux.conf over defaults
        assert_eq!(keys_of(&l, "TMUX · PREFIX", "Kill session"), vec!["Ctrl+B › X", "Ctrl+A › X"]);
    }

    #[test]
    fn no_server_no_config_is_defaults() {
        let env = Env::test(&fixtures(), FakeRunner(HashMap::new()));   // fixtures/ has no .tmux.conf
        let l = Tmux.load(&env);
        assert_eq!(l.origin, Origin::Defaults);
        assert_eq!(keys_of(&l, "TMUX · PREFIX", "New window"), vec!["Ctrl+B › c"]);
    }
}
