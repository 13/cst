//! bash: the live readline bindings (`bind -p`) and editing mode.
use super::shell::readline_bind;
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_term, Term};
use crate::model::{Binding, Change};

pub const READLINE_DEFAULTS: &str = include_str!("../defaults/readline.txt");
pub const BASH_SCRIPT: &str = "bind -p; echo ---; bind -v";

pub struct Bash;

impl Source for Bash {
    fn app(&self) -> &'static str { "bash" }
    fn binary(&self) -> &'static str { "bash" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(READLINE_DEFAULTS, &|k| parse_term(k, Term::Readline), &|g| g.to_lowercase());
        let live = match env.run("bash", &["-ic", BASH_SCRIPT]) {
            None => Err("bash -ic bind failed".to_string()),
            Some(out) => {
                let (binds, vars) = out.split_once("\n---\n").or_else(|| out.split_once("---\n")).unwrap_or((&out, ""));
                let group = if vars.lines().any(|l| l.trim() == "set editing-mode vi") { "Vi" } else { "Emacs" };
                let changes: Vec<Change> = binds.lines().filter_map(readline_bind)
                    .map(|(seq, desc)| Change::Bind(Binding { scope: group.to_lowercase(), group: group.into(), seq, desc, id: String::new() }))
                    .collect();
                if changes.is_empty() { Err("bash bind -p printed nothing".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("bash")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn load(out: Option<String>) -> Loaded {
        let runs = out.map(|o| HashMap::from([(format!("bash -ic {BASH_SCRIPT}"), o)])).unwrap_or_default();
        Bash.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn real_emacs_capture() {
        let out = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bash/emacs.txt")).unwrap();
        let l = load(Some(out));
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(l.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), vec!["BASH · EMACS"]);
        assert!(keys_of(&l, "BASH · EMACS", "Edit command in $EDITOR").contains(&"Ctrl+X › Ctrl+E".to_string()));
        assert!(keys_of(&l, "BASH · EMACS", "Forward one word").contains(&"Ctrl+→".to_string()));
        assert!(keys_of(&l, "BASH · EMACS", "Previous command").contains(&"↑".to_string()));
        assert!(!l.sections[0].rows.iter().any(|r| r.desc.contains("not bound") || r.desc == "Self insert"));
    }

    #[test]
    fn vi_mode_and_failures() {
        let l = load(Some("\"\\C-a\": beginning-of-line\n---\nset editing-mode vi\n".into()));
        assert_eq!(l.sections[0].title, "BASH · VI");
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: bash -ic bind failed"));
        assert_eq!(keys_of(&l, "BASH · EMACS", "Undo"), vec!["Ctrl+_"]);
        let l = load(Some("---\nset editing-mode emacs\n".into()));
        assert_eq!(l.note.as_deref(), Some("config: bash bind -p printed nothing"));
    }
}
