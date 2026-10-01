//! alacritty: [[keyboard.bindings]] in alacritty.toml (+ imports) over defaults.
use super::{describe, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{combo, Seq};
use crate::model::{Binding, Change};
use std::path::Path;
use toml::{Table, Value};

const DEFAULTS: &str = include_str!("../defaults/alacritty.txt");

const ACTIONS: &[(&str, &str)] = &[
    ("Copy", "Copy"), ("Paste", "Paste"), ("PasteSelection", "Paste selection"),
    ("IncreaseFontSize", "Bigger font"), ("DecreaseFontSize", "Smaller font"), ("ResetFontSize", "Reset font size"),
    ("ScrollPageUp", "Scroll page up"), ("ScrollPageDown", "Scroll page down"), ("ScrollLineUp", "Scroll line up"),
    ("ScrollLineDown", "Scroll line down"), ("ScrollToTop", "Scroll to top"), ("ScrollToBottom", "Scroll to bottom"),
    ("ClearHistory", "Clear history"), ("Hide", "Hide"), ("Minimize", "Minimize"), ("Quit", "Quit"),
    ("ToggleFullscreen", "Toggle fullscreen"), ("ToggleMaximized", "Toggle maximized"), ("ToggleViMode", "Toggle vi mode"),
    ("SearchForward", "Search forward"), ("SearchBackward", "Search backward"),
    ("SpawnNewInstance", "New window"), ("CreateNewWindow", "New window"), ("ClearSelection", "Clear selection"),
];

pub struct Alacritty;

impl Source for Alacritty {
    fn app(&self) -> &'static str { "alacritty" }
    fn binary(&self) -> &'static str { "alacritty" }
    fn load(&self, env: &Env) -> Loaded { load_from(env, &env.config("alacritty/alacritty.toml")) }
}

/// "Control|Shift+C" (defaults notation) → combo
fn default_keys(s: &str) -> Seq {
    let (mods, key) = s.rsplit_once('+').filter(|(m, _)| !m.is_empty()).unwrap_or(("", s));
    let mods: Vec<&str> = mods.split('|').filter(|m| !m.is_empty()).collect();
    vec![combo(&mods, key)]
}

pub fn load_from(env: &Env, path: &Path) -> Loaded {
    let defaults = parse_defaults(DEFAULTS, &default_keys, &|_| String::new());
    let live = if path.exists() { parse(env, path).map(Some) } else { Ok(None) };
    layer(defaults, live, true).into_loaded("alacritty")
}

fn read_table(env: &Env, path: &Path) -> Result<Table, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = env.read(path).ok_or(format!("{name}: unreadable"))?;
    text.parse::<Table>().map_err(|e| format!("{name}: {}", e.to_string().lines().next().unwrap_or("parse error")))
}

fn parse(env: &Env, path: &Path) -> Result<Vec<Change>, String> {
    let main = read_table(env, path)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let imports = main.get("general").and_then(|g| g.get("import")).or(main.get("import"))
        .and_then(Value::as_array).cloned().unwrap_or_default();
    let mut changes = Vec::new();
    for imp in imports.iter().filter_map(Value::as_str) {
        // missing imports are ignored, as alacritty does
        if let Ok(t) = read_table(env, &env.expand(imp, dir)) { bindings(&t, &mut changes); }
    }
    bindings(&main, &mut changes);
    Ok(changes)
}

fn bindings(t: &Table, out: &mut Vec<Change>) {
    let list = t.get("keyboard").and_then(|k| k.get("bindings")).or(t.get("key_bindings"))
        .and_then(Value::as_array).cloned().unwrap_or_default();
    for b in list.iter().filter_map(Value::as_table) {
        let Some(key) = b.get("key").and_then(Value::as_str) else { continue };
        let mods: Vec<&str> = b.get("mods").and_then(Value::as_str).unwrap_or("").split('|').map(str::trim).collect();
        let seq = vec![combo(&mods, key)];
        let mode = b.get("mode").and_then(Value::as_str).unwrap_or("");
        let group = if mode.contains("Vi") && !mode.contains("~Vi") { "Vi mode" } else { "" };
        let action = b.get("action").and_then(Value::as_str);
        if matches!(action, Some(a) if a.eq_ignore_ascii_case("None") || a.eq_ignore_ascii_case("ReceiveChar")) {
            out.push(Change::Unbind { scope: String::new(), seq });
            continue;
        }
        let desc = if let Some(a) = action {
            describe(ACTIONS, a).unwrap_or_else(|| a.to_string())
        } else if let Some(c) = b.get("chars").and_then(Value::as_str) {
            format!("Send {c:?}")
        } else if let Some(cmd) = b.get("command") {
            let prog = cmd.as_str().or(cmd.get("program").and_then(Value::as_str)).unwrap_or("command");
            format!("Run {prog}")
        } else { continue };
        let group = if group.is_empty() { group_of(&desc) } else { group.to_string() };
        out.push(Change::Bind(Binding::new(&group, seq, &desc)));
    }
}

fn group_of(desc: &str) -> String {
    DEFAULTS.lines().map(|l| l.split(" :: ").collect::<Vec<_>>())
        .find(|f| f.len() >= 3 && f[2].trim() == desc)
        .map(|f| f[0].trim().to_string()).unwrap_or_else(|| "Custom".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, desc: &str) -> Vec<String> {
        l.sections.iter().flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(crate::keys::seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn bindings_imports_and_unbinds() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/alacritty");
        let env = Env::test(&dir, FakeRunner(HashMap::new()));
        let l = load_from(&env, &dir.join("alacritty.toml"));
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "New window"), vec!["Ctrl+Shift+N"]);
        assert!(keys_of(&l, "Paste").is_empty());               // action None unbinds
        assert_eq!(keys_of(&l, "Send \"ls\\n\""), vec!["Alt+L"]);
        assert_eq!(keys_of(&l, "Run alacritty"), vec!["F2"]);
        assert_eq!(keys_of(&l, "Bigger font"), vec!["Ctrl++", "Ctrl+="]); // imported theme.toml rebinds Ctrl+= (pushed last)
        assert_eq!(keys_of(&l, "Reset font size"), vec!["Ctrl+0"]);
    }

    #[test]
    fn broken_toml_falls_back_with_note() {
        let dir = std::env::temp_dir().join("cst-alacritty-broken");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("alacritty.toml"), "[[keyboard.bindings]\nkey=").unwrap();
        let env = Env::test(&dir, FakeRunner(HashMap::new()));
        let l = load_from(&env, &dir.join("alacritty.toml"));
        assert_eq!(l.origin, Origin::Defaults);
        assert!(l.note.as_deref().unwrap().starts_with("config: alacritty.toml"));
    }
}
