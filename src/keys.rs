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
    ("tab", "Tab"), ("btab", "⇧Tab"), ("backtab", "⇧Tab"), ("backspace", "Bksp"), ("bs", "Bksp"), ("bspace", "Bksp"),
    ("delete", "Del"), ("del", "Del"), ("dc", "Del"), ("insert", "Ins"), ("ins", "Ins"), ("ic", "Ins"),
    ("home", "Home"), ("end", "End"),
    ("page_up", "PgUp"), ("pageup", "PgUp"), ("page-up", "PgUp"), ("pgup", "PgUp"), ("ppage", "PgUp"), ("prior", "PgUp"),
    ("page_down", "PgDn"), ("pagedown", "PgDn"), ("page-down", "PgDn"), ("pgdn", "PgDn"), ("npage", "PgDn"),
    ("comma", ","), ("period", "."), ("less", "<"), ("lt", "<"), ("greater", ">"), ("gt", ">"),
    ("equal", "="), ("equals", "="), ("minus", "-"), ("plus", "+"), ("slash", "/"),
    ("backslash", "\\"), ("bslash", "\\"), ("bar", "|"), ("asciicircum", "^"), ("dead_circumflex", "^"),
    ("grave", "`"), ("apostrophe", "'"), ("semicolon", ";"), ("bracketleft", "["), ("bracketright", "]"),
    ("numpadadd", "+"), ("numpadsubtract", "-"),
    ("rubout", "Bksp"), ("ret", "Enter"), ("lfd", "Enter"), ("newline", "Enter"), ("spc", "Space"),
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
    if let Some(code) = s.strip_prefix('#').and_then(|c| c.parse::<u32>().ok())
        && (10..=18).contains(&code) {
            return (code - 9).to_string();
        }
    // alacritty "Key0".."Key9"
    if let Some(d) = lower.strip_prefix("key").filter(|d| d.len() == 1 && d.chars().all(|c| c.is_ascii_digit())) {
        return d.to_string();
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
        if c == '<'
            && let Some(end) = rest.find('>').filter(|&e| e > 1) {
                let inner = &rest[1..end];
                if inner.eq_ignore_ascii_case("leader") {
                    seq.push(vec!["Leader".to_string()]);
                } else {
                    seq.push(parse_prefixed(inner, '-'));
                }
                rest = &rest[end + 1..];
                continue;
            }
        seq.push(vec![if c == ' ' { "Space".to_string() } else { key_name(&c.to_string(), false) }]);
        rest = &rest[c.len_utf8()..];
    }
    seq
}

pub fn seq_text(seq: &Seq) -> String {
    seq.iter().map(|c| c.join("+")).collect::<Vec<_>>().join(" › ")
}

/// Notation of terminal byte sequences: zsh `bindkey` (`^X`, `^[`),
/// readline (`\C-x`, `\M-x`, `\e`, octal) and fish 3 (`\cx`, `\e`, `\x7f`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Term { Zsh, Readline, Fish3 }

#[derive(Clone, Copy, Debug, PartialEq)]
enum Unit { Ctrl(char), Esc, Meta(char), Ch(char) }

fn byte_unit(b: u32) -> Unit {
    match b {
        0x1b => Unit::Esc,
        0x7f => Unit::Ctrl('?'),
        0..=0x1f => Unit::Ctrl(char::from_u32(b + 0x40).unwrap_or('?')),
        _ => Unit::Ch(char::from_u32(b).unwrap_or('?')),
    }
}

fn units(s: &str, style: Term) -> Vec<Unit> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '^' && style == Term::Zsh {
            match it.next() {
                Some('[') => out.push(Unit::Esc),
                Some(n) => out.push(Unit::Ctrl(n.to_ascii_uppercase())),
                None => out.push(Unit::Ch('^')),
            }
            continue;
        }
        if c != '\\' {
            out.push(Unit::Ch(c));
            continue;
        }
        match it.next() {
            Some('C') if style == Term::Readline && it.peek() == Some(&'-') => {
                it.next();
                if let Some(n) = it.next() { out.push(Unit::Ctrl(n.to_ascii_uppercase())) }
            }
            Some('M') if it.peek() == Some(&'-') => {
                it.next();
                if let Some(n) = it.next() { out.push(Unit::Meta(n)) }
            }
            Some('c') if style == Term::Fish3 => if let Some(n) = it.next() { out.push(Unit::Ctrl(n.to_ascii_uppercase())) },
            Some('b') if style == Term::Fish3 => out.push(Unit::Ctrl('?')),
            Some('e') | Some('E') => out.push(Unit::Esc),
            Some('t') => out.push(Unit::Ctrl('I')),
            Some('r') | Some('n') => out.push(Unit::Ctrl('M')),
            Some('x') if style == Term::Fish3 => {
                let hex: String = (0..2).filter_map(|_| it.next_if(|d| d.is_ascii_hexdigit())).collect();
                out.push(u32::from_str_radix(&hex, 16).map(byte_unit).unwrap_or(Unit::Ch('x')));
            }
            Some(d) if style == Term::Readline && d.is_digit(8) => {
                let mut oct = d.to_string();
                while oct.len() < 3 { match it.next_if(|x| x.is_digit(8)) { Some(x) => oct.push(x), None => break } }
                out.push(byte_unit(u32::from_str_radix(&oct, 8).unwrap_or(0)));
            }
            Some(o) => out.push(Unit::Ch(o)),
            None => out.push(Unit::Ch('\\')),
        }
    }
    out
}

