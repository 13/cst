//! awesome: the running WM's own cheatsheet sections (awesome-client), or a
//! scan of keys.lua when awesome is not running.
use super::{layer, Env, Loaded, Source};
use crate::keys::combo;
use crate::model::{Binding, Change};

/// Builds the Mod+i sections inside awesome (lib/cheatsheet) and returns them
/// joined by control characters: records \28, fields \30, alternatives \29, keys \31.
pub const SCRIPT: &str = r#"local cs = require("lib/cheatsheet") local out = {} for _, s in ipairs(cs.build(require("awful").key.hotkeys)) do for _, r in ipairs(s.rows) do local alts = {} for _, k in ipairs(r.keys) do alts[#alts + 1] = table.concat(k, "\31") end out[#out + 1] = s.title .. "\30" .. r.description .. "\30" .. table.concat(alts, "\29") end end return table.concat(out, "\28")"#;

const SECTIONS: [&str; 7] = ["Windows", "Tags", "Layout", "Apps", "Media & volume", "System", "Other"];

fn section_of(group: &str) -> &'static str {
    match group {
        "client" | "screen" => "Windows", "tag" => "Tags", "layout" => "Layout",
        "Programs" | "launcher" => "Apps", "media" | "volume" | "Fn" => "Media & volume",
        "awesome" => "System", _ => "Other",
    }
}

pub struct Awesome;

impl Source for Awesome {
    fn app(&self) -> &'static str { "awesome" }
    fn binary(&self) -> &'static str { "awesome" }
    fn load(&self, env: &Env) -> Loaded {
        let live = env.run("awesome-client", &[SCRIPT]).and_then(|o| parse_client(&o))
            .or_else(|| env.read(&env.config("awesome/keys.lua")).map(|t| scan_keys_lua(&t)))
            .map(Some)
            .ok_or_else(|| "awesome not running and keys.lua not found".to_string());
        layer(vec![], live, false).into_loaded("awesome")
    }
}

/// `   string "…"` from awesome-client; key names are already display names.
fn parse_client(out: &str) -> Option<Vec<Change>> {
    let start = out.find("string \"")? + "string \"".len();
    let end = out.rfind('"').filter(|&e| e >= start)?;
    let mut changes = Vec::new();
    for rec in out[start..end].split('\u{1c}').filter(|r| !r.is_empty()) {
        let f: Vec<&str> = rec.split('\u{1e}').collect();
        if f.len() != 3 { return None; }
        for alt in f[2].split('\u{1d}') {
            let keys: Vec<String> = alt.split('\u{1f}').map(String::from).collect();
            changes.push(Change::Bind(Binding::new(f[0], vec![keys], f[1])));
        }
    }
    (!changes.is_empty()).then_some(changes)
}

fn quoted(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let q = s.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let end = s[1..].find(q)? + 1;
    Some((s[1..end].to_string(), &s[end + 1..]))
}

fn field(s: &str, name: &str) -> Option<String> {
    let i = s.find(&format!("{name} ="))?;
    quoted(&s[i + name.len() + 2..]).map(|(v, _)| v)
}

/// `awful.key({ mods }, "key", …, { description = …, group = … })` entries.
fn scan_keys_lua(text: &str) -> Vec<Change> {
    let starts: Vec<usize> = text.match_indices("awful.key(").map(|(i, _)| i).collect();
    let mut found: Vec<(usize, Binding)> = Vec::new();
    for (n, &i) in starts.iter().enumerate() {
        let chunk = &text[i + "awful.key(".len()..starts.get(n + 1).copied().unwrap_or(text.len())];
        let Some(open) = chunk.find('{') else { continue };
        let Some(close) = chunk[open..].find('}').map(|c| c + open) else { continue };
        let mods: Vec<String> = chunk[open + 1..close].split(',').map(|m| {
            let m = m.trim().trim_matches('"').trim_matches('\'');
            match m { "modkey" => "Mod4", "altkey" => "Mod1", other => other }.to_string()
        }).filter(|m| !m.is_empty()).collect();
        let Some((key, _)) = quoted(chunk[close + 1..].trim_start().trim_start_matches(',')) else { continue };
        let Some(desc) = field(chunk, "description") else { continue };
        let group = section_of(&field(chunk, "group").unwrap_or_default());
        let mods: Vec<&str> = mods.iter().map(String::as_str).collect();
        let order = SECTIONS.iter().position(|s| *s == group).unwrap_or(SECTIONS.len());
        found.push((order, Binding::new(group, vec![combo(&mods, &key)], &desc)));
    }
    found.sort_by_key(|(o, _)| *o); // stable: file order within a section
    found.into_iter().map(|(_, b)| Change::Bind(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn env(client: Option<&str>) -> Env {
        let cfg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/awesome");
        let mut runs = HashMap::new();
        if let Some(out) = client { runs.insert(format!("awesome-client {SCRIPT}"), out.to_string()); }
        let mut e = Env::test(&cfg, FakeRunner(runs));
        e.vars.insert("XDG_CONFIG_HOME".into(), cfg.display().to_string());
        e
    }

    fn titles(l: &Loaded) -> Vec<&str> { l.sections.iter().map(|s| s.title.as_str()).collect() }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn running_awesome_sections() {
        let out = "   string \"Windows\u{1e}Close\u{1e}Super\u{1f}Shift\u{1f}C\u{1c}Tags\u{1e}View tag\u{1e}Super\u{1f}1…9\u{1d}Super\u{1f}Ctrl\u{1f}1…9\u{1c}Apps\u{1e}Say \"hi\"\u{1e}Super\u{1f}+\"\n";
        let l = Awesome.load(&env(Some(out)));
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(titles(&l), vec!["AWESOME · WINDOWS", "AWESOME · TAGS", "AWESOME · APPS"]);
        assert_eq!(keys_of(&l, "AWESOME · TAGS", "View tag"), vec!["Super+1…9", "Super+Ctrl+1…9"]);
        assert_eq!(keys_of(&l, "AWESOME · APPS", "Say \"hi\""), vec!["Super++"]);
    }

    #[test]
    fn keys_lua_fallback() {
        let l = Awesome.load(&env(Some("error: attempt to index nil")));
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(titles(&l), vec!["AWESOME · WINDOWS", "AWESOME · TAGS", "AWESOME · MEDIA & VOLUME", "AWESOME · SYSTEM"]);
        assert_eq!(keys_of(&l, "AWESOME · SYSTEM", "Show keyboard shortcuts"), vec!["Super+I"]);
        assert_eq!(keys_of(&l, "AWESOME · WINDOWS", "Close"), vec!["Super+Shift+C"]);
        assert_eq!(keys_of(&l, "AWESOME · TAGS", "View previous"), vec!["Super+←"]);
        assert_eq!(keys_of(&l, "AWESOME · MEDIA & VOLUME", "Mute"), vec!["Mute"]);
    }

    #[test]
    fn nothing_available() {
        let mut e = env(None);
        e.vars.insert("XDG_CONFIG_HOME".into(), "/nonexistent".into());
        let l = Awesome.load(&e);
        assert_eq!(l.note.as_deref(), Some("config: awesome not running and keys.lua not found"));
    }
}
