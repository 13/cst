//! cst: keyboard shortcut sheet of the installed apps.
use cst::model::Section;
use cst::sources::{self, Env, Loaded, Origin};
use cst::{theme, ui};

const HELP: &str = "\
cst — keyboard shortcuts of your installed apps

Usage: cst            pick an app (Enter opens it, Esc goes back)
       cst APP        open that app's sheet
       cst --list     show detected apps and where their keys come from
       cst --help | --version

Keys: type to filter · press a shortcut (Ctrl+…, Alt+…, F5) to look it up
      Backspace · Tab/Shift+Tab next/previous app
      ↑ ↓ PgUp PgDn Home End scroll · Esc clear filter, back to the picker, quit
      Ctrl+C twice quit
";

fn origin_name(o: Origin) -> &'static str {
    match o { Origin::Live => "live", Origin::Defaults => "defaults", Origin::Mixed => "mixed" }
}

/// Picker entries, one per loaded app.
fn entries(loaded: &[(&str, Loaded)]) -> Vec<ui::layout::Entry> {
    loaded.iter().map(|(app, l)| ui::layout::Entry {
        name: app.to_string(),
        origin: origin_name(l.origin).into(),
        count: l.sections.iter().flat_map(|s| &s.rows).map(|r| r.alts.len()).sum(),
        warn: l.note.is_some(),
    }).collect()
}

fn list_text(apps: &[&str], loaded: &[(&str, Loaded)]) -> String {
    let mut out = String::new();
    for app in apps {
        match loaded.iter().find(|(a, _)| a == app) {
            None => out.push_str(&format!("{app:<10} not installed\n")),
            Some((_, l)) => {
                let origin = origin_name(l.origin);
                let n: usize = l.sections.iter().flat_map(|s| &s.rows).map(|r| r.alts.len()).sum();
                let note = l.note.as_ref().map(|n| format!("  ⚠ {n}")).unwrap_or_default();
                out.push_str(&format!("{app:<10} {origin:<10} {n} bindings{note}\n"));
            }
        }
    }
    out
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = args.first().map(String::as_str);
    match arg {
        Some("-h" | "--help") => { print!("{HELP}"); return 0; }
        Some("-V" | "--version") => { println!("cst {}", env!("CARGO_PKG_VERSION")); return 0; }
        Some(a) if a.starts_with('-') && a != "--list" => { eprint!("cst: unknown option {a}\n\n{HELP}"); return 2; }
        _ => {}
    }
    let env = Env::system();
    let all = sources::all();
    let loaded = sources::load_installed(&all, &env);
    if arg == Some("--list") {
        let names: Vec<&str> = all.iter().map(|s| s.app()).collect();
        print!("{}", list_text(&names, &loaded));
        return 0;
    }
    let apps: Vec<&str> = loaded.iter().map(|(a, _)| *a).collect();
    let mut st = ui::input::State { screen: ui::input::Screen::Picker, ..Default::default() };
    if let Some(app) = arg {
        match apps.iter().position(|a| *a == app) {
            Some(i) => { st.focus = Some(i); st.screen = ui::input::Screen::Sheet; }
            None => { eprintln!("cst: unknown or not installed app '{app}' (installed: {})", apps.join(", ")); return 2; }
        }
    }
    let sections: Vec<Section> = loaded.iter().flat_map(|(_, l)| l.sections.clone()).collect();
    let theme = theme::load(&env);
    let mode = theme::ColorMode::detect(&env);
    match ui::run(&sections, &apps, &entries(&loaded), &theme, mode, st) {
        Ok(()) => 0,
        Err(e) => { eprintln!("cst: {e}"); 1 }
    }
}

fn main() {
    std::process::exit(run());
}

#[cfg(test)]
mod tests {
    use super::*;
    use cst::model::{build, Binding};
    use cst::sources::{Loaded, Origin};

    #[test]
    fn list_lines() {
        let secs = build("tmux", &[Binding::new("Prefix", vec![vec!["c".into()]], "New window")], Some("config: x".into()));
        let loaded = vec![("tmux", Loaded { sections: secs, origin: Origin::Mixed, note: Some("config: x".into()) })];
        let t = list_text(&["awesome", "tmux"], &loaded);
        assert_eq!(t, "awesome    not installed\ntmux       mixed      1 bindings  ⚠ config: x\n");
    }

    #[test]
    fn picker_entries_from_loaded() {
        let secs = build("tmux", &[Binding::new("Prefix", vec![vec!["c".into()]], "New window"),
            Binding::new("Prefix", vec![vec!["d".into()]], "Detach")], Some("config: x".into()));
        let loaded = vec![("tmux", Loaded { sections: secs, origin: Origin::Mixed, note: Some("config: x".into()) })];
        let e = entries(&loaded);
        assert_eq!(e, vec![cst::ui::layout::Entry { name: "tmux".into(), origin: "mixed".into(), count: 2, warn: true }]);
    }
}