fn ctrl_combo(c: char, alt: bool) -> Combo {
    let plain = |name: &str| if alt { combo(&["alt"], name) } else { vec![name.to_string()] };
    let mods: &[&str] = if alt { &["ctrl", "alt"] } else { &["ctrl"] };
    match c {
        'I' => plain("Tab"),
        'M' | 'J' => plain("Enter"),
        '?' => plain("Bksp"),
        '[' => plain("Esc"),
        '@' => combo(mods, "Space"),
        _ => combo(mods, &c.to_string()),
    }
}

/// CSI (`ESC [`) / SS3 (`ESC O`) sequence body (params + final) → key.
fn csi(body: &str, ss3: bool) -> Option<Combo> {
    let fin = body.chars().last()?;
    let params = &body[..body.len() - fin.len_utf8()];
    let mut nums = params.split(';').map(|p| p.parse::<u32>().ok());
    let (first, mut modp) = (nums.next().flatten(), nums.next().flatten());
    // legacy rxvt/xterm `ESC [ 5 D`: on a letter final the lone parameter is the modifier
    if fin != '~' && modp.is_none() && first.is_some_and(|f| f > 1) { modp = first; }
    let key: String = match (fin, first) {
        ('A', _) => "↑".into(), ('B', _) => "↓".into(), ('C', _) => "→".into(), ('D', _) => "←".into(),
        ('H', _) => "Home".into(), ('F', _) => "End".into(), ('Z', _) => "⇧Tab".into(),
        ('P', _) if ss3 => "F1".into(), ('Q', _) if ss3 => "F2".into(),
        ('R', _) if ss3 => "F3".into(), ('S', _) if ss3 => "F4".into(),
        ('~', Some(1 | 7)) => "Home".into(), ('~', Some(4 | 8)) => "End".into(),
        ('~', Some(2)) => "Ins".into(), ('~', Some(3)) => "Del".into(),
        ('~', Some(5)) => "PgUp".into(), ('~', Some(6)) => "PgDn".into(),
        ('~', Some(n @ 11..=15)) => format!("F{}", n - 10),
        ('~', Some(n @ 17..=21)) => format!("F{}", n - 11),
        ('~', Some(n @ 23..=24)) => format!("F{}", n - 12),
        _ => return None,
    };
    let mods: Vec<&str> = match modp {
        Some(m) if m >= 2 => [(1, "shift"), (2, "alt"), (4, "ctrl")].iter()
            .filter(|(bit, _)| (m - 1) & bit != 0).map(|(_, n)| *n).collect(),
        _ => vec![],
    };
    Some(combo(&mods, &key))
}

/// ESC/Meta + a character; an upper-case letter is a shifted key (Alt+Shift+C ≠ Alt+C).
fn alt_char(c: char) -> Combo {
    if c == ' ' { return combo(&["alt"], "Space"); }
    let mods: &[&str] = if c.is_ascii_uppercase() { &["alt", "shift"] } else { &["alt"] };
    combo(mods, &c.to_string())
}

pub fn parse_term(s: &str, style: Term) -> Seq {
    let u = units(s, style);
    let mut seq = Vec::new();
    let mut i = 0;
    while i < u.len() {
        match u[i] {
            Unit::Esc if matches!(u.get(i + 1), Some(Unit::Ch('[' | 'O'))) => {
                let ss3 = u[i + 1] == Unit::Ch('O');
                let (mut j, mut body) = (i + 2, String::new());
                while let Some(Unit::Ch(c)) = u.get(j) {
                    body.push(*c);
                    j += 1;
                    if c.is_ascii_alphabetic() || *c == '~' { break; }
                }
                let raw = || vec![format!("Esc{}{body}", if ss3 { 'O' } else { '[' })];
                seq.push(csi(&body, ss3).unwrap_or_else(raw));
                i = j;
            }
            Unit::Esc => match u.get(i + 1) {
                Some(&Unit::Ch(c)) => { seq.push(alt_char(c)); i += 2; }
                Some(&Unit::Ctrl(c)) => { seq.push(ctrl_combo(c, true)); i += 2; }
                _ => { seq.push(vec!["Esc".into()]); i += 1; }
            },
            Unit::Meta(c) => { seq.push(alt_char(c)); i += 1; }
            Unit::Ctrl(c) => { seq.push(ctrl_combo(c, false)); i += 1; }
            Unit::Ch(c) => { seq.push(vec![if c == ' ' { "Space".into() } else { key_name(&c.to_string(), false) }]); i += 1; }
        }
    }
    seq
}

