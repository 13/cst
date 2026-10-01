# cst Shells + App Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add zsh, bash, fish, nushell and readline (inputrc) sheets, and an app picker that plain `cst` opens; ship as v0.2.0.

**Architecture:** One shared terminal-key parser (`keys::parse_term`) and one shell helper module (`sources/shell.rs`: widget names, drop list, readline line parser) feed five new `Source`s that follow the existing pattern (live layer over bundled defaults, `layer()`, fixtures). The picker is a second screen in the existing UI: `State.screen`, `input::apply_picker`, `layout::picker_frame`.

**Tech Stack:** Rust (existing crate), no new crates.

**Spec:** `docs/superpowers/specs/2026-10-02-cst-shells-picker-design.md`

**Deviation from the spec (deliberate):** nushell's live layer asks `nu` for tab-separated lines with NUON-encoded fields instead of JSON, so `cst` needs no JSON parser (the crate list stays crossterm, toml, unicode-width).

## Global Constraints

- No new crates.
- App order: awesome, kitty, wezterm, alacritty, tmux, zellij, zsh, bash, fish, nushell, readline, nvim, yazi, lazygit, fzf.
- Shell widgets dropped everywhere: `self-insert`, `self-insert-unmeta`, `undefined-key`, `digit-argument`, `neg-argument`, `do-lowercase-version`, `bracketed-paste`, `bracketed-paste-begin`, `beep`.
- Every new source: live layer through `Env::run`/`Env::read`, bundled defaults in `src/defaults/<app>.txt`, failure → defaults + `⚠ config: …` note.
- Picker: plain `cst` only; `cst <app>` opens the sheet directly; `All apps` is always the first entry.
- All existing tests keep passing; clippy `-D warnings` clean.

## Review Focus

1. Unknown or exotic escape sequences (`^[[200~`, `\e[57399u`, a lone `^[`) must not panic and must render as something readable (verbatim). Test: Task 1 `term_unknown_sequences_verbatim`.
2. A shell whose rc prints to stdout (banners, `neofetch` in `.zshrc`) must not break parsing: unmatched lines are ignored. Test: Task 3 fixture starts with a banner line.
3. A user with an empty `$INPUTRC` / no inputrc must not see a readline sheet, and one with `$include` loops must not hang. Tests: Task 5 `not_installed_without_inputrc`, `include_loop_terminates`.
4. Picker with a filter matching nothing, Enter, then Esc: no panic, sensible state (All apps still selectable). Test: Task 8 `filter_without_matches_keeps_all_apps`.
5. Very small terminal while the picker is open (< 20×5 and a 1-column picker taller than the screen): no panic, selection stays visible. Tests: Task 9 `picker_too_small`, `picker_scrolls_to_selection`.

## File Structure

```
src/keys.rs                    + Term, parse_term (Task 1)
src/sources/mod.rs             Source::installed, load_installed uses it, all() order (Tasks 1, 3–7)
src/sources/shell.rs           DROP, widget_desc, readline_bind (Task 2)
src/sources/zsh.rs             (Task 3)    src/defaults/zsh.txt
src/sources/bash.rs            (Task 4)    src/defaults/readline.txt (shared by bash + readline)
src/sources/readline.rs        (Task 5)
src/sources/fish.rs            (Task 6)    src/defaults/fish.txt
src/sources/nushell.rs         (Task 7)    src/defaults/nushell.txt
src/ui/input.rs                Screen, picker state, picker_entries, apply_picker, sheet Esc rule (Task 8)
src/ui/layout.rs               Entry, picker_cols, picker_frame (Task 9)
src/ui/mod.rs, src/main.rs     wiring: picker on plain `cst`, entries (Task 10)
README.md, docs/*.svg          shells + picker docs, screenshots (Task 11)
tests/fixtures/{zsh,bash,readline,fish,nushell}/
```

---

### Task 1: `Source::installed` and `keys::parse_term`

**Files:**
- Modify: `src/keys.rs`, `src/sources/mod.rs`

**Interfaces:**
- Produces: `pub enum Term { Zsh, Readline, Fish3 }`; `pub fn parse_term(s: &str, style: Term) -> Seq`; `Source::installed(&self, env: &Env) -> bool` (default `env.which(self.binary())`); keys `NAMES` gains `("rubout","Bksp")`, `("ret","Enter")`, `("lfd","Enter")`, `("newline","Enter")`, `("spc","Space")`.

- [ ] **Step 1: Failing tests** — append to `keys::tests`:

```rust
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
    fn readline_key_names() {
        assert_eq!(key_name("RUBOUT", false), "Bksp");
        assert_eq!(key_name("RET", false), "Enter");
        assert_eq!(key_name("SPC", false), "Space");
    }
```

and to `sources::tests`:

```rust
    struct NoBinary;
    impl Source for NoBinary {
        fn app(&self) -> &'static str { "nobinary" }
        fn binary(&self) -> &'static str { "definitely-not-installed-cst" }
        fn installed(&self, _: &Env) -> bool { true }
        fn load(&self, _: &Env) -> Loaded { layer(vec![Binding::new("", kp("a"), "A")], Ok(None), true).into_loaded("nobinary") }
    }

    #[test]
    fn installed_override_is_used() {
        let srcs: Vec<Box<dyn Source>> = vec![Box::new(NoBinary)];
        assert_eq!(load_installed(&srcs, &Env::system()).len(), 1);
    }
```

- [ ] **Step 2: Run** `cargo test -q --lib` — Expected: compile errors (`Term`, `parse_term`, `installed` not defined).

- [ ] **Step 3: Implement.** In `src/sources/mod.rs`, add to the trait and use it:

```rust
pub trait Source: Sync {
    fn app(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    /// Whether to show this app; most sources: its binary is on PATH.
    fn installed(&self, env: &Env) -> bool { env.which(self.binary()) }
    fn load(&self, env: &Env) -> Loaded;
}
```

and in `load_installed` replace `.filter(|s| env.which(s.binary()))` with `.filter(|s| s.installed(env))`.

In `src/keys.rs` add the five `NAMES` entries listed above, then:

```rust
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
    let (first, modp) = (nums.next().flatten(), nums.next().flatten());
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
                Some(&Unit::Ch(c)) => {
                    let k = if c == ' ' { "Space".to_string() } else { c.to_string() };
                    seq.push(combo(&["alt"], &k));
                    i += 2;
                }
                Some(&Unit::Ctrl(c)) => { seq.push(ctrl_combo(c, true)); i += 2; }
                _ => { seq.push(vec!["Esc".into()]); i += 1; }
            },
            Unit::Meta(c) => { seq.push(combo(&["alt"], &c.to_string())); i += 1; }
            Unit::Ctrl(c) => { seq.push(ctrl_combo(c, false)); i += 1; }
            Unit::Ch(c) => { seq.push(vec![if c == ' ' { "Space".into() } else { key_name(&c.to_string(), false) }]); i += 1; }
        }
    }
    seq
}
```

`combo(&["alt"], "Space")` relies on `key_name("Space")` → `"Space"`; `combo(&["alt"], "Bksp")` keeps `"Bksp"` (multi-char names pass through).

- [ ] **Step 4: Run** `cargo test -q --lib` — Expected: all pass (earlier 72 + 6 new).

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: terminal key-sequence parser for shells; Source::installed"`

---

### Task 2: `sources/shell.rs` — widget names, drop list, readline lines

**Files:**
- Create: `src/sources/shell.rs`; Modify: `src/sources/mod.rs` (`pub mod shell;`)

**Interfaces:**
- Consumes: `keys::{parse_term, parse_prefixed, Term, Seq, Combo}`, `sources::humanize`
- Produces: `pub const DROP: &[&str]`; `pub fn widget_desc(name: &str) -> String`; `pub fn readline_bind(line: &str) -> Option<(Seq, String)>` (None for non-binding lines and dropped widgets); `pub fn readline_keyname(name: &str) -> Combo`.

