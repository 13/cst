//! nvim: normal-mode mappings that have a description, over core motions.
use super::{default_group, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/nvim.txt");

/// Prints `lhs<TAB>desc` per normal-mode mapping with a description.
pub const NVIM_LUA: &str = "lua for _, m in ipairs(vim.api.nvim_get_keymap('n')) do if m.desc and m.desc ~= '' and not m.lhs:find('<Plug>', 1, true) and not m.lhs:find('<SNR>', 1, true) then io.stdout:write(m.lhs .. '\\t' .. m.desc .. '\\n') end end";

pub struct Nvim;

impl Source for Nvim {
    fn app(&self) -> &'static str { "nvim" }
    fn binary(&self) -> &'static str { "nvim" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new());
        let live = env.run("nvim", &["--headless", "-c", NVIM_LUA, "-c", "qa!"])
            .map(|out| Some(out.lines().filter_map(|l| l.split_once('\t')).map(|(lhs, desc)| {
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
        let out = " ff\tFind files\n<C-W>d\tShow diagnostics\n<C-S>\tSave\n\nbroken line\n";
        let key = format!("nvim --headless -c {NVIM_LUA} -c qa!");
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::from([(key, out.to_string())])));
        let l = Nvim.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Show diagnostics"), vec!["Ctrl+W › d"]);
        assert_eq!(keys_of(&l, "NVIM · FILES", "Save"), vec![": › w › Enter", "Ctrl+S"]);   // joins the default's group
        assert_eq!(keys_of(&l, "NVIM · MOTION", "Top of file"), vec!["g › g"]);
    }

    #[test]
    fn failure_notes() {
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new()));
        let l = Nvim.load(&env);
        assert_eq!(l.note.as_deref(), Some("config: nvim --headless failed"));
        assert_eq!(keys_of(&l, "NVIM · EDITING", "Redo"), vec!["Ctrl+R"]);
    }
}