/// Emacs key descriptions: space-separated steps, `C-`/`M-`/`S-` prefixes,
/// `<f1>`-style names, `RET` `SPC` `TAB` `ESC` and `DEL` (= backspace).
pub fn parse_emacs(s: &str) -> Seq {
    s.split_whitespace().map(|step| {
        let inner = step.strip_prefix('<').and_then(|x| x.strip_suffix('>')).filter(|x| !x.is_empty()).unwrap_or(step);
        let mut c = parse_prefixed(inner, '-');
        if step.rsplit('-').next().is_some_and(|k| k == "DEL")
            && let Some(k) = c.last_mut() { *k = "Bksp".into(); }
        c
    }).collect()
}

/// nano: `^X` Ctrl, `M-X` Alt, `Sh-M-X` Alt+Shift, plain names (`F1`, `Ins`, `Bsp`).
pub fn parse_nano(s: &str) -> Combo {
    if let Some(k) = s.strip_prefix("Sh-M-").filter(|k| !k.is_empty()) { return combo(&["alt", "shift"], k); }
    if let Some(k) = s.strip_prefix("M-").filter(|k| !k.is_empty()) { return combo(&["alt"], k); }
    if let Some(k) = s.strip_prefix('^').filter(|k| !k.is_empty()) { return combo(&["ctrl"], k); }
    combo(&[], if s.eq_ignore_ascii_case("bsp") { "Bksp" } else { s })
}

