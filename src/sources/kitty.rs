//! kitty: `map`/`unmap` lines of kitty.conf (and includes) over defaults.
use super::{describe, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_prefixed, Seq};
use crate::model::{Binding, Change};
use std::path::Path;

const DEFAULTS: &str = include_str!("../defaults/kitty.txt");

const ACTIONS: &[(&str, &str)] = &[
    ("new_tab", "New tab"), ("new_tab_with_cwd", "New tab"), ("close_tab", "Close tab"),
    ("next_tab", "Next tab"), ("previous_tab", "Previous tab"), ("move_tab_forward", "Move tab forward"),
    ("move_tab_backward", "Move tab backward"), ("set_tab_title", "Rename tab"), ("goto_tab", "Go to tab"),
    ("new_window", "New window"), ("new_window_with_cwd", "New window"), ("new_os_window", "New OS window"),
    ("close_window", "Close window"), ("next_window", "Next window"), ("previous_window", "Previous window"),
    ("move_window_forward", "Move window forward"), ("move_window_backward", "Move window backward"),
    ("start_resizing_window", "Resize window"), ("next_layout", "Next layout"),
    ("neighboring_window", "Focus neighbouring window"), ("focus_visible_window", "Focus window by number"),
    ("copy_to_clipboard", "Copy"), ("paste_from_clipboard", "Paste"), ("paste_from_selection", "Paste selection"),
    ("scroll_line_up", "Scroll line up"), ("scroll_line_down", "Scroll line down"),
    ("scroll_page_up", "Scroll page up"), ("scroll_page_down", "Scroll page down"),
    ("scroll_home", "Scroll to top"), ("scroll_end", "Scroll to bottom"), ("show_scrollback", "Browse scrollback"),
    ("scroll_to_prompt -1", "Previous prompt"), ("scroll_to_prompt 1", "Next prompt"),
    ("change_font_size all +2.0", "Bigger font"), ("change_font_size all -2.0", "Smaller font"),
    ("change_font_size all 0", "Reset font size"), ("change_font_size", "Change font size"),
    ("open_url_with_hints", "Open URL"), ("kitten hints", "Hints"), ("kitten unicode_input", "Unicode input"),
    ("edit_config_file", "Edit config"), ("load_config_file", "Reload config"),
    ("toggle_fullscreen", "Toggle fullscreen"), ("toggle_maximized", "Toggle maximized"),
    ("kitty_shell", "Kitty shell"), ("clear_terminal", "Clear terminal"),
];

pub struct Kitty;

impl Source for Kitty {
    fn app(&self) -> &'static str { "kitty" }
    fn binary(&self) -> &'static str { "kitty" }
    fn load(&self, env: &Env) -> Loaded { load_from(env, &env.config("kitty/kitty.conf")) }
}

pub fn load_from(env: &Env, path: &Path) -> Loaded {
    let parsed = parse(env, path);
    let kitty_mod = match &parsed { Ok(Some((_, m))) => m.clone(), _ => "ctrl+shift".to_string() };
    let keys = |k: &str| seq(k, &kitty_mod);
    let defaults = parse_defaults(DEFAULTS, &keys, &|_| String::new());
    layer(defaults, parsed.map(|o| o.map(|(c, _)| c)), true).into_loaded("kitty")
}

fn seq(keys: &str, kitty_mod: &str) -> Seq {
    keys.split('>').map(|k| parse_prefixed(&k.replace("kitty_mod", kitty_mod), '+')).collect()
}

/// A config line that matters, in file order (includes inlined).
enum Line { Map(String, String), Unmap(String), Clear }

