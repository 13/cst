//! vi: classic vi (ex-vi, busybox, nvi) — a curated command reference. Hidden
//! when `vi` is really vim or nvim (their own sheets cover it).
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;

const DEFAULTS: &str = include_str!("../defaults/vi.txt");

pub struct Vi;

impl Source for Vi {
    fn app(&self) -> &'static str { "vi" }
    fn binary(&self) -> &'static str { "vi" }
    fn installed(&self, env: &Env) -> bool {
        env.resolve_bin("vi").and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .is_some_and(|n| !(n.starts_with("vim") || n.starts_with("nvim")))
    }
    fn load(&self, _env: &Env) -> Loaded {
        layer(parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new()), Ok(None), true).into_loaded("vi")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;

    fn exe(path: &std::path::Path) {
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn env_with(dir: &std::path::Path) -> Env {
        let mut e = Env::test(dir, FakeRunner(HashMap::new()));
        e.vars.insert("PATH".into(), dir.display().to_string());
        e
    }

    #[test]
    fn vi_hidden_when_it_is_vim() {
        let dir = std::env::temp_dir().join("cst-vi-link");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("alt")).unwrap();
        exe(&dir.join("vim"));
        std::os::unix::fs::symlink(dir.join("vim"), dir.join("alt/vi")).unwrap();   // vi → alt/vi → vim
        std::os::unix::fs::symlink(dir.join("alt/vi"), dir.join("vi")).unwrap();
        assert!(!Vi.installed(&env_with(&dir)));
    }

    #[test]
    fn classic_vi_shown_with_defaults() {
        let dir = std::env::temp_dir().join("cst-vi-real");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        exe(&dir.join("vi"));
        let env = env_with(&dir);
        assert!(Vi.installed(&env));
        let l = Vi.load(&env);
        assert_eq!(l.origin, Origin::Defaults);
        let keys = l.sections.iter().filter(|s| s.title == "VI · EX COMMANDS").flat_map(|s| &s.rows)
            .find(|r| r.desc == "Quit without saving").map(|r| seq_text(&r.alts[0])).unwrap();
        assert_eq!(keys, ": › q › ! › Enter");
    }
}
