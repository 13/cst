//! zellij: the keybinds block of config.kdl over the default keymap.
use super::kdl::{self, Node};
use super::{describe, humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_prefixed;
use crate::model::{Binding, Change};
use std::path::Path;

const DEFAULTS: &str = include_str!("../defaults/zellij.txt");

const ACTIONS: &[(&str, &str)] = &[
    ("NewPane down", "New pane below"), ("NewPane right", "New pane right"), ("NewPane stacked", "New stacked pane"),
    ("NewPane", "New pane"), ("CloseFocus", "Close pane"), ("ToggleFocusFullscreen", "Fullscreen pane"),
    ("TogglePaneFrames", "Toggle pane frames"), ("ToggleFloatingPanes", "Toggle floating panes"),
    ("TogglePaneEmbedOrFloating", "Float/embed pane"), ("TogglePanePinned", "Pin pane"), ("SwitchFocus", "Next pane"),
    ("MoveFocus left", "Focus left"), ("MoveFocus right", "Focus right"), ("MoveFocus up", "Focus up"),
    ("MoveFocus down", "Focus down"), ("MoveFocusOrTab left", "Focus left"), ("MoveFocusOrTab right", "Focus right"),
    ("FocusNextPane", "Next pane"), ("FocusPreviousPane", "Previous pane"), ("PaneNameInput", "Rename pane"),
    ("NewTab", "New tab"), ("CloseTab", "Close tab"), ("GoToNextTab", "Next tab"), ("GoToPreviousTab", "Previous tab"),
    ("GoToTab", "Go to tab"), ("ToggleTab", "Last tab"), ("TabNameInput", "Rename tab"), ("ToggleActiveSyncTab", "Sync panes"),
    ("BreakPane", "Break pane to tab"), ("BreakPaneLeft", "Break pane left"), ("BreakPaneRight", "Break pane right"),
    ("Resize Increase", "Grow"), ("Resize Decrease", "Shrink"), ("Resize Increase left", "Grow left"),
    ("Resize Increase right", "Grow right"), ("Resize Increase up", "Grow up"), ("Resize Increase down", "Grow down"),
    ("Resize Decrease left", "Shrink left"), ("Resize Decrease right", "Shrink right"),
    ("Resize Decrease up", "Shrink up"), ("Resize Decrease down", "Shrink down"),
    ("MovePane left", "Move pane left"), ("MovePane right", "Move pane right"), ("MovePane up", "Move pane up"),
    ("MovePane down", "Move pane down"), ("MovePane", "Move pane"), ("MovePaneBackwards", "Move pane back"),
    ("ScrollUp", "Scroll up"), ("ScrollDown", "Scroll down"), ("PageScrollUp", "Page up"), ("PageScrollDown", "Page down"),
    ("HalfPageScrollUp", "Half page up"), ("HalfPageScrollDown", "Half page down"), ("ScrollToBottom", "Scroll to bottom"),
    ("ScrollToTop", "Scroll to top"), ("EditScrollback", "Edit scrollback"), ("Search down", "Search down"),
    ("Search up", "Search up"), ("SearchToggleOption CaseSensitivity", "Toggle case sensitivity"),
    ("SearchToggleOption Wrap", "Toggle wrap"), ("SearchToggleOption WholeWord", "Toggle whole word"),
    ("Quit", "Quit"), ("Detach", "Detach"), ("Clear", "Clear"), ("Run", "Run command"),
    ("LaunchOrFocusPlugin", "Plugin"), ("Write", "Send bytes"), ("WriteChars", "Send text"),
];

pub struct Zellij;

impl Source for Zellij {
    fn app(&self) -> &'static str { "zellij" }
    fn binary(&self) -> &'static str { "zellij" }
    fn load(&self, env: &Env) -> Loaded { load_from(env, &env.config("zellij/config.kdl")) }
}

pub fn load_from(env: &Env, path: &Path) -> Loaded {
    let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_prefixed(k, ' ')], &|g| g.to_lowercase());
    let live = if !path.exists() { Ok(None) } else {
        env.read(path).ok_or_else(|| "config.kdl: unreadable".to_string())
            .and_then(|t| kdl::parse(&t).map_err(|e| format!("config.kdl: {e}")))
            .map(|nodes| Some(changes(&nodes)))
    };
    layer(defaults, live, true).into_loaded("zellij")
}

