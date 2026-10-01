# cst Editor Sheets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add nano, vi, vim, NvChad, micro and emacs sheets, fix the nvim sheet missing mappings registered after startup, and release v0.3.0.

**Architecture:** Same pattern as every source (live layer over bundled defaults through `layer()`); new key notations in `keys.rs` (`parse_emacs`, `parse_nano`, `parse_micro`); a shared nvim dump (`nvim::dump`) used by both the nvim and nvchad sources; small `Env` additions (`data`, `resolve_bin`, `etc`).

**Tech Stack:** Rust (existing crate), no new crates.

**Spec:** `docs/superpowers/specs/2026-10-03-cst-editors-design.md`

## Global Constraints

- No new crates.
- App order: awesome, kitty, wezterm, alacritty, tmux, zellij, zsh, bash, fish, nushell, readline, nano, vi, vim, nvim, nvchad, micro, emacs, yazi, lazygit, fzf.
- Every command through `Env::run` (1 s timeout, own session); every file through `Env::read`.
- Captures and screenshots are made outside the tool session's environment: `env -u ZDOTDIR -u CLAUDE_CODE -u CLAUDECODE -u BASH_ENV …`.
- clippy `-D warnings` clean; all existing tests keep passing.

## Review Focus

1. NvChad installed but nvim's dump fails or times out: nvchad shows a ⚠ note, nvim falls back to defaults; neither panics. Test: Task 3 `nvchad_dump_failure_notes`.
2. A user mapping whose description happens to equal an NvChad one (overriding NvChad's key) goes to the nvchad sheet with the user's key — acceptable and tested so it is a decision, not an accident. Test: Task 3 `user_override_of_nvchad_desc_lands_in_nvchad`.
3. `vi` that is a symlink chain to vim (`vi → /etc/alternatives/vi → vim`) must be hidden; busybox/ex-vi shown. Test: Task 4 `vi_hidden_when_it_is_vim`.
4. A malformed `bindings.json` (trailing comma, comments, nested objects) must not panic; it gives defaults + note. Test: Task 6 `micro_bad_json_notes`.
5. nanorc lines with odd spacing, quoted macros containing spaces, and unknown menus. Test: Task 5 `nanorc_bind_unbind_macro`.

## File Structure

```
src/keys.rs                 + parse_emacs, parse_nano, parse_micro (Task 1)
src/sources/mod.rs          + Env::data, Env::resolve_bin, Env::etc; all() order (Tasks 1, 3–7)
src/sources/nvim.rs         dump() from vim.schedule with modes; NvChad split (Tasks 2, 3)
src/sources/nvchad.rs       (Task 3)
src/sources/vim.rs, vi.rs   (Task 4)   src/defaults/vim.txt, vi.txt
src/sources/nano.rs         (Task 5)   src/defaults/nano.txt
src/sources/micro.rs        (Task 6)   src/defaults/micro.txt   (includes a small JSON object reader)
src/sources/emacs.rs        (Task 6)   src/defaults/emacs.txt
tests/fixtures/{nvim,nvchad,vim,vi,nano,micro}/
README.md, docs/*.svg       (Task 7)
```

---

### Task 1: Key notations and `Env` helpers

**Files:**
- Modify: `src/keys.rs`, `src/sources/mod.rs`

**Interfaces:**
- Produces:
  - `pub fn parse_emacs(s: &str) -> Seq` — `C-x C-f`, `M-x`, `C-M-a`, `<f1>`, `RET`, `SPC`, `TAB`, `ESC`, `DEL` (backspace), `C-x 4 f`.
  - `pub fn parse_nano(s: &str) -> Combo` — `^S`, `M-U`, `Sh-M-C`, `F6`, `^Space`, `Bsp`, `^Left`, `Ins`, `Del`…
  - `pub fn parse_micro(s: &str) -> Combo` — `Ctrl-s`, `Alt-,`, `CtrlShift-Left`, `AltShift-Up`, `F1`, `Home`, `Ctrl--`.
  - `Env::data(rel) -> PathBuf` (`$XDG_DATA_HOME` | `~/.local/share`), `Env::etc(rel) -> PathBuf` (`$CST_SYSCONFDIR` | `/etc` — a test hook), `Env::resolve_bin(bin) -> Option<PathBuf>` (first executable in `$PATH`, symlinks resolved).

- [ ] **Step 1: Failing tests** — append to `keys::tests`:

```rust
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
            ("AltShift-Up", "Alt+Shift+↑"), ("F1", "F1"), ("Home", "Home"), ("Ctrl--", "Ctrl+-"), ("CtrlAlt-x", "Ctrl+Alt+X")] {
            assert_eq!(parse_micro(raw).join("+"), want, "{raw}");
        }
    }
```

and to `sources::tests`:

```rust
    #[test]
    fn env_dirs_and_resolve_bin() {
        let dir = std::env::temp_dir().join("cst-resolve");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("vim");
        std::fs::write(&real, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(&real, dir.join("vi")).unwrap();
        let mut env = Env::test(Path::new("/home/x"), FakeRunner(HashMap::new()));
        env.vars.insert("PATH".into(), dir.display().to_string());
        assert_eq!(env.resolve_bin("vi").unwrap().file_name().unwrap(), "vim");
        assert_eq!(env.resolve_bin("nope"), None);
        assert_eq!(env.data("nvim"), PathBuf::from("/home/x/.local/share/nvim"));
        assert_eq!(env.etc("nanorc"), PathBuf::from("/etc/nanorc"));
        env.vars.insert("XDG_DATA_HOME".into(), "/d".into());
        env.vars.insert("CST_SYSCONFDIR".into(), "/e".into());
        assert_eq!((env.data("nvim"), env.etc("nanorc")), (PathBuf::from("/d/nvim"), PathBuf::from("/e/nanorc")));
    }
```

- [ ] **Step 2: Run** `cargo test -q --lib` — Expected: compile errors.

- [ ] **Step 3: Implement.** `src/keys.rs` (above the tests):

```rust
/// Emacs key descriptions: space-separated steps, `C-`/`M-`/`S-` prefixes,
/// `<f1>`-style names, `RET` `SPC` `TAB` `ESC` and `DEL` (= backspace).
pub fn parse_emacs(s: &str) -> Seq {
    s.split_whitespace().map(|step| {
        let inner = step.strip_prefix('<').and_then(|x| x.strip_suffix('>')).filter(|x| !x.is_empty()).unwrap_or(step);
        let mut c = parse_prefixed(inner, '-');
        if step.rsplit('-').next().is_some_and(|k| k == "DEL") {
            if let Some(k) = c.last_mut() { *k = "Bksp".into(); }
        }
        c
    }).collect()
}

/// nano: `^X` Ctrl, `M-X` Alt, `Sh-M-X` Alt+Shift, plain names (`F1`, `Ins`, `Bsp`).
pub fn parse_nano(s: &str) -> Combo {
    if let Some(k) = s.strip_prefix("Sh-M-").filter(|k| !k.is_empty()) { return combo(&["alt", "shift"], k); }
    if let Some(k) = s.strip_prefix("M-").filter(|k| !k.is_empty()) { return combo(&["alt"], k); }
    if let Some(k) = s.strip_prefix('^').filter(|k| !k.is_empty()) { return combo(&["ctrl"], k); }
    combo(&[], if s.eq_ignore_ascii_case("bsp") { "bksp" } else { s })
}

/// micro: `Ctrl-s`, `CtrlShift-Left`, `Alt-,`, `Ctrl--`: a modifier word
/// (any of Ctrl/Alt/Shift run together) before the first `-` that follows it.
pub fn parse_micro(s: &str) -> Combo {
    if let Some(i) = s.find('-').filter(|&i| i > 0 && i + 1 < s.len()) {
        let (prefix, key) = (&s[..i], &s[i + 1..]);
        let mut rest = prefix;
        let mut mods = Vec::new();
        while !rest.is_empty() {
            match ["Ctrl", "Alt", "Shift"].iter().find(|m| rest.starts_with(**m)) {
                Some(m) => { mods.push(m.to_lowercase()); rest = &rest[m.len()..]; }
                None => break,
            }
        }
        if rest.is_empty() && !mods.is_empty() {
            let mods: Vec<&str> = mods.iter().map(String::as_str).collect();
            return combo(&mods, key);
        }
    }
    combo(&[], s)
}
```

`src/sources/mod.rs`, in `impl Env`:

```rust
    pub fn data(&self, rel: &str) -> PathBuf {
        self.var("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| self.home.join(".local/share")).join(rel)
    }

    /// System config dir (`/etc`); `CST_SYSCONFDIR` overrides it for tests.
    pub fn etc(&self, rel: &str) -> PathBuf {
        PathBuf::from(self.var("CST_SYSCONFDIR").unwrap_or("/etc")).join(rel)
    }

    /// First executable `bin` in `$PATH`, with symlinks resolved.
    pub fn resolve_bin(&self, bin: &str) -> Option<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        self.var("PATH").unwrap_or("").split(':').map(|d| Path::new(d).join(bin))
            .find(|p| std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false))
            .and_then(|p| std::fs::canonicalize(p).ok())
    }
```

- [ ] **Step 4: Run** `cargo test -q --lib` — Expected: all pass. `cargo clippy --all-targets -- -D warnings` clean.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: emacs, nano and micro key notations; Env data/etc/resolve_bin"`

---

### Task 2: nvim — dump after startup, with modes

**Files:**
- Modify: `src/sources/nvim.rs`
- Create: `tests/fixtures/nvim/nvchad-dump.txt` (real capture)

**Interfaces:**
- Produces: `pub const NVIM_LUA: &str` (new body), `pub fn dump(env: &Env) -> Option<String>` returning lines `mode<TAB>lhs<TAB>desc` for modes n, i, v; runner key `nvim --headless -c <NVIM_LUA>`.

The dump runs inside `vim.schedule` (after plugins and distributions register their mappings in the first event-loop tick), flushes stdout, then quits. The nvim sheet keeps showing normal-mode mappings only.

- [ ] **Step 1: Fixture** (real NvChad setup, outside the session environment):

```bash
mkdir -p tests/fixtures/nvim
D="for _, mode in ipairs({'n','i','v'}) do for _, m in ipairs(vim.api.nvim_get_keymap(mode)) do if m.desc and m.desc ~= '' and not m.lhs:find('<Plug>', 1, true) and not m.lhs:find('<SNR>', 1, true) then io.stdout:write(mode .. '\t' .. m.lhs .. '\t' .. m.desc .. '\n') end end end io.stdout:flush()"
env -u ZDOTDIR -u CLAUDE_CODE -u CLAUDECODE timeout 5 nvim --headless -c "lua vim.schedule(function() $D vim.cmd('qa!') end)" </dev/null 2>/dev/null > tests/fixtures/nvim/nvchad-dump.txt
wc -l < tests/fixtures/nvim/nvchad-dump.txt                      # ~123
grep -P "\tgeneral save file$|\tCMD enter command mode$" tests/fixtures/nvim/nvchad-dump.txt
```

Expected: `n	<C-S>	general save file` and `n	;	CMD enter command mode`.

- [ ] **Step 2: Failing tests** — in `nvim.rs` tests, replace the runner key and the sample output format, and add the real capture test:

```rust
    #[test]
    fn mappings_with_desc_over_defaults() {
        let out = "n\t ff\tFind files\nn\t<C-W>d\tShow diagnostics\nn\t<C-S>\tSave\ni\t<C-B>\tInsert-only map\n\nbroken line\n";
        let key = format!("nvim --headless -c {NVIM_LUA}");
        let env = Env::test(std::path::Path::new("/"), FakeRunner(HashMap::from([(key, out.to_string())])));
        let l = Nvim.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Show diagnostics"), vec!["Ctrl+W › d"]);
        assert_eq!(keys_of(&l, "NVIM · FILES", "Save"), vec![": › w › Enter", "Ctrl+S"]);
        assert_eq!(keys_of(&l, "NVIM · MOTION", "Top of file"), vec!["g › g"]);
        assert!(keys_of(&l, "NVIM · MAPPINGS", "Insert-only map").is_empty());     // normal mode only
    }

    #[test]
    fn real_capture_includes_late_mappings() {
        let out = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nvim/nvchad-dump.txt")).unwrap();
        let key = format!("nvim --headless -c {NVIM_LUA}");
        let env = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::from([(key, out)])));
        let l = Nvim.load(&env);   // no NvChad data dir here: everything stays in nvim
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "Telescope find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVIM · MAPPINGS", "CMD enter command mode"), vec![";"]);
    }
```

(`failure_falls_back` keeps its expectations.)

- [ ] **Step 3: Run** `cargo test -q --lib nvim` — Expected: FAIL (old runner key / old format).

- [ ] **Step 4: Implement** — in `nvim.rs` replace `NVIM_LUA` and the live-layer code:

```rust
/// Runs after the first event-loop tick (when distributions like NvChad
/// register their mappings), prints `mode<TAB>lhs<TAB>desc`, flushes, quits.
pub const NVIM_LUA: &str = "lua vim.schedule(function() for _, mode in ipairs({'n', 'i', 'v'}) do for _, m in ipairs(vim.api.nvim_get_keymap(mode)) do if m.desc and m.desc ~= '' and not m.lhs:find('<Plug>', 1, true) and not m.lhs:find('<SNR>', 1, true) then io.stdout:write(mode .. '\\t' .. m.lhs .. '\\t' .. m.desc .. '\\n') end end end io.stdout:flush() vim.cmd('qa!') end)";

/// `mode<TAB>lhs<TAB>desc` lines from the user's nvim, or None.
pub fn dump(env: &Env) -> Option<String> {
    env.run("nvim", &["--headless", "-c", NVIM_LUA])
}

/// Rows of a dump: (mode, lhs, desc).
pub fn rows(dump: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    dump.lines().filter_map(|l| {
        let mut f = l.splitn(3, '\t');
        Some((f.next()?, f.next()?, f.next()?))
    })
}
```

and in `Nvim::load`:

```rust
        let live = dump(env)
            .map(|out| Some(rows(&out).filter(|(mode, _, _)| *mode == "n").map(|(_, lhs, desc)| {
                let group = match default_group(&defaults, desc).as_str() { "Custom" => "Mappings".to_string(), g => g.to_string() };
                Change::Bind(Binding::new(&group, parse_vim(lhs), desc))
            }).collect()))
            .ok_or_else(|| "nvim --headless failed".to_string());
```

- [ ] **Step 5: Run** `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "fix(nvim): read mappings after startup (NvChad and lazy plugins), with modes"`

---

### Task 3: NvChad source and the nvim/NvChad split

**Files:**
- Create: `src/sources/nvchad.rs`, `tests/fixtures/nvchad/data/nvim/lazy/NvChad/lua/nvchad/mappings.lua` (copy of the real file)
- Modify: `src/sources/nvim.rs` (skip NvChad descriptions), `src/sources/mod.rs` (`pub mod nvchad;`, `all()` after nvim)

**Interfaces:**
- Produces: `pub struct NvChad;` (app `"nvchad"`, binary `"nvim"`, `installed` = nvim on PATH and the mappings file exists); `pub fn nvchad_descs(env: &Env) -> Option<HashSet<String>>` (lower-cased `desc = "…"`/`'…'` strings of NvChad's `mappings.lua`); `fn split_desc(desc: &str, mode: &str) -> (String, String)`.

Groups (first word of the description, case-insensitive) → (section, strip the word?): general → General ✓, toggle → Toggle ✓, telescope → Telescope ✓, terminal → Terminal ✓, whichkey → Whichkey ✓, nvimtree → NvimTree ✓, buffer → Buffer ✓, lsp → LSP ✓, comment → Comment ✓, format → Format ✓, blankline → Blankline ✓, nvcheatsheet → Cheatsheet ✓, switch → Windows (keep), move → Insert (keep). Otherwise insert mode → Insert, else Other. Scope = mode, so the same key in n and i is two rows.

- [ ] **Step 1: Fixture** —

```bash
mkdir -p tests/fixtures/nvchad/data/nvim/lazy/NvChad/lua/nvchad
cp ~/.local/share/nvim/lazy/NvChad/lua/nvchad/mappings.lua tests/fixtures/nvchad/data/nvim/lazy/NvChad/lua/nvchad/
grep -c 'desc = ' tests/fixtures/nvchad/data/nvim/lazy/NvChad/lua/nvchad/mappings.lua   # 46
```

- [ ] **Step 2: Failing tests** — bottom of `src/sources/nvchad.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::nvim::{Nvim, NVIM_LUA};
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn env(dump: Option<String>) -> Env {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nvchad");
        let runs = dump.map(|d| HashMap::from([(format!("nvim --headless -c {NVIM_LUA}"), d)])).unwrap_or_default();
        let mut e = Env::test(&root, FakeRunner(runs));
        e.vars.insert("XDG_DATA_HOME".into(), root.join("data").display().to_string());
        e
    }

    fn real_dump() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nvim/nvchad-dump.txt")).unwrap()
    }

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn descriptions_from_nvchad_source() {
        let d = nvchad_descs(&env(None)).unwrap();
        assert!(d.len() >= 40, "{}", d.len());          // 46 desc lines; repeated descriptions collapse
        assert!(d.contains("telescope find files") && d.contains("move beginning of line"));
    }

    #[test]
    fn sections_from_description_words() {
        assert_eq!(split_desc("telescope find files", "n"), ("Telescope".into(), "Find files".into()));
        assert_eq!(split_desc("LSP diagnostic loclist", "n"), ("LSP".into(), "Diagnostic loclist".into()));
        assert_eq!(split_desc("switch window left", "n"), ("Windows".into(), "Switch window left".into()));
        assert_eq!(split_desc("move beginning of line", "i"), ("Insert".into(), "Move beginning of line".into()));
        assert_eq!(split_desc("something else", "i"), ("Insert".into(), "Something else".into()));
        assert_eq!(split_desc("something else", "v"), ("Other".into(), "Something else".into()));
        assert_eq!(split_desc("general", "n"), ("General".into(), "General".into()));
    }

    #[test]
    fn real_nvchad_sheet() {
        let e = env(Some(real_dump()));
        if std::path::Path::new("/usr/bin/nvim").exists() {    // CI has no nvim
            let mut x = env(None);
            x.vars.insert("PATH".into(), "/usr/bin".into());
            assert!(NvChad.installed(&x));
        }
        let l = NvChad.load(&e);
        assert_eq!(l.origin, Origin::Live);
        assert_eq!(keys_of(&l, "NVCHAD · TELESCOPE", "Find files"), vec!["Space › f › f"]);
        assert_eq!(keys_of(&l, "NVCHAD · GENERAL", "Save file"), vec!["Ctrl+S"]);
        assert_eq!(keys_of(&l, "NVCHAD · WINDOWS", "Switch window left"), vec!["Ctrl+H"]);
        assert_eq!(keys_of(&l, "NVCHAD · INSERT", "Move beginning of line"), vec!["Ctrl+B"]);
        assert_eq!(keys_of(&l, "NVCHAD · TOGGLE", "Line number"), vec!["Space › n"]);
        assert!(keys_of(&l, "NVCHAD · OTHER", "CMD enter command mode").is_empty());   // user's own map stays in nvim
        // …and nvim no longer lists NvChad's mappings
        let n = Nvim.load(&e);
        assert!(n.sections.iter().flat_map(|s| &s.rows).all(|r| r.desc != "Telescope find files"));
        assert_eq!(keys_of(&n, "NVIM · MAPPINGS", "CMD enter command mode"), vec![";"]);
    }

    #[test]
    fn user_override_of_nvchad_desc_lands_in_nvchad() {
        let l = NvChad.load(&env(Some("n\t<leader>ff\ttelescope find files\n".into())));
        assert_eq!(keys_of(&l, "NVCHAD · TELESCOPE", "Find files"), vec!["Leader › f › f"]);
    }

    #[test]
    fn nvchad_dump_failure_notes() {
        let l = NvChad.load(&env(None));
        assert_eq!(l.note.as_deref(), Some("config: nvim --headless failed"));
        assert!(l.sections.iter().all(|s| s.rows.is_empty()));
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib nvchad` (after adding `pub mod nvchad;`) — Expected: compile errors.

- [ ] **Step 4: Implement** `src/sources/nvchad.rs` (above the tests):

```rust
//! NvChad: its own mappings (those whose description appears in NvChad's
//! lua/nvchad/mappings.lua), sectioned by the description's first word.
use super::nvim::{dump, rows};
use super::{layer, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};
use std::collections::HashSet;
use std::path::PathBuf;

const GROUPS: &[(&str, &str, bool)] = &[
    ("general", "General", true), ("toggle", "Toggle", true), ("telescope", "Telescope", true),
    ("terminal", "Terminal", true), ("whichkey", "Whichkey", true), ("nvimtree", "NvimTree", true),
    ("buffer", "Buffer", true), ("lsp", "LSP", true), ("comment", "Comment", true), ("format", "Format", true),
    ("blankline", "Blankline", true), ("nvcheatsheet", "Cheatsheet", true),
    ("switch", "Windows", false), ("move", "Insert", false),
];

pub struct NvChad;

fn mappings_file(env: &Env) -> PathBuf { env.data("nvim/lazy/NvChad/lua/nvchad/mappings.lua") }

pub fn nvchad_descs(env: &Env) -> Option<HashSet<String>> {
    let text = env.read(&mappings_file(env))?;
    let mut out = HashSet::new();
    for part in text.split("desc =").skip(1) {
        let part = part.trim_start();
        let Some(q) = part.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        if let Some(end) = part[1..].find(q) { out.insert(part[1..1 + end].to_lowercase()); }
    }
    Some(out)
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

pub fn split_desc(desc: &str, mode: &str) -> (String, String) {
    let first = desc.split_whitespace().next().unwrap_or("").to_lowercase();
    match GROUPS.iter().find(|(w, _, _)| *w == first) {
        Some((_, group, true)) => {
            let rest = desc.split_once(char::is_whitespace).map(|(_, r)| r.trim()).unwrap_or("");
            (group.to_string(), cap(if rest.is_empty() { desc } else { rest }))
        }
        Some((_, group, false)) => (group.to_string(), cap(desc)),
        None if mode == "i" => ("Insert".into(), cap(desc)),
        None => ("Other".into(), cap(desc)),
    }
}

impl Source for NvChad {
    fn app(&self) -> &'static str { "nvchad" }
    fn binary(&self) -> &'static str { "nvim" }
    fn installed(&self, env: &Env) -> bool { env.which("nvim") && mappings_file(env).is_file() }
    fn load(&self, env: &Env) -> Loaded {
        let descs = nvchad_descs(env).unwrap_or_default();
        let live = dump(env).map(|out| Some(rows(&out)
            .filter(|(_, _, desc)| descs.contains(&desc.to_lowercase()))
            .map(|(mode, lhs, desc)| {
                let (group, d) = split_desc(desc, mode);
                Change::Bind(Binding { scope: mode.to_string(), group, seq: parse_vim(lhs), desc: d, id: String::new() })
            }).collect()))
            .ok_or_else(|| "nvim --headless failed".to_string());
        layer(vec![], live, false).into_loaded("nvchad")
    }
}
```

In `nvim.rs` `Nvim::load`, before mapping rows: `let nvchad = super::nvchad::nvchad_descs(env).unwrap_or_default();` and add `.filter(|(_, _, desc)| !nvchad.contains(&desc.to_lowercase()))` to the row filter chain.

- [ ] **Step 5: Register** `pub mod nvchad;`; `all()`: `Box::new(nvim::Nvim), Box::new(nvchad::NvChad),`. Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: NvChad sheet; nvim keeps your own mappings"`

---
### Task 4: vim and vi sources

**Files:**
- Create: `src/sources/vim.rs`, `src/sources/vi.rs`, `src/defaults/vim.txt`, `src/defaults/vi.txt`, `tests/fixtures/vim/home/.vimrc`, `tests/fixtures/vim/map.txt`
- Modify: `src/sources/mod.rs` (`pub mod vi; pub mod vim;`, `all()` order)

**Interfaces:**
- Produces: `pub struct Vim;` (app/binary `"vim"`), `pub const VIM_REDIR: &str = "redir! > /dev/stdout | silent map | silent imap | redir END"`; `pub struct Vi;` (app/binary `"vi"`, `installed` = `resolve_bin("vi")` whose file name doesn't start with `vim`/`nvim`).

vim live layer: only when a user vimrc exists (`~/.vimrc`, else `~/.vim/vimrc`) — `vim -Es -u <vimrc> -c VIM_REDIR -c qa!` (ex silent mode skips initialisation unless `-u` names the vimrc). No vimrc → defaults only. Output lines: columns 0–2 the mode (`n`, `i`, blank = normal/visual/op-pending, others skipped), then lhs, then optional flags `*&@`, then rhs. Dropped: lhs with `<Plug>`/`<SNR>`, rhs `<Nop>` or starting `<Plug>`. Description: rhs `:Cmd<CR>` or `<Cmd>Cmd<CR>` → `Run :Cmd`; else `Keys: <rhs>`. Sections `Your mappings` (normal) / `Your insert mappings`. Defaults kept.

- [ ] **Step 1: Fixtures and defaults**

`tests/fixtures/vim/home/.vimrc`: `" test vimrc` (one line). `tests/fixtures/vim/map.txt` (two leading empty lines, as vim prints them):

```


n  <Plug>(fzf-normal) * <Nop>
n  <Space>e    * :NERDTreeToggle<CR>
   <C-P>       * :Files<CR>
n  gb          * <Cmd>bnext<CR>
v  <           * <gv
i  jk          * <Esc>
n  <Plug>(foo) & <SNR>12_x
No mapping found
```

`src/defaults/vim.txt` (vim notation, `parse_vim`):

```
Motion :: h :: Left
Motion :: j :: Down
Motion :: k :: Up
Motion :: l :: Right
Motion :: w :: Next word
Motion :: b :: Previous word
Motion :: e :: End of word
Motion :: 0 :: Start of line
Motion :: ^ :: First non-blank
Motion :: $ :: End of line
Motion :: gg :: Top of file
Motion :: G :: End of file
Motion :: % :: Matching bracket
Motion :: <C-d> :: Half page down
Motion :: <C-u> :: Half page up
Motion :: <C-o> :: Jump back
Motion :: <C-i> :: Jump forward
Motion :: zz :: Center cursor line
Editing :: i :: Insert
Editing :: a :: Append
Editing :: o :: Open line below
Editing :: O :: Open line above
Editing :: x :: Delete char
Editing :: dd :: Delete line
Editing :: yy :: Yank line
Editing :: p :: Paste after
Editing :: P :: Paste before
Editing :: u :: Undo
Editing :: <C-r> :: Redo
Editing :: . :: Repeat
Editing :: ciw :: Change word
Editing :: v :: Visual mode
Editing :: V :: Visual line mode
Editing :: <C-v> :: Visual block mode
Search :: / :: Search forward
Search :: ? :: Search backward
Search :: n :: Next match
Search :: N :: Previous match
Search :: * :: Search word under cursor
Windows :: <C-w>s :: Split
Windows :: <C-w>v :: Vertical split
Windows :: <C-w>w :: Next window
Windows :: <C-w>q :: Close window
Windows :: gt :: Next tab
Windows :: gT :: Previous tab
Files :: :w<CR> :: Save
Files :: :q<CR> :: Quit
Files :: ZZ :: Save and quit
```

`src/defaults/vi.txt`:

```
Motion :: h :: Left
Motion :: j :: Down
Motion :: k :: Up
Motion :: l :: Right
Motion :: w :: Next word
Motion :: b :: Previous word
Motion :: 0 :: Start of line
Motion :: $ :: End of line
Motion :: 1G :: Top of file
Motion :: G :: End of file
Motion :: H :: Top of screen
Motion :: L :: Bottom of screen
Motion :: <C-f> :: Page down
Motion :: <C-b> :: Page up
Motion :: % :: Matching bracket
Editing :: i :: Insert
Editing :: a :: Append
Editing :: A :: Append at end of line
Editing :: o :: Open line below
Editing :: O :: Open line above
Editing :: x :: Delete char
Editing :: dd :: Delete line
Editing :: dw :: Delete word
Editing :: cw :: Change word
Editing :: r :: Replace char
Editing :: J :: Join lines
Editing :: yy :: Yank line
Editing :: p :: Paste after
Editing :: u :: Undo
Editing :: . :: Repeat
Search :: / :: Search forward
Search :: ? :: Search backward
Search :: n :: Next match
Search :: f :: Find char on line
Ex commands :: :w<CR> :: Save
Ex commands :: :q<CR> :: Quit
Ex commands :: :q!<CR> :: Quit without saving
Ex commands :: ZZ :: Save and quit
Ex commands :: :%s/a/b/g<CR> :: Replace all
```

- [ ] **Step 2: Failing tests** — `src/sources/vim.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, title: &str, desc: &str) -> Vec<String> {
        l.sections.iter().filter(|s| s.title == title).flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn user_mappings_over_defaults() {
        let home = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vim/home");
        let out = std::fs::read_to_string(home.parent().unwrap().join("map.txt")).unwrap();
        let key = format!("vim -Es -u {} -c {VIM_REDIR} -c qa!", home.join(".vimrc").display());
        let l = Vim.load(&Env::test(&home, FakeRunner(HashMap::from([(key, out)]))));
        assert_eq!(l.origin, Origin::Mixed);
        let m = "VIM · YOUR MAPPINGS";
        assert_eq!(keys_of(&l, m, "Run :NERDTreeToggle"), vec!["Space › e"]);
        assert_eq!(keys_of(&l, m, "Run :Files"), vec!["Ctrl+P"]);
        assert_eq!(keys_of(&l, m, "Run :bnext"), vec!["g › b"]);
        assert_eq!(keys_of(&l, "VIM · YOUR INSERT MAPPINGS", "Keys: <Esc>"), vec!["j › k"]);
        assert!(!l.sections.iter().flat_map(|s| &s.rows).any(|r| r.desc.contains("<gv") || r.desc.contains("Nop") || r.desc.contains("SNR")));
        assert_eq!(keys_of(&l, "VIM · MOTION", "Top of file"), vec!["g › g"]);
    }

    #[test]
    fn no_vimrc_is_defaults_without_running_vim() {
        let l = Vim.load(&Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::new())));
        assert_eq!((l.origin, l.note.clone()), (Origin::Defaults, None));
        assert_eq!(keys_of(&l, "VIM · EDITING", "Redo"), vec!["Ctrl+R"]);
    }
}
```

`src/sources/vi.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;

    fn exe(path: &std::path::Path) {
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn env_with(dir: &std::path::Path) -> Env {
        let mut e = Env::test(dir, FakeRunner(HashMap::new()));
        e.vars.insert("PATH".into(), dir.display().to_string());
        e
    }

    #[test]
    fn vi_hidden_when_it_is_vim() {
        let dir = std::env::temp_dir().join("cst-vi-link");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("alt")).unwrap();
        exe(&dir.join("vim"));
        std::os::unix::fs::symlink(dir.join("vim"), dir.join("alt/vi")).unwrap();   // vi → alt/vi → vim
        std::os::unix::fs::symlink(dir.join("alt/vi"), dir.join("vi")).unwrap();
        assert!(!Vi.installed(&env_with(&dir)));
    }

    #[test]
    fn classic_vi_shown_with_defaults() {
        let dir = std::env::temp_dir().join("cst-vi-real");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        exe(&dir.join("vi"));
        let env = env_with(&dir);
        assert!(Vi.installed(&env));
        let l = Vi.load(&env);
        assert_eq!(l.origin, Origin::Defaults);
        let keys = l.sections.iter().filter(|s| s.title == "VI · EX COMMANDS").flat_map(|s| &s.rows)
            .find(|r| r.desc == "Quit without saving").map(|r| seq_text(&r.alts[0])).unwrap();
        assert_eq!(keys, ": › q › ! › Enter");
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib vim vi` (after registering both modules) — Expected: compile errors.

- [ ] **Step 4: Implement** `src/sources/vim.rs`:

```rust
//! vim: curated defaults plus the user's own mappings (`:map` / `:imap`
//! through `vim -Es`), labelled by what they run.
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/vim.txt");
pub const VIM_REDIR: &str = "redir! > /dev/stdout | silent map | silent imap | redir END";

pub struct Vim;

impl Source for Vim {
    fn app(&self) -> &'static str { "vim" }
    fn binary(&self) -> &'static str { "vim" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new());
        let vimrc = [env.home.join(".vimrc"), env.home.join(".vim/vimrc")].into_iter().find(|p| p.is_file());
        let live = match vimrc {
            None => Ok(None),
            Some(rc) => env.run("vim", &["-Es", "-u", &rc.display().to_string(), "-c", VIM_REDIR, "-c", "qa!"])
                .map(|out| Some(out.lines().filter_map(map_line).collect()))
                .ok_or_else(|| "vim -Es failed".to_string()),
        };
        layer(defaults, live, true).into_loaded("vim")
    }
}

fn map_line(line: &str) -> Option<Change> {
    let mode = line.get(..3)?.trim();
    let group = match mode { "" | "n" => "Your mappings", "i" => "Your insert mappings", _ => return None };
    let rest = line.get(3..)?.trim_start();
    let (lhs, rhs) = rest.split_once(char::is_whitespace)?;
    let rhs = rhs.trim_start().trim_start_matches(['*', '&', '@']).trim();
    if lhs.contains("<Plug>") || lhs.contains("<SNR>") || rhs == "<Nop>" || rhs.starts_with("<Plug>") || rhs.is_empty() {
        return None;
    }
    let cmd = rhs.strip_prefix("<Cmd>").or_else(|| rhs.strip_prefix(':')).and_then(|c| c.strip_suffix("<CR>"));
    let desc = match cmd { Some(c) => format!("Run :{c}"), None => format!("Keys: {rhs}") };
    Some(Change::Bind(Binding { scope: mode.to_string(), group: group.into(), seq: parse_vim(lhs), desc, id: String::new() }))
}
```

`src/sources/vi.rs`:

```rust
//! vi: classic vi (ex-vi, busybox, nvi) — a curated command reference. Hidden
//! when `vi` is really vim or nvim (their own sheets cover it).
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_vim;

const DEFAULTS: &str = include_str!("../defaults/vi.txt");

pub struct Vi;

impl Source for Vi {
    fn app(&self) -> &'static str { "vi" }
    fn binary(&self) -> &'static str { "vi" }
    fn installed(&self, env: &Env) -> bool {
        env.resolve_bin("vi").and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .is_some_and(|n| !(n.starts_with("vim") || n.starts_with("nvim")))
    }
    fn load(&self, _env: &Env) -> Loaded {
        layer(parse_defaults(DEFAULTS, &|k| parse_vim(k), &|_| String::new()), Ok(None), true).into_loaded("vi")
    }
}
```

`build` capitalises descriptions, so `Keys: <Esc>` stays as is and `Run :bnext` too.

- [ ] **Step 5: Register** `pub mod vi; pub mod vim;`; in `all()` insert `Box::new(vi::Vi), Box::new(vim::Vim),` before `nvim::Nvim` (nano lands before them in Task 5). Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: vim (defaults + your mappings) and classic vi sheets"`

---

### Task 5: nano source

**Files:**
- Create: `src/sources/nano.rs`, `src/defaults/nano.txt`, `tests/fixtures/nano/etc/nanorc`, `tests/fixtures/nano/home/.nanorc`, `tests/fixtures/nano/home/.config/nano/nanorc`
- Modify: `src/sources/mod.rs` (`pub mod nano;`, `all()` before vi)

**Interfaces:**
- Produces: `pub struct Nano;` (app/binary `"nano"`).

Files in order `env.etc("nanorc")`, `~/.nanorc`, `config("nano/nanorc")` (missing files skipped; none at all → defaults only). Lines (after trimming, `#` comments skipped): `bind KEY FUNCTION MENU`, `bind KEY "TEXT" MENU` (macro), `unbind KEY MENU`; only menus `main` and `all`. Functions via the table below (group, description), unknown → group `Other`, `humanize(function)`; macros → group `Macros`, `Macro: TEXT`. `unbind` → `Unbind`. Defaults kept.

- [ ] **Step 1: Fixtures and defaults**

`tests/fixtures/nano/etc/nanorc`:
```
## system nanorc: only examples
# bind ^S savefile main
# unbind ^K main
set tabsize 4
```

`tests/fixtures/nano/home/.nanorc`:
```
# my nanorc
bind ^S savefile main
bind ^Q exit all
unbind ^J main
bind M-1 "{cut}{paste}" main
bind ^E end browser
  bind   ^B   wherewas    all
```

`tests/fixtures/nano/home/.config/nano/nanorc`:
```
bind ^X cut main
```

`src/defaults/nano.txt` (nano notation, `parse_nano`):
```
File :: ^S :: Save
File :: ^O :: Write out (save as)
File :: ^R :: Insert file
File :: ^X :: Exit
Edit :: ^K :: Cut
Edit :: ^U :: Paste
Edit :: M-6 :: Copy
Edit :: M-A :: Set mark
Edit :: M-U :: Undo
Edit :: M-E :: Redo
Edit :: ^J :: Justify
Edit :: M-} :: Indent
Edit :: M-{ :: Unindent
Edit :: M-3 :: Toggle comment
Edit :: ^D :: Delete
Edit :: M-T :: Cut to end of file
Search :: ^W :: Search
Search :: ^Q :: Search backward
Search :: M-W :: Find next
Search :: M-Q :: Find previous
Search :: ^\ :: Replace
Navigation :: ^/ :: Go to line
Navigation :: ^A :: Start of line
Navigation :: ^E :: End of line
Navigation :: ^Y :: Page up
Navigation :: ^V :: Page down
Navigation :: M-\ :: First line
Navigation :: M-/ :: Last line
Navigation :: ^Left :: Previous word
Navigation :: ^Right :: Next word
Navigation :: M-] :: Matching bracket
Other :: ^G :: Help
Other :: ^C :: Show position
Other :: ^T :: Execute command
Other :: M-D :: Word count
Other :: ^Z :: Suspend
Other :: ^L :: Refresh
Other :: M-N :: Toggle line numbers
```

- [ ] **Step 2: Failing tests** — `src/sources/nano.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, desc: &str) -> Vec<String> {
        l.sections.iter().flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    #[test]
    fn nanorc_bind_unbind_macro() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nano");
        let mut env = Env::test(&root.join("home"), FakeRunner(HashMap::new()));
        env.vars.insert("CST_SYSCONFDIR".into(), root.join("etc").display().to_string());
        let l = Nano.load(&env);
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "Exit"), vec!["Ctrl+Q"]);                  // ^X rebound to cut, ^Q bound to exit
        assert_eq!(keys_of(&l, "Cut"), vec!["Ctrl+K", "Ctrl+X"]);
        assert!(keys_of(&l, "Justify").is_empty());                        // unbind ^J
        assert_eq!(keys_of(&l, "Macro: {cut}{paste}"), vec!["Alt+1"]);
        assert_eq!(keys_of(&l, "End of line"), vec!["Ctrl+E"]);            // the browser-menu bind is ignored
        assert_eq!(keys_of(&l, "Search backward"), vec!["Ctrl+B"]);        // odd spacing; default ^Q replaced
        assert_eq!(keys_of(&l, "Save"), vec!["Ctrl+S"]);
    }

    #[test]
    fn no_nanorc_is_defaults() {
        let mut env = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::new()));
        env.vars.insert("CST_SYSCONFDIR".into(), "/nonexistent".into());
        let l = Nano.load(&env);
        assert_eq!(l.origin, Origin::Defaults);
        assert_eq!(keys_of(&l, "Undo"), vec!["Alt+U"]);
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib nano` — Expected: compile errors.

- [ ] **Step 4: Implement**:

```rust
//! nano: bind/unbind lines of the nanorc files over nano's default shortcuts.
use super::{humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_nano;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/nano.txt");

const FUNCS: &[(&str, &str, &str)] = &[
    ("savefile", "File", "Save"), ("writeout", "File", "Write out (save as)"), ("insert", "File", "Insert file"),
    ("exit", "File", "Exit"), ("cut", "Edit", "Cut"), ("paste", "Edit", "Paste"), ("copy", "Edit", "Copy"),
    ("mark", "Edit", "Set mark"), ("undo", "Edit", "Undo"), ("redo", "Edit", "Redo"), ("justify", "Edit", "Justify"),
    ("indent", "Edit", "Indent"), ("unindent", "Edit", "Unindent"), ("comment", "Edit", "Toggle comment"),
    ("delete", "Edit", "Delete"), ("cutrestoffile", "Edit", "Cut to end of file"), ("zap", "Edit", "Delete without cut"),
    ("whereis", "Search", "Search"), ("wherewas", "Search", "Search backward"), ("findnext", "Search", "Find next"),
    ("findprevious", "Search", "Find previous"), ("replace", "Search", "Replace"),
    ("gotoline", "Navigation", "Go to line"), ("home", "Navigation", "Start of line"), ("end", "Navigation", "End of line"),
    ("pageup", "Navigation", "Page up"), ("pagedown", "Navigation", "Page down"), ("firstline", "Navigation", "First line"),
    ("lastline", "Navigation", "Last line"), ("prevword", "Navigation", "Previous word"), ("nextword", "Navigation", "Next word"),
    ("findbracket", "Navigation", "Matching bracket"), ("help", "Other", "Help"), ("location", "Other", "Show position"),
    ("execute", "Other", "Execute command"), ("wordcount", "Other", "Word count"), ("suspend", "Other", "Suspend"),
    ("refresh", "Other", "Refresh"), ("linenumbers", "Other", "Toggle line numbers"),
];

pub struct Nano;

impl Source for Nano {
    fn app(&self) -> &'static str { "nano" }
    fn binary(&self) -> &'static str { "nano" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_nano(k)], &|_| String::new());
        let texts: Vec<String> = [env.etc("nanorc"), env.home.join(".nanorc"), env.config("nano/nanorc")]
            .iter().filter_map(|p| env.read(p)).collect();
        let live = if texts.is_empty() { Ok(None) } else { Ok(Some(texts.iter().flat_map(|t| t.lines()).filter_map(rc_line).collect())) };
        layer(defaults, live, true).into_loaded("nano")
    }
}

fn rc_line(line: &str) -> Option<Change> {
    let line = line.trim();
    let mut w = line.split_whitespace();
    let cmd = w.next()?;
    let key = w.next()?;
    let seq = vec![parse_nano(key)];
    let menu_ok = |m: &str| m == "main" || m == "all";
    match cmd {
        "unbind" => w.next().filter(|m| menu_ok(m)).map(|_| Change::Unbind { scope: String::new(), seq }),
        "bind" => {
            let rest = line.split_once(key)?.1.trim();
            let (group, desc, menu) = if let Some(q) = rest.strip_prefix('"') {
                let end = q.rfind('"')?;
                ("Macros".to_string(), format!("Macro: {}", &q[..end]), q[end + 1..].trim())
            } else {
                let (func, menu) = rest.split_once(char::is_whitespace)?;
                let (g, d) = FUNCS.iter().find(|(f, _, _)| *f == func)
                    .map(|(_, g, d)| (g.to_string(), d.to_string()))
                    .unwrap_or_else(|| ("Other".into(), humanize(func)));
                (g, d, menu.trim())
            };
            menu_ok(menu).then(|| Change::Bind(Binding::new(&group, seq, &desc)))
        }
        _ => None,
    }
}
```

`line.split_once(key)` finds the key's first occurrence after `bind` (the key never equals `bind`).

- [ ] **Step 5: Register** `pub mod nano;`, `all()`: `Box::new(nano::Nano),` before `vi::Vi`. Run `cargo test -q --lib` — Expected: all pass.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: nano sheet from nanorc bind/unbind over defaults"`

---

### Task 6: micro and emacs sources

**Files:**
- Create: `src/sources/micro.rs`, `src/sources/emacs.rs`, `src/defaults/micro.txt`, `src/defaults/emacs.txt`, `tests/fixtures/micro/micro/bindings.json`
- Modify: `src/sources/mod.rs` (`pub mod emacs; pub mod micro;`, `all()` after nvchad)

**Interfaces:**
- Produces: `pub struct Micro;` (app/binary `"micro"`), `fn json_object(s: &str) -> Result<Vec<(String, Json)>, String>` with `enum Json { Str(String), Obj(Vec<(String, Json)>), Other }`; `pub struct Emacs;` (app/binary `"emacs"`).

micro: `config("micro/bindings.json")`; top-level string entries plus the string entries of a top-level `"buffer"` object. Value → first action of a `,`/`|`/`&` chain; `None` → `Unbind`; `lua:plugin.fn` → `humanize(fn)`; `command:…`/`command-edit:…` → `Command: …`; else `humanize(action)`. Group via `default_group`. Strict JSON (trailing commas, comments → error → note).

- [ ] **Step 1: Fixtures and defaults**

`tests/fixtures/micro/micro/bindings.json`:
```json
{
    "Ctrl-y": "Undo",
    "Alt-/": "lua:comment.comment",
    "CtrlShift-Up": "SelectUp",
    "Ctrl-q": "None",
    "Ctrl-r": "command:setlocal ruler off",
    "buffer": { "Alt-s": "SaveAs" },
    "terminal": { "Ctrl-q": "Unsplit" }
}
```

`src/defaults/micro.txt` (micro notation, `parse_micro`):
```
File :: Ctrl-s :: Save
File :: Ctrl-o :: Open file
File :: Ctrl-q :: Quit
Edit :: Ctrl-z :: Undo
Edit :: Ctrl-y :: Redo
Edit :: Ctrl-c :: Copy
Edit :: Ctrl-x :: Cut
Edit :: Ctrl-v :: Paste
Edit :: Ctrl-k :: Cut line
Edit :: Ctrl-d :: Duplicate line
Edit :: Ctrl-a :: Select all
Edit :: Alt-/ :: Comment
Edit :: Alt-Up :: Move lines up
Edit :: Alt-Down :: Move lines down
Search :: Ctrl-f :: Find
Search :: Ctrl-n :: Find next
Search :: Ctrl-p :: Find previous
Navigation :: Ctrl-l :: Jump to line
Navigation :: Ctrl-Home :: Start of buffer
Navigation :: Ctrl-End :: End of buffer
Navigation :: Home :: Start of line
Navigation :: End :: End of line
Tabs & splits :: Ctrl-t :: New tab
Tabs & splits :: Alt-, :: Previous tab
Tabs & splits :: Alt-. :: Next tab
Tabs & splits :: Ctrl-w :: Next split
Other :: Ctrl-e :: Command prompt
Other :: Ctrl-g :: Help
Other :: Ctrl-b :: Shell mode
Other :: Ctrl-u :: Toggle macro
Other :: Ctrl-j :: Play macro
```

`src/defaults/emacs.txt` (emacs notation, `parse_emacs`):
```
Files :: C-x C-f :: Open file
Files :: C-x C-s :: Save
Files :: C-x C-w :: Save as
Files :: C-x s :: Save all
Files :: C-x C-c :: Quit
Movement :: C-f :: Forward char
Movement :: C-b :: Backward char
Movement :: C-n :: Next line
Movement :: C-p :: Previous line
Movement :: M-f :: Forward word
Movement :: M-b :: Backward word
Movement :: C-a :: Start of line
Movement :: C-e :: End of line
Movement :: C-v :: Page down
Movement :: M-v :: Page up
Movement :: M-< :: Start of buffer
Movement :: M-> :: End of buffer
Movement :: M-g g :: Go to line
Movement :: C-l :: Recenter
Editing :: C-d :: Delete char
Editing :: M-d :: Kill word
Editing :: C-k :: Kill line
Editing :: C-SPC :: Set mark
Editing :: C-w :: Kill region (cut)
Editing :: M-w :: Copy region
Editing :: C-y :: Yank (paste)
Editing :: M-y :: Cycle yanks
Editing :: C-/ :: Undo
Editing :: C-t :: Transpose chars
Editing :: M-; :: Comment
Search :: C-s :: Search forward
Search :: C-r :: Search backward
Search :: M-% :: Query replace
Windows & buffers :: C-x b :: Switch buffer
Windows & buffers :: C-x C-b :: List buffers
Windows & buffers :: C-x k :: Kill buffer
Windows & buffers :: C-x 2 :: Split below
Windows & buffers :: C-x 3 :: Split right
Windows & buffers :: C-x o :: Other window
Windows & buffers :: C-x 1 :: Only this window
Windows & buffers :: C-x 0 :: Close window
Help :: C-g :: Cancel
Help :: M-x :: Run command
Help :: C-h k :: Describe key
Help :: C-h f :: Describe function
Help :: C-h t :: Tutorial
```

- [ ] **Step 2: Failing tests**

`src/sources/micro.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn keys_of(l: &Loaded, desc: &str) -> Vec<String> {
        l.sections.iter().flat_map(|s| &s.rows).find(|r| r.desc == desc)
            .map(|r| r.alts.iter().map(seq_text).collect()).unwrap_or_default()
    }

    fn env(cfg: &std::path::Path) -> Env {
        let mut e = Env::test(std::path::Path::new("/nonexistent"), FakeRunner(HashMap::new()));
        e.vars.insert("XDG_CONFIG_HOME".into(), cfg.display().to_string());
        e
    }

    #[test]
    fn json_reader() {
        let o = json_object(r#"{"a": "x\"y", "b": {"c": "d"}, "e": [1, {"f": 2}], "g": true}"#).unwrap();
        assert_eq!(o[0], ("a".into(), Json::Str("x\"y".into())));
        assert_eq!(o[1], ("b".into(), Json::Obj(vec![("c".into(), Json::Str("d".into()))])));
        assert_eq!((o[2].1.clone(), o[3].1.clone()), (Json::Other, Json::Other));
        assert!(json_object(r#"{"a": "b",}"#).is_err());
        assert!(json_object("{ // c\n }").is_err());
        assert!(json_object(r#"{"a": "#).is_err());
    }

    #[test]
    fn bindings_over_defaults() {
        let l = Micro.load(&env(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/micro")));
        assert_eq!(l.origin, Origin::Mixed);
        assert_eq!(keys_of(&l, "Undo"), vec!["Ctrl+Z", "Ctrl+Y"]);
        assert!(keys_of(&l, "Redo").is_empty());                            // its Ctrl-y now undoes
        assert_eq!(keys_of(&l, "Comment"), vec!["Alt+/"]);
        assert_eq!(keys_of(&l, "Select up"), vec!["Ctrl+Shift+↑"]);
        assert!(keys_of(&l, "Quit").is_empty());                            // Ctrl-q: None
        assert_eq!(keys_of(&l, "Command: setlocal ruler off"), vec!["Ctrl+R"]);
        assert_eq!(keys_of(&l, "Save as"), vec!["Alt+S"]);                  // from the buffer object
        assert!(keys_of(&l, "Unsplit").is_empty());                         // terminal mode ignored
    }

    #[test]
    fn micro_bad_json_notes() {
        let dir = std::env::temp_dir().join("cst-micro-bad");
        std::fs::create_dir_all(dir.join("micro")).unwrap();
        std::fs::write(dir.join("micro/bindings.json"), "{\"Ctrl-y\": \"Undo\",}").unwrap();
        let l = Micro.load(&env(&dir));
        assert_eq!(l.origin, Origin::Defaults);
        assert!(l.note.as_deref().unwrap().starts_with("config: bindings.json"));
    }
}
```

`src/sources/emacs.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::seq_text;
    use crate::sources::{FakeRunner, Origin};
    use std::collections::HashMap;

    #[test]
    fn defaults_only() {
        let l = Emacs.load(&Env::test(std::path::Path::new("/"), FakeRunner(HashMap::new())));
        assert_eq!(l.origin, Origin::Defaults);
        let row = l.sections.iter().filter(|s| s.title == "EMACS · FILES").flat_map(|s| &s.rows).find(|r| r.desc == "Open file").unwrap();
        assert_eq!(seq_text(&row.alts[0]), "Ctrl+X › Ctrl+F");
    }
}
```

- [ ] **Step 3: Run** `cargo test -q --lib micro emacs` — Expected: compile errors.

- [ ] **Step 4: Implement** `src/sources/micro.rs`:

```rust
//! micro: ~/.config/micro/bindings.json over micro's default keys.
use super::{default_group, humanize, layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_micro;
use crate::model::{Binding, Change};

const DEFAULTS: &str = include_str!("../defaults/micro.txt");

#[derive(Clone, Debug, PartialEq)]
pub enum Json { Str(String), Obj(Vec<(String, Json)>), Other }

struct P<'a> { s: &'a [u8], i: usize }

impl P<'_> {
    fn ws(&mut self) { while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) { self.i += 1; } }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.s.get(self.i) == Some(&c) { self.i += 1; Ok(()) } else { Err(format!("expected '{}' at byte {}", c as char, self.i)) }
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.s.get(self.i) {
                None => return Err("unterminated string".into()),
                Some(b'"') => { self.i += 1; break; }
                Some(b'\\') => {
                    let e = *self.s.get(self.i + 1).ok_or("unterminated string")?;
                    out.push(match e { b'n' => b'\n', b't' => b'\t', other => other });
                    self.i += 2;
                }
                Some(&c) => { out.push(c); self.i += 1; }
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.s.get(self.i) {
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'{') => Ok(Json::Obj(self.object()?)),
            Some(b'[') => { self.array()?; Ok(Json::Other) }
            Some(c) if c.is_ascii_alphanumeric() || *c == b'-' => {
                while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || b"-+.".contains(c)) { self.i += 1; }
                Ok(Json::Other)
            }
            _ => Err(format!("unexpected input at byte {}", self.i)),
        }
    }
    fn array(&mut self) -> Result<(), String> {
        self.eat(b'[')?;
        self.ws();
        if self.s.get(self.i) == Some(&b']') { self.i += 1; return Ok(()); }
        loop {
            self.value()?;
            self.ws();
            match self.s.get(self.i) { Some(b',') => self.i += 1, Some(b']') => { self.i += 1; return Ok(()); } _ => return Err("bad array".into()) }
        }
    }
    fn object(&mut self) -> Result<Vec<(String, Json)>, String> {
        self.eat(b'{')?;
        let mut out = Vec::new();
        self.ws();
        if self.s.get(self.i) == Some(&b'}') { self.i += 1; return Ok(out); }
        loop {
            self.ws();
            let k = self.string()?;
            self.eat(b':')?;
            out.push((k, self.value()?));
            self.ws();
            match self.s.get(self.i) { Some(b',') => self.i += 1, Some(b'}') => { self.i += 1; return Ok(out); } _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)) }
        }
    }
}

/// A JSON object (strict: no trailing commas or comments).
pub fn json_object(s: &str) -> Result<Vec<(String, Json)>, String> {
    let mut p = P { s: s.as_bytes(), i: 0 };
    let o = p.object()?;
    p.ws();
    if p.i != p.s.len() { return Err("trailing data".into()); }
    Ok(o)
}

pub struct Micro;

impl Source for Micro {
    fn app(&self) -> &'static str { "micro" }
    fn binary(&self) -> &'static str { "micro" }
    fn load(&self, env: &Env) -> Loaded {
        let defaults = parse_defaults(DEFAULTS, &|k| vec![parse_micro(k)], &|_| String::new());
        let path = env.config("micro/bindings.json");
        let live = if !path.is_file() { Ok(None) } else {
            env.read(&path).ok_or_else(|| "bindings.json: unreadable".to_string())
                .and_then(|t| json_object(&t).map_err(|e| format!("bindings.json: {e}")))
                .map(|o| Some(changes(&o, &defaults)))
        };
        layer(defaults, live, true).into_loaded("micro")
    }
}

fn changes(o: &[(String, Json)], defaults: &[Binding]) -> Vec<Change> {
    let buffer = o.iter().find(|(k, _)| k == "buffer").and_then(|(_, v)| if let Json::Obj(b) = v { Some(b.as_slice()) } else { None }).unwrap_or(&[]);
    o.iter().chain(buffer).filter_map(|(key, v)| {
        let Json::Str(action) = v else { return None };
        let seq = vec![parse_micro(key)];
        let first = action.split([',', '|', '&']).next().unwrap_or("").trim();
        if first == "None" { return Some(Change::Unbind { scope: String::new(), seq }); }
        let desc = if let Some(f) = first.strip_prefix("lua:") { humanize(f.rsplit('.').next().unwrap_or(f)) }
            else if let Some(c) = first.strip_prefix("command:").or_else(|| first.strip_prefix("command-edit:")) { format!("Command: {c}") }
            else { humanize(first) };
        Some(Change::Bind(Binding::new(&default_group(defaults, &desc), seq, &desc)))
    }).collect()
}
```

`src/sources/emacs.rs`:
```rust
//! emacs: a curated reference of the default keys (live keymaps need a running emacs).
use super::{layer, parse_defaults, Env, Loaded, Source};
use crate::keys::parse_emacs;

const DEFAULTS: &str = include_str!("../defaults/emacs.txt");

pub struct Emacs;

impl Source for Emacs {
    fn app(&self) -> &'static str { "emacs" }
    fn binary(&self) -> &'static str { "emacs" }
    fn load(&self, _env: &Env) -> Loaded {
        layer(parse_defaults(DEFAULTS, &|k| parse_emacs(k), &|_| String::new()), Ok(None), true).into_loaded("emacs")
    }
}
```

`humanize("SelectUp")` = "Select up", `humanize("comment")` = "Comment", `humanize("SaveAs")` = "Save as".

- [ ] **Step 5: Register** `pub mod emacs; pub mod micro;`; `all()`: after `nvchad::NvChad` add `Box::new(micro::Micro), Box::new(emacs::Emacs),`. Run `cargo test -q --lib` — Expected: all pass; clippy clean.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: micro (bindings.json) and emacs sheets"`

---

### Task 7: Order check, README, screenshots, release v0.3.0

**Files:**
- Modify: `README.md`, `docs/screenshot.svg`, `docs/picker.svg`

- [ ] **Step 1: Order and list** — `cargo build --release -q && env -u ZDOTDIR -u CLAUDE_CODE -u CLAUDECODE target/release/cst --list` — Expected 21 lines in the global app order; nano `defaults`, vi `defaults`, vim `defaults` (no vimrc), nvim `mixed`, nvchad `live` (≈46 bindings), micro and emacs `not installed`. `time` it: under 150 ms.

- [ ] **Step 2: README** — in **Supported apps** add after readline:

```markdown
| nano | `bind`/`unbind` in `/etc/nanorc`, `~/.nanorc`, `~/.config/nano/nanorc` over nano's defaults |
| vi | a classic-vi command reference (hidden when `vi` is really vim or nvim) |
| vim | vim's essential commands plus your own mappings (`:map` / `:imap`) |
```

replace the nvim row with:

```markdown
| nvim | mappings with a description (read after startup, so plugins' maps are included) over core motions |
| NvChad | NvChad's own mappings, sectioned like its cheatsheet (Telescope, Terminal, NvimTree…) |
| micro | `~/.config/micro/bindings.json` over micro's defaults |
| emacs | a reference of the default keys |
```

- [ ] **Step 3: Screenshots** (real environment) — `env -u ZDOTDIR -u CLAUDE_CODE -u CLAUDECODE -u BASH_ENV bash -c 'CST_SHOT_OUT=docs/picker.svg scripts/screenshot.sh && CST_SHOT_KEYS=Enter scripts/screenshot.sh'`. Check no private strings: `grep -o '>[^<]*<' docs/*.svg | grep -iE "/home|@|token|ssh" || echo clean`.

- [ ] **Step 4: Commit, merge, push, CI** — commit `docs: editors in README, new screenshots`; fast-forward `main`; `git push`; `gh run watch` the CI run — green.

- [ ] **Step 5: Release** — `scripts/release.sh 0.3.0`; watch the release run (build, pacman, aur all ✓); `gh release view v0.3.0 -R 13/cst --json assets -q '.assets | length'` → 5; `curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash && cst --version` → `cst 0.3.0`.