- [ ] **Step 1: Failing tests** (bottom of the new `src/sources/shell.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;

    #[test]
    fn widget_names() {
        assert_eq!(widget_desc("accept-line"), "Run command");
        assert_eq!(widget_desc("history-incremental-search-backward"), "Search history backward");
        assert_eq!(widget_desc("vi-forward-char"), "Forward one char");      // vi- prefix stripped, then table
        assert_eq!(widget_desc("vi-join"), "Join");                           // vi- stripped, humanized
        assert_eq!(widget_desc("__fish_paginate"), "Paginate");
        assert_eq!(widget_desc("fish_clipboard_copy"), "Clipboard copy");
        assert_eq!(widget_desc("my-widget"), "My widget");
    }

    fn rb(line: &str) -> Option<(String, String)> { readline_bind(line).map(|(s, d)| (seq_text(&s), d)) }

    #[test]
    fn readline_lines() {
        assert_eq!(rb("\"\\C-x\\C-e\": edit-and-execute-command"), Some(("Ctrl+X › Ctrl+E".into(), "Edit command in $EDITOR".into())));
        assert_eq!(rb("\"\\e[1;5C\": forward-word"), Some(("Ctrl+→".into(), "Forward one word".into())));
        assert_eq!(rb("Control-a: beginning-of-line"), Some(("Ctrl+A".into(), "Start of line".into())));
        assert_eq!(rb("Meta-DEL: backward-kill-word"), Some(("Alt+Bksp".into(), "Delete previous word".into())));
        assert_eq!(rb("\"\\C-xg\": \"git status\\n\""), Some(("Ctrl+X › g".into(), "Macro: git status\\n".into())));
        assert_eq!(rb("\"a\": self-insert"), None);
        assert_eq!(rb("# emacs-editing-mode (not bound)"), None);
        assert_eq!(rb("set editing-mode vi"), None);
        assert_eq!(rb("\"\\\"\": weird \" quote"), Some(("\"".into(), "Weird".into())));
        assert_eq!(rb("garbage"), None);
    }
}
```

- [ ] **Step 2: Run** `cargo test -q --lib shell` (after adding `pub mod shell;` to `sources/mod.rs`) — Expected: compile errors.

- [ ] **Step 3: Implement** (above the tests):

```rust
//! Shared by the shell sources: widget/function names → descriptions, the
//! widgets never shown, and readline `"keys": function` lines.
use super::humanize;
use crate::keys::{parse_prefixed, parse_term, Combo, Seq, Term};

/// Bound everywhere, never interesting on a cheatsheet.
pub const DROP: &[&str] = &[
    "self-insert", "self-insert-unmeta", "undefined-key", "digit-argument", "neg-argument",
    "do-lowercase-version", "bracketed-paste", "bracketed-paste-begin", "beep",
];

/// zsh widgets, readline functions and fish input functions share most names.
const NAMES: &[(&str, &str)] = &[
    ("accept-line", "Run command"), ("execute", "Run command"),
    ("beginning-of-line", "Start of line"), ("end-of-line", "End of line"),
    ("backward-char", "Back one char"), ("forward-char", "Forward one char"),
    ("backward-word", "Back one word"), ("forward-word", "Forward one word"),
    ("emacs-backward-word", "Back one word"), ("emacs-forward-word", "Forward one word"),
    ("backward-delete-char", "Delete previous char"), ("backward-delete-char-unix", "Delete previous char"),
    ("delete-char", "Delete char"), ("delete-char-or-list", "Delete char or list completions"),
    ("kill-line", "Delete to end of line"), ("backward-kill-line", "Delete to start of line"),
    ("unix-line-discard", "Delete to start of line"), ("kill-whole-line", "Delete line"),
    ("backward-kill-word", "Delete previous word"), ("unix-word-rubout", "Delete previous word"),
    ("kill-word", "Delete next word"), ("yank", "Paste deleted text"), ("yank-pop", "Cycle pasted text"),
    ("clear-screen", "Clear screen"), ("clear-display", "Clear screen"),
    ("history-incremental-search-backward", "Search history backward"),
    ("history-incremental-search-forward", "Search history forward"),
    ("history-incremental-pattern-search-backward", "Search history backward"),
    ("reverse-search-history", "Search history backward"), ("forward-search-history", "Search history forward"),
    ("up-line-or-history", "Previous command"), ("down-line-or-history", "Next command"),
    ("up-history", "Previous command"), ("down-history", "Next command"),
    ("previous-history", "Previous command"), ("next-history", "Next command"),
    ("up-or-search", "Previous command"), ("down-or-search", "Next command"),
    ("up-line-or-beginning-search", "Previous matching command"), ("down-line-or-beginning-search", "Next matching command"),
    ("history-search-backward", "Previous matching command"), ("history-search-forward", "Next matching command"),
    ("history-beginning-search-backward", "Previous matching command"), ("history-beginning-search-forward", "Next matching command"),
    ("expand-or-complete", "Complete"), ("complete", "Complete"), ("menu-complete", "Cycle completions"),
    ("reverse-menu-complete", "Cycle completions backward"), ("list-choices", "List completions"),
    ("possible-completions", "List completions"), ("complete-and-search", "Complete and search"),
    ("transpose-chars", "Swap chars"), ("transpose-words", "Swap words"),
    ("undo", "Undo"), ("redo", "Redo"),
    ("edit-command-line", "Edit command in $EDITOR"), ("edit-and-execute-command", "Edit command in $EDITOR"),
    ("edit_command_buffer", "Edit command in $EDITOR"),
    ("push-line", "Park command"), ("push-line-or-edit", "Park command"),
    ("capitalize-word", "Capitalize word"), ("up-case-word", "Uppercase word"), ("upcase-word", "Uppercase word"),
    ("down-case-word", "Lowercase word"), ("downcase-word", "Lowercase word"),
    ("insert-last-word", "Insert last argument"), ("yank-last-arg", "Insert last argument"),
    ("history-token-search-backward", "Insert last argument"),
    ("quoted-insert", "Insert next key literally"), ("send-break", "Cancel"), ("abort", "Cancel"),
    ("cancel-commandline", "Cancel"), ("cmd-mode", "Normal mode"), ("vi-cmd-mode", "Normal mode"),
    ("vi-movement-mode", "Normal mode"), ("run-help", "Help for command"), ("which-command", "Which command"),
    ("exchange-point-and-mark", "Swap cursor and mark"), ("set-mark-command", "Set mark"),
    ("overwrite-mode", "Toggle overwrite"), ("vi-editing-mode", "Switch to vi mode"),
    ("emacs-editing-mode", "Switch to emacs mode"), ("redisplay", "Redraw line"), ("repaint", "Redraw line"),
];

fn lookup(name: &str) -> Option<String> {
    NAMES.iter().find(|(k, _)| *k == name).map(|(_, d)| d.to_string())
}

pub fn widget_desc(name: &str) -> String {
    let name = name.trim_start_matches('_');
    let name = name.strip_prefix("fish_").unwrap_or(name);
    lookup(name)
        .or_else(|| name.strip_prefix("vi-").and_then(lookup))
        .unwrap_or_else(|| humanize(name.strip_prefix("vi-").unwrap_or(name)))
}

/// Readline key names: `Control-x`, `Meta-DEL`, `C-x`, `RUBOUT`, `ESC`, `RET`…
/// (readline's DEL is the backspace key).
pub fn readline_keyname(name: &str) -> Combo {
    let mut c = parse_prefixed(name, '-');
    let last = name.rsplit('-').next().unwrap_or(name);
    if last.eq_ignore_ascii_case("del") {
        if let Some(k) = c.last_mut() { *k = "Bksp".into(); }
    }
    c
}

fn closing_quote(s: &str) -> Option<usize> {
    let mut esc = false;
    for (i, ch) in s.char_indices() {
        if esc { esc = false } else if ch == '\\' { esc = true } else if ch == '"' { return Some(i) }
    }
    None
}

/// `"keys": function`, `"keys": "macro"` or `Keyname: function`.
pub fn readline_bind(line: &str) -> Option<(Seq, String)> {
    let line = line.trim();
    let (keys, target) = if let Some(rest) = line.strip_prefix('"') {
        let end = closing_quote(rest)?;
        let target = rest[end + 1..].trim_start().strip_prefix(':')?;
        (parse_term(&rest[..end], Term::Readline), target.trim())
    } else {
        let (name, target) = line.split_once(':')?;
        if name.is_empty() || name.contains(char::is_whitespace) { return None; }
        (vec![readline_keyname(name)], target.trim())
    };
    if keys.is_empty() || target.is_empty() { return None; }
    let desc = if let Some(m) = target.strip_prefix('"') {
        format!("Macro: {}", m.strip_suffix('"').unwrap_or(m))
    } else {
        let f = target.split_whitespace().next()?;
        if DROP.contains(&f) { return None; }
        widget_desc(f)
    };
    Some((keys, desc))
}
```

- [ ] **Step 4: Run** `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: shared shell helpers (widget names, readline lines)"`

---
### Task 3: zsh source

**Files:**
- Create: `src/sources/zsh.rs`, `src/defaults/zsh.txt`, `tests/fixtures/zsh/vi.txt` (real capture), `tests/fixtures/zsh/emacs.txt`
- Modify: `src/sources/mod.rs` (`pub mod zsh;`, `all()` after zellij); `src/sources/shell.rs` (make `closing_quote` `pub`)

**Interfaces:**
- Produces: `pub struct Zsh;` (app/binary `"zsh"`), `pub const ZSH_SCRIPT: &str = "bindkey -lL main; print -- ---; bindkey -M main; print -- ---; bindkey -M vicmd"`; runner call `env.run("zsh", &["-ic", ZSH_SCRIPT])`.

Output layout: part 0 holds `bindkey -A viins main` (vi) or `bindkey -A emacs main`; part 1 the main keymap; part 2 vicmd; parts separated by a line `---`. Lines `"<keys>" <widget>`; a range `"^A"-"^C" widget` is skipped; a string binding `"<keys>" "<text>"` becomes `Macro: <text>`. Sections: vi → `Insert` (scope `main`) and `Normal` (scope `vicmd`); emacs → `Emacs` only. Live replaces defaults (`keep_defaults = false`). Lines that don't match (rc output) are ignored; no bindings at all → `Err("zsh bindkey printed nothing")`; command failure → `Err("zsh -ic bindkey failed")`.

- [ ] **Step 1: Fixtures**

```bash
mkdir -p tests/fixtures/zsh
{ echo "Welcome back, ben!"; zsh -ic 'bindkey -lL main; print -- ---; bindkey -M main; print -- ---; bindkey -M vicmd' 2>/dev/null; } > tests/fixtures/zsh/vi.txt
head -2 tests/fixtures/zsh/vi.txt     # banner, then "bindkey -A viins main"
printf 'bindkey -A emacs main\n---\n"^A" beginning-of-line\n"^X^E" edit-command-line\n"^A"-"^C" self-insert\n"^Xa" "ls -la^J"\n---\n' > tests/fixtures/zsh/emacs.txt
```

The banner line is the rc-output case from Review Focus 2.

`src/defaults/zsh.txt` (zsh notation):

```
# fallback when `zsh -ic bindkey` fails: zsh emacs keymap essentials
Emacs :: ^A :: Start of line
Emacs :: ^E :: End of line
Emacs :: ^B :: Back one char
Emacs :: ^F :: Forward one char
Emacs :: ^[b :: Back one word
Emacs :: ^[f :: Forward one word
Emacs :: ^D :: Delete char or list completions
Emacs :: ^H :: Delete previous char
Emacs :: ^K :: Delete to end of line
Emacs :: ^U :: Delete line
Emacs :: ^W :: Delete previous word
Emacs :: ^[d :: Delete next word
Emacs :: ^Y :: Paste deleted text
Emacs :: ^L :: Clear screen
Emacs :: ^R :: Search history backward
Emacs :: ^S :: Search history forward
Emacs :: ^P :: Previous command
Emacs :: ^N :: Next command
Emacs :: ^[. :: Insert last argument
Emacs :: ^T :: Swap chars
Emacs :: ^_ :: Undo
Emacs :: ^I :: Complete
Emacs :: ^[q :: Park command
Emacs :: ^G :: Cancel
```

- [ ] **Step 2: Failing tests** (bottom of `src/sources/zsh.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/zsh/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn load(out: Option<String>) -> Loaded {
        let runs = out.map(|o| HashMap::from([(format!("zsh -ic {ZSH_SCRIPT}"), o)])).unwrap_or_default();
        Zsh.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn real_vi_mode_capture() {
        let l = load(Some(fixture("vi.txt")));
        assert_eq!(l.origin, Origin::Live);
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["ZSH · INSERT", "ZSH · NORMAL"]);
        assert!(keys_of(&l, "ZSH · INSERT", "Previous command").contains(&"↑".to_string()));
        assert!(keys_of(&l, "ZSH · NORMAL", "Redo").contains(&"Ctrl+R".to_string()));
        let descs: Vec<&str> = l.sections.iter().flat_map(|s| &s.rows).map(|r| r.desc.as_str()).collect();
        assert!(!descs.iter().any(|d| *d == "Self insert" || *d == "Bracketed paste"));
    }

    #[test]
    fn emacs_mode_ranges_and_macros() {
        let l = load(Some(fixture("emacs.txt")));
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["ZSH · EMACS"]);
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Edit command in $EDITOR"), vec!["Ctrl+X › Ctrl+E"]);
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Start of line"), vec!["Ctrl+A"]);   // the range line didn't override it
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Macro: ls -la^J"), vec!["Ctrl+X › a"]);
    }

    #[test]
    fn failure_falls_back() {
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: zsh -ic bindkey failed"));
        assert_eq!(keys_of(&l, "ZSH · EMACS", "Search history backward"), vec!["Ctrl+R"]);
        let l = load(Some("just a banner\n".into()));
        assert_eq!(l.note.as_deref(), Some("config: zsh bindkey printed nothing"));
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib zsh` — Expected: compile errors.

- [ ] **Step 4: Implement** (above the tests); make `closing_quote` in `shell.rs` `pub`:

```rust
//! zsh: the live line-editor keymaps (`bindkey`), main and, in vi mode, vicmd.
use super::shell::{closing_quote, widget_desc, DROP};
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_term, Term};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/zsh.txt");
pub const ZSH_SCRIPT: &str = "bindkey -lL main; print -- ---; bindkey -M main; print -- ---; bindkey -M vicmd";

pub struct Zsh;

impl Source for Zsh {
    fn app(&self) -> &'static str { "zsh" }
    fn binary(&self) -> &'static str { "zsh" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_term(k, Term::Zsh), &|g| g.to_lowercase());
        let live = match env.run("zsh", &["-ic", ZSH_SCRIPT]) {
            None => Err("zsh -ic bindkey failed".to_string()),
            Some(out) => {
                let changes = parse(&out);
                if changes.is_empty() { Err("zsh bindkey printed nothing".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("zsh")
    }
}

/// `"keys" widget` → (keys, description); None for ranges, dropped widgets, other lines.
fn bind_line(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix('"')?;
    let end = closing_quote(rest)?;
    let after = &rest[end + 1..];
    if after.starts_with('-') { return None; } // "^A"-"^C" range
    let target = after.trim();
    if let Some(m) = target.strip_prefix('"') {
        return Some((rest[..end].to_string(), format!("Macro: {}", m.strip_suffix('"').unwrap_or(m))));
    }
    let widget = target.split_whitespace().next()?;
    if DROP.contains(&widget) { return None; }
    Some((rest[..end].to_string(), widget_desc(widget)))
}

fn parse(out: &str) -> Vec<Change> {
    let mut part = 0;
    let mut vi = false;
    let mut changes = Vec::new();
    for line in out.lines() {
        if line.trim() == "---" { part += 1; continue; }
        match part {
            0 => if line.contains("bindkey -A") && line.trim_end().ends_with(" main") { vi = line.contains(" viins ") },
            1 | 2 => if let Some((keys, desc)) = bind_line(line) {
                let (scope, group) = match (part, vi) { (2, _) => ("vicmd", "Normal"), (_, true) => ("main", "Insert"), _ => ("main", "Emacs") };
                if part == 2 && !vi { continue; }
                changes.push(Change::Bind(Binding { scope: scope.into(), group: group.into(), seq: parse_term(&keys, Term::Zsh), desc, id: String::new() }));
            },
            _ => {}
        }
    }
    changes
}
```

- [ ] **Step 5: Register** `pub mod zsh;` and add `Box::new(zsh::Zsh)` after zellij in `all()`. Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: zsh sheet from live bindkey"`

---

### Task 4: bash source (and shared readline defaults)

**Files:**
- Create: `src/sources/bash.rs`, `src/defaults/readline.txt`, `tests/fixtures/bash/emacs.txt` (real capture)
- Modify: `src/sources/mod.rs` (`pub mod bash;`, `all()` after zsh)

**Interfaces:**
- Produces: `pub struct Bash;` (app/binary `"bash"`); `pub const BASH_SCRIPT: &str = "bind -p; echo ---; bind -v"`; `pub const READLINE_DEFAULTS: &str = include_str!("../defaults/readline.txt")` in `bash.rs` (reused by Task 5).

Live: lines before `---` through `shell::readline_bind`; after it, `set editing-mode vi` selects section `Vi`, else `Emacs` (scope = group lowercased). `keep_defaults = false`. No bindings → `Err("bash bind -p printed nothing")`; failure → `Err("bash -ic bind failed")`.

- [ ] **Step 1: Fixture and defaults**

```bash
mkdir -p tests/fixtures/bash
{ echo "motd: have a nice day"; bash -ic 'bind -p; echo ---; bind -v' 2>/dev/null; } > tests/fixtures/bash/emacs.txt
grep -c '": ' tests/fixtures/bash/emacs.txt      # a few hundred binding lines
```

`src/defaults/readline.txt` (readline notation):

```
# GNU readline emacs essentials (fallback for bash, base for ~/.inputrc)
Emacs :: \C-a :: Start of line
Emacs :: \C-e :: End of line
Emacs :: \C-b :: Back one char
Emacs :: \C-f :: Forward one char
Emacs :: \eb :: Back one word
Emacs :: \ef :: Forward one word
Emacs :: \C-d :: Delete char
Emacs :: \C-h :: Delete previous char
Emacs :: \C-k :: Delete to end of line
Emacs :: \C-u :: Delete to start of line
Emacs :: \C-w :: Delete previous word
Emacs :: \ed :: Delete next word
Emacs :: \C-y :: Paste deleted text
Emacs :: \ey :: Cycle pasted text
Emacs :: \C-l :: Clear screen
Emacs :: \C-r :: Search history backward
Emacs :: \C-s :: Search history forward
Emacs :: \C-p :: Previous command
Emacs :: \C-n :: Next command
Emacs :: \e. :: Insert last argument
Emacs :: \C-t :: Swap chars
Emacs :: \et :: Swap words
Emacs :: \C-_ :: Undo
Emacs :: \C-x\C-e :: Edit command in $EDITOR
Emacs :: \C-g :: Cancel
Emacs :: \C-v :: Insert next key literally
Emacs :: \eu :: Uppercase word
Emacs :: \el :: Lowercase word
Emacs :: \ec :: Capitalize word
Emacs :: \t :: Complete
```

- [ ] **Step 2: Failing tests** (bottom of `src/sources/bash.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn load(out: Option<String>) -> Loaded {
        let runs = out.map(|o| HashMap::from([(format!("bash -ic {BASH_SCRIPT}"), o)])).unwrap_or_default();
        Bash.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn real_emacs_capture() {
        let out = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bash/emacs.txt")).unwrap();
        let l = load(Some(out));
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(l.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), vec!["BASH · EMACS"]);
        assert!(keys_of(&l, "BASH · EMACS", "Edit command in $EDITOR").contains(&"Ctrl+X › Ctrl+E".to_string()));
        assert!(keys_of(&l, "BASH · EMACS", "Forward one word").contains(&"Ctrl+→".to_string()));
        assert!(keys_of(&l, "BASH · EMACS", "Previous command").contains(&"↑".to_string()));
        assert!(!l.sections[0].rows.iter().any(|r| r.desc.contains("not bound") || r.desc == "Self insert"));
    }

    #[test]
    fn vi_mode_and_failures() {
        let l = load(Some("\"\\C-a\": beginning-of-line\n---\nset editing-mode vi\n".into()));
        assert_eq!(l.sections[0].title, "BASH · VI");
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: bash -ic bind failed"));
        assert_eq!(keys_of(&l, "BASH · EMACS", "Undo"), vec!["Ctrl+_"]);
        let l = load(Some("---\nset editing-mode emacs\n".into()));
        assert_eq!(l.note.as_deref(), Some("config: bash bind -p printed nothing"));
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib bash` — Expected: compile errors.

- [ ] **Step 4: Implement**:

```rust
//! bash: the live readline bindings (`bind -p`) and editing mode.
use super::shell::readline_bind;
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_term, Term};
use crate::model::{Binding, Change};

pub const READLINE_DEFAULTS: &str = include_str!("../defaults/readline.txt");
pub const BASH_SCRIPT: &str = "bind -p; echo ---; bind -v";

pub struct Bash;

impl Source for Bash {
    fn app(&self) -> &'static str { "bash" }
    fn binary(&self) -> &'static str { "bash" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(READLINE_DEFAULTS, &|k| parse_term(k, Term::Readline), &|g| g.to_lowercase());
        let live = match env.run("bash", &["-ic", BASH_SCRIPT]) {
            None => Err("bash -ic bind failed".to_string()),
            Some(out) => {
                let (binds, vars) = out.split_once("\n---\n").or_else(|| out.split_once("---\n")).unwrap_or((&out, ""));
                let group = if vars.lines().any(|l| l.trim() == "set editing-mode vi") { "Vi" } else { "Emacs" };
                let changes: Vec<Change> = binds.lines().filter_map(readline_bind)
                    .map(|(seq, desc)| Change::Bind(Binding { scope: group.to_lowercase(), group: group.into(), seq, desc, id: String::new() }))
                    .collect();
                if changes.is_empty() { Err("bash bind -p printed nothing".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("bash")
    }
}
```

- [ ] **Step 5: Register** `pub mod bash;`, `all()` after zsh. Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: bash sheet from live bind -p"`

---

### Task 5: readline (inputrc) source

**Files:**
- Create: `src/sources/readline.rs`, `tests/fixtures/readline/home/.inputrc`, `tests/fixtures/readline/home/extra.inputrc`
- Modify: `src/sources/mod.rs` (`pub mod readline;`, `all()` after nushell — nushell is added in Task 7; until then after fish/bash)

**Interfaces:**
- Produces: `pub struct Readline;` (app `"readline"`, binary `""`, `installed` overridden: `$INPUTRC` (non-empty) or `~/.inputrc` exists).

Parsing state: `vi: bool` (from `set editing-mode`), `keymap: String` (default `emacs`, `vi-insert` after `set editing-mode vi`, changed by `set keymap X`), condition stack for `$if`/`$else`/`$endif` (`mode=emacs|vi` against `vi`, `term=…` and `version…` true, any other word — an application name — false), `$include <path>` (`~/` and relative paths resolved, depth ≤ 8). Bindings via `shell::readline_bind`, group by keymap: `emacs*` → `Emacs`, `vi-insert` → `Vi insert`, `vi`/`vi-command`/`vi-move` → `Vi normal`; scope = group lowercased. Over `READLINE_DEFAULTS` (`keep_defaults = true`).

- [ ] **Step 1: Fixtures**

`tests/fixtures/readline/home/.inputrc`:

```
# test inputrc
set editing-mode emacs
$include ~/extra.inputrc
"\C-x\C-r": re-read-init-file
Control-k: kill-whole-line
$if mode=vi
"\C-w": nothing-here
$endif
$if Bash
"\C-xq": "bash only"
$else
"\C-xs": "not bash"
$endif
$if term=xterm
"\e[1;5D": backward-word
$endif
set keymap vi-command
"gg": beginning-of-history
```

`tests/fixtures/readline/home/extra.inputrc`:

```
"\C-xe": "echo extra"
```

- [ ] **Step 2: Failing tests** (bottom of `src/sources/readline.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn home() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/readline/home") }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn inputrc_over_defaults() {
        let env = Env::test(&home(), FakeRunner(HashMap::new()));
        assert!(Readline.installed(&env));
        let l = Readline.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        let e = "READLINE · EMACS";
        assert_eq!(keys_of(&l, e, "Re read init file"), vec!["Ctrl+X › Ctrl+R"]);
        assert_eq!(keys_of(&l, e, "Delete line"), vec!["Ctrl+K"]);               // Control-k replaced the default
        assert!(keys_of(&l, e, "Delete to end of line").is_empty());
        assert!(keys_of(&l, e, "Nothing here").is_empty());                      // $if mode=vi
        assert!(keys_of(&l, e, "Macro: bash only").is_empty());                  // $if Bash
        assert_eq!(keys_of(&l, e, "Macro: not bash"), vec!["Ctrl+X › s"]);      // $else
        assert!(keys_of(&l, e, "Back one word").contains(&"Ctrl+←".to_string())); // $if term=
        assert_eq!(keys_of(&l, e, "Macro: echo extra"), vec!["Ctrl+X › e"]);    // $include
        assert_eq!(keys_of(&l, "READLINE · VI NORMAL", "Beginning of history"), vec!["g › g"]);
    }

    #[test]
    fn not_installed_without_inputrc() {
        let env = Env::test(&home().join("nope"), FakeRunner(HashMap::new()));
        assert!(!Readline.installed(&env));
        let mut env = Env::test(&home().join("nope"), FakeRunner(HashMap::new()));
        env.vars.insert("INPUTRC".into(), home().join(".inputrc").display().to_string());
        assert!(Readline.installed(&env));
        env.vars.insert("INPUTRC".into(), String::new());
        assert!(!Readline.installed(&env));
    }

    #[test]
    fn include_loop_terminates() {
        let dir = std::env::temp_dir().join("cst-inputrc-loop");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".inputrc"), "$include ~/.inputrc\n\"\\C-xz\": undo\n").unwrap();
        let l = Readline.load(&Env::test(&dir, FakeRunner(HashMap::new())));
        assert!(keys_of(&l, "READLINE · EMACS", "Undo").contains(&"Ctrl+X › z".to_string()));
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib readline` — Expected: compile errors.

- [ ] **Step 4: Implement**:

```rust
//! readline: ~/.inputrc (or $INPUTRC) over GNU readline's emacs defaults —
//! the bindings python, gdb, psql and other readline programs use.
use super::bash::READLINE_DEFAULTS;
use super::shell::readline_bind;
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{parse_term, Term};
use crate::model::{Binding, Change};
use std::path::{Path, PathBuf};

pub struct Readline;

fn inputrc(env: &Env) -> PathBuf {
    env.var("INPUTRC").map(PathBuf::from).unwrap_or_else(|| env.home.join(".inputrc"))
}

impl Source for Readline {
    fn app(&self) -> &'static str { "readline" }
    fn binary(&self) -> &'static str { "" }
    fn installed(&self, env: &Env) -> bool { inputrc(env).is_file() }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(READLINE_DEFAULTS, &|k| parse_term(k, Term::Readline), &|g| g.to_lowercase());
        let path = inputrc(env);
        let live = if !path.is_file() { Ok(None) } else {
            let mut st = St { vi: false, keymap: "emacs".into(), conds: vec![], changes: vec![] };
            parse(env, &path, 0, &mut st).map(|_| Some(st.changes))
        };
        layer(defaults, live, true).into_loaded("readline")
    }
}

struct St { vi: bool, keymap: String, conds: Vec<bool>, changes: Vec<Change> }

fn group(keymap: &str) -> &'static str {
    match keymap {
        k if k.starts_with("emacs") => "Emacs",
        "vi-insert" => "Vi insert",
        _ => "Vi normal",
    }
}

fn parse(env: &Env, path: &Path, depth: u8, st: &mut St) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = env.read(path).ok_or(format!("{name}: unreadable"))?;
    let dir = path.parent().unwrap_or(Path::new("/"));
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(cond) = line.strip_prefix("$if") {
            let cond = cond.trim();
            let ok = if let Some(m) = cond.strip_prefix("mode=") { (m.trim() == "vi") == st.vi }
                else { cond.starts_with("term") || cond.starts_with("version") };
            st.conds.push(ok);
            continue;
        }
        if line.starts_with("$else") { if let Some(c) = st.conds.last_mut() { *c = !*c; } continue; }
        if line.starts_with("$endif") { st.conds.pop(); continue; }
        if !st.conds.iter().all(|c| *c) { continue; }
        if let Some(p) = line.strip_prefix("$include") {
            if depth < 8 { let _ = parse(env, &env.expand(p.trim(), dir), depth + 1, st); }
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() == Some(&"set") {
            match (words.get(1).copied(), words.get(2).copied()) {
                (Some("editing-mode"), Some(m)) => {
                    st.vi = m == "vi";
                    st.keymap = if st.vi { "vi-insert".into() } else { "emacs".into() };
                }
                (Some("keymap"), Some(k)) => st.keymap = k.to_string(),
                _ => {}
            }
            continue;
        }
        if let Some((seq, desc)) = readline_bind(line) {
            let g = group(&st.keymap);
            st.changes.push(Change::Bind(Binding { scope: g.to_lowercase(), group: g.into(), seq, desc, id: String::new() }));
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Register** `pub mod readline;` and add `Box::new(readline::Readline)` to `all()` after bash (Task 7 inserts fish/nushell before it). Run `cargo test -q --lib` — Expected: all pass. (`load_installed` calls `installed`, so the empty binary name is never looked up.)

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: readline sheet from inputrc"`

---

### Task 6: fish source

**Files:**
- Create: `src/sources/fish.rs`, `src/defaults/fish.txt`, `tests/fixtures/fish/fish4.txt`, `tests/fixtures/fish/fish3.txt`
- Modify: `src/sources/mod.rs` (`pub mod fish;`, `all()` after bash)

**Interfaces:**
- Produces: `pub struct Fish;` (app/binary `"fish"`); runner `env.run("fish", &["-c", "bind"])`.

Line format `bind [--preset|-p] [--user] [-s] [-M|--mode MODE] [-m|--sets-mode NEW] [-k|--key] KEY COMMAND…`. Words: whitespace-separated, `'…'`/`"…"` quoted (quotes removed), backslashes kept (they belong to fish 3 key escapes). KEY: empty → skipped (the `''` self-insert default); `-k` → terminfo name (`dc` Del, `ic` Ins, `ppage` PgUp, `npage` PgDn, `btab` ⇧Tab, `sleft`/`sright` Shift+←/→, others via `key_name`); contains `\` → `parse_term(Fish3)`; contains `,` (and is longer than 1) → fish 4 sequence of `parse_prefixed(part, '-')`; a known name or a `mod-key` form → one combo; otherwise (e.g. `dd`) → one step per character. COMMAND: `commandline -f X` → X; else the first word; dropped if in `DROP`; description `widget_desc`. Section = `humanize(mode)` (default mode `default`), scope = mode. `keep_defaults = false`.

- [ ] **Step 1: Fixtures**

`tests/fixtures/fish/fish4.txt`:

```
bind --preset '' self-insert
bind --preset ctrl-a beginning-of-line
bind --preset ctrl-e end-of-line
bind --preset alt-left prevd-or-backward-word
bind --preset ctrl-x,ctrl-e edit_command_buffer
bind --preset alt-. history-token-search-backward
bind --preset -M insert ctrl-r history-pager
bind ctrl-g 'commandline -f cancel'
bind --preset -M visual -m default y fish_clipboard_copy end-selection repaint-mode
```

`tests/fixtures/fish/fish3.txt`:

```
bind --preset \ca beginning-of-line
bind --preset \e\[A up-or-search
bind --preset -k dc delete-char
bind --preset \cx\ce edit_command_buffer
bind --preset -M default dd kill-whole-line
```

`src/defaults/fish.txt` (fish 4 notation):

```
# fallback when `fish -c bind` fails
Default :: ctrl-a :: Start of line
Default :: ctrl-e :: End of line
Default :: alt-left :: Back one word
Default :: alt-right :: Forward one word
Default :: ctrl-w :: Delete previous word
Default :: ctrl-u :: Delete to start of line
Default :: ctrl-k :: Delete to end of line
Default :: ctrl-y :: Paste deleted text
Default :: ctrl-r :: History pager
Default :: alt-up :: Insert last argument
Default :: ctrl-z :: Undo
Default :: alt-e :: Edit command in $EDITOR
Default :: alt-h :: Help for command
Default :: alt-l :: List directory
Default :: alt-p :: Pipe to pager
Default :: alt-s :: Prepend sudo
Default :: ctrl-c :: Cancel
Default :: ctrl-l :: Clear screen
Default :: tab :: Complete
Default :: shift-tab :: Complete and search
```

- [ ] **Step 2: Failing tests** (bottom of `src/sources/fish.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn load(name: Option<&str>) -> Loaded {
        let runs = name.map(|n| HashMap::from([("fish -c bind".to_string(),
            std::fs::read_to_string(format!("{}/tests/fixtures/fish/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap())])).unwrap_or_default();
        Fish.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn words_keep_backslashes() {
        assert_eq!(words(r"bind --preset \e\[A 'commandline -f x' y"), vec!["bind", "--preset", r"\e\[A", "commandline -f x", "y"]);
    }

    #[test]
    fn fish4_notation() {
        let l = load(Some("fish4.txt"));
        assert_eq!(l.origin, Origin::Live);
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["FISH · DEFAULT", "FISH · INSERT", "FISH · VISUAL"]);
        let d = "FISH · DEFAULT";
        assert_eq!(keys_of(&l, d, "Start of line"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, d, "Edit command in $EDITOR"), vec!["Ctrl+X › Ctrl+E"]);
        assert_eq!(keys_of(&l, d, "Insert last argument"), vec!["Alt+."]);
        assert_eq!(keys_of(&l, d, "Prevd or backward word"), vec!["Alt+←"]);
        assert_eq!(keys_of(&l, d, "Cancel"), vec!["Ctrl+G"]);                      // commandline -f cancel
        assert_eq!(keys_of(&l, "FISH · INSERT", "History pager"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, "FISH · VISUAL", "Clipboard copy"), vec!["y"]);
        assert!(!l.sections.iter().flat_map(|s| &s.rows).any(|r| r.desc == "Self insert"));
    }

    #[test]
    fn fish3_notation() {
        let l = load(Some("fish3.txt"));
        let d = "FISH · DEFAULT";
        assert_eq!(keys_of(&l, d, "Start of line"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, d, "Previous command"), vec!["↑"]);
        assert_eq!(keys_of(&l, d, "Delete char"), vec!["Del"]);
        assert_eq!(keys_of(&l, d, "Edit command in $EDITOR"), vec!["Ctrl+X › Ctrl+E"]);
        assert_eq!(keys_of(&l, d, "Delete line"), vec!["d › d"]);
    }

    #[test]
    fn failure_falls_back() {
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: fish -c bind failed"));
        assert_eq!(keys_of(&l, "FISH · DEFAULT", "Complete and search"), vec!["Shift+Tab"]);
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib fish` — Expected: compile errors.

- [ ] **Step 4: Implement**:

```rust
//! fish: the live `bind` table (fish 4 `ctrl-x` and fish 3 `\cx` notations).
use super::shell::{widget_desc, DROP};
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{combo, key_name, parse_prefixed, parse_term, Seq, Term};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/fish.txt");

pub struct Fish;

impl Source for Fish {
    fn app(&self) -> &'static str { "fish" }
    fn binary(&self) -> &'static str { "fish" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &fish4_key, &|g| g.to_lowercase());
        let live = match env.run("fish", &["-c", "bind"]) {
            None => Err("fish -c bind failed".to_string()),
            Some(out) => {
                let changes: Vec<Change> = out.lines().filter_map(bind_line).collect();
                if changes.is_empty() { Err("fish bind printed nothing".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("fish")
    }
}

/// Whitespace-separated words; '…' and "…" quotes removed; backslashes kept.
pub fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut cur, mut in_word, mut quote) = (String::new(), false, None);
    for c in line.chars() {
        match (c, quote) {
            ('\'' | '"', None) => { quote = Some(c); in_word = true; }
            (q, Some(open)) if q == open => quote = None,
            (c, None) if c.is_whitespace() => if in_word { out.push(std::mem::take(&mut cur)); in_word = false; },
            (c, _) => { cur.push(c); in_word = true; }
        }
    }
    if in_word { out.push(cur); }
    out
}

fn terminfo(name: &str) -> Seq {
    vec![match name {
        "dc" => vec!["Del".into()], "ic" => vec!["Ins".into()],
        "ppage" => vec!["PgUp".into()], "npage" => vec!["PgDn".into()],
        "btab" => vec!["⇧Tab".into()],
        "sleft" => combo(&["shift"], "left"), "sright" => combo(&["shift"], "right"),
        other => vec![key_name(other, false)],
    }]
}

fn fish4_key(key: &str) -> Seq {
    if key.chars().count() > 1 && key.contains(',') {
        return key.split(',').map(|k| parse_prefixed(k, '-')).collect();
    }
    let one = parse_prefixed(key, '-');
    let known = one.len() > 1 || key.chars().count() == 1 || key_name(key, false) != key;
    if known { vec![one] } else { key.chars().map(|c| vec![key_name(&c.to_string(), false)]).collect() }
}

fn bind_line(line: &str) -> Option<Change> {
    let w = words(line);
    if w.first().map(String::as_str) != Some("bind") { return None; }
    let (mut mode, mut i, mut ti) = ("default".to_string(), 1, false);
    while let Some(f) = w.get(i) {
        match f.as_str() {
            "--preset" | "-p" | "--user" | "-s" | "--silent" => i += 1,
            "-M" | "--mode" => { mode = w.get(i + 1)?.clone(); i += 2; }
            "-m" | "--sets-mode" => i += 2,
            "-k" | "--key" => { ti = true; i += 1; }
            f if f.starts_with("--mode=") => { mode = f["--mode=".len()..].to_string(); i += 1; }
            f if f.starts_with("--sets-mode=") => i += 1,
            _ => break,
        }
    }
    let key = w.get(i)?;
    if key.is_empty() { return None; }
    let seq = if ti { terminfo(key) } else if key.contains('\\') { parse_term(key, Term::Fish3) } else { fish4_key(key) };
    let cmd: Vec<&str> = w[i + 1..].iter().flat_map(|c| c.split_whitespace()).collect();
    let func = match cmd.first() {
        Some(&"commandline") => cmd.iter().position(|c| *c == "-f").and_then(|p| cmd.get(p + 1)).copied().unwrap_or("commandline"),
        Some(f) => *f,
        None => return None,
    };
    if DROP.contains(&func) { return None; }
    Some(Change::Bind(Binding { scope: mode.clone(), group: humanize(&mode), seq, desc: widget_desc(func), id: String::new() }))
}
```

`combo(&["shift"], "left")` gives `Shift+←`; `parse_prefixed("shift-tab", '-')` gives `Shift+Tab` (the defaults test expects that).

- [ ] **Step 5: Register** `pub mod fish;`, `all()` after bash. Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: fish sheet from live bind (fish 3 and 4 notations)"`

---

### Task 7: nushell source

**Files:**
- Create: `src/sources/nushell.rs`, `src/defaults/nushell.txt`, `tests/fixtures/nushell/keys.tsv`
- Modify: `src/sources/mod.rs` (`pub mod nushell;`, `all()` after fish, before readline); `src/keys.rs` (`NAMES` + `("backtab","⇧Tab")`)

**Interfaces:**
- Produces: `pub struct Nushell;` (app `"nushell"`, binary `"nu"`), `pub const NU_SCRIPT: &str`; runner `env.run("nu", &["-c", NU_SCRIPT])`.

`NU_SCRIPT` prints one line per binding: `kind<TAB>mode<TAB>modifier<TAB>key<TAB>event`, kind `d` (from `keybindings default`) or `u` (from `$env.config.keybindings`), mode and event NUON-encoded:

```
let d = (keybindings default | each {|r| ["d" ($r.mode | to nuon) ($r.modifier | into string) ($r.code | into string) ($r.event | to nuon)] | str join (char tab) }); let u = ($env.config.keybindings | each {|r| ["u" ($r.mode | to nuon) ($r.modifier | into string) ($r.keycode | into string) ($r.event | to nuon)] | str join (char tab) }); $d | append $u | str join (char nl)
```

Mapping: modes `emacs`/`vi_insert`/`vi_normal` (a NUON list `[emacs, vi_insert]` → one binding per mode) → `humanize` (`Emacs`, `Vi insert`, `Vi normal`), scope = raw mode. Modifier words (any of `|`, `_`, space, `+` separated, case-insensitive): control/ctrl, alt, shift, super; `none` ignored. Key: `Char('a')`/`char_a` → `a`, `F(1)`/`f1` → `F1`, others via `key_name` (`Up` ↑, `Enter`, `BackTab` ⇧Tab). Event: `null` → `Unbind`; a quoted debug string (defaults) → `Menu("x")` → `humanize(x)`, else the first identifier that isn't `Edit`, `UntilFound`, `Multiple`, `Menu` → `humanize`; a record (`{send: menu, name: x}` → `humanize(x)`; `{send: x}`/`{edit: x}` → `humanize(x)`; `{until: [...]}` → its first). `d` rows come first, `u` rows override on the same (mode, key). `keep_defaults = false`; bundled defaults only on failure.

- [ ] **Step 1: Fixture** `tests/fixtures/nushell/keys.tsv` — tab-separated (write it with `printf` so the tabs are real):

```bash
mkdir -p tests/fixtures/nushell
printf '%s\n' \
 $'d\temacs\tCONTROL\tChar(\'a\')\t"Edit([MoveToLineStart])"' \
 $'d\temacs\tCONTROL\tChar(\'r\')\t"Menu(\\"history_menu\\")"' \
 $'d\temacs\tNONE\tTab\t"UntilFound([Menu(\\"completion_menu\\"), MenuNext])"' \
 $'d\tvi_insert\tCONTROL\tChar(\'c\')\t"CtrlC"' \
 $'d\temacs\tCONTROL\tChar(\'w\')\t"Edit([CutWordLeft])"' \
 $'u\t[emacs, vi_insert]\tcontrol\tchar_t\t{send: menu, name: fzf_menu}' \
 $'u\temacs\tcontrol\tchar_w\tnull' \
 $'u\temacs\talt\tenter\t{edit: insertnewline}' \
 > tests/fixtures/nushell/keys.tsv
cat -A tests/fixtures/nushell/keys.tsv | head -2
```

Expected `cat -A`: `d^Iemacs^ICONTROL^IChar('a')^I"Edit([MoveToLineStart])"$`.

`src/defaults/nushell.txt` (`mod+key` notation, `parse_prefixed(k, '+')`):

```
# fallback when `nu -c` fails: reedline emacs essentials
Emacs :: ctrl+a :: Move to line start
Emacs :: ctrl+e :: Move to line end
Emacs :: ctrl+b :: Move left
Emacs :: ctrl+f :: Move right
Emacs :: alt+b :: Move word left
Emacs :: alt+f :: Move word right
Emacs :: ctrl+w :: Cut word left
Emacs :: ctrl+u :: Cut from start
Emacs :: ctrl+k :: Cut to end
Emacs :: ctrl+y :: Paste cut buffer
Emacs :: ctrl+r :: History menu
Emacs :: ctrl+l :: Clear screen
Emacs :: ctrl+z :: Undo
Emacs :: ctrl+o :: Open editor
Emacs :: tab :: Completion menu
Emacs :: ctrl+c :: Cancel
Emacs :: ctrl+d :: Exit
```

- [ ] **Step 2: Failing tests** (bottom of `src/sources/nushell.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    fn load(out: Option<String>) -> Loaded {
        let runs = out.map(|o| HashMap::from([(format!("nu -c {NU_SCRIPT}"), o)])).unwrap_or_default();
        Nushell.load(&Env::test(std::path::Path::new("/"), FakeRunner(runs)))
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn defaults_and_user_bindings() {
        let out = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nushell/keys.tsv")).unwrap();
        let l = load(Some(out));
        assert_eq!(l.origin, Origin::Live);
        let titles: Vec<&str> = l.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["NUSHELL · EMACS", "NUSHELL · VI INSERT"]);
        let e = "NUSHELL · EMACS";
        assert_eq!(keys_of(&l, e, "Move to line start"), vec!["Ctrl+A"]);
        assert_eq!(keys_of(&l, e, "History menu"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, e, "Completion menu"), vec!["Tab"]);
        assert_eq!(keys_of(&l, e, "Fzf menu"), vec!["Ctrl+T"]);
        assert_eq!(keys_of(&l, "NUSHELL · VI INSERT", "Fzf menu"), vec!["Ctrl+T"]);
        assert_eq!(keys_of(&l, e, "Insertnewline"), vec!["Alt+Enter"]);
        assert!(keys_of(&l, e, "Cut word left").is_empty());                    // user null unbinds the default
        assert_eq!(keys_of(&l, "NUSHELL · VI INSERT", "Ctrl c"), vec!["Ctrl+C"]);
    }

    #[test]
    fn failure_falls_back() {
        let l = load(None);
        assert_eq!(l.note.as_deref(), Some("config: nu keybindings failed"));
        assert_eq!(keys_of(&l, "NUSHELL · EMACS", "Open editor"), vec!["Ctrl+O"]);
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib nushell` — Expected: compile errors.

- [ ] **Step 4: Implement**:

```rust
//! nushell: reedline's default keybindings plus $env.config.keybindings,
//! read through `nu -c` as tab-separated lines with NUON fields.
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::{combo, parse_prefixed, Combo};
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/nushell.txt");
pub const NU_SCRIPT: &str = "let d = (keybindings default | each {|r| [\"d\" ($r.mode | to nuon) ($r.modifier | into string) ($r.code | into string) ($r.event | to nuon)] | str join (char tab) }); let u = ($env.config.keybindings | each {|r| [\"u\" ($r.mode | to nuon) ($r.modifier | into string) ($r.keycode | into string) ($r.event | to nuon)] | str join (char tab) }); $d | append $u | str join (char nl)";

pub struct Nushell;

impl Source for Nushell {
    fn app(&self) -> &'static str { "nushell" }
    fn binary(&self) -> &'static str { "nu" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_prefixed(k, '+')], &|g| g.to_lowercase().replace(' ', "_"));
        let live = match env.run("nu", &["-c", NU_SCRIPT]) {
            None => Err("nu keybindings failed".to_string()),
            Some(out) => {
                let changes: Vec<Change> = out.lines().flat_map(line_changes).collect();
                if changes.is_empty() { Err("nu printed no keybindings".to_string()) } else { Ok(Some(changes)) }
            }
        };
        layer(defaults, live, false).into_loaded("nushell")
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).map(|x| x.replace("\\\"", "\"")).unwrap_or_else(|| s.to_string())
}

fn modes(nuon: &str) -> Vec<String> {
    nuon.trim().trim_start_matches('[').trim_end_matches(']').split(',')
        .map(unquote).filter(|m| !m.is_empty()).collect()
}

fn key(modifier: &str, code: &str) -> Combo {
    let mods: Vec<&str> = modifier.split(|c: char| !c.is_ascii_alphanumeric()).filter(|m| !m.is_empty() && !m.eq_ignore_ascii_case("none")).collect();
    let code = code.trim();
    let k = if let Some(c) = code.strip_prefix("Char('").and_then(|c| c.strip_suffix("')")) { c.to_string() }
        else if let Some(c) = code.strip_prefix("char_") { c.to_string() }
        else if let Some(n) = code.strip_prefix("F(").and_then(|c| c.strip_suffix(')')) { format!("F{n}") }
        else { code.to_string() };
    combo(&mods, if k == " " { "space" } else { &k })
}

const WRAPPERS: &[&str] = &["Edit", "UntilFound", "Multiple", "Menu"];

/// None = unbind (`null`).
fn event_desc(nuon: &str) -> Option<String> {
    let s = nuon.trim();
    if s.is_empty() || s == "null" { return None; }
    if s.starts_with('"') {
        let dbg = unquote(s);
        if let Some(rest) = dbg.split("Menu(\"").nth(1) {
            return Some(humanize(rest.split('"').next().unwrap_or(rest)));
        }
        return dbg.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).find(|w| !w.is_empty() && !WRAPPERS.contains(w)).map(humanize);
    }
    let field = |name: &str| s.split(&format!("{name}:")).nth(1)
        .map(|v| unquote(v.trim_start().split([',', '}', ']']).next().unwrap_or("")));
    match field("send") {
        Some(v) if v.eq_ignore_ascii_case("menu") => field("name").map(|n| humanize(&n)),
        Some(v) => Some(humanize(&v)),
        None => field("edit").map(|v| humanize(&v)),
    }
}

fn line_changes(line: &str) -> Vec<Change> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() != 5 || !(f[0] == "d" || f[0] == "u") { return vec![]; }
    let seq = vec![key(f[2], f[3])];
    let desc = event_desc(f[4]);
    modes(f[1]).into_iter().map(|mode| match &desc {
        Some(d) => Change::Bind(Binding { scope: mode.clone(), group: humanize(&mode), seq: seq.clone(), desc: d.clone(), id: String::new() }),
        None => Change::Unbind { scope: mode, seq: seq.clone() },
    }).collect()
}
```

Check against the fixture: `{until: [...]}` records have no top-level `send:` before the nested one — `field("send")` finds the first `send:` inside, which is the first event of the list (what the spec asks for). `CtrlC` → `humanize` → `Ctrl c`.

- [ ] **Step 5: Register** `pub mod nushell;` and finalise `all()` order: awesome, kitty, wezterm, alacritty, tmux, zellij, zsh, bash, fish, nushell, readline, nvim, yazi, lazygit, fzf. Add `("backtab", "⇧Tab")` to `keys::NAMES`. Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: nushell sheet from reedline defaults and config keybindings"`

---
### Task 8: Picker state and keys

**Files:**
- Modify: `src/ui/input.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Screen { Picker, #[default] Sheet }
// State gains: pub screen: Screen, pub from_picker: bool, pub picked: usize, pub pick_query: String
// Key gains: Left, Right, Enter (from_event maps KeyCode::Left/Right/Enter)
pub fn picker_entries(apps: &[&str], query: &str) -> Vec<Option<usize>>   // None = "All apps", always first
pub fn apply_picker(st: &mut State, key: Key, apps: &[&str], cols: usize)
```

Picker keys: printable → `pick_query` (selection back to 0); Backspace / Ctrl+U edit it; ←/→ and Tab/Shift+Tab move by one, wrapping; ↓/↑ move by `cols` (one row), clamped; Home/PgUp first, End/PgDn last; Enter opens the selected entry (`focus` = `None` for All apps or `Some(app index)`, `screen = Sheet`, `from_picker = true`, sheet query cleared, scroll 0); Esc clears `pick_query`, or quits when it's empty; Ctrl+C/D quit. Sheet change: Esc with an empty query returns to the picker when `from_picker`, else quits (as now). Sheet ignores Left/Right/Enter.

Plan choice (spec says "↑/↓/←/→ … move the selection (wrapping)"): ←/→/Tab wrap; ↑/↓ move a whole row in the grid and clamp, which is what a grid of columns needs. A filter that matches no app still leaves `All apps` (the spec keeps it first always), so Enter always has a target.

- [ ] **Step 1: Failing tests** — append to `input::tests`:

```rust
    const APPS: [&str; 3] = ["kitty", "tmux", "zsh"];

    fn picker() -> State { State { screen: Screen::Picker, ..Default::default() } }

    #[test]
    fn picker_entries_keep_all_apps_first() {
        assert_eq!(picker_entries(&APPS, ""), vec![None, Some(0), Some(1), Some(2)]);
        assert_eq!(picker_entries(&APPS, "T"), vec![None, Some(0), Some(1)]);   // kitty, tmux
        assert_eq!(picker_entries(&APPS, "zzz"), vec![None]);
    }

    #[test]
    fn picker_moves_wrap_and_rows() {
        let mut st = picker();
        apply_picker(&mut st, Key::Left, &APPS, 2);
        assert_eq!(st.picked, 3);                       // wraps to the last of 4 entries
        apply_picker(&mut st, Key::Right, &APPS, 2);
        assert_eq!(st.picked, 0);
        apply_picker(&mut st, Key::Down, &APPS, 2);
        assert_eq!(st.picked, 2);                       // one row = 2 columns
        apply_picker(&mut st, Key::Down, &APPS, 2);
        assert_eq!(st.picked, 3);                       // clamped
        apply_picker(&mut st, Key::Up, &APPS, 2);
        assert_eq!(st.picked, 1);
        apply_picker(&mut st, Key::Home, &APPS, 2);
        assert_eq!(st.picked, 0);
        apply_picker(&mut st, Key::Tab, &APPS, 2);
        assert_eq!(st.picked, 1);
    }

    #[test]
    fn picker_enter_opens_sheet_and_esc_comes_back() {
        let mut st = picker();
        for c in "t".chars() { apply_picker(&mut st, Key::Char(c), &APPS, 2); }
        apply_picker(&mut st, Key::End, &APPS, 2);      // [All, kitty, tmux] → tmux
        apply_picker(&mut st, Key::Enter, &APPS, 2);
        assert_eq!((st.screen, st.focus, st.from_picker), (Screen::Sheet, Some(1), true));
        apply(&mut st, Key::Char('x'), 3, 10);
        apply(&mut st, Key::Esc, 3, 10);                // clears the sheet filter
        assert_eq!(st.screen, Screen::Sheet);
        apply(&mut st, Key::Esc, 3, 10);                // empty filter: back to the picker
        assert_eq!((st.screen, st.quit, st.picked, st.pick_query.as_str()), (Screen::Picker, false, 2, "t"));
        apply_picker(&mut st, Key::Esc, &APPS, 2);      // clears the picker filter
        assert_eq!((st.pick_query.as_str(), st.quit), ("", false));
        apply_picker(&mut st, Key::Esc, &APPS, 2);
        assert!(st.quit);
    }

    #[test]
    fn filter_without_matches_keeps_all_apps() {
        let mut st = picker();
        for c in "zzz".chars() { apply_picker(&mut st, Key::Char(c), &APPS, 2); }
        apply_picker(&mut st, Key::Down, &APPS, 2);
        apply_picker(&mut st, Key::Right, &APPS, 2);
        assert_eq!(st.picked, 0);
        apply_picker(&mut st, Key::Enter, &APPS, 2);
        assert_eq!((st.screen, st.focus), (Screen::Sheet, None));
    }

    #[test]
    fn sheet_without_picker_still_quits_on_esc() {
        let mut st = State::default();
        apply(&mut st, Key::Esc, 3, 10);
        assert!(st.quit);
        assert_eq!(from_event(&ev(KeyCode::Enter, KeyModifiers::NONE)), Some(Key::Enter));
        assert_eq!(from_event(&ev(KeyCode::Left, KeyModifiers::NONE)), Some(Key::Left));
    }
```

- [ ] **Step 2: Run** `cargo test -q --lib input` — Expected: compile errors (`Screen`, `apply_picker`, `Key::Enter` …).

- [ ] **Step 3: Implement.** Replace the `State` struct and extend `Key`/`from_event`/`apply` in `src/ui/input.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Screen { Picker, #[default] Sheet }

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub query: String, pub focus: Option<usize>, pub scroll: usize, pub quit: bool,
    pub screen: Screen, pub from_picker: bool, pub picked: usize, pub pick_query: String,
}
```

`Key` gets `Left, Right, Enter`; `from_event` gets `KeyCode::Left => Key::Left, KeyCode::Right => Key::Right, KeyCode::Enter => Key::Enter`. In `apply` (the sheet) replace the Esc arm and ignore the new keys:

```rust
        Key::Esc if st.query.is_empty() => if st.from_picker { st.screen = Screen::Picker } else { st.quit = true },
        // …existing arms…
        Key::Left | Key::Right | Key::Enter => {}
```

Then add:

```rust
/// Picker rows for a filter: `None` (All apps) first, then matching apps.
pub fn picker_entries(apps: &[&str], query: &str) -> Vec<Option<usize>> {
    let q = query.to_lowercase();
    std::iter::once(None)
        .chain(apps.iter().enumerate().filter(|(_, a)| a.to_lowercase().contains(&q)).map(|(i, _)| Some(i)))
        .collect()
}

/// cols: entries per row in the picker grid (layout::picker_cols).
pub fn apply_picker(st: &mut State, key: Key, apps: &[&str], cols: usize) {
    let n = picker_entries(apps, &st.pick_query).len();
    let cols = cols.max(1);
    match key {
        Key::Char(c) => { st.pick_query.push(c); st.picked = 0; }
        Key::Backspace => { st.pick_query.pop(); st.picked = 0; }
        Key::ClearQuery => { st.pick_query.clear(); st.picked = 0; }
        Key::Esc if st.pick_query.is_empty() => st.quit = true,
        Key::Esc => { st.pick_query.clear(); st.picked = 0; }
        Key::Quit => st.quit = true,
        Key::Right | Key::Tab => st.picked = (st.picked + 1) % n,
        Key::Left | Key::BackTab => st.picked = (st.picked + n - 1) % n,
        Key::Down => st.picked = (st.picked + cols).min(n - 1),
        Key::Up => st.picked = st.picked.saturating_sub(cols),
        Key::Home | Key::PageUp => st.picked = 0,
        Key::End | Key::PageDown => st.picked = n - 1,
        Key::Enter => {
            st.focus = picker_entries(apps, &st.pick_query)[st.picked.min(n - 1)];
            st.screen = Screen::Sheet;
            st.from_picker = true;
            st.query.clear();
            st.scroll = 0;
        }
    }
}
```

- [ ] **Step 4: Run** `cargo test -q --lib` — Expected: all pass (existing sheet tests unchanged).

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: picker state and keys"`

---

### Task 9: Picker layout

**Files:**
- Modify: `src/ui/layout.rs`

**Interfaces:**
- Consumes: `input::{State, picker_entries}`
- Produces:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Entry { pub name: String, pub origin: String, pub count: usize, pub warn: bool }  // one per app, same order as View.apps
pub fn picker_cols(w: usize) -> usize                         // clamp((w - 2 + 3) / (30 + 3), 1, 4)
pub fn picker_frame(v: &View, entries: &[Entry], st: &State, w: usize, h: usize) -> Frame
```

Screen: the sheet's header (refactor the header drawing in `frame` into `fn header(f: &mut Frame, info: &str)` and use it in both), filter line (`Choose an app…`, or `pick_query` + `▏`, plus `No matches` when no app matches), entries from row 3 in a row-major grid of `picker_cols(w)` columns (width as the sheet: `(w − 2 − 3(n−1)) / n`). Entry cell: `▸ ` or two spaces, the name (Title style when selected, else Text), right-aligned muted `origin  count` (All apps: just the total) and ` ⚠` when the app has a note. Rows scroll so the selected row is always visible. Below 20×5: `Terminal too small`.

- [ ] **Step 1: Failing tests** — append to `layout::tests`:

```rust
    fn entries() -> Vec<Entry> {
        vec![
            Entry { name: "kitty".into(), origin: "defaults".into(), count: 40, warn: false },
            Entry { name: "tmux".into(), origin: "mixed".into(), count: 123, warn: true },
            Entry { name: "nvim".into(), origin: "mixed".into(), count: 81, warn: false },
        ]
    }

    fn picker_state() -> State { State { screen: crate::ui::input::Screen::Picker, ..Default::default() } }

    #[test]
    fn picker_snapshots() {
        let s = sample();
        snapshot("picker_120x30.txt", &picker_frame(&view(&s), &entries(), &picker_state(), 120, 30).text());
        snapshot("picker_60x20.txt", &picker_frame(&view(&s), &entries(), &picker_state(), 60, 20).text());
    }

    #[test]
    fn picker_content_and_selection() {
        let s = sample();
        let f = picker_frame(&view(&s), &entries(), &picker_state(), 120, 30);
        let t = f.text();
        assert!(t.lines().nth(1).unwrap().starts_with(" Choose an app…"));
        assert!(t.contains("▸ All apps") && t.contains("244"));               // 40 + 123 + 81
        assert!(t.contains("mixed  123 ⚠") && t.contains("defaults  40"));
        let (x, y) = find(&f, "All apps").unwrap();
        assert_eq!(f.rows[y][x].style, Style::Title);
        let st = State { picked: 2, ..picker_state() };                      // tmux
        let f = picker_frame(&view(&s), &entries(), &st, 120, 30);
        assert!(f.text().contains("▸ tmux"));
        let st = State { pick_query: "zzz".into(), ..picker_state() };
        let t = picker_frame(&view(&s), &entries(), &st, 120, 30).text();
        assert!(t.contains("zzz▏  No matches") && t.contains("▸ All apps"));
    }

    #[test]
    fn picker_cols_by_width() {
        assert_eq!((picker_cols(20), picker_cols(64), picker_cols(65), picker_cols(120), picker_cols(500)), (1, 1, 2, 3, 4));
    }

    #[test]
    fn picker_too_small() {
        let s = sample();
        assert_eq!(picker_frame(&view(&s), &entries(), &picker_state(), 19, 5).text().trim(), "Terminal too small");
    }

    #[test]
    fn picker_scrolls_to_selection() {
        let names: Vec<String> = (0..10).map(|i| format!("app{i}")).collect();
        let apps: Vec<&str> = names.iter().map(String::as_str).collect();
        let es: Vec<Entry> = names.iter().map(|n| Entry { name: n.clone(), origin: "live".into(), count: 1, warn: false }).collect();
        let v = View { sections: &[], apps: &apps, theme_name: "t", plain: false };
        let st = State { picked: 10, ..picker_state() };                     // last entry: app9
        let t = picker_frame(&v, &es, &st, 30, 8).text();
        assert!(t.contains("▸ app9"), "{t}");
        assert!(!t.contains("All apps"));                                    // scrolled past the top
    }
```

- [ ] **Step 2: Run** `cargo test -q --lib layout` — Expected: compile errors (`Entry`, `picker_frame`, `picker_cols`).

- [ ] **Step 3: Implement.** In `layout.rs`: import `super::input::picker_entries`; extract the header:

```rust
/// Row 0: title left, `info` right-aligned when it fits.
fn header(f: &mut Frame, info: &str) {
    let right = f.w - 1;
    let end = f.put(1, 0, "Keyboard shortcuts", Style::Header, right);
    if end + 2 + cells(info) <= right { f.put(right - cells(info), 0, info, Style::Muted, right); }
}
```

and in `frame` replace the two header lines with `header(&mut f, &info);`. Then add:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Entry { pub name: String, pub origin: String, pub count: usize, pub warn: bool }

const PICK_MIN: usize = 30;

pub fn picker_cols(w: usize) -> usize {
    ((w.saturating_sub(2) + GAP) / (PICK_MIN + GAP)).clamp(1, 4)
}

pub fn picker_frame(v: &View, entries: &[Entry], st: &State, w: usize, h: usize) -> Frame {
    let mut f = Frame::new(w, h);
    if w < 20 || h < 5 {
        f.put(0, 0, "Terminal too small", Style::Muted, w);
        return f;
    }
    let right = w - 1;
    header(&mut f, &format!("{} apps · {}", v.apps.len(), v.theme_name));
    let visible = picker_entries(v.apps, &st.pick_query);
    if st.pick_query.is_empty() {
        f.put(1, 1, "Choose an app…", Style::Muted, right);
    } else {
        let x = f.put(1, 1, &st.pick_query, Style::Text, right);
        let x = f.put(x, 1, "▏", Style::Text, right);
        if visible.len() == 1 { f.put(x + 2, 1, "No matches", Style::Muted, right); }
    }
    let n = picker_cols(w);
    let col_w = (w - 2 - GAP * (n - 1)) / n;
    let (top, body_h) = (3, h - 4);
    let picked = st.picked.min(visible.len() - 1);
    let first_row = (picked / n).saturating_sub(body_h - 1);
    let total: usize = entries.iter().map(|e| e.count).sum();
    for (k, entry) in visible.iter().enumerate() {
        let row = k / n;
        if row < first_row || row - first_row >= body_h { continue; }
        let (y, x0) = (top + row - first_row, 1 + (k % n) * (col_w + GAP));
        let (name, info) = match entry {
            None => ("All apps".to_string(), total.to_string()),
            Some(i) => match entries.get(*i) {
                Some(e) => (e.name.clone(), format!("{}  {}{}", e.origin, e.count, if e.warn { " ⚠" } else { "" })),
                None => (v.apps[*i].to_string(), String::new()),
            },
        };
        let selected = k == picked;
        let x = f.put(x0, y, if selected { "▸ " } else { "  " }, Style::Title, x0 + col_w);
        let info_w = cells(&info).min(col_w);
        let name = truncate(&name, (x0 + col_w).saturating_sub(x + info_w + 1));
        f.put(x, y, &name, if selected { Style::Title } else { Style::Text }, x0 + col_w);
        f.put(x0 + col_w - info_w, y, &info, Style::Muted, x0 + col_w);
    }
    f
}
```

- [ ] **Step 4: Run** `cargo test -q --lib` — Expected: all pass; the first run writes `tests/snapshots/picker_120x30.txt` and `picker_60x20.txt`. Inspect them (`cat tests/snapshots/picker_*.txt`): header, filter placeholder, `▸ All apps` first with 244, the three apps with right-aligned origin/count, 3 columns at 120 wide and 1 at 60.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: picker screen layout"`

---

### Task 10: Wire the picker into `cst`

**Files:**
- Modify: `src/ui/mod.rs`, `src/main.rs`

**Interfaces:**
- `ui::run(sections, apps, entries: &[layout::Entry], theme, mode, st)`; `main::entries(loaded: &[(&str, Loaded)]) -> Vec<Entry>`.

- [ ] **Step 1: Failing test** — append to `main.rs` tests:

```rust
    #[test]
    fn picker_entries_from_loaded() {
        let secs = build("tmux", &[Binding::new("Prefix", vec![vec!["c".into()]], "New window"),
            Binding::new("Prefix", vec![vec!["d".into()]], "Detach")], Some("config: x".into()));
        let loaded = vec![("tmux", Loaded { sections: secs, origin: Origin::Mixed, note: Some("config: x".into()) })];
        let e = entries(&loaded);
        assert_eq!(e, vec![cst::ui::layout::Entry { name: "tmux".into(), origin: "mixed".into(), count: 2, warn: true }]);
    }
```

- [ ] **Step 2: Run** `cargo test -q` — Expected: `cannot find function entries`.

- [ ] **Step 3: Implement.** In `main.rs`:

```rust
fn origin_name(o: Origin) -> &'static str {
    match o { Origin::Live => "live", Origin::Defaults => "defaults", Origin::Mixed => "mixed" }
}

fn entries(loaded: &[(&str, Loaded)]) -> Vec<ui::layout::Entry> {
    loaded.iter().map(|(app, l)| ui::layout::Entry {
        name: app.to_string(),
        origin: origin_name(l.origin).into(),
        count: l.sections.iter().flat_map(|s| &s.rows).map(|r| r.alts.len()).sum(),
        warn: l.note.is_some(),
    }).collect()
}
```

use `origin_name` in `list_text` too (replacing its inline match). In `run()`: with no argument set `st.screen = ui::input::Screen::Picker`; pass `&entries(&loaded)` to `ui::run`. Update `HELP`: `cst` → "pick an app (Enter opens it, Esc goes back)".

In `ui::run` add the `entries: &[layout::Entry]` parameter and switch per screen inside the loop:

```rust
            let (w, h) = terminal::size()?;
            let (w, h) = (w as usize, h as usize);
            let f = match st.screen {
                input::Screen::Picker => layout::picker_frame(&view, entries, &st, w, h),
                input::Screen::Sheet => {
                    let (f, max) = layout::frame(&view, &st, w, h);
                    st.scroll = st.scroll.min(max);
                    f
                }
            };
            term.draw(&mut out, &f)?;
            match event::read()? {
                Event::Key(k) => if let Some(key) = input::from_event(&k) {
                    match st.screen {
                        input::Screen::Picker => input::apply_picker(&mut st, key, apps, layout::picker_cols(w)),
                        input::Screen::Sheet => input::apply(&mut st, key, apps.len(), h.saturating_sub(4).max(1)),
                    }
                },
                Event::Resize(..) => term.invalidate(),
                _ => {}
            }
```

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --all-targets -- -D warnings` — Expected: all pass, clippy clean.

- [ ] **Step 5: End to end** — `cargo build --release -q && target/release/cst --list` — Expected: 15 lines; zsh and bash `live`; fish, nushell and readline `not installed` on this machine (no fish, no nu, no inputrc). Then drive the TUI in a private tmux session:

```bash
t(){ tmux -L cstpick "$@"; }
t -f /dev/null new -d -x 120 -y 30 -s p "env -u TMUX COLORTERM=truecolor $PWD/target/release/cst; echo EXITED; sleep 20"; sleep 0.8
t capture-pane -p -t p | head -6                    # picker: "Choose an app…", "▸ All apps"
t send -t p z s h; sleep 0.2; t send -t p Right Enter; sleep 0.3
t capture-pane -p -t p | head -4                    # zsh sheet: "focus: zsh", "ZSH · INSERT"
t send -t p Escape; sleep 0.2; t capture-pane -p -t p | sed -n 2p   # back in the picker, filter "zsh▏"
t send -t p Escape Escape; sleep 0.3; t capture-pane -p -t p | grep -c EXITED   # 1
t kill-server
```

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: plain cst opens the app picker"`

---

### Task 11: README, screenshots, release v0.2.0

**Files:**
- Modify: `README.md`, `scripts/screenshot.sh` (`CST_SHOT_OUT`), `docs/screenshot.svg`
- Create: `docs/picker.svg`

- [ ] **Step 1: `screenshot.sh` output path** — replace the hard-coded `"$root/docs/screenshot.svg"` with `"${CST_SHOT_OUT:-$root/docs/screenshot.svg}"` (both in the redirect and the final echo). `shellcheck scripts/screenshot.sh` — silent.

- [ ] **Step 2: Screenshots** —

```bash
cargo build --release -q
CST_SHOT_OUT=docs/picker.svg scripts/screenshot.sh
CST_SHOT_KEYS="Enter" scripts/screenshot.sh          # All apps sheet
grep -c "<text" docs/picker.svg docs/screenshot.svg
```

Expected: both written, hundreds of `<text>` elements in the sheet, dozens in the picker. List anything that looks private in the task report.

- [ ] **Step 3: README** — in **Supported apps** add these rows after zellij:

```markdown
| zsh | the live line editor (`bindkey`): insert and, in vi mode, normal keymap |
| bash | the live readline bindings (`bind -p`), emacs or vi |
| fish | the live `bind` table (fish 3 and 4 notations), per mode |
| nushell | reedline defaults plus `$env.config.keybindings` |
| readline | `~/.inputrc` (or `$INPUTRC`) over readline's defaults — shown when you have one |
```

In **Usage** replace the code block and add the picker keys:

````markdown
```sh
cst            # pick an app (or All apps)
cst tmux       # straight to one app
cst --list     # detected apps and where their keys come from
```

In the picker: type to filter, arrows or `Tab` to move, `Enter` to open, `Esc` to go back
from a sheet or to quit.
````

and under the screenshot in the header block add `<img src="docs/picker.svg" alt="the cst app picker" width="100%">` after a line `<br>`.

- [ ] **Step 4: Commit and push** — `git add -A && git commit -m "docs: shells and picker in README, new screenshots" && git push` — then watch CI: `gh run watch "$(gh run list -R 13/cst -w ci -L 1 --json databaseId -q '.[0].databaseId')" -R 13/cst --exit-status` — Expected: green.

- [ ] **Step 5: Release v0.2.0** — `scripts/release.sh 0.2.0`, then `gh run watch "$(gh run list -R 13/cst -w release -L 1 --json databaseId -q '.[0].databaseId')" -R 13/cst --exit-status` — Expected: build ✓, pacman ✓, aur ✓ (published if `AUR_SSH_KEY` is set, else the warning). `gh release view v0.2.0 -R 13/cst --json assets -q '.assets[].name'` lists the five assets with `cst-bin-0.2.0-1-x86_64.pkg.tar.zst`.

- [ ] **Step 6: Upgrade the local install** — `curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash && cst --version` — Expected: `cst 0.2.0`.
