//! Key and modifier names: every notation the sources read (kitty `ctrl+t`,
//! tmux `C-a`, zellij `Ctrl a`, vim `<C-w>`, fzf `ctrl-a`) becomes a Combo of
//! display names, modifiers first in the order Super, Ctrl, Alt, Shift.

pub type Combo = Vec<String>;
pub type Seq = Vec<Combo>;

pub const MOD_ORDER: [&str; 4] = ["Super", "Ctrl", "Alt", "Shift"];

pub fn mod_name(s: &str) -> Option<&'static str> {
    Some(match s.to_ascii_lowercase().as_str() {
        "super" | "mod4" | "cmd" | "win" | "d" | "logo" => "Super",
        "ctrl" | "control" | "c" | "primary" => "Ctrl",
        "alt" | "mod1" | "m" | "a" | "opt" | "option" | "meta" => "Alt",
        "shift" | "s" => "Shift",
        _ => return None,
    })
}

const NAMES: &[(&str, &str)] = &[
    ("return", "Enter"), ("enter", "Enter"), ("cr", "Enter"), ("space", "Space"),
    ("escape", "Esc"), ("esc", "Esc"), ("left", "←"), ("right", "→"), ("up", "↑"), ("down", "↓"),
    ("leftarrow", "←"), ("rightarrow", "→"), ("uparrow", "↑"), ("downarrow", "↓"),
    ("tab", "Tab"), ("btab", "⇧Tab"), ("backspace", "Bksp"), ("bs", "Bksp"), ("bspace", "Bksp"),
    ("delete", "Del"), ("del", "Del"), ("dc", "Del"), ("insert", "Ins"), ("ins", "Ins"), ("ic", "Ins"),
    ("home", "Home"), ("end", "End"),
    ("page_up", "PgUp"), ("pageup", "PgUp"), ("page-up", "PgUp"), ("pgup", "PgUp"), ("ppage", "PgUp"), ("prior", "PgUp"),
    ("page_down", "PgDn"), ("pagedown", "PgDn"), ("page-down", "PgDn"), ("pgdn", "PgDn"), ("npage", "PgDn"),
    ("comma", ","), ("period", "."), ("less", "<"), ("lt", "<"), ("greater", ">"), ("gt", ">"),
    ("equal", "="), ("equals", "="), ("minus", "-"), ("plus", "+"), ("slash", "/"),
    ("backslash", "\\"), ("bslash", "\\"), ("bar", "|"), ("asciicircum", "^"), ("dead_circumflex", "^"),
    ("grave", "`"), ("apostrophe", "'"), ("semicolon", ";"), ("bracketleft", "["), ("bracketright", "]"),
    ("numpadadd", "+"), ("numpadsubtract", "-"),
    ("xf86audioraisevolume", "Vol+"), ("xf86audiolowervolume", "Vol−"), ("xf86audiomute", "Mute"),
    ("xf86audiomicmute", "Mic mute"), ("xf86audioplay", "Play"), ("xf86audionext", "Next"),
    ("xf86audioprev", "Prev"), ("xf86audiostop", "Stop"),
    ("xf86monbrightnessup", "Bright+"), ("xf86monbrightnessdown", "Bright−"),
];

pub fn key_name(s: &str, has_mods: bool) -> String {
    let lower = s.to_ascii_lowercase();
    if let Some((_, n)) = NAMES.iter().find(|(k, _)| *k == lower) {
        return n.to_string();
    }
    // "#10".."#18": X keycodes of the 1..9 row (awesome)
    if let Some(code) = s.strip_prefix('#').and_then(|c| c.parse::<u32>().ok()) {
        if (10..=18).contains(&code) {
            return (code - 9).to_string();
        }
    }
    if lower.len() >= 2 && lower.starts_with('f') && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return lower.to_uppercase();
    }
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return if has_mods { c.to_uppercase().to_string() } else { c.to_string() };
    }
    s.to_string()
}

/// Modifiers (any spelling, unknown ones dropped) in display order, then the key.
pub fn combo(mods: &[&str], key: &str) -> Combo {
    let names: Vec<&str> = mods.iter().filter_map(|m| mod_name(m)).collect();
    let mut c: Combo = MOD_ORDER.iter().filter(|m| names.contains(m)).map(|m| m.to_string()).collect();
    let has_mods = !c.is_empty();
    c.push(key_name(key, has_mods));
    c
}

