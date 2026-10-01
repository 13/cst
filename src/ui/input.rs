//! Interactive state of the sheet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State { pub query: String, pub focus: Option<usize>, pub scroll: usize, pub quit: bool }

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key { Char(char), Backspace, ClearQuery, Esc, Quit, Tab, BackTab, Up, Down, PageUp, PageDown, Home, End }

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
        Key::Esc if st.query.is_empty() => st.quit = true,
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
}
