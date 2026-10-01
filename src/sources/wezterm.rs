//! wezterm: `wezterm show-keys` is the effective key table (user config
//! included), so defaults are only a fallback when the command fails.
use super::{describe, humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{combo, Seq};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/wezterm.txt");
const SHIFTED: &str = "!\"#$%&()*+:<>?@^_{|}~";

const ACTIONS: &[(&str, &str)] = &[
    ("ActivateTabRelative(1)", "Next tab"), ("ActivateTabRelative(-1)", "Previous tab"),
    ("ActivateTab", "Go to tab"), ("SpawnTab", "New tab"), ("CloseCurrentTab", "Close tab"),
    ("MoveTabRelative(1)", "Move tab right"), ("MoveTabRelative(-1)", "Move tab left"),
    ("SpawnWindow", "New window"), ("SplitVertical", "Split vertical"), ("SplitHorizontal", "Split horizontal"),
    ("ActivatePaneDirection", "Focus pane"), ("AdjustPaneSize", "Resize pane"), ("TogglePaneZoomState", "Zoom pane"),
    ("CloseCurrentPane", "Close pane"), ("CopyTo", "Copy"), ("PasteFrom", "Paste"),
    ("IncreaseFontSize", "Bigger font"), ("DecreaseFontSize", "Smaller font"), ("ResetFontSize", "Reset font size"),
    ("ToggleFullScreen", "Toggle fullscreen"), ("ActivateCopyMode", "Copy mode"), ("QuickSelect", "Quick select"),
    ("Search", "Search"), ("ShowLauncher", "Launcher"), ("ShowLauncherArgs", "Launcher"),
    ("ShowTabNavigator", "Tab navigator"), ("ReloadConfiguration", "Reload config"),
    ("ScrollByPage(-1)", "Scroll page up"), ("ScrollByPage(1)", "Scroll page down"), ("ScrollByPage", "Scroll page"),
    ("ScrollToPrompt(-1)", "Previous prompt"), ("ScrollToPrompt(1)", "Next prompt"),
    ("ClearScrollback", "Clear scrollback"), ("ActivateCommandPalette", "Command palette"),
    ("CharSelect", "Character select"), ("Hide", "Hide"), ("ShowDebugOverlay", "Debug overlay"),
    ("QuitApplication", "Quit"), ("Nop", ""), ("DisableDefaultAssignment", ""),
];

pub struct Wezterm;

impl Source for Wezterm {
    fn app(&self) -> &'static str { "wezterm" }
    fn binary(&self) -> &'static str { "wezterm" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &default_keys, &|_| String::new());
        let live = env.run("wezterm", &["show-keys"]).map(|t| Some(parse_show_keys(&t)))
            .ok_or_else(|| "wezterm show-keys failed".to_string());
        layer(defaults, live, false).into_loaded("wezterm")
    }
}

fn default_keys(s: &str) -> Seq {
    let (mods, key) = s.rsplit_once('+').filter(|(m, _)| !m.is_empty()).unwrap_or(("", s));
    let mods: Vec<&str> = mods.split('|').collect();
    vec![key_combo(&mods, key)]
}

/// Upper-case letters and shifted symbols imply Shift (wezterm lists both
/// `CTRL C` and `SHIFT | CTRL C` for one binding; this makes them equal).
fn key_combo(mods: &[&str], key: &str) -> Vec<String> {
    let mut mods: Vec<&str> = mods.to_vec();
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if (c.is_ascii_uppercase() || SHIFTED.contains(c)) && !mods.iter().any(|m| m.eq_ignore_ascii_case("shift")) {
            mods.push("SHIFT");
        }
    }
    combo(&mods, key)
}

pub fn parse_show_keys(text: &str) -> Vec<Change> {
    let mut table = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.starts_with('\t') && !line.trim().is_empty() && !line.starts_with('-') {
            if line.starts_with("Mouse") { break; }
            table = line.strip_prefix("Key Table: ").unwrap_or("").trim().to_string();
            continue;
        }
        let Some((left, action)) = line.split_once("->") else { continue };
        let tokens: Vec<&str> = left.split_whitespace().collect();
        let Some((key, mods)) = tokens.split_last() else { continue };
        let mods: Vec<&str> = mods.iter().copied().filter(|m| *m != "|").collect();
        let mut seq: Seq = Vec::new();
        if mods.contains(&"LEADER") { seq.push(vec!["Leader".to_string()]); }
        seq.push(key_combo(&mods, key));
        let action = action.trim();
        let desc = describe(ACTIONS, action).unwrap_or_else(|| {
            // CopyMode(Close) → "Close"; other unknown actions verbatim
            action.strip_prefix("CopyMode(").and_then(|a| a.strip_suffix(')'))
                .map(|a| humanize(a.split('(').next().unwrap_or(a)))
                .unwrap_or_else(|| action.to_string())
        });
        if desc.is_empty() { continue; }
        let group = if table.is_empty() { String::new() } else { humanize(&table) };
        out.push(Change::Bind(Binding { scope: table.clone(), group, seq, desc, id: String::new() }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn fixture() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/wezterm/show-keys.txt")).unwrap()
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn show_keys_is_the_live_table() {
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::from([("wezterm show-keys".into(), fixture())])));
        let l = Wezterm.load(&env);
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(keys_of(&l, "WEZTERM", "Next tab"), vec!["Ctrl+Tab"]);
        assert_eq!(keys_of(&l, "WEZTERM", "Previous tab"), vec!["Ctrl+Shift+Tab"]);
        assert_eq!(keys_of(&l, "WEZTERM", "Copy"), vec!["Ctrl+Shift+C"]);           // CTRL C ≡ SHIFT|CTRL C
        assert_eq!(keys_of(&l, "WEZTERM", "Go to tab"), vec!["Ctrl+Shift+!", "Ctrl+Shift+@"]);
        assert_eq!(keys_of(&l, "WEZTERM", "New tab"), vec!["Super+T", "Leader › c"]);
        assert_eq!(keys_of(&l, "WEZTERM", "Split vertical"), vec!["Ctrl+Alt+Shift+\""]);
        assert_eq!(keys_of(&l, "WEZTERM", "EmitEvent(\"toggle-something\")"), vec!["F11"]);
        assert_eq!(keys_of(&l, "WEZTERM · COPY MODE", "Close"), vec!["Esc"]);
        assert!(!l.sections.iter().any(|s| s.title.contains("MOUSE")));
    }

    #[test]
    fn failing_command_falls_back_with_note() {
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new()));
        let l = Wezterm.load(&env);
        assert_eq!(l.origin, Origin::Defaults);
        assert_eq!(l.note.as_deref(), Some("config: wezterm show-keys failed"));
        assert_eq!(keys_of(&l, "WEZTERM · TABS", "New tab"), vec!["Ctrl+Shift+T"]);
    }
}
