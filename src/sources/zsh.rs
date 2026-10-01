//! zsh: the live line-editor keymaps (`bindkey`), main and, in vi mode, vicmd.
use super::shell::{closing_quote, widget_desc, DROP};
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_term, Term};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/zsh.txt");
pub const ZSH_SCRIPT: &str = "bindkey -lL main; print -- ---; bindkey -M main; print -- ---; bindkey -M vicmd";

pub struct Zsh;

impl Source for Zsh {
    fn app(&self) -> &'static str { "zsh" }
    fn binary(&self) -> &'static str { "zsh" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_term(k, Term::Zsh), &|g| g.to_lowercase());
        let live = match env.run("zsh", &["-ic", ZSH_SCRIPT]) {
            None => Err("zsh -ic bindkey failed".to_string()),
            Some(out) => {
                let changes = parse(&out);
                if changes.is_empty() { Err("zsh bindkey printed nothing".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("zsh")
    }
}

/// `"keys" widget` → (keys, description); None for ranges, dropped widgets, other lines.
fn bind_line(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix('"')?;
    let end = closing_quote(rest)?;
    let after = &rest[end + 1..];
    if after.starts_with('-') { return None; } // "^A"-"^C" range
    let target = after.trim();
    if let Some(m) = target.strip_prefix('"') {
        return Some((rest[..end].to_string(), format!("Macro: {}", m.strip_suffix('"').unwrap_or(m))));
    }
    let widget = target.split_whitespace().next()?;
    if DROP.contains(&widget) { return None; }
    Some((rest[..end].to_string(), widget_desc(widget)))
}

fn parse(out: &str) -> Vec<Change> {
    let mut part = 0;
    let mut vi = false;
    let mut changes = Vec::new();
    for line in out.lines() {
        if line.trim() == "---" { part += 1; continue; }
        match part {
            0 => if line.contains("bindkey -A") && line.trim_end().ends_with(" main") { vi = line.contains(" viins ") },
            1 | 2 => if let Some((keys, desc)) = bind_line(line) {
                if part == 2 && !vi { continue; }
                let (scope, group) = match (part, vi) { (2, _) => ("vicmd", "Normal"), (_, true) => ("main", "Insert"), _ => ("main", "Emacs") };
                changes.push(Change::Bind(Binding { scope: scope.into(), group: group.into(), seq: parse_term(&keys, Term::Zsh), desc, id: String::new() }));
            },
            _ => {}
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/zsh/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn load(out: Option<String>) -> Loaded {
        let runs = out.map(|o| HashMap::from([(format!("zsh -ic {ZSH_SCRIPT}"), o)])).unwrap_or_default();
        Zsh.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn real_vi_mode_capture() {
        let l = load(Some(fixture("vi.txt")));
        assert_eq!(l.origin, Origin::Live);
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["ZSH · INSERT", "ZSH · NORMAL"]);
        assert!(keys_of(&l, "ZSH · INSERT", "Previous command").contains(&"↑".to_string()));
        assert!(keys_of(&l, "ZSH · NORMAL", "Redo").contains(&"Ctrl+R".to_string()));
        let descs: Vec<&str> = l.sections.iter().flat_map(|s| &s.rows).map(|r| r.desc.as_str()).collect();
        assert!(!descs.iter().any(|d| *d == "Self insert" || *d == "Bracketed paste"));
    }

    #[test]
    fn real_emacs_capture_with_fzf_and_p10k() {
        let l = load(Some(fixture("real.txt")));
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["ZSH · EMACS"]);
        let e = "ZSH · EMACS";
        assert!(keys_of(&l, e, "Search history (fzf)").contains(&"Ctrl+R".to_string()));
        assert!(keys_of(&l, e, "Insert file (fzf)").contains(&"Ctrl+T".to_string()));
        assert!(keys_of(&l, e, "Change directory (fzf)").contains(&"Alt+C".to_string()));
        assert!(keys_of(&l, e, "Capitalize word").contains(&"Alt+Shift+C".to_string()));
        assert!(keys_of(&l, e, "Previous matching command").contains(&"↑".to_string()));
        assert!(keys_of(&l, e, "Edit command in $EDITOR").contains(&"Ctrl+X › Ctrl+E".to_string()));
    }

    #[test]
    fn emacs_mode_ranges_and_macros() {
        let l = load(Some(fixture("emacs.txt")));
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["ZSH · EMACS"]);
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Edit command in $EDITOR"), vec!["Ctrl+X › Ctrl+E"]);
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Start of line"), vec!["Ctrl+A"]);   // the range line didn't override it
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Macro: ls -la^J"), vec!["Ctrl+X › a"]);
    }

    #[test]
    fn failure_falls_back() {
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: zsh -ic bindkey failed"));
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Search history backward"), vec!["Ctrl+R"]);
        let l = load(Some("just a banner\n".into()));
        assert_eq!(l.note.as_deref(), Some("config: zsh bindkey printed nothing"));
    }
}
