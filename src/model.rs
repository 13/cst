//! Bindings from every source, merged by layer, grouped into sections of
//! rows (same description = one row with alternative key sequences).
use crate::keys::Seq;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Binding { pub scope: String, pub group: String, pub seq: Seq, pub desc: String, pub id: String }

impl Binding {
    pub fn new(group: &str, seq: Seq, desc: &str) -> Binding {
        Binding { group: group.into(), seq, desc: desc.into(), ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change { Bind(Binding), Unbind { scope: String, seq: Seq }, Remove(String), Clear(Option<String>) }

pub fn merge(mut base: Vec<Binding>, changes: Vec<Change>) -> Vec<Binding> {
    for c in changes {
        match c {
            Change::Bind(n) => {
                base.retain(|b| !(b.scope == n.scope && b.seq == n.seq) && (n.id.is_empty() || b.id != n.id));
                base.push(n);
            }
            Change::Unbind { scope, seq } => base.retain(|b| !(b.scope == scope && b.seq == seq)),
            Change::Remove(id) => base.retain(|b| b.id != id),
            Change::Clear(None) => base.clear(),
            Change::Clear(Some(scope)) => base.retain(|b| b.scope != scope),
        }
    }
    base
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub desc: String, pub alts: Vec<Seq> }

#[derive(Clone, Debug, PartialEq)]
pub struct Section { pub app: String, pub title: String, pub note: Option<String>, pub rows: Vec<Row> }

fn capitalize(s: &str) -> String {
    let mut c = s.trim().chars();
    match c.next() { Some(f) => f.to_uppercase().chain(c).collect(), None => String::new() }
}

pub fn build(app: &str, bindings: &[Binding], note: Option<String>) -> Vec<Section> {
    let mut groups: Vec<String> = Vec::new();
    for b in bindings {
        if !groups.contains(&b.group) { groups.push(b.group.clone()); }
    }
    let mut out: Vec<Section> = groups.iter().map(|g| {
        let mut rows: Vec<Row> = Vec::new();
        for b in bindings.iter().filter(|b| &b.group == g) {
            let desc = capitalize(&b.desc);
            match rows.iter_mut().find(|r| r.desc.to_lowercase() == desc.to_lowercase()) {
                Some(r) => if !r.alts.contains(&b.seq) { r.alts.push(b.seq.clone()) },
                None => rows.push(Row { desc, alts: vec![b.seq.clone()] }),
            }
        }
        rows.sort_by_key(|r| r.desc.to_lowercase());
        let title = if g.is_empty() { app.to_uppercase() } else { format!("{} · {}", app.to_uppercase(), g.to_uppercase()) };
        Section { app: app.into(), title, note: None, rows }
    }).collect();
    if note.is_some() {
        if out.is_empty() {
            out.push(Section { app: app.into(), title: app.to_uppercase(), note: None, rows: vec![] });
        }
        out[0].note = note;
    }
    out
}

pub fn row_matches(s: &Section, r: &Row, q: &str) -> bool {
    q.is_empty()
        || s.app.to_lowercase().contains(q)
        || s.title.to_lowercase().contains(q)
        || r.desc.to_lowercase().contains(q)
        || r.alts.iter().flatten().flatten().any(|k| k.to_lowercase().contains(q))
}

pub fn section_height(s: &Section) -> usize {
    1 + s.note.is_some() as usize + s.rows.len() + 1
}

/// Greedy: tallest first into the shortest column; each column keeps the
/// original order (port of lib/cheatsheet.lua columns()).
pub fn columns(heights: &[usize], n: usize) -> Vec<Vec<usize>> {
    let n = n.min(heights.len());
    if n == 0 { return vec![]; }
    let mut order: Vec<usize> = (0..heights.len()).collect();
    order.sort_by(|&a, &b| heights[b].cmp(&heights[a]).then(a.cmp(&b)));
    let mut cols = vec![Vec::new(); n];
    let mut h = vec![0usize; n];
    for i in order {
        let best = (0..n).min_by_key(|&c| (h[c], c)).unwrap();
        cols[best].push(i);
        h[best] += heights[i];
    }
    for c in &mut cols { c.sort(); }
    cols
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::parse_prefixed;

    fn b(scope: &str, group: &str, keys: &str, desc: &str) -> Binding {
        Binding { scope: scope.into(), group: group.into(), seq: vec![parse_prefixed(keys, '+')], desc: desc.into(), id: String::new() }
    }

    #[test]
    fn live_bind_overrides_same_scope_and_seq() {
        let out = merge(vec![b("", "Tabs", "ctrl+t", "New tab")], vec![Change::Bind(b("", "Custom", "ctrl+t", "Launch"))]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].desc, "Launch");
    }

    #[test]
    fn other_scope_is_untouched() {
        let out = merge(vec![b("pane", "Pane", "n", "New pane")], vec![Change::Bind(b("tab", "Tab", "n", "New tab"))]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn unbind_remove_and_clear() {
        let base = vec![b("a", "A", "x", "X"), b("b", "B", "y", "Y"), Binding { id: "u.quit".into(), ..b("", "U", "q", "Quit") }];
        let out = merge(base.clone(), vec![Change::Unbind { scope: "a".into(), seq: vec![parse_prefixed("x", '+')] }]);
        assert_eq!(out.len(), 2);
        let out = merge(base.clone(), vec![Change::Remove("u.quit".into())]);
        assert_eq!(out.len(), 2);
        let out = merge(base.clone(), vec![Change::Clear(Some("b".into()))]);
        assert_eq!(out.len(), 2);
        let out = merge(base, vec![Change::Clear(None), Change::Bind(b("", "G", "z", "Z"))]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn rebinding_same_id_replaces() {
        let base = vec![Binding { id: "u.quit".into(), ..b("", "U", "q", "Quit") }];
        let out = merge(base, vec![Change::Bind(Binding { id: "u.quit".into(), ..b("", "U", "Q", "Quit") })]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seq, vec![vec!["Q".to_string()]]);
    }

    #[test]
    fn build_groups_merges_and_sorts() {
        let secs = build("kitty", &[b("", "Tabs", "ctrl+t", "new tab"), b("", "Tabs", "super+t", "New Tab"),
            b("", "Tabs", "ctrl+w", "close tab"), b("", "", "f11", "fullscreen")], Some("config: bad".into()));
        assert_eq!(secs.len(), 2);
        assert_eq!(secs[0].title, "KITTY · TABS");
        assert_eq!(secs[0].note.as_deref(), Some("config: bad"));
        assert_eq!(secs[0].rows[0].desc, "Close tab");
        assert_eq!(secs[0].rows[1].desc, "New tab");
        assert_eq!(secs[0].rows[1].alts.len(), 2);
        assert_eq!(secs[1].title, "KITTY");
        assert_eq!(secs[1].note, None);
    }

    #[test]
    fn note_without_bindings_still_shows() {
        let secs = build("wezterm", &[], Some("config: wezterm show-keys failed".into()));
        assert_eq!(secs.len(), 1);
        assert!(secs[0].rows.is_empty());
    }

    #[test]
    fn duplicate_alternatives_dropped() {
        let secs = build("x", &[b("", "", "ctrl+t", "A"), b("", "", "ctrl+t", "a")], None);
        assert_eq!(secs[0].rows[0].alts.len(), 1);
    }

    #[test]
    fn filter_matches_title_desc_and_keys() {
        let secs = build("tmux", &[b("", "Prefix", "ctrl+a", "Split right")], None);
        let (s, r) = (&secs[0], &secs[0].rows[0]);
        assert!(row_matches(s, r, "split"));
        assert!(row_matches(s, r, "tmux"));
        assert!(row_matches(s, r, "prefix"));
        assert!(row_matches(s, r, "ctrl"));
        assert!(!row_matches(s, r, "zzz"));
    }

    #[test]
    fn columns_balance_and_keep_order() {
        assert_eq!(columns(&[10, 2, 2, 6], 2), vec![vec![0], vec![1, 2, 3]]);
        assert_eq!(columns(&[3, 3], 4), vec![vec![0], vec![1]]);
        assert_eq!(columns(&[], 3), Vec::<Vec<usize>>::new());
    }
}
