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
    ("fzf-history-widget", "Search history (fzf)"), ("fzf-file-widget", "Insert file (fzf)"),
    ("fzf-cd-widget", "Change directory (fzf)"), ("fzf-completion", "Complete (fzf)"),
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
    if last.eq_ignore_ascii_case("del")
        && let Some(k) = c.last_mut() { *k = "Bksp".into(); }
    c
}

/// Index of the first unescaped `"` in `s`.
pub fn closing_quote(s: &str) -> Option<usize> {
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
