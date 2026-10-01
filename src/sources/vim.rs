//! vim: curated defaults plus the user's own mappings (`:map` / `:imap`
//! through `vim -Es`), labelled by what they run.
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/vim.txt");
pub const VIM_REDIR: &str = "redir! > /dev/stdout | silent map | silent imap | redir END";
/// `-N`: `-u` keeps 'compatible' on, which breaks most vimrcs and plugins.
/// vim exits non-zero on any vimrc error even though the maps were printed,
/// so its status is ignored (`; true`). Args: $1 vimrc, $2 VIM_REDIR.
pub const VIM_SH: &str = "vim -N -Es -u \"$1\" -c \"$2\" -c 'qa!' </dev/null; true";

pub struct Vim;

impl Source for Vim {
    fn app(&self) -> &'static str { "vim" }
    fn binary(&self) -> &'static str { "vim" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new());
        let vimrc = [env.home.join(".vimrc"), env.home.join(".vim/vimrc"), env.config("vim/vimrc")].into_iter().find(|p| p.is_file());
        let live = match vimrc {
            None => Ok(None),
            Some(rc) => env.run("sh", &["-c", VIM_SH, "sh", &rc.display().to_string(), VIM_REDIR])
                .map(|out| Some(out.lines().filter_map(map_line).collect()))
                .ok_or_else(|| "vim -Es failed".to_string()),
        };
        layer(defaults, live, true).into_loaded("vim")
    }
}

fn map_line(line: &str) -> Option<Change> {
    let mode = line.get(..3)?.trim();
    let group = match mode { "" | "n" => "Your mappings", "i" => "Your insert mappings", _ => return None };
    let rest = line.get(3..)?.trim_start();
    let (lhs, rhs) = rest.split_once(char::is_whitespace)?;
    // one flag column (`*` noremap, `&` script-local, `@` buffer-local) before the rhs
    let rhs = rhs.trim_start();
    let rhs = match rhs.split_once(' ') { Some((f, r)) if !f.is_empty() && f.chars().all(|c| "*&@".contains(c)) => r.trim(), _ => rhs.trim() };
    if lhs.contains("<Plug>") || lhs.contains("<SNR>") || rhs == "<Nop>" || rhs.starts_with("<Plug>") || rhs.is_empty() {
        return None;
    }
    let cmd = rhs.strip_prefix("<Cmd>").or_else(|| rhs.strip_prefix(':')).and_then(|c| c.strip_suffix("<CR>"));
    let desc = match cmd { Some(c) => format!("Run :{c}"), None => format!("Keys: {rhs}") };
    let scope = if mode == "i" { "i" } else { "" };   // normal maps replace the default on the same key
    Some(Change::Bind(Binding { scope: scope.to_string(), group: group.into(), seq: parse_vim(lhs), desc, id: String::new() }))
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
    fn user_mappings_over_defaults() {
        let home = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vim/home");
        let out = std::fs::read_to_string(home.parent().unwrap().join("map.txt")).unwrap();
        let key = format!("sh -c {VIM_SH} sh {} {VIM_REDIR}", home.join(".vimrc").display());
        let l = Vim.load(&Env::test(&home, FakeRunner(HashMap::from([(key, out)]))));
        assert_eq!(l.origin, Origin::Mixed);
        let m = "VIM · YOUR MAPPINGS";
        assert_eq!(keys_of(&l, m, "Run :NERDTreeToggle"), vec!["Space › e"]);
        assert_eq!(keys_of(&l, m, "Run :Files"), vec!["Ctrl+P"]);
        assert_eq!(keys_of(&l, m, "Run :bnext"), vec!["g › b"]);
        assert_eq!(keys_of(&l, "VIM · YOUR INSERT MAPPINGS", "Keys: <Esc>"), vec!["j › k"]);
        assert!(!l.sections.iter().flat_map(|s| &s.rows).any(|r| r.desc.contains("<gv") || r.desc.contains("Nop") || r.desc.contains("SNR")));
        assert_eq!(keys_of(&l, "VIM · MOTION", "Top of file"), vec!["g › g"]);
    }

    #[test]
    fn xdg_vimrc_and_nnoremap_replaces_default() {
        // vim 9.1 also reads ~/.config/vim/vimrc; an nnoremap on a default key replaces that row
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vim");
        let mut env = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::from([(
            format!("sh -c {VIM_SH} sh {} {VIM_REDIR}", root.join("xdg/vim/vimrc").display()),
            "n  <C-R>       * :Telescope<CR>\nn  *           * *zz\n".to_string())])));
        env.vars.insert("XDG_CONFIG_HOME".into(), root.join("xdg").display().to_string());
        let l = Vim.load(&env);
        assert!(keys_of(&l, "VIM · EDITING", "Redo").is_empty());
        assert_eq!(keys_of(&l, "VIM · YOUR MAPPINGS", "Run :Telescope"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, "VIM · YOUR MAPPINGS", "Keys: *zz"), vec!["*"]);
    }

    #[test]
    fn no_vimrc_is_defaults_without_running_vim() {
        let l = Vim.load(&Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::new())));
        assert_eq!((l.origin, l.note.clone()), (Origin::Defaults, None));
        assert_eq!(keys_of(&l, "VIM · EDITING", "Redo"), vec!["Ctrl+R"]);
    }
}
