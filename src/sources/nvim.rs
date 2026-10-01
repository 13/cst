//! nvim: normal-mode mappings that have a description, over core motions.
use super::{default_group, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/nvim.txt");

/// Prints `lhs<TAB>desc` per normal-mode mapping with a description.
/// Runs after the first event-loop tick (when distributions like NvChad
/// register their mappings), prints `mode<TAB>lhs<TAB>desc`, flushes, quits.
pub const NVIM_LUA: &str = "lua vim.schedule(function() for _, mode in ipairs({'n', 'i', 'v', 't'}) do for _, m in ipairs(vim.api.nvim_get_keymap(mode)) do if m.desc and m.desc ~= '' and not m.lhs:find('<Plug>', 1, true) and not m.lhs:find('<SNR>', 1, true) then io.stdout:write(mode .. '\\t' .. m.lhs .. '\\t' .. m.desc .. '\\n') end end end io.stdout:flush() vim.cmd('qa!') end)";

/// `mode<TAB>lhs<TAB>desc` lines from the user's nvim, or None.
pub fn dump(env: &Env) -> Option<String> {
    env.run("nvim", &["--headless", "-i", "NONE", "-c", NVIM_LUA])
}

/// Rows of a dump: (mode, lhs, desc).
pub fn rows(dump: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    dump.lines().filter_map(|l| {
        let mut f = l.splitn(3, '\t');
        Some((f.next()?, f.next()?, f.next()?))
    })
}

pub struct Nvim;

impl Source for Nvim {
    fn app(&self) -> &'static str { "nvim" }
    fn binary(&self) -> &'static str { "nvim" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new());
        // NvChad's own mappings have their own sheet
        let nvchad = super::nvchad::nvchad_descs(env).unwrap_or_default();
        let live = dump(env)
            .map(|out| Some(rows(&out).filter(|(mode, _, desc)| *mode == "n" && !nvchad.contains(*desc)).map(|(_, lhs, desc)| {
                let group = match default_group(&defaults, desc).as_str() { "Custom" => "Mappings".to_string(), g => g.to_string() };
                Change::Bind(Binding::new(&group, parse_vim(lhs), desc))
            }).collect()))
            .ok_or_else(|| "nvim --headless failed".to_string());
        layer(defaults, live, true).into_loaded("nvim")
    }
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
    fn mappings_with_desc_over_defaults() {
        let out = "n\t ff\tFind files\nn\t<C-W>d\tShow diagnostics\nn\t<C-S>\tSave\ni\t<C-B>\tInsert-only map\n\nbroken line\n";
        let key = format!("nvim --headless -i NONE -c {NVIM_LUA}");
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::from([(key, out.to_string())])));
        let l = Nvim.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Show diagnostics"), vec!["Ctrl+W › d"]);
        assert_eq!(keys_of(&l, "NVIM · FILES", "Save"), vec![": › w › Enter", "Ctrl+S"]);
        assert_eq!(keys_of(&l, "NVIM · MOTION", "Top of file"), vec!["g › g"]);
        assert!(keys_of(&l, "NVIM · MAPPINGS", "Insert-only map").is_empty());     // normal mode only
    }

    #[test]
    fn real_capture_includes_late_mappings() {
        let out = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nvim/nvchad-dump.txt")).unwrap();
        let key = format!("nvim --headless -i NONE -c {NVIM_LUA}");
        let env = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::from([(key, out)])));
        let l = Nvim.load(&env);   // no NvChad data dir here: everything stays in nvim
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Telescope find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "CMD enter command mode"), vec![";"]);
    }

    #[test]
    fn failure_notes() {
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new()));
        let l = Nvim.load(&env);
        assert_eq!(l.note.as_deref(), Some("config: nvim --headless failed"));
        assert_eq!(keys_of(&l, "NVIM · EDITING", "Redo"), vec!["Ctrl+R"]);
    }
}
