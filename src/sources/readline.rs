//! readline: ~/.inputrc (or $INPUTRC) over GNU readline's emacs defaults —
//! the bindings python, gdb, psql and other readline programs use.
use super::bash::READLINE_DEFAULTS;
use super::shell::readline_bind;
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_term, Term};
use crate::model::{Binding, Change};
use std::path::{Path, PathBuf};

pub struct Readline;

fn inputrc(env: &Env) -> PathBuf {
    env.var("INPUTRC").map(PathBuf::from).unwrap_or_else(|| env.home.join(".inputrc"))
}

impl Source for Readline {
    fn app(&self) -> &'static str { "readline" }
    fn binary(&self) -> &'static str { "" }
    fn installed(&self, env: &Env) -> bool { inputrc(env).is_file() }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(READLINE_DEFAULTS, &|k| parse_term(k, Term::Readline), &|g| g.to_lowercase());
        let path = inputrc(env);
        let live = if !path.is_file() { Ok(None) } else {
            let mut st = St { vi: false, keymap: "emacs".into(), conds: vec![], changes: vec![] };
            parse(env, &path, 0, &mut st).map(|_| Some(st.changes))
        };
        layer(defaults, live, true).into_loaded("readline")
    }
}

struct St { vi: bool, keymap: String, conds: Vec<bool>, changes: Vec<Change> }

fn group(keymap: &str) -> &'static str {
    match keymap {
        k if k.starts_with("emacs") => "Emacs",
        "vi-insert" => "Vi insert",
        _ => "Vi normal",
    }
}

fn parse(env: &Env, path: &Path, depth: u8, st: &mut St) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = env.read(path).ok_or(format!("{name}: unreadable"))?;
    let dir = path.parent().unwrap_or(Path::new("/"));
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(cond) = line.strip_prefix("$if") {
            let cond = cond.trim();
            let ok = if let Some(m) = cond.strip_prefix("mode=") { (m.trim() == "vi") == st.vi }
                else { cond.starts_with("term") || cond.starts_with("version") };
            st.conds.push(ok);
            continue;
        }
        if line.starts_with("$else") { if let Some(c) = st.conds.last_mut() { *c = !*c; } continue; }
        if line.starts_with("$endif") { st.conds.pop(); continue; }
        if !st.conds.iter().all(|c| *c) { continue; }
        if let Some(p) = line.strip_prefix("$include") {
            if depth < 8 { let _ = parse(env, &env.expand(p.trim(), dir), depth + 1, st); }
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() == Some(&"set") {
            match (words.get(1).copied(), words.get(2).copied()) {
                (Some("editing-mode"), Some(m)) => {
                    st.vi = m == "vi";
                    st.keymap = if st.vi { "vi-insert".into() } else { "emacs".into() };
                }
                (Some("keymap"), Some(k)) => st.keymap = k.to_string(),
                _ => {}
            }
            continue;
        }
        if let Some((seq, desc)) = readline_bind(line) {
            let g = group(&st.keymap);
            st.changes.push(Change::Bind(Binding { scope: g.to_lowercase(), group: g.into(), seq, desc, id: String::new() }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn home() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/readline/home") }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn inputrc_over_defaults() {
        let env = Env::test(&home(), FakeRunner(HashMap::new()));
        assert!(Readline.installed(&env));
        let l = Readline.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        let e = "READLINE · EMACS";
        assert_eq!(keys_of(&l, e, "Re read init file"), vec!["Ctrl+X › Ctrl+R"]);
        assert_eq!(keys_of(&l, e, "Delete line"), vec!["Ctrl+K"]);               // Control-k replaced the default
        assert!(keys_of(&l, e, "Delete to end of line").is_empty());
        assert!(keys_of(&l, e, "Nothing here").is_empty());                      // $if mode=vi
        assert!(keys_of(&l, e, "Macro: bash only").is_empty());                  // $if Bash
        assert_eq!(keys_of(&l, e, "Macro: not bash"), vec!["Ctrl+X › s"]);      // $else
        assert!(keys_of(&l, e, "Back one word").contains(&"Ctrl+←".to_string())); // $if term=
        assert_eq!(keys_of(&l, e, "Macro: echo extra"), vec!["Ctrl+X › e"]);    // $include
        assert_eq!(keys_of(&l, "READLINE · VI NORMAL", "Beginning of history"), vec!["g › g"]);
    }

    #[test]
    fn not_installed_without_inputrc() {
        let env = Env::test(&home().join("nope"), FakeRunner(HashMap::new()));
        assert!(!Readline.installed(&env));
        let mut env = Env::test(&home().join("nope"), FakeRunner(HashMap::new()));
        env.vars.insert("INPUTRC".into(), home().join(".inputrc").display().to_string());
        assert!(Readline.installed(&env));
        env.vars.insert("INPUTRC".into(), String::new());
        assert!(!Readline.installed(&env));
    }

    #[test]
    fn include_loop_terminates() {
        let dir = std::env::temp_dir().join("cst-inputrc-loop");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".inputrc"), "$include ~/.inputrc\n\"\\C-xz\": undo\n").unwrap();
        let l = Readline.load(&Env::test(&dir, FakeRunner(HashMap::new())));
        assert!(keys_of(&l, "READLINE · EMACS", "Undo").contains(&"Ctrl+X › z".to_string()));
    }
}
