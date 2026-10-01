//! emacs: a curated reference of the default keys (live keymaps need a running emacs).
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_emacs;

const DEFAULTS: &str = include_str!("../defaults/emacs.txt");

pub struct Emacs;

impl Source for Emacs {
    fn app(&self) -> &'static str { "emacs" }
    fn binary(&self) -> &'static str { "emacs" }
    fn load(&self, _env: &Env) -> Loaded {
        layer(parse_defaults(DEFAULTS, &|k| parse_emacs(k), &|_| String::new()), Ok(None), true).into_loaded("emacs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    #[test]
    fn defaults_only() {
        let l = Emacs.load(&Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new())));
        assert_eq!(l.origin, Origin::Defaults);
        let row = l.sections.iter().filter(|s| s.title == "EMACS · FILES").flat_map(|s| &s.rows).find(|r| r.desc == "Open file").unwrap();
        assert_eq!(seq_text(&row.alts[0]), "Ctrl+X › Ctrl+F");
    }
}
