//! Colours of the active awesome theme (the infocard palette the Mod+i
//! sheet uses), with built-in catppuccin as the fallback.
use crate::sources::Env;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Debug, PartialEq)]
pub struct Theme { pub name: String, pub bg: Rgb, pub fg: Rgb, pub muted: Rgb, pub accent: Rgb, pub track: Rgb }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorMode { True, Ansi256, None }

impl ColorMode {
    pub fn detect(env: &Env) -> ColorMode {
        if env.vars.contains_key("NO_COLOR") { return ColorMode::None; }
        match env.var("COLORTERM") { Some("truecolor") | Some("24bit") => ColorMode::True, _ => ColorMode::Ansi256 }
    }
}

pub fn hex(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#')?;
    if !(h.len() == 6 || h.len() == 8) || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Rgb(p(0)?, p(2)?, p(4)?))
}

enum Rhs { Lit(Rgb), Ref(String) }

/// `theme.<path> = "#rrggbb"` or `= theme.<path>` assignments, references
/// followed up to 8 steps; the last assignment of a path wins, as in Lua.
pub fn resolve(lua: &str) -> HashMap<String, Rgb> {
    let mut raw: HashMap<String, Rhs> = HashMap::new();
    for line in lua.lines() {
        let Some(rest) = line.trim().strip_prefix("theme.") else { continue };
        let Some((lhs, rhs)) = rest.split_once('=') else { continue };
        let (lhs, rhs) = (lhs.trim().to_string(), rhs.trim());
        if let Some(q) = rhs.strip_prefix('"') {
            if let Some(c) = q.split('"').next().and_then(hex) { raw.insert(lhs, Rhs::Lit(c)); }
        } else if let Some(r) = rhs.strip_prefix("theme.") {
            let path: String = r.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.').collect();
            raw.insert(lhs, Rhs::Ref(path));
        }
    }
    let mut out = HashMap::new();
    for key in raw.keys() {
        let mut k = key.clone();
        for _ in 0..8 {
            match raw.get(&k) {
                Some(Rhs::Lit(c)) => { out.insert(key.clone(), *c); break; }
                Some(Rhs::Ref(r)) => k = r.clone(),
                None => break,
            }
        }
    }
    out
}