fn collect(env: &Env, path: &Path, depth: u8, out: &mut Vec<Line>, kitty_mod: &mut String) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = env.read(path).ok_or(format!("{name}: unreadable"))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let mut words = line.split_whitespace();
        match words.next() {
            Some("kitty_mod") => if let Some(m) = words.next() { *kitty_mod = m.to_string() },
            Some("clear_all_shortcuts") => if words.next() == Some("yes") { out.push(Line::Clear) },
            Some("include") => {
                let p = words.collect::<Vec<_>>().join(" ");
                if depth < 8 && !p.contains('*') {
                    let _ = collect(env, &env.expand(&p, dir), depth + 1, out, kitty_mod);
                }
            }
            Some("unmap") => if let Some(k) = words.next() { out.push(Line::Unmap(k.to_string())) },
            Some("map") => {
                let mut rest: Vec<&str> = words.collect();
                while rest.first().is_some_and(|w| w.starts_with("--")) {
                    let opt = rest.remove(0);
                    if !opt.contains('=') && !rest.is_empty() { rest.remove(0); }
                }
                if rest.is_empty() { continue; }
                let keys = rest.remove(0).to_string();
                let action = rest.join(" ");
                let action = action.split(" #").next().unwrap_or("").trim().to_string();
                out.push(Line::Map(keys, action));
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn parse(env: &Env, path: &Path) -> Result<Option<(Vec<Change>, String)>, String> {
    if !path.exists() { return Ok(None); }
    let mut lines = Vec::new();
    let mut kitty_mod = "ctrl+shift".to_string();
    collect(env, path, 0, &mut lines, &mut kitty_mod)?;
    let changes = lines.into_iter().map(|l| match l {
        Line::Clear => Change::Clear(None),
        Line::Unmap(k) => Change::Unbind { scope: String::new(), seq: seq(&k, &kitty_mod) },
        Line::Map(k, a) if a.is_empty() || a == "no_op" || a == "discard_event" =>
            Change::Unbind { scope: String::new(), seq: seq(&k, &kitty_mod) },
        Line::Map(k, a) => {
            let (group, desc) = match describe(ACTIONS, &a) {
                Some(d) => (group_of(&d).unwrap_or_else(|| "Misc".into()), d),
                None => ("Custom".to_string(), a),
            };
            Change::Bind(Binding::new(&group, seq(&k, &kitty_mod), &desc))
        }
    }).collect();
    Ok(Some((changes, kitty_mod)))
}

/// Live bindings land in the same group as the default with that description.
fn group_of(desc: &str) -> Option<String> {
    DEFAULTS.lines().map(|l| l.split(" :: ").collect::<Vec<_>>())
        .find(|f| f.len() >= 3 && f[2].trim() == desc)
        .map(|f| f[0].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn env() -> (Env, PathBuf) {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kitty");
        (Env::test(&dir, FakeRunner(HashMap::new())), dir)
    }

    fn row<'a>(l: &'a Loaded, desc: &str) -> Option<&'a crate::model::Row> {
        l.sections.iter().flat_map(|s| &s.rows).find(|r| r.desc == desc)
    }

    fn keys_of(l: &Loaded, desc: &str) -> Vec<String> {
        row(l, desc).map(|r| r.alts.iter().map(crate::keys::seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn live_config_over_defaults_with_kitty_mod() {
        let (env, dir) = env();
        let l = load_from(&env, &dir.join("kitty.conf"));
        assert_eq!(l.origin, Origin::Mixed);
        // kitty_mod is ctrl+alt; the custom launch replaced the default New tab key
        assert_eq!(keys_of(&l, "Launch --type=tab"), vec!["Ctrl+Alt+T"]);
        assert!(row(&l, "New tab").is_none());
        // unmap removed the default Close window
        assert!(row(&l, "Close window").is_none());
        // include followed; exact action wins over prefix
        assert_eq!(keys_of(&l, "Bigger font"), vec!["Ctrl+Alt+="]);
        // merge keeps defaults first, then pushes live bindings
        assert_eq!(keys_of(&l, "New window"), vec!["Ctrl+Alt+Enter", "Ctrl+Shift+Enter"]);
        assert!(keys_of(&l, "Toggle fullscreen").contains(&"F11".to_string()));
        assert_eq!(keys_of(&l, "Launch nvim"), vec!["Ctrl+E"]);
    }

    #[test]
    fn missing_config_is_defaults_only() {
        let (env, dir) = env();
        let l = load_from(&env, &dir.join("nope.conf"));
        assert_eq!(l.origin, Origin::Defaults);
        assert_eq!(keys_of(&l, "New tab"), vec!["Ctrl+Shift+T"]);
        assert!(l.note.is_none());
    }
}