fn cleared(n: &Node) -> bool {
    n.props.iter().any(|(k, v)| k == "clear-defaults" && v == "true")
}

fn changes(nodes: &[Node]) -> Vec<Change> {
    let mut out = Vec::new();
    let Some(kb) = nodes.iter().find(|n| n.name == "keybinds") else { return out };
    if cleared(kb) { out.push(Change::Clear(None)); }
    for mode in &kb.children {
        // shared blocks are scoped by their whole signature so they don't override each other
        let shared = mode.name.starts_with("shared");
        let scope = if shared { std::iter::once(mode.name.clone()).chain(mode.args.iter().cloned()).collect::<Vec<_>>().join(" ") } else { mode.name.clone() };
        let group = if shared { "Shared".to_string() } else { humanize(&mode.name) };
        if cleared(mode) { out.push(Change::Clear(Some(if shared { "shared".into() } else { scope.clone() }))); }
        for b in &mode.children {
            for key in &b.args {
                let seq = vec![parse_prefixed(key, ' ')];
                match b.name.as_str() {
                    "bind" => out.push(Change::Bind(Binding { scope: scope.clone(), group: group.clone(), seq, desc: describe_actions(&b.children), id: String::new() })),
                    "unbind" => out.push(Change::Unbind { scope: scope.clone(), seq }),
                    _ => {}
                }
            }
        }
    }
    out
}

fn describe_actions(actions: &[Node]) -> String {
    let text = |a: &Node| std::iter::once(a.name.clone()).chain(a.args.iter().cloned()).collect::<Vec<_>>().join(" ");
    match actions.iter().find(|a| a.name != "SwitchToMode") {
        Some(a) => describe(ACTIONS, &text(a)).unwrap_or_else(|| {
            let args = a.args.join(" ");
            if args.is_empty() { humanize(&a.name) } else { format!("{} {args}", humanize(&a.name)) }
        }),
        None => actions.first().and_then(|a| a.args.first()).map(|m| format!("{} mode", humanize(m))).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn env(dir: &str) -> Env {
        let home = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zellij").join(dir);
        Env::test(&home, FakeRunner(HashMap::new()))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn small_config() {
        let e = env("small");
        let l = load_from(&e, &e.home.join("config.kdl"));
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "ZELLIJ · PANE", "New pane"), vec!["n"]);
        assert_eq!(keys_of(&l, "ZELLIJ · PANE", "New pane below"), vec!["d"]);
        assert_eq!(keys_of(&l, "ZELLIJ · PANE", "Focus left"), vec!["Alt+H", "Alt+←"]);
        assert!(keys_of(&l, "ZELLIJ · PANE", "Close pane").is_empty());   // pane defaults cleared, /- dropped
        assert!(keys_of(&l, "ZELLIJ · TAB", "Close tab").is_empty());     // unbind x
        assert_eq!(keys_of(&l, "ZELLIJ · TAB", "New tab"), vec!["n"]);     // tab defaults kept
        assert_eq!(keys_of(&l, "ZELLIJ · TAB", "Go to tab"), vec!["1", "2"]);
        assert_eq!(keys_of(&l, "ZELLIJ · SHARED", "Pane mode"), vec!["Ctrl+P"]);
        assert_eq!(keys_of(&l, "ZELLIJ · SHARED", "Normal mode"), vec!["Ctrl+P"]);
        assert_eq!(keys_of(&l, "ZELLIJ · SHARED", "Quit"), vec!["Ctrl+Q"]);
    }

    #[test]
    fn real_config_clears_defaults() {
        let e = env("real");
        let l = load_from(&e, &e.home.join("config.kdl"));
        assert_eq!(l.origin, Origin::Live);
        assert!(l.note.is_none());
        assert_eq!(keys_of(&l, "ZELLIJ · PANE", "New pane"), vec!["n"]);
        assert_eq!(keys_of(&l, "ZELLIJ · LOCKED", "Normal mode"), vec!["Ctrl+G"]);
    }

    #[test]
    fn broken_file_notes() {
        let dir = std::env::temp_dir().join("cst-zellij-broken");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.kdl"), "keybinds {\n  pane {\n").unwrap();
        let e = Env::test(&dir, FakeRunner(HashMap::new()));
        let l = load_from(&e, &dir.join("config.kdl"));
        assert_eq!(l.origin, Origin::Defaults);
        assert!(l.note.as_deref().unwrap().starts_with("config: config.kdl"));
    }
}