/// "ctrl+shift+t" / "C-a" / "Ctrl g": strip leading `<mod><sep>` while the
/// prefix names a modifier and something follows; the rest is the key.
pub fn parse_prefixed(s: &str, sep: char) -> Combo {
    let mut mods = Vec::new();
    let mut rest = s.trim();
    while let Some(i) = rest.find(sep) {
        let (head, tail) = (&rest[..i], &rest[i + sep.len_utf8()..]);
        if i == 0 || tail.is_empty() || mod_name(head).is_none() {
            break;
        }
        mods.push(head);
        rest = tail;
    }
    combo(&mods, rest)
}

/// Vim-style key sequence: `<C-w>h`, `gg`, `<leader>f`, a literal space.
pub fn parse_vim(s: &str) -> Seq {
    let mut seq = Vec::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some(end) = rest.find('>').filter(|&e| e > 1) {
                let inner = &rest[1..end];
                if inner.eq_ignore_ascii_case("leader") {
                    seq.push(vec!["Leader".to_string()]);
                } else {
                    seq.push(parse_prefixed(inner, '-'));
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        seq.push(vec![if c == ' ' { "Space".to_string() } else { key_name(&c.to_string(), false) }]);
        rest = &rest[c.len_utf8()..];
    }
    seq
}

pub fn seq_text(seq: &Seq) -> String {
    seq.iter().map(|c| c.join("+")).collect::<Vec<_>>().join(" › ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Combo { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn modifiers_normalised_and_ordered() {
        assert_eq!(combo(&["shift", "Control", "Mod4"], "t"), s(&["Super", "Ctrl", "Shift", "T"]));
        assert_eq!(combo(&["Mod1", "alt"], "x"), s(&["Alt", "X"]));
    }

    #[test]
    fn letters_keep_case_without_mods() {
        assert_eq!(key_name("g", false), "g");
        assert_eq!(key_name("G", false), "G");
        assert_eq!(key_name("g", true), "G");
    }

    #[test]
    fn friendly_names() {
        for (raw, want) in [("Return", "Enter"), ("CR", "Enter"), ("space", "Space"), ("Escape", "Esc"),
            ("Left", "←"), ("LeftArrow", "←"), ("BSpace", "Bksp"), ("DC", "Del"), ("PPage", "PgUp"),
            ("page_down", "PgDn"), ("equal", "="), ("comma", ","), ("BTab", "⇧Tab"), ("#12", "3"),
            ("f11", "F11"), ("XF86AudioMute", "Mute"), ("Home", "Home")] {
            assert_eq!(key_name(raw, false), want, "{raw}");
        }
    }

    #[test]
    fn prefixed_notations() {
        assert_eq!(parse_prefixed("ctrl+shift+t", '+'), s(&["Ctrl", "Shift", "T"]));
        assert_eq!(parse_prefixed("Ctrl g", ' '), s(&["Ctrl", "G"]));
        assert_eq!(parse_prefixed("C-M-Left", '-'), s(&["Ctrl", "Alt", "←"]));
        assert_eq!(parse_prefixed("ctrl-alt-a", '-'), s(&["Ctrl", "Alt", "A"]));
        assert_eq!(parse_prefixed("page-up", '-'), s(&["PgUp"]));
        assert_eq!(parse_prefixed("-", '-'), s(&["-"]));
        assert_eq!(parse_prefixed("ctrl++", '+'), s(&["Ctrl", "+"]));
        assert_eq!(parse_prefixed("%", '-'), s(&["%"]));
    }

    #[test]
    fn vim_notation() {
        assert_eq!(parse_vim("gg"), vec![s(&["g"]), s(&["g"])]);
        assert_eq!(parse_vim("<C-w>h"), vec![s(&["Ctrl", "W"]), s(&["h"])]);
        assert_eq!(parse_vim(" ff"), vec![s(&["Space"]), s(&["f"]), s(&["f"])]);
        assert_eq!(parse_vim("<leader>e"), vec![s(&["Leader"]), s(&["e"])]);
        assert_eq!(parse_vim("<CR>"), vec![s(&["Enter"])]);
        assert_eq!(parse_vim("<"), vec![s(&["<"])]);
    }

    #[test]
    fn seq_text_joins() {
        assert_eq!(seq_text(&vec![s(&["Ctrl", "A"]), s(&["%"])]), "Ctrl+A › %");
    }
}
