//! Interactive state: the app picker and the sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Screen { Picker, #[default] Sheet }

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub query: String, pub focus: Option<usize>, pub scroll: usize, pub quit: bool,
    pub screen: Screen, pub from_picker: bool, pub picked: usize, pub pick_query: String,
    /// A shortcut pressed in a sheet: rows using it are lit (shown, not run).
    pub pressed: Option<Combo>,
    /// When Ctrl+C was last pressed; a second press within a second quits.
    pub ctrl_c_at: Option<Instant>,
}

use crate::keys::{combo, Combo};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum Key { Char(char), Backspace, Esc, Tab, BackTab, Up, Down, Left, Right, Enter, PageUp, PageDown, Home, End, Combo(Combo) }

/// A key with Ctrl/Alt/Super, Shift on a non-character key, or an F-key, as
/// the sheets name it (Ctrl+C, Alt+←, Ctrl+Shift+T, F5); None otherwise.
fn combo_of(e: &KeyEvent) -> Option<Combo> {
    let m = e.modifiers;
    let (ctrl, alt, sup) = (m.contains(KeyModifiers::CONTROL), m.contains(KeyModifiers::ALT),
        m.intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META));
    let mut shift = m.contains(KeyModifiers::SHIFT);
    let name = match e.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) if c.is_ascii_uppercase() => { shift = true; c.to_ascii_lowercase().to_string() }
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Left => "left".into(), KeyCode::Right => "right".into(), KeyCode::Up => "up".into(), KeyCode::Down => "down".into(),
        KeyCode::Enter => "enter".into(), KeyCode::Tab => "tab".into(), KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(), KeyCode::Insert => "insert".into(), KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(), KeyCode::PageUp => "pageup".into(), KeyCode::PageDown => "pagedown".into(),
        KeyCode::Esc => "esc".into(), KeyCode::F(n) => format!("f{n}"),
        _ => return None,
    };
    let lookup = ctrl || alt || sup || matches!(e.code, KeyCode::F(_))
        || (m.contains(KeyModifiers::SHIFT) && !matches!(e.code, KeyCode::Char(_)));
    if !lookup { return None; }
    let mods: Vec<&str> = [(sup, "super"), (ctrl, "ctrl"), (alt, "alt"), (shift, "shift")].iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect();
    Some(combo(&mods, &name))
}

pub fn from_event(e: &KeyEvent) -> Option<Key> {
    if e.kind == KeyEventKind::Release { return None; }
    if e.code == KeyCode::BackTab { return Some(Key::BackTab); }
    if let Some(c) = combo_of(e) { return Some(Key::Combo(c)); }
    Some(match e.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Enter => Key::Enter,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        _ => return None,
    })
}

/// apps: number of apps (for focus cycling); page: body height (for PgUp/PgDn).
pub fn apply(st: &mut State, key: Key, apps: usize, page: usize) {
    match key {
        Key::Combo(c) => { st.pressed = Some(c); st.query.clear(); st.scroll = 0; }
        Key::Char(c) => { st.pressed = None; st.query.push(c); st.scroll = 0; }
        Key::Backspace if st.pressed.is_some() => st.pressed = None,
        Key::Backspace => { st.query.pop(); }
        Key::Esc if st.pressed.is_some() => st.pressed = None,
        // back to the picker when the sheet was opened from it
        Key::Esc if st.query.is_empty() => if st.from_picker { st.screen = Screen::Picker } else { st.quit = true },
        Key::Esc => st.query.clear(),
        Key::Tab => {
            if apps > 0 { st.focus = Some(match st.focus { Some(i) if i + 1 < apps => i + 1, _ => 0 }); }
            st.scroll = 0;
        }
        Key::BackTab => {
            if apps > 0 { st.focus = Some(match st.focus { Some(i) if i > 0 && i < apps => i - 1, _ => apps - 1 }); }
            st.scroll = 0;
        }
        Key::Up => st.scroll = st.scroll.saturating_sub(1),
        Key::Down => st.scroll = st.scroll.saturating_add(1),
        Key::PageUp => st.scroll = st.scroll.saturating_sub(page),
        Key::PageDown => st.scroll = st.scroll.saturating_add(page),
        Key::Home => st.scroll = 0,
        Key::End => st.scroll = usize::MAX / 2,
        Key::Left | Key::Right | Key::Enter => {}
    }
}