/// micro: `Ctrl-s`, `CtrlShift-Left`, `CtrlShiftUp`, `Ctrl-Shift-Up`, `AltUp`,
/// `Alt-,`, `Ctrl--`: Ctrl/Alt/Shift prefixes, each optionally followed by `-`.
pub fn parse_micro(s: &str) -> Combo {
    let mut rest = s;
    let mut mods = Vec::new();
    while let Some(m) = ["Ctrl", "Alt", "Shift"].iter().find(|m| rest.starts_with(**m) && rest.len() > m.len()) {
        mods.push(m.to_lowercase());
        rest = &rest[m.len()..];
        if rest.len() > 1 { rest = rest.strip_prefix('-').unwrap_or(rest); }
    }
    let mods: Vec<&str> = mods.iter().map(String::as_str).collect();
    combo(&mods, rest)
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
            ("f11", "F11"), ("XF86AudioMute", "Mute"), ("Home", "Home"), ("Key0", "0")] {
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

    fn t(s: &str, style: Term) -> String { seq_text(&parse_term(s, style)) }

    #[test]
    fn term_zsh() {
        assert_eq!(t("^X^E", Term::Zsh), "Ctrl+X › Ctrl+E");
        assert_eq!(t("^[[A", Term::Zsh), "↑");
        assert_eq!(t("^[[1;5C", Term::Zsh), "Ctrl+→");
        assert_eq!(t("^[[1;3D", Term::Zsh), "Alt+←");
        assert_eq!(t("^[b", Term::Zsh), "Alt+B");
        assert_eq!(t("^?", Term::Zsh), "Bksp");
        assert_eq!(t("^I", Term::Zsh), "Tab");
        assert_eq!(t("^J", Term::Zsh), "Enter");
        assert_eq!(t("^[", Term::Zsh), "Esc");
        assert_eq!(t("^[^H", Term::Zsh), "Ctrl+Alt+H");
        assert_eq!(t("^[[3~", Term::Zsh), "Del");
        assert_eq!(t("^[[3;5~", Term::Zsh), "Ctrl+Del");
        assert_eq!(t("^[OH", Term::Zsh), "Home");
        assert_eq!(t("^[[Z", Term::Zsh), "⇧Tab");
        assert_eq!(t("^[[15~", Term::Zsh), "F5");
        assert_eq!(t("gg", Term::Zsh), "g › g");
        assert_eq!(t("^[ ", Term::Zsh), "Alt+Space");
    }

    #[test]
    fn term_readline() {
        assert_eq!(t("\\C-x\\C-e", Term::Readline), "Ctrl+X › Ctrl+E");
        assert_eq!(t("\\e[1;5C", Term::Readline), "Ctrl+→");
        assert_eq!(t("\\M-b", Term::Readline), "Alt+B");
        assert_eq!(t("\\eb", Term::Readline), "Alt+B");
        assert_eq!(t("\\C-?", Term::Readline), "Bksp");
        assert_eq!(t("\\177", Term::Readline), "Bksp");
        assert_eq!(t("\\\\", Term::Readline), "\\");
        assert_eq!(t("\\\"", Term::Readline), "\"");
        assert_eq!(t("\\t", Term::Readline), "Tab");
    }

    #[test]
    fn term_fish3() {
        assert_eq!(t("\\cx", Term::Fish3), "Ctrl+X");
        assert_eq!(t("\\e\\[A", Term::Fish3), "↑");
        assert_eq!(t("\\x7f", Term::Fish3), "Bksp");
        assert_eq!(t("\\e.", Term::Fish3), "Alt+.");
        assert_eq!(t("\\r", Term::Fish3), "Enter");
    }

    #[test]
    fn term_unknown_sequences_verbatim() {
        assert_eq!(t("^[[200~", Term::Zsh), "Esc[200~");
        assert_eq!(t("\\e[57399u", Term::Readline), "Esc[57399u");
        assert_eq!(t("^[[", Term::Zsh), "Esc[");
        assert_eq!(t("^", Term::Zsh), "^");
        assert_eq!(t("\\", Term::Readline), "\\");
        assert_eq!(t("", Term::Zsh), "");
    }

    #[test]
    fn term_legacy_ctrl_arrows_and_alt_case() {
        assert_eq!(t("\\e[5D", Term::Readline), "Ctrl+←");    // rxvt/old xterm: param 5 = Ctrl
        assert_eq!(t("^[[5C", Term::Zsh), "Ctrl+→");
        assert_eq!(t("^[[2A", Term::Zsh), "Shift+↑");
        assert_eq!(t("^[C", Term::Zsh), "Alt+Shift+C");           // ESC + uppercase is a shifted letter
        assert_eq!(t("^[c", Term::Zsh), "Alt+C");
        assert_eq!(t("\\eL", Term::Readline), "Alt+Shift+L");
    }

    #[test]
    fn readline_key_names() {
        assert_eq!(key_name("RUBOUT", false), "Bksp");
        assert_eq!(key_name("RET", false), "Enter");
        assert_eq!(key_name("SPC", false), "Space");
    }

    #[test]
    fn emacs_notation() {
        assert_eq!(seq_text(&parse_emacs("C-x C-f")), "Ctrl+X › Ctrl+F");
        assert_eq!(seq_text(&parse_emacs("M-x")), "Alt+X");
        assert_eq!(seq_text(&parse_emacs("C-M-a")), "Ctrl+Alt+A");
        assert_eq!(seq_text(&parse_emacs("<f1> k")), "F1 › k");
        assert_eq!(seq_text(&parse_emacs("C-x 4 f")), "Ctrl+X › 4 › f");
        assert_eq!(seq_text(&parse_emacs("RET")), "Enter");
        assert_eq!(seq_text(&parse_emacs("M-DEL")), "Alt+Bksp");
        assert_eq!(seq_text(&parse_emacs("C-/")), "Ctrl+/");
        assert_eq!(seq_text(&parse_emacs("M->")), "Alt+>");
        assert_eq!(seq_text(&parse_emacs("M-<")), "Alt+<");
    }

    #[test]
    fn nano_notation() {
        for (raw, want) in [("^S", "Ctrl+S"), ("M-U", "Alt+U"), ("M-u", "Alt+U"), ("Sh-M-C", "Alt+Shift+C"),
            ("F6", "F6"), ("^Space", "Ctrl+Space"), ("Bsp", "Bksp"), ("^Left", "Ctrl+←"), ("Ins", "Ins"), ("M-\\", "Alt+\\"), ("^", "^")] {
            assert_eq!(parse_nano(raw).join("+"), want, "{raw}");
        }
    }

    #[test]
    fn micro_notation() {
        for (raw, want) in [("Ctrl-s", "Ctrl+S"), ("Alt-,", "Alt+,"), ("CtrlShift-Left", "Ctrl+Shift+←"),
            ("AltShift-Up", "Alt+Shift+↑"), ("F1", "F1"), ("Home", "Home"), ("Ctrl--", "Ctrl+-"), ("CtrlAlt-x", "Ctrl+Alt+X"),
            ("CtrlShiftUp", "Ctrl+Shift+↑"), ("Ctrl-Shift-Up", "Ctrl+Shift+↑"), ("AltUp", "Alt+↑"), ("CtrlHome", "Ctrl+Home"), ("Shift", "Shift")] {
            assert_eq!(parse_micro(raw).join("+"), want, "{raw}");
        }
    }
}
