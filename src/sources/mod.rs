//! Where bindings come from: each app is a Source that reads its live config
//! (files, or commands through Env's Runner) and layers it over bundled
//! defaults. Nothing outside this module touches the filesystem.
use crate::keys::Seq;
use crate::model::{build, merge, Binding, Change, Section};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub mod alacritty;
pub mod awesome;
pub mod bash;
pub mod fish;
mod kdl;
pub mod fzf;
pub mod kitty;
pub mod lazygit;
pub mod shell;
pub mod nushell;
pub mod nvim;
pub mod readline;
pub mod tmux;
pub mod wezterm;
pub mod yazi;
pub mod zellij;
pub mod zsh;

pub trait Runner: Send + Sync {
    fn run(&self, cmd: &str, args: &[&str]) -> Option<String>;
}

pub struct SystemRunner { pub timeout: Duration }

impl Default for SystemRunner {
    fn default() -> Self { SystemRunner { timeout: Duration::from_secs(1) } }
}

unsafe extern "C" {
    fn setsid() -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

impl Runner for SystemRunner {
    fn run(&self, cmd: &str, args: &[&str]) -> Option<String> {
        use std::os::unix::process::CommandExt;
        let mut command = Command::new(cmd);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        // A new session without a controlling terminal: an interactive shell
        // (zsh -i) can't take over our terminal, and a timeout can kill the
        // whole group it started.
        unsafe { command.pre_exec(|| { setsid(); Ok(()) }); }
        let mut child = command.spawn().ok()?;
        let mut out = child.stdout.take()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = out.read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // a background grandchild may hold the pipe open: the deadline covers the read too
                    let buf = rx.recv_timeout(self.timeout.saturating_sub(start.elapsed())).ok()?;
                    return status.success().then(|| String::from_utf8_lossy(&buf).into_owned());
                }
                Ok(None) if start.elapsed() < self.timeout => std::thread::sleep(Duration::from_millis(5)),
                _ => {
                    unsafe { kill(-(child.id() as i32), 9); } // the child's whole process group
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }
}

pub struct FakeRunner(pub HashMap<String, String>);

impl Runner for FakeRunner {
    fn run(&self, cmd: &str, args: &[&str]) -> Option<String> {
        let key = std::iter::once(cmd).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        self.0.get(&key).cloned()
    }
}

pub struct Env { pub home: PathBuf, pub vars: HashMap<String, String>, pub runner: Arc<dyn Runner> }

impl Env {
    pub fn system() -> Env {
        let vars: HashMap<String, String> = std::env::vars().collect();
        let home = PathBuf::from(vars.get("HOME").cloned().unwrap_or_default());
        Env { home, vars, runner: Arc::new(SystemRunner::default()) }
    }

    pub fn test(home: &Path, runner: FakeRunner) -> Env {
        let vars = HashMap::from([("HOME".to_string(), home.display().to_string())]);
        Env { home: home.to_path_buf(), vars, runner: Arc::new(runner) }
    }

    pub fn var(&self, k: &str) -> Option<&str> {
        self.vars.get(k).map(|s| s.as_str()).filter(|s| !s.is_empty())
    }

    pub fn config(&self, rel: &str) -> PathBuf {
        self.var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| self.home.join(".config")).join(rel)
    }

    pub fn cache(&self, rel: &str) -> PathBuf {
        self.var("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| self.home.join(".cache")).join(rel)
    }

    pub fn read(&self, p: &Path) -> Option<String> {
        let meta = std::fs::metadata(p).ok()?;
        if !meta.is_file() || meta.len() > 1 << 20 { return None; }
        let bytes = std::fs::read(p).ok()?;
        Some(String::from_utf8_lossy(&bytes).replace('\r', ""))
    }

    pub fn which(&self, bin: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        self.var("PATH").unwrap_or("").split(':').any(|dir| {
            std::fs::metadata(Path::new(dir).join(bin)).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
        })
    }

    pub fn expand(&self, p: &str, base: &Path) -> PathBuf {
        if let Some(rest) = p.strip_prefix("~/") { self.home.join(rest) }
        else if Path::new(p).is_absolute() { PathBuf::from(p) }
        else { base.join(p) }
    }

    pub fn run(&self, cmd: &str, args: &[&str]) -> Option<String> { self.runner.run(cmd, args) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin { Live, Defaults, Mixed }

#[derive(Clone, Debug)]
pub struct Loaded { pub sections: Vec<Section>, pub origin: Origin, pub note: Option<String> }

pub struct Layered { pub bindings: Vec<Binding>, pub origin: Origin, pub note: Option<String> }

impl Layered {
    pub fn into_loaded(self, app: &str) -> Loaded {
        Loaded { sections: build(app, &self.bindings, self.note.clone()), origin: self.origin, note: self.note }
    }
}

/// live: Ok(Some) parsed config, Ok(None) no config (defaults only), Err(reason).
pub fn layer(defaults: Vec<Binding>, live: Result<Option<Vec<Change>>, String>, keep_defaults: bool) -> Layered {
    match live {
        Ok(Some(changes)) => {
            // by effect: defaults contributed nothing → Live (a cleared config),
            // the live layer changed nothing → Defaults, else Mixed
            let live_only = merge(vec![], changes.clone());
            let bindings = if keep_defaults { merge(defaults.clone(), changes) } else { live_only.clone() };
            let origin = if bindings == live_only { Origin::Live }
                else if bindings == defaults { Origin::Defaults }
                else { Origin::Mixed };
            Layered { bindings, origin, note: None }
        }
        Ok(None) => Layered { bindings: defaults, origin: Origin::Defaults, note: None },
        Err(reason) => Layered { bindings: defaults, origin: Origin::Defaults, note: Some(format!("config: {reason}")) },
    }
}

/// `group :: keys :: description [:: id]` per line; `#` comments.
pub fn parse_defaults(text: &str, parse: &dyn Fn(&str) -> Seq, scope_of: &dyn Fn(&str) -> String) -> Vec<Binding> {
    text.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { return None; }
        let f: Vec<&str> = line.split(" :: ").map(str::trim).collect();
        if f.len() < 3 { return None; }
        Some(Binding {
            scope: scope_of(f[0]), group: f[0].into(), seq: parse(f[1]), desc: f[2].into(),
            id: f.get(3).map(|s| s.to_string()).unwrap_or_default(),
        })
    }).collect()
}

/// Live bindings join the group of the default with the same description.
pub fn default_group(defaults: &[Binding], desc: &str) -> String {
    defaults.iter().find(|d| d.desc.eq_ignore_ascii_case(desc)).map(|d| d.group.clone()).unwrap_or_else(|| "Custom".into())
}

/// Exact action first, else the longest whole-word prefix (args dropped).
pub fn describe(table: &[(&str, &str)], action: &str) -> Option<String> {
    let a = action.trim().to_lowercase();
    if let Some((_, d)) = table.iter().find(|(k, _)| k.to_lowercase() == a) {
        return Some(d.to_string());
    }
    table.iter()
        .filter(|(k, _)| {
            let k = k.to_lowercase();
            a.starts_with(&k) && matches!(a[k.len()..].chars().next(), Some(' ') | Some('('))
        })
        .max_by_key(|(k, _)| k.len())
        .map(|(_, d)| d.to_string())
}

/// "toggle-preview" / "nextTab" / "copy_to_clipboard" → "Toggle preview" …
pub fn humanize(s: &str) -> String {
    let mut words = String::new();
    for (i, c) in s.chars().enumerate() {
        if c == '-' || c == '_' { words.push(' '); }
        else if c.is_uppercase() && i > 0 { words.push(' '); words.extend(c.to_lowercase()); }
        else if i == 0 { words.extend(c.to_uppercase()); }
        else { words.push(c); }
    }
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub trait Source: Sync {
    fn app(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    /// Whether to show this app; most sources: its binary is on PATH.
    fn installed(&self, env: &Env) -> bool { env.which(self.binary()) }
    fn load(&self, env: &Env) -> Loaded;
}

/// Every source in display order.
pub fn all() -> Vec<Box<dyn Source>> {
    vec![Box::new(awesome::Awesome), Box::new(kitty::Kitty), Box::new(wezterm::Wezterm), Box::new(alacritty::Alacritty), Box::new(tmux::Tmux), Box::new(zellij::Zellij), Box::new(zsh::Zsh), Box::new(bash::Bash), Box::new(fish::Fish), Box::new(nushell::Nushell), Box::new(readline::Readline), Box::new(nvim::Nvim), Box::new(yazi::Yazi), Box::new(lazygit::Lazygit), Box::new(fzf::Fzf)]
}

/// Installed sources, loaded in parallel, in the order given.
pub fn load_installed(sources: &[Box<dyn Source>], env: &Env) -> Vec<(&'static str, Loaded)> {
    let installed: Vec<&Box<dyn Source>> = sources.iter().filter(|s| s.installed(env)).collect();
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // a crashing source becomes a note, not stderr noise
    let out = std::thread::scope(|scope| {
        let handles: Vec<_> = installed.iter().map(|s| scope.spawn(move || s.load(env))).collect();
        handles.into_iter().zip(&installed).map(|(h, s)| {
            let loaded = h.join().unwrap_or_else(|_| {
                layer(vec![], Err(format!("{} source crashed", s.app())), true).into_loaded(s.app())
            });
            (s.app(), loaded)
        }).collect()
    });
    std::panic::set_hook(prev);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::parse_prefixed;

    fn kp(s: &str) -> Seq { vec![parse_prefixed(s, '+')] }

    #[test]
    fn defaults_file_format() {
        let text = "# comment\n\nTabs :: ctrl+t :: New tab\nTabs :: ctrl+w :: Close tab :: tabs.close\r\nbad line\n";
        let d = parse_defaults(text, &kp, &|g| g.to_lowercase());
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].scope, "tabs");
        assert_eq!(d[0].seq, kp("ctrl+t"));
        assert_eq!(d[1].id, "tabs.close");
        assert_eq!(d[1].desc, "Close tab");
    }

    #[test]
    fn layer_origins_and_notes() {
        let d = vec![Binding::new("", kp("a"), "A")];
        let l = layer(d.clone(), Ok(None), true);
        assert_eq!((l.origin, l.bindings.len(), l.note.is_none()), (Origin::Defaults, 1, true));
        let l = layer(d.clone(), Ok(Some(vec![Change::Bind(Binding::new("", kp("b"), "B"))])), true);
        assert_eq!((l.origin, l.bindings.len()), (Origin::Mixed, 2));
        let l = layer(d.clone(), Ok(Some(vec![Change::Bind(Binding::new("", kp("b"), "B"))])), false);
        assert_eq!((l.origin, l.bindings.len()), (Origin::Live, 1));
        let l = layer(d.clone(), Ok(Some(vec![Change::Clear(None)])), true);
        assert_eq!(l.origin, Origin::Live);
        // a config that changes nothing (kitty.conf without map lines) is Defaults
        let l = layer(d.clone(), Ok(Some(vec![])), true);
        assert_eq!(l.origin, Origin::Defaults);
        // cleared, then a live binding identical to a default: still pure Live
        let l = layer(d.clone(), Ok(Some(vec![Change::Clear(None), Change::Bind(Binding::new("", kp("a"), "A"))])), true);
        assert_eq!(l.origin, Origin::Live);
        let l = layer(d, Err("x.conf: line 3".into()), true);
        assert_eq!((l.origin, l.note.as_deref()), (Origin::Defaults, Some("config: x.conf: line 3")));
    }

    #[test]
    fn describe_exact_then_word_prefix() {
        let t = [("new_tab", "New tab"), ("goto_tab", "Go to tab"), ("split-window -h", "Split right"), ("split-window", "Split down")];
        assert_eq!(describe(&t, "new_tab").as_deref(), Some("New tab"));
        assert_eq!(describe(&t, "NEW_TAB").as_deref(), Some("New tab"));
        assert_eq!(describe(&t, "goto_tab 3").as_deref(), Some("Go to tab"));
        assert_eq!(describe(&t, "split-window -h -c '#{pane_current_path}'").as_deref(), Some("Split right"));
        assert_eq!(describe(&t, "new_tabular"), None);
        assert_eq!(describe(&t, "unknown"), None);
    }

    #[test]
    fn read_strips_carriage_returns() {
        let p = std::env::temp_dir().join("cst-crlf.conf");
        std::fs::write(&p, "a\r\nb\r\n").unwrap();
        assert_eq!(Env::system().read(&p).as_deref(), Some("a\nb\n"));
        assert_eq!(Env::system().read(Path::new("/nonexistent/x")), None);
    }

    #[test]
    fn humanize_words() {
        assert_eq!(humanize("toggle-preview"), "Toggle preview");
        assert_eq!(humanize("nextTab"), "Next tab");
        assert_eq!(humanize("copy_to_clipboard"), "Copy to clipboard");
    }

    #[test]
    fn system_runner_times_out_and_reports_failure() {
        let r = SystemRunner { timeout: Duration::from_millis(200) };
        let t = Instant::now();
        assert_eq!(r.run("sleep", &["5"]), None);
        assert!(t.elapsed() < Duration::from_secs(2));
        assert_eq!(r.run("sh", &["-c", "echo hi"]).as_deref(), Some("hi\n"));
        assert_eq!(r.run("false", &[]), None);
        assert_eq!(r.run("/nonexistent/cmd", &[]), None);
    }

    #[test]
    fn runner_deadline_covers_a_grandchild_holding_stdout() {
        // the child exits at once but a background grandchild keeps the pipe open
        let r = SystemRunner { timeout: Duration::from_millis(300) };
        let t = Instant::now();
        let _ = r.run("sh", &["-c", "echo hi; sleep 4 &"]);
        assert!(t.elapsed() < Duration::from_secs(2), "took {:?}", t.elapsed());
    }

    struct Boom;
    impl Source for Boom {
        fn app(&self) -> &'static str { "boom" }
        fn binary(&self) -> &'static str { "sh" }
        fn load(&self, _: &Env) -> Loaded { panic!("bad input") }
    }
    struct Fine;
    impl Source for Fine {
        fn app(&self) -> &'static str { "fine" }
        fn binary(&self) -> &'static str { "sh" }
        fn load(&self, _: &Env) -> Loaded { layer(vec![Binding::new("", kp("a"), "A")], Ok(None), true).into_loaded("fine") }
    }
    struct Missing;
    impl Source for Missing {
        fn app(&self) -> &'static str { "missing" }
        fn binary(&self) -> &'static str { "definitely-not-installed-cst" }
        fn load(&self, _: &Env) -> Loaded { unreachable!() }
    }

    #[test]
    fn panicking_source_becomes_note_and_order_is_kept() {
        let srcs: Vec<Box<dyn Source>> = vec![Box::new(Boom), Box::new(Missing), Box::new(Fine)];
        let out = load_installed(&srcs, &Env::system());
        let names: Vec<_> = out.iter().map(|(a, _)| *a).collect();
        assert_eq!(names, vec!["boom", "fine"]);
        assert!(out[0].1.note.as_deref().unwrap().contains("crashed"));
        assert_eq!(out[0].1.sections[0].title, "BOOM");
        assert_eq!(out[1].1.sections[0].rows.len(), 1);
    }

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

    #[test]
    fn children_get_their_own_session() {
        // a child that grabs the terminal (zsh -i) must not be able to keep it
        let r = SystemRunner::default();
        let out = r.run("sh", &["-c", "echo $$; ps -o sid= -p $$"]).unwrap();
        let v: Vec<&str> = out.split_whitespace().collect();
        assert_eq!(v[0], v[1], "child is not a session leader: {out}");
    }

    #[test]
    fn timeout_kills_the_whole_process_group() {
        let dir = std::env::temp_dir().join("cst-pgroup");
        std::fs::create_dir_all(&dir).unwrap();
        let pidfile = dir.join("pid");
        let _ = std::fs::remove_file(&pidfile);
        let r = SystemRunner { timeout: Duration::from_millis(300) };
        let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());
        assert_eq!(r.run("sh", &["-c", &script]), None);
        let pid = std::fs::read_to_string(&pidfile).unwrap().trim().to_string();
        std::thread::sleep(Duration::from_millis(100));
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists(), "grandchild {pid} survived the timeout");
    }
}