/// Every key goes through here first: Ctrl+C twice within a second quits,
/// any other key forgets the first press.
pub fn note_key(st: &mut State, key: &Key, now: Instant) {
    let is_ctrl_c = matches!(key, Key::Combo(c) if c.len() == 2 && c[0] == "Ctrl" && c[1] == "C");
    if !is_ctrl_c { st.ctrl_c_at = None; return; }
    if st.ctrl_c_at.is_some_and(|t| now.duration_since(t) <= Duration::from_secs(1)) { st.quit = true; }
    st.ctrl_c_at = Some(now);
}

/// Picker rows for a filter: indices of the matching apps, sorted by name.
pub fn picker_entries(apps: &[&str], query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    let mut out: Vec<usize> = (0..apps.len()).filter(|&i| apps[i].to_lowercase().contains(&q)).collect();
    out.sort_by_key(|&i| apps[i].to_lowercase());
    out
}

/// cols: entries per row in the picker grid (layout::picker_cols).
pub fn apply_picker(st: &mut State, key: Key, apps: &[&str], cols: usize) {
    let entries = picker_entries(apps, &st.pick_query);
    let n = entries.len();
    let cols = cols.max(1);
    let moving = matches!(key, Key::Right | Key::Tab | Key::Left | Key::BackTab | Key::Down | Key::Up | Key::Home | Key::PageUp | Key::End | Key::PageDown | Key::Enter);
    if n == 0 && moving { return; }   // nothing matches: nothing to move to or open
    match key {
        Key::Char(c) => { st.pick_query.push(c); st.picked = 0; }
        Key::Backspace => { st.pick_query.pop(); st.picked = 0; }
        Key::Esc if st.pick_query.is_empty() => st.quit = true,
        Key::Esc => { st.pick_query.clear(); st.picked = 0; }
        Key::Combo(_) => {}
        Key::Right | Key::Tab => st.picked = (st.picked + 1) % n,
        Key::Left | Key::BackTab => st.picked = (st.picked + n - 1) % n,
        Key::Down => st.picked = (st.picked + cols).min(n - 1),
        Key::Up => st.picked = st.picked.saturating_sub(cols),
        Key::Home | Key::PageUp => st.picked = 0,
        Key::End | Key::PageDown => st.picked = n - 1,
        Key::Enter => {
            st.focus = Some(entries[st.picked.min(n - 1)]);
            st.screen = Screen::Sheet;
            st.from_picker = true;
            st.query.clear();
            st.scroll = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    fn ev(code: KeyCode, m: KeyModifiers) -> KeyEvent { KeyEvent::new(code, m) }

    #[test]
    fn events_map_to_keys() {
        assert_eq!(from_event(&ev(KeyCode::Char('S'), KeyModifiers::SHIFT)), Some(Key::Char('S')));
        assert_eq!(from_event(&ev(KeyCode::BackTab, KeyModifiers::SHIFT)), Some(Key::BackTab));
        let mut release = ev(KeyCode::Char('a'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(from_event(&release), None);
    }

    #[test]
    fn typing_editing_and_esc() {
        let mut st = State { scroll: 5, ..Default::default() };
        for c in "sp".chars() { apply(&mut st, Key::Char(c), 3, 10); }
        assert_eq!((st.query.as_str(), st.scroll), ("sp", 0));
        apply(&mut st, Key::Backspace, 3, 10);
        assert_eq!(st.query, "s");
        apply(&mut st, Key::Esc, 3, 10);
        assert_eq!((st.query.as_str(), st.quit), ("", false));
        apply(&mut st, Key::Esc, 3, 10);
        assert!(st.quit);
        let mut st = State::default();
        apply(&mut st, Key::Backspace, 3, 10); // empty: no panic
    }

    #[test]
    fn tab_cycles_through_apps_and_wraps() {
        let mut st = State::default();
        let seq: Vec<Option<usize>> = (0..4).map(|_| { apply(&mut st, Key::Tab, 3, 10); st.focus }).collect();
        assert_eq!(seq, vec![Some(0), Some(1), Some(2), Some(0)]);     // never a combined view
        apply(&mut st, Key::BackTab, 3, 10);
        assert_eq!(st.focus, Some(2));
        let mut none = State::default();
        apply(&mut none, Key::Tab, 0, 10);                             // no apps: no panic
        assert_eq!(none.focus, None);
    }

    #[test]
    fn scrolling_keys() {
        let mut st = State::default();
        apply(&mut st, Key::Down, 3, 10);
        apply(&mut st, Key::PageDown, 3, 10);
        assert_eq!(st.scroll, 11);
        apply(&mut st, Key::Up, 3, 10);
        apply(&mut st, Key::PageUp, 3, 10);
        apply(&mut st, Key::Up, 3, 10);
        assert_eq!(st.scroll, 0);
        apply(&mut st, Key::End, 3, 10);
        assert!(st.scroll > 1_000_000);   // clamped by the layout's max scroll in run()
        apply(&mut st, Key::Home, 3, 10);
        assert_eq!(st.scroll, 0);
    }

    const APPS: [&str; 3] = ["tmux", "Kitty", "zsh"];   // detection order, mixed case

    fn picker() -> State { State { screen: Screen::Picker, ..Default::default() } }

    #[test]
    fn picker_entries_sorted_by_name() {
        assert_eq!(picker_entries(&APPS, ""), vec![1, 0, 2]);          // Kitty, tmux, zsh
        assert_eq!(picker_entries(&APPS, "T"), vec![1, 0]);            // Kitty, tmux
        assert_eq!(picker_entries(&APPS, "zzz"), Vec::<usize>::new());
    }

    #[test]
    fn picker_moves_wrap_and_rows() {
        let mut st = picker();
        apply_picker(&mut st, Key::Left, &APPS, 2);
        assert_eq!(st.picked, 2);                       // wraps to the last of 3 entries
        apply_picker(&mut st, Key::Right, &APPS, 2);
        assert_eq!(st.picked, 0);
        apply_picker(&mut st, Key::Down, &APPS, 2);
        assert_eq!(st.picked, 2);                       // one row = 2 columns
        apply_picker(&mut st, Key::Down, &APPS, 2);
        assert_eq!(st.picked, 2);                       // clamped
        apply_picker(&mut st, Key::Up, &APPS, 2);
        assert_eq!(st.picked, 0);
        apply_picker(&mut st, Key::Tab, &APPS, 2);
        assert_eq!(st.picked, 1);
        apply_picker(&mut st, Key::Home, &APPS, 2);
        assert_eq!(st.picked, 0);
    }

    #[test]
    fn picker_enter_opens_sheet_and_esc_comes_back() {
        let mut st = picker();
        for c in "t".chars() { apply_picker(&mut st, Key::Char(c), &APPS, 2); }
        apply_picker(&mut st, Key::End, &APPS, 2);      // [Kitty, tmux] → tmux
        apply_picker(&mut st, Key::Enter, &APPS, 2);
        assert_eq!((st.screen, st.focus, st.from_picker), (Screen::Sheet, Some(0), true));
        apply(&mut st, Key::Char('x'), 3, 10);
        apply(&mut st, Key::Esc, 3, 10);                // clears the sheet filter
        assert_eq!(st.screen, Screen::Sheet);
        apply(&mut st, Key::Esc, 3, 10);                // empty filter: back to the picker
        assert_eq!((st.screen, st.quit, st.picked, st.pick_query.as_str()), (Screen::Picker, false, 1, "t"));
        apply_picker(&mut st, Key::Esc, &APPS, 2);      // clears the picker filter
        assert_eq!((st.pick_query.as_str(), st.quit), ("", false));
        apply_picker(&mut st, Key::Esc, &APPS, 2);
        assert!(st.quit);
    }

    #[test]
    fn filter_without_matches_enter_does_nothing() {
        let mut st = picker();
        for c in "zzz".chars() { apply_picker(&mut st, Key::Char(c), &APPS, 2); }
        for k in [Key::Down, Key::Right, Key::Left, Key::Up, Key::End, Key::Tab, Key::BackTab] { apply_picker(&mut st, k, &APPS, 2); }
        assert_eq!(st.picked, 0);
        apply_picker(&mut st, Key::Enter, &APPS, 2);
        assert_eq!((st.screen, st.focus), (Screen::Picker, None));
    }

    #[test]
    fn sheet_without_picker_still_quits_on_esc() {
        let mut st = State::default();
        apply(&mut st, Key::Esc, 3, 10);
        assert!(st.quit);
        assert_eq!(from_event(&ev(KeyCode::Enter, KeyModifiers::NONE)), Some(Key::Enter));
        assert_eq!(from_event(&ev(KeyCode::Left, KeyModifiers::NONE)), Some(Key::Left));
    }

    fn combo_of(v: &[&str]) -> Key { Key::Combo(v.iter().map(|s| s.to_string()).collect()) }

    #[test]
    fn combos_from_events() {
        use KeyModifiers as M;
        let k = |code, m| from_event(&ev(code, m));
        assert_eq!(k(KeyCode::Char('c'), M::CONTROL), Some(combo_of(&["Ctrl", "C"])));
        assert_eq!(k(KeyCode::Char('u'), M::CONTROL), Some(combo_of(&["Ctrl", "U"])));     // a lookup now, not "clear"
        assert_eq!(k(KeyCode::Left, M::ALT), Some(combo_of(&["Alt", "←"])));
        assert_eq!(k(KeyCode::F(5), M::NONE), Some(combo_of(&["F5"])));
        assert_eq!(k(KeyCode::F(5), M::SHIFT), Some(combo_of(&["Shift", "F5"])));
        assert_eq!(k(KeyCode::Up, M::SHIFT), Some(combo_of(&["Shift", "↑"])));
        assert_eq!(k(KeyCode::Char('t'), M::CONTROL | M::SHIFT), Some(combo_of(&["Ctrl", "Shift", "T"])));  // kitty protocol
        assert_eq!(k(KeyCode::Char('T'), M::CONTROL | M::SHIFT), Some(combo_of(&["Ctrl", "Shift", "T"])));
        assert_eq!(k(KeyCode::Char('T'), M::ALT), Some(combo_of(&["Alt", "Shift", "T"])));                 // legacy Alt+Shift+T
        assert_eq!(k(KeyCode::Char(' '), M::CONTROL), Some(combo_of(&["Ctrl", "Space"])));
        assert_eq!(k(KeyCode::Tab, M::CONTROL), Some(combo_of(&["Ctrl", "Tab"])));
        assert_eq!(k(KeyCode::Char('x'), M::SUPER), Some(combo_of(&["Super", "X"])));
        assert_eq!(k(KeyCode::Char('x'), M::NONE), Some(Key::Char('x')));
        assert_eq!(k(KeyCode::Char('X'), M::SHIFT), Some(Key::Char('X')));
        assert_eq!(k(KeyCode::BackTab, M::SHIFT), Some(Key::BackTab));
        assert_eq!(k(KeyCode::Left, M::NONE), Some(Key::Left));
    }

    #[test]
    fn lookup_letters_and_clearing() {
        let mut st = State::default();
        apply(&mut st, combo_of(&["Ctrl", "B"]), 3, 10);
        assert_eq!(st.pressed, Some(vec!["Ctrl".to_string(), "B".to_string()]));
        apply(&mut st, Key::Char('x'), 3, 10);                         // typing switches to a text filter
        assert_eq!((st.pressed.clone(), st.query.as_str()), (None, "x"));
        apply(&mut st, combo_of(&["Ctrl", "B"]), 3, 10);               // a lookup replaces the text filter
        assert_eq!((st.pressed.is_some(), st.query.as_str()), (true, ""));
        apply(&mut st, Key::Backspace, 3, 10);
        assert_eq!(st.pressed, None);
        apply(&mut st, combo_of(&["F5"]), 3, 10);
        apply(&mut st, Key::Esc, 3, 10);                               // Esc clears the lookup first
        assert_eq!((st.pressed.clone(), st.quit), (None, false));
        apply(&mut st, Key::Esc, 3, 10);
        assert!(st.quit);
        let mut p = State { screen: Screen::Picker, ..Default::default() };
        apply_picker(&mut p, combo_of(&["Ctrl", "B"]), &["a", "b"], 2);   // the picker ignores lookups
        assert_eq!((p.pressed.clone(), p.screen, p.quit), (None, Screen::Picker, false));
    }

    #[test]
    fn ctrl_c_twice_within_a_second_quits() {
        use std::time::Duration;
        let t0 = std::time::Instant::now();
        let cc = combo_of(&["Ctrl", "C"]);
        let mut st = State::default();
        note_key(&mut st, &cc, t0);
        assert!(!st.quit && st.ctrl_c_at == Some(t0));
        note_key(&mut st, &cc, t0 + Duration::from_millis(500));
        assert!(st.quit);
        let mut st = State::default();
        note_key(&mut st, &cc, t0);
        note_key(&mut st, &cc, t0 + Duration::from_secs(2));            // too slow: just another lookup
        assert!(!st.quit);
        let mut st = State::default();
        note_key(&mut st, &cc, t0);
        note_key(&mut st, &Key::Char('a'), t0 + Duration::from_millis(100));   // another key in between resets
        assert_eq!(st.ctrl_c_at, None);
        note_key(&mut st, &cc, t0 + Duration::from_millis(200));
        assert!(!st.quit);
    }
}