/// alpha/255 of fg over bg.
pub fn blend(fg: Rgb, bg: Rgb, alpha: u8) -> Rgb {
    let mix = |f: u8, b: u8| ((f as u32 * alpha as u32 + b as u32 * (255 - alpha as u32) + 127) / 255) as u8;
    Rgb(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}

pub fn catppuccin() -> Theme {
    let (bg, fg) = (Rgb(0x31, 0x32, 0x44), Rgb(0xcd, 0xd6, 0xf4));
    Theme { name: "catppuccin".into(), bg, fg, muted: blend(fg, bg, 0x99), accent: Rgb(0x89, 0xb4, 0xfa), track: Rgb(0x45, 0x47, 0x5a) }
}

pub fn from_lua(name: &str, lua: &str) -> Theme {
    let m = resolve(lua);
    let base = catppuccin();
    let pick = |keys: &[&str], fallback: Rgb| keys.iter().find_map(|k| m.get(*k).copied()).unwrap_or(fallback);
    let bg = pick(&["infocard_bg", "bg_normal"], base.bg);
    let fg = pick(&["infocard_fg", "fg_normal"], base.fg);
    Theme {
        name: name.into(), bg, fg,
        // infocard's own fallback: fg at alpha 0x99
        muted: pick(&["infocard_fg_muted"], blend(fg, bg, 0x99)),
        accent: pick(&["infocard_accent", "menu_accent", "colors.base0D"], base.accent),
        track: pick(&["infocard_border_color", "colors.base03"], base.track),
    }
}

/// Cached theme name, else config.lua's default_theme, else built-in catppuccin.
pub fn load(env: &Env) -> Theme {
    let theme_file = |name: &str| env.config(&format!("awesome/themes/{name}/theme.lua"));
    let cached = env.read(&env.cache("awesome/theme")).map(|t| t.lines().next().unwrap_or("").trim().to_string());
    let configured = env.read(&env.config("awesome/config.lua")).and_then(|t| {
        t.lines().find_map(|l| l.trim().strip_prefix("config.default_theme")
            .and_then(|r| r.trim().strip_prefix('='))
            .and_then(|r| r.trim().strip_prefix('"'))
            .and_then(|r| r.split('"').next()).map(String::from))
    });
    [cached, configured].into_iter().flatten()
        .filter(|n| !n.is_empty() && !n.contains('/'))
        .find_map(|n| env.read(&theme_file(&n)).map(|lua| from_lua(&n, &lua)))
        .unwrap_or_else(catppuccin)
}

/// Nearest xterm-256 colour: 6×6×6 cube or the 24-step grey ramp.
pub fn to_256(c: Rgb) -> u8 {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let idx = |v: u8| STEPS.iter().enumerate().min_by_key(|(_, s)| (v as i32 - **s as i32).abs()).unwrap().0;
    let (r, g, b) = (idx(c.0), idx(c.1), idx(c.2));
    let cube = Rgb(STEPS[r], STEPS[g], STEPS[b]);
    let avg = (c.0 as u32 + c.1 as u32 + c.2 as u32) / 3;
    let gi = ((avg.saturating_sub(8) + 5) / 10).min(23) as u8;
    let gv = 8 + 10 * gi;
    let dist = |a: Rgb| (a.0 as i32 - c.0 as i32).pow(2) + (a.1 as i32 - c.1 as i32).pow(2) + (a.2 as i32 - c.2 as i32).pow(2);
    if dist(Rgb(gv, gv, gv)) < dist(cube) { 232 + gi } else { 16 + 36 * r as u8 + 6 * g as u8 + b as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::FakeRunner;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fixture_env() -> Env {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/theme");
        let mut e = Env::test(&root, FakeRunner(HashMap::new()));
        e.vars.insert("XDG_CONFIG_HOME".into(), root.join("config").display().to_string());
        e.vars.insert("XDG_CACHE_HOME".into(), root.join("cache").display().to_string());
        e
    }

    #[test]
    fn resolves_references_and_comments() {
        let lua = "theme.colors = {}\ntheme.colors.base02 = \"#313244\" -- surface0\n-- theme.x = \"#ffffff\"\ntheme.a = theme.colors.base02\ntheme.b = theme.a\ntheme.loop = theme.loop\ntheme.size = dpi(12)\n";
        let m = resolve(lua);
        assert_eq!(m.get("a"), Some(&Rgb(0x31, 0x32, 0x44)));
        assert_eq!(m.get("b"), Some(&Rgb(0x31, 0x32, 0x44)));
        assert!(!m.contains_key("x") && !m.contains_key("loop") && !m.contains_key("size"));
    }

    #[test]
    fn real_catppuccin_colours() {
        let t = load(&fixture_env());   // cached "solarized-dark" missing → config.lua default
        assert_eq!(t.name, "catppuccin");
        assert_eq!(t.bg, hex("#313244").unwrap());
        assert_eq!(t.fg, hex("#cdd6f4").unwrap());
        assert_eq!(t.muted, hex("#8f94ae").unwrap());
        assert_eq!(t.accent, hex("#89b4fa").unwrap());
        assert_eq!(t.track, hex("#45475a").unwrap());
    }

    #[test]
    fn nothing_found_is_builtin_catppuccin() {
        let mut e = fixture_env();
        e.vars.insert("XDG_CONFIG_HOME".into(), "/nonexistent".into());
        assert_eq!(load(&e), catppuccin());
    }

    #[test]
    fn hex_and_256() {
        assert_eq!(hex("#89b4fa"), Some(Rgb(0x89, 0xb4, 0xfa)));
        assert_eq!(hex("#89b4faCC"), Some(Rgb(0x89, 0xb4, 0xfa)));
        assert_eq!(hex("89b4fa"), None);
        assert_eq!(to_256(Rgb(0, 0, 0)), 16);
        assert_eq!(to_256(Rgb(255, 255, 255)), 231);
        assert_eq!(to_256(Rgb(128, 128, 128)), 244);
    }

    #[test]
    fn colour_mode() {
        let mut e = fixture_env();
        assert_eq!(ColorMode::detect(&e), ColorMode::Ansi256);
        e.vars.insert("COLORTERM".into(), "truecolor".into());
        assert_eq!(ColorMode::detect(&e), ColorMode::True);
        e.vars.insert("NO_COLOR".into(), "".into());
        assert_eq!(ColorMode::detect(&e), ColorMode::None);
    }
}
