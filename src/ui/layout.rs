//! Sections → screen: header, filter line, balanced columns of key-cap rows
//! (the awesome Mod+i sheet, in cells). Filtering dims, never moves, rows.
use super::input::{picker_entries, State};
use super::render::{Frame, Style};
use crate::keys::Seq;
use crate::model::{columns, row_matches, section_height, Row, Section};
use unicode_width::UnicodeWidthChar;

pub struct View<'a> { pub sections: &'a [Section], pub apps: &'a [&'a str], pub theme_name: &'a str, pub plain: bool }

const COL_MIN: usize = 44;
const GAP: usize = 3;

type Line = Vec<(String, Style)>;

/// Cells `Frame::put` will use: the sum of character widths (str::width
/// treats emoji sequences differently, which would misalign the caps).
fn cells(s: &str) -> usize { s.chars().map(|c| c.width().unwrap_or(0)).sum() }

fn width(l: &Line) -> usize { l.iter().map(|(t, _)| cells(t)).sum() }

fn truncate(s: &str, max: usize) -> String {
    if cells(s) <= max { return s.to_string(); }
    if max == 0 { return String::new(); }
    let (mut out, mut w) = (String::new(), 0);
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max { break; }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

fn keys_line(alts: &[Seq], plain: bool, lit: bool) -> Line {
    let cap = if lit { Style::Cap } else { Style::Muted };
    let mut l = Line::new();
    for (i, seq) in alts.iter().enumerate() {
        if i > 0 { l.push(("/".into(), Style::Muted)); }
        for (j, combo) in seq.iter().enumerate() {
            if j > 0 { l.push(("›".into(), Style::Muted)); }
            for (k, key) in combo.iter().enumerate() {
                if k > 0 { l.push(("+".into(), Style::Muted)); }
                l.push((if plain { format!("[{key}]") } else { format!(" {key} ") }, cap));
            }
        }
    }
    l
}

/// Description left, key caps right-aligned to the column edge; only the
/// first alternative (plus …) when they don't all fit beside 8 cells of text.
fn row_line(r: &Row, w: usize, plain: bool, lit: bool) -> Line {
    let mut keys = keys_line(&r.alts, plain, lit);
    if width(&keys) + 1 + (w / 3).min(8) > w && r.alts.len() > 1 {
        keys = keys_line(&r.alts[..1], plain, lit);
        keys.push(("…".into(), Style::Muted));
    }
    let kw = width(&keys).min(w);
    let desc = truncate(&r.desc, w.saturating_sub(kw + 1));
    let pad = w.saturating_sub(cells(&desc) + kw);
    let mut line = vec![(desc, if lit { Style::Text } else { Style::Muted }), (" ".repeat(pad), Style::Text)];
    line.extend(keys);
    line
}

fn section_lines(s: &Section, w: usize, plain: bool, q: &str) -> Vec<Line> {
    let lit = |r: &Row| q.is_empty() || row_matches(s, r, q);
    let any = q.is_empty() || s.rows.iter().any(&lit);
    let mut out = vec![vec![(truncate(&s.title, w), if any { Style::Title } else { Style::Muted })]];
    if let Some(n) = &s.note { out.push(vec![(truncate(&format!("⚠ {n}"), w), Style::Muted)]); }
    out.extend(s.rows.iter().map(|r| row_line(r, w, plain, lit(r))));
    out.push(vec![]);
    out
}

/// The whole screen for this state, and the largest useful scroll offset.
pub fn frame(v: &View, st: &State, w: usize, h: usize) -> (Frame, usize) {
    let mut f = Frame::new(w, h);
    if w < 20 || h < 5 {
        f.put(0, 0, "Terminal too small", Style::Muted, w);
        return (f, 0);
    }
    let right = w - 1;
    let focus = st.focus.and_then(|i| v.apps.get(i));
    let info = match focus { Some(a) => format!("focus: {a}"), None => format!("{} apps · {}", v.apps.len(), v.theme_name) };
    header(&mut f, &info);

    let q = st.query.to_lowercase();
    let shown: Vec<&Section> = v.sections.iter().filter(|s| focus.is_none_or(|a| s.app == *a)).collect();
    if st.query.is_empty() {
        f.put(1, 1, "Type to filter…", Style::Muted, right);
    } else {
        let x = f.put(1, 1, &st.query, Style::Text, right);
        let x = f.put(x, 1, "▏", Style::Text, right);
        if !shown.iter().any(|s| s.rows.iter().any(|r| row_matches(s, r, &q))) {
            f.put(x + 2, 1, "No matches", Style::Muted, right);
        }
    }

    let (top, body_h) = (3, h - 4);
    if shown.is_empty() {
        f.put(1, top, "No supported apps found", Style::Muted, right);
        return (f, 0);
    }
    let n = (w / COL_MIN).clamp(1, 4);
    let col_w = (w - 2 - GAP * (n - 1)) / n;
    let heights: Vec<usize> = shown.iter().map(|s| section_height(s)).collect();
    let cols: Vec<Vec<Line>> = columns(&heights, n).iter()
        .map(|idx| idx.iter().flat_map(|&i| section_lines(shown[i], col_w, v.plain, &q)).collect())
        .collect();
    let max_scroll = cols.iter().map(Vec::len).max().unwrap_or(0).saturating_sub(body_h);
    let scroll = st.scroll.min(max_scroll);
    for (c, lines) in cols.iter().enumerate() {
        let x0 = 1 + c * (col_w + GAP);
        for (i, line) in lines.iter().skip(scroll).take(body_h).enumerate() {
            let mut x = x0;
            for (t, s) in line { x = f.put(x, top + i, t, *s, x0 + col_w); }
        }
    }
    if scroll > 0 { f.put(right - cells("↑ more"), 2, "↑ more", Style::Muted, w); }
    if scroll < max_scroll { f.put(right - cells("↓ more"), h - 1, "↓ more", Style::Muted, w); }
    (f, max_scroll)
}

/// Row 0: title left, `info` right-aligned when it fits.
fn header(f: &mut Frame, info: &str) {
    let right = f.w - 1;
    let end = f.put(1, 0, "Keyboard shortcuts", Style::Header, right);
    if end + 2 + cells(info) <= right { f.put(right - cells(info), 0, info, Style::Muted, right); }
}

/// One picker entry per app, in the same order as `View.apps`.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry { pub name: String, pub origin: String, pub count: usize, pub warn: bool }

const PICK_MIN: usize = 30;

pub fn picker_cols(w: usize) -> usize {
    ((w.saturating_sub(2) + GAP) / (PICK_MIN + GAP)).clamp(1, 4)
}

/// The app picker: `All apps` then each app with origin and binding count,
/// row-major in `picker_cols` columns, scrolled so the selection is visible.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{parse_prefixed, parse_vim};
    use crate::model::{build, Binding};
    use std::path::PathBuf;

    const APPS: [&str; 3] = ["kitty", "tmux", "nvim"];

    fn sample() -> Vec<Section> {
        let kp = |s: &str| vec![parse_prefixed(s, '+')];
        let tp = |k: &str| vec![parse_prefixed("C-b", '-'), vec![k.to_string()]];
        let mut v = build("kitty", &[Binding::new("Tabs", kp("ctrl+shift+t"), "New tab"),
            Binding::new("Tabs", kp("ctrl+shift+q"), "Close tab"), Binding::new("Tabs", kp("super+t"), "New tab")], None);
        v.extend(build("tmux", &[Binding::new("Prefix", tp("%"), "Split right"), Binding::new("Prefix", tp("\""), "Split down")],
            Some("config: .tmux.conf: unreadable".into())));
        v.extend(build("nvim", &[Binding::new("Mappings", parse_vim(" ff"), "日本語のテスト"),
            Binding::new("Mappings", parse_vim("gg"), "Top of file")], None));
        v
    }

    fn view(s: &[Section]) -> View<'_> { View { sections: s, apps: &APPS, theme_name: "catppuccin", plain: false } }

    fn snapshot(name: &str, text: &str) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots").join(name);
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() || !path.exists() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            return;
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "snapshot {name} changed; rerun with UPDATE_SNAPSHOTS=1 if intended");
    }

    /// (x, y) of the first cell sequence spelling `needle` (ASCII needles).
    fn find(f: &Frame, needle: &str) -> Option<(usize, usize)> {
        let n: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
        f.rows.iter().enumerate().find_map(|(y, row)| {
            (0..row.len().saturating_sub(n.len() - 1)).find(|&x| row[x..x + n.len()].iter().map(|c| &c.ch).eq(n.iter())).map(|x| (x, y))
        })
    }

    fn last_ink(f: &Frame, y: usize, from: usize, to: usize) -> usize {
        (from..to.min(f.w)).rev().find(|&x| !f.rows[y][x].ch.trim().is_empty()).unwrap()
    }

    #[test]
    fn snapshots() {
        let s = sample();
        snapshot("sheet_120x40.txt", &frame(&view(&s), &State::default(), 120, 40).0.text());
        snapshot("sheet_60x20.txt", &frame(&view(&s), &State::default(), 60, 20).0.text());
    }

    #[test]
    fn header_and_filter_line() {
        let s = sample();
        let t = frame(&view(&s), &State::default(), 120, 40).0.text();
        let first: Vec<&str> = t.lines().collect();
        assert!(first[0].starts_with(" Keyboard shortcuts") && first[0].ends_with("3 apps · catppuccin"));
        assert_eq!(first[1], " Type to filter…");
        let st = State { query: "zzz".into(), ..Default::default() };
        assert!(frame(&view(&s), &st, 120, 40).0.text().contains("zzz▏  No matches"));
    }

    #[test]
    fn caps_styled_and_filter_dims() {
        let s = sample();
        let (f, _) = frame(&view(&s), &State::default(), 120, 40);
        let (x, y) = find(&f, "Ctrl").unwrap();
        assert_eq!(f.rows[y][x].style, Style::Cap);
        let st = State { query: "split".into(), ..Default::default() };
        let (f, _) = frame(&view(&s), &st, 120, 40);
        let (x, y) = find(&f, "New tab").unwrap();
        assert_eq!(f.rows[y][x].style, Style::Muted);
        let (x, y) = find(&f, "Split right").unwrap();
        assert_eq!(f.rows[y][x].style, Style::Text);
        let (x, y) = find(&f, "KITTY").unwrap();
        assert_eq!(f.rows[y][x].style, Style::Muted);
        let (x, y) = find(&f, "TMUX").unwrap();
        assert_eq!(f.rows[y][x].style, Style::Title);
        assert_eq!(find(&f, "Split right").unwrap().1, find(&frame(&view(&s), &State::default(), 120, 40).0, "Split right").unwrap().1); // nothing moves
    }

    #[test]
    fn wide_chars_keep_caps_aligned() {
        let s = sample();
        let (f, _) = frame(&view(&s), &State::default(), 120, 40);
        let (x0, y0) = find(&f, "NVIM").unwrap();   // nvim shares its column with kitty; only rows below its title
        let col_w = (120 - 2 - 3) / 2;
        let rows: Vec<usize> = (y0 + 1..f.h).filter(|&y| f.rows[y][x0..x0 + col_w].iter().any(|c| c.style == Style::Cap)).collect();
        let edges: Vec<usize> = rows.iter().map(|&y| last_ink(&f, y, x0, x0 + col_w)).collect();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0], edges[1]);
    }

    #[test]
    fn emoji_sequences_keep_caps_aligned_and_whole() {
        let kp = |s: &str| vec![parse_prefixed(s, '+')];
        let s = build("x", &[Binding::new("", kp("ctrl+x"), "\u{2699}\u{FE0F} Settings"),
            Binding::new("", kp("ctrl+y"), "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} Family"),
            Binding::new("", kp("ctrl+z"), "Plain")], None);
        let v = View { sections: &s, apps: &["x"], theme_name: "t", plain: false };
        let (f, _) = frame(&v, &State::default(), 60, 20);
        let rows: Vec<usize> = (0..f.h).filter(|&y| f.rows[y].iter().any(|c| c.style == Style::Cap)).collect();
        let edges: Vec<usize> = rows.iter().map(|&y| last_ink(&f, y, 0, 60)).collect();
        assert_eq!(edges.len(), 3);
        assert!(edges.iter().all(|&e| e == edges[0]), "caps misaligned: {edges:?}");
        let t = f.text();
        assert!(t.contains("Ctrl + X") && t.contains("Ctrl + Y"), "{t}");
    }

    #[test]
    fn narrow_terminal_one_column_with_ellipsis() {
        let s = sample();
        let (f, _) = frame(&view(&s), &State::default(), 30, 40);
        let t = f.text();
        assert!(t.contains("New tab") && t.contains('…'));
        assert!(f.rows.iter().all(|r| r.len() == 30));
    }

    #[test]
    fn too_small() {
        let s = sample();
        assert_eq!(frame(&view(&s), &State::default(), 19, 5).0.text().trim(), "Terminal too small");
        assert_eq!(frame(&view(&s), &State::default(), 40, 4).0.text().trim(), "Terminal too small");
    }

    #[test]
    fn scrolling_indicators_and_clamp() {
        let s = sample();
        let (f, max) = frame(&view(&s), &State::default(), 60, 8);
        assert!(max > 0);
        assert!(f.text().lines().nth(7).unwrap().ends_with("↓ more"));
        assert!(!f.text().lines().nth(2).unwrap_or("").contains("↑ more"));
        let (f, max2) = frame(&view(&s), &State { scroll: 1000, ..Default::default() }, 60, 8);
        assert_eq!(max2, max);
        assert!(f.text().lines().nth(2).unwrap().ends_with("↑ more"));
        assert!(!f.text().contains("↓ more"));
    }

    #[test]
    fn focus_shows_one_app() {
        let s = sample();
        let t = frame(&view(&s), &State { focus: Some(1), ..Default::default() }, 120, 40).0.text();
        assert!(t.contains("TMUX · PREFIX") && t.contains("focus: tmux") && !t.contains("KITTY"));
        assert!(t.contains("⚠ config: .tmux.conf: unreadable"));
    }

    #[test]
    fn empty_and_plain() {
        let t = frame(&View { sections: &[], apps: &[], theme_name: "x", plain: false }, &State::default(), 80, 10).0.text();
        assert!(t.contains("No supported apps found"));
        let s = sample();
        let t = frame(&View { plain: true, ..view(&s) }, &State::default(), 120, 40).0.text();
        assert!(t.contains("[Ctrl]+[Shift]+[T]"));
    }

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
}
