//! Interactive state: the app picker and the sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Screen { Picker, #[default] Sheet }

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub query: String, pub focus: Option<usize>, pub scroll: usize, pub quit: bool,
    pub screen: Screen, pub from_picker: bool, pub picked: usize, pub pick_query: String,
}

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key { Char(char), Backspace, ClearQuery, Esc, Quit, Tab, BackTab, Up, Down, Left, Right, Enter, PageUp, PageDown, Home, End }

pub fn from_event(e: &KeyEvent) -> Option<Key> {
    if e.kind == KeyEventKind::Release { return None; }
    let ctrl = e.modifiers.contains(KeyModifiers::CONTROL);
    Some(match e.code {
        KeyCode::Char('c') | KeyCode::Char('d') if ctrl => Key::Quit,
        KeyCode::Char('u') if ctrl => Key::ClearQuery,
        KeyCode::Char(c) if !ctrl && !e.modifiers.contains(KeyModifiers::ALT) => Key::Char(c),
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
        Key::Char(c) => { st.query.push(c); st.scroll = 0; }
        Key::Backspace => { st.query.pop(); }
        Key::ClearQuery => st.query.clear(),
        // back to the picker when the sheet was opened from it
        Key::Esc if st.query.is_empty() => if st.from_picker { st.screen = Screen::Picker } else { st.quit = true },
        Key::Esc => st.query.clear(),
        Key::Quit => st.quit = true,
        Key::Tab => {
            st.focus = match st.focus { None if apps > 0 => Some(0), Some(i) if i + 1 < apps => Some(i + 1), _ => None };
            st.scroll = 0;
        }
        Key::BackTab => {
            st.focus = match st.focus { None if apps > 0 => Some(apps - 1), Some(i) if i > 0 => Some(i - 1), _ => None };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    fn ev(code: KeyCode, m: KeyModifiers) -> KeyEvent { KeyEvent::new(code, m) }

    #[test]
    fn events_map_to_keys() {
        assert_eq!(from_event(&ev(KeyCode::Char('c'), KeyModifiers::CONTROL)), Some(Key::Quit));
        assert_eq!(from_event(&ev(KeyCode::Char('d'), KeyModifiers::CONTROL)), Some(Key::Quit));
        assert_eq!(from_event(&ev(KeyCode::Char('u'), KeyModifiers::CONTROL)), Some(Key::ClearQuery));
        assert_eq!(from_event(&ev(KeyCode::Char('S'), KeyModifiers::SHIFT)), Some(Key::Char('S')));
        assert_eq!(from_event(&ev(KeyCode::Char('x'), KeyModifiers::ALT)), None);
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
        let mut st = State { query: "abc".into(), ..Default::default() };
        apply(&mut st, Key::ClearQuery, 3, 10);
        assert_eq!(st.query, "");
        apply(&mut st, Key::Backspace, 3, 10); // empty: no panic
    }

    #[test]
    fn focus_cycles_through_apps_and_back_to_all() {
        let mut st = State::default();
        let seq: Vec<Option<usize>> = (0..4).map(|_| { apply(&mut st, Key::Tab, 3, 10); st.focus }).collect();
        assert_eq!(seq, vec![Some(0), Some(1), Some(2), None]);
        apply(&mut st, Key::BackTab, 3, 10);
        assert_eq!(st.focus, Some(2));
        let mut none = State::default();
        apply(&mut none, Key::Tab, 0, 10);
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
}
