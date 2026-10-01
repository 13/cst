//! nano: bind/unbind lines of the nanorc files over nano's default shortcuts.
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_nano;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/nano.txt");

const FUNCS: &[(&str, &str, &str)] = &[
    ("savefile", "File", "Save"), ("writeout", "File", "Write out (save as)"), ("insert", "File", "Insert file"),
    ("exit", "File", "Exit"), ("cut", "Edit", "Cut"), ("paste", "Edit", "Paste"), ("copy", "Edit", "Copy"),
    ("mark", "Edit", "Set mark"), ("undo", "Edit", "Undo"), ("redo", "Edit", "Redo"), ("justify", "Edit", "Justify"),
    ("indent", "Edit", "Indent"), ("unindent", "Edit", "Unindent"), ("comment", "Edit", "Toggle comment"),
    ("delete", "Edit", "Delete"), ("cutrestoffile", "Edit", "Cut to end of file"), ("zap", "Edit", "Delete without cut"),
    ("whereis", "Search", "Search"), ("wherewas", "Search", "Search backward"), ("findnext", "Search", "Find next"),
    ("findprevious", "Search", "Find previous"), ("replace", "Search", "Replace"),
    ("gotoline", "Navigation", "Go to line"), ("home", "Navigation", "Start of line"), ("end", "Navigation", "End of line"),
    ("pageup", "Navigation", "Page up"), ("pagedown", "Navigation", "Page down"), ("firstline", "Navigation", "First line"),
    ("lastline", "Navigation", "Last line"), ("prevword", "Navigation", "Previous word"), ("nextword", "Navigation", "Next word"),
    ("findbracket", "Navigation", "Matching bracket"), ("help", "Other", "Help"), ("location", "Other", "Show position"),
    ("execute", "Other", "Execute command"), ("wordcount", "Other", "Word count"), ("suspend", "Other", "Suspend"),
    ("refresh", "Other", "Refresh"), ("linenumbers", "Other", "Toggle line numbers"),
];

pub struct Nano;

impl Source for Nano {
    fn app(&self) -> &'static str { "nano" }
    fn binary(&self) -> &'static str { "nano" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_nano(k)], &|_| String::new());
        let texts: Vec<String> = [env.etc("nanorc"), env.home.join(".nanorc"), env.config("nano/nanorc")]
            .iter().filter_map(|p| env.read(p)).collect();
        let live = if texts.is_empty() { Ok(None) } else { Ok(Some(texts.iter().flat_map(|t| t.lines()).filter_map(rc_line).collect())) };
        layer(defaults, live, true).into_loaded("nano")
    }
}

fn rc_line(line: &str) -> Option<Change> {
    let line = line.trim();
    let mut w = line.split_whitespace();
    let cmd = w.next()?;
    let key = w.next()?;
    let seq = vec![parse_nano(key)];
    let menu_ok = |m: &str| m == "main" || m == "all";
    match cmd {
        "unbind" => w.next().filter(|m| menu_ok(m)).map(|_| Change::Unbind { scope: String::new(), seq }),
        "bind" => {
            let rest = line.split_once(key)?.1.trim();
            let (group, desc, menu) = if let Some(q) = rest.strip_prefix('"') {
                let end = q.rfind('"')?;
                ("Macros".to_string(), format!("Macro: {}", &q[..end]), q[end + 1..].trim())
            } else {
                let (func, menu) = rest.split_once(char::is_whitespace)?;
                let (g, d) = FUNCS.iter().find(|(f, _, _)| *f == func)
                    .map(|(_, g, d)| (g.to_string(), d.to_string()))
                    .unwrap_or_else(|| ("Other".into(), humanize(func)));
                (g, d, menu.trim())
            };
            menu_ok(menu).then(|| Change::Bind(Binding::new(&group, seq, &desc)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, desc: &str) -> Vec<String> {
        l.sections.iter().flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn nanorc_bind_unbind_macro() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nano");
        let mut env = Env::test(&root.join("home"), FakeRunner(HashMap::new()));
        env.vars.insert("CST_SYSCONFDIR".into(), root.join("etc").display().to_string());
        let l = Nano.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "Exit"), vec!["Ctrl+Q"]);                  // ^X rebound to cut, ^Q bound to exit
        assert_eq!(keys_of(&l, "Cut"), vec!["Ctrl+K", "Ctrl+X"]);
        assert!(keys_of(&l, "Justify").is_empty());                        // unbind ^J
        assert_eq!(keys_of(&l, "Macro: {cut}{paste}"), vec!["Alt+1"]);
        assert_eq!(keys_of(&l, "End of line"), vec!["Ctrl+E"]);            // the browser-menu bind is ignored
        assert_eq!(keys_of(&l, "Search backward"), vec!["Ctrl+B"]);        // odd spacing; default ^Q replaced
        assert_eq!(keys_of(&l, "Save"), vec!["Ctrl+S"]);
    }

    #[test]
    fn no_nanorc_is_defaults() {
        let mut env = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::new()));
        env.vars.insert("CST_SYSCONFDIR".into(), "/nonexistent".into());
        let l = Nano.load(&env);
        assert_eq!(l.origin, Origin::Defaults);
        assert_eq!(keys_of(&l, "Undo"), vec!["Alt+U"]);
    }
}
