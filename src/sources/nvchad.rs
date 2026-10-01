//! NvChad: its own mappings (those whose description appears in NvChad's
//! lua/nvchad/mappings.lua), sectioned by the description's first word.
use super::nvim::{dump, rows};
use super::{layer, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};
use std::collections::HashSet;
use std::path::PathBuf;

const GROUPS: &[(&str, &str, bool)] = &[
    ("general", "General", true), ("toggle", "Toggle", true), ("telescope", "Telescope", true),
    ("terminal", "Terminal", true), ("whichkey", "Whichkey", true), ("nvimtree", "NvimTree", true),
    ("buffer", "Buffer", true), ("lsp", "LSP", true), ("comment", "Comment", true), ("format", "Format", true),
    ("blankline", "Blankline", true), ("nvcheatsheet", "Cheatsheet", true),
    ("switch", "Windows", false), ("move", "Insert", false),
];

pub struct NvChad;

fn mappings_file(env: &Env) -> PathBuf { env.data("nvim/lazy/NvChad/lua/nvchad/mappings.lua") }

/// The `desc = "…"` strings of NvChad's own mappings file (compared exactly:
/// nvim's built-ins like "Toggle comment" differ only in case).
pub fn nvchad_descs(env: &Env) -> Option<HashSet<String>> {
    let text = env.read(&mappings_file(env))?;
    let mut out = HashSet::new();
    for part in text.split("desc =").skip(1) {
        let part = part.trim_start();
        let Some(q) = part.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        if let Some(end) = part[1..].find(q) { out.insert(part[1..1 + end].to_string()); }
    }
    Some(out)
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// (section, description) from the description's first word.
pub fn split_desc(desc: &str, mode: &str) -> (String, String) {
    let first = desc.split_whitespace().next().unwrap_or("").to_lowercase();
    match GROUPS.iter().find(|(w, _, _)| *w == first) {
        Some((_, group, true)) => {
            let rest = desc.split_once(char::is_whitespace).map(|(_, r)| r.trim()).unwrap_or("");
            (group.to_string(), cap(if rest.is_empty() { desc } else { rest }))
        }
        Some((_, group, false)) => (group.to_string(), cap(desc)),
        None if mode == "i" => ("Insert".into(), cap(desc)),
        None => ("Other".into(), cap(desc)),
    }
}

impl Source for NvChad {
    fn app(&self) -> &'static str { "nvchad" }
    fn binary(&self) -> &'static str { "nvim" }
    fn installed(&self, env: &Env) -> bool { env.which("nvim") && mappings_file(env).is_file() }
    fn load(&self, env: &Env) -> Loaded {
        let descs = nvchad_descs(env).unwrap_or_default();
        let live = dump(env).map(|out| Some(rows(&out)
            .filter(|(_, _, desc)| descs.contains(*desc))
            .map(|(mode, lhs, desc)| {
                let (group, d) = split_desc(desc, mode);
                Change::Bind(Binding { scope: mode.to_string(), group, seq: parse_vim(lhs), desc: d, id: String::new() })
            }).collect()))
            .ok_or_else(|| "nvim --headless failed".to_string());
        layer(vec![], live, false).into_loaded("nvchad")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::nvim::{Nvim, NVIM_LUA};
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn env(dump: Option<String>) -> Env {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nvchad");
        let runs = dump.map(|d| HashMap::from([(format!("nvim --headless -i NONE -c {NVIM_LUA}"), d)])).unwrap_or_default();
        let mut e = Env::test(&root, FakeRunner(runs));
        e.vars.insert("XDG_DATA_HOME".into(), root.join("data").display().to_string());
        e
    }

    fn real_dump() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nvim/nvchad-dump.txt")).unwrap()
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn descriptions_from_nvchad_source() {
        let d = nvchad_descs(&env(None)).unwrap();
        assert!(d.len() >= 40, "{}", d.len());          // 46 desc lines; repeated descriptions collapse
        assert!(d.contains("telescope find files") && d.contains("move beginning of line"));
    }

    #[test]
    fn sections_from_description_words() {
        assert_eq!(split_desc("telescope find files", "n"), ("Telescope".into(), "Find files".into()));
        assert_eq!(split_desc("LSP diagnostic loclist", "n"), ("LSP".into(), "Diagnostic loclist".into()));
        assert_eq!(split_desc("switch window left", "n"), ("Windows".into(), "Switch window left".into()));
        assert_eq!(split_desc("move beginning of line", "i"), ("Insert".into(), "Move beginning of line".into()));
        assert_eq!(split_desc("something else", "i"), ("Insert".into(), "Something else".into()));
        assert_eq!(split_desc("something else", "v"), ("Other".into(), "Something else".into()));
        assert_eq!(split_desc("general", "n"), ("General".into(), "General".into()));
    }

    #[test]
    fn real_nvchad_sheet() {
        let e = env(Some(real_dump()));
        if std::path::Path::new("/usr/bin/nvim").exists() {    // CI has no nvim
            let mut x = env(None);
            x.vars.insert("PATH".into(), "/usr/bin".into());
            assert!(NvChad.installed(&x));
        }
        let l = NvChad.load(&e);
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(keys_of(&l, "NVCHAD · TELESCOPE", "Find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVCHAD · GENERAL", "Save file"), vec!["Ctrl+S"]);
        assert_eq!(keys_of(&l, "NVCHAD · WINDOWS", "Switch window left"), vec!["Ctrl+H"]);
        assert_eq!(keys_of(&l, "NVCHAD · INSERT", "Move beginning of line"), vec!["Ctrl+B"]);
        assert_eq!(keys_of(&l, "NVCHAD · TOGGLE", "Line number"), vec!["Space › n"]);
        assert!(keys_of(&l, "NVCHAD · OTHER", "CMD enter command mode").is_empty());   // user's own map stays in nvim
        assert_eq!(keys_of(&l, "NVCHAD · TERMINAL", "Escape terminal mode"), vec!["Ctrl+X"]);   // t mode
        assert!(!l.sections.iter().flat_map(|s| &s.rows).flat_map(|r| &r.alts).any(|a| seq_text(a) == "g › c"));
        assert!(keys_of(&Nvim.load(&e), "NVIM · EDITING", "Toggle comment").contains(&"g › c".to_string()));  // nvim's own, case differs
        // …and nvim no longer lists NvChad's mappings
        let n = Nvim.load(&e);
        assert!(n.sections.iter().flat_map(|s| &s.rows).all(|r| r.desc != "Telescope find files"));
        assert_eq!(keys_of(&n, "NVIM · MAPPINGS", "CMD enter command mode"), vec![";"]);
    }

    #[test]
    fn user_override_of_nvchad_desc_lands_in_nvchad() {
        let l = NvChad.load(&env(Some("n\t<leader>ff\ttelescope find files\n".into())));
        assert_eq!(keys_of(&l, "NVCHAD · TELESCOPE", "Find files"), vec!["Leader › f › f"]);
    }

    #[test]
    fn nvchad_dump_failure_notes() {
        let l = NvChad.load(&env(None));
        assert_eq!(l.note.as_deref(), Some("config: nvim --headless failed"));
        assert!(l.sections.iter().all(|s| s.rows.is_empty()));
    }
}
