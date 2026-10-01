# cst — terminal shortcut sheet

Date: 2026-10-01

## Goal

`cst` is a full-screen terminal app that shows the keyboard shortcuts of
the installed apps (awesome, kitty, wezterm, alacritty, tmux, zellij, nvim,
yazi, lazygit, fzf) in the style of the awesome Mod+i sheet
(`~/.config/awesome/components/cheatsheet.lua`): sections in balanced
columns, key caps on the right, type-to-filter that dims non-matching rows.

Success: `cst` starts in well under 100 ms, shows the bindings actually in
effect (custom ones included) for every installed app, never crashes on a
broken or missing config, and looks like the awesome sheet in the active
awesome theme's colours.

## Decisions (from the user)

- Keys come from the live configs, merged over bundled defaults.
- Full-screen TUI (no print mode).
- Fast, safe, minimal dependencies → Rust, `crossterm`, `toml`,
  `unicode-width`; no TUI framework. The binary has no runtime dependencies.
- All ten apps in v1.

## Architecture

```
src/
  main.rs            args (`cst [app]`, `cst --list`, `--help`), wiring
  keys.rs            key/modifier normalisation and friendly names
  model.rs           Section/Row, merge, filter, column balancing
  theme.rs           awesome theme → colours
  sources/mod.rs     Source trait, detection, parallel load with timeout
  sources/<app>.rs   one per app: live parser + defaults
  defaults/<app>.toml bundled defaults (include_str!)
  ui/layout.rs       sections → positioned lines for a given width
  ui/render.rs       lines → terminal (crossterm), or a text buffer in tests
  ui/input.rs        key event → state change
tests/fixtures/<app>/ sample configs and command outputs
```

Each unit is pure except `sources/*` (files, commands) and `ui/render.rs`
(terminal). Sources return data; nothing else touches the filesystem.

## Model

```rust
struct Combo(Vec<String>);            // keys pressed together: ["Ctrl","Shift","T"]
struct Binding { seq: Vec<Combo>, description: String }   // seq: prefix sequences, e.g. tmux C-a then %
struct Row { description: String, alts: Vec<Vec<Combo>> } // alternatives
struct Section { app: String, title: String, note: Option<String>, rows: Vec<Row> }
```

- Section title: `APP · GROUP` (e.g. `ZELLIJ · PANE`, `TMUX · PREFIX`,
  `AWESOME · TAGS`); apps without groups use just `APP`.
- App order: awesome, kitty, wezterm, alacritty, tmux, zellij, nvim, yazi,
  lazygit, fzf. Within an app, group order as the source declares it.
- Rows with the same description (case-insensitive) in one section merge;
  their sequences become alternatives. Descriptions get a capital first
  letter. Rows sorted by description.
- `filter(sections, query) -> set of (section, row)`: case-insensitive
  substring over app, section title, description and key names. Empty query
  matches everything.
- `columns(sections, n) -> Vec<Vec<&Section>>`: same greedy balancing by
  row count (+1 per title) as `lib/cheatsheet.lua`, keeping order.

## Keys (`keys.rs`)

- Modifiers normalised and ordered Super, Ctrl, Alt, Shift. Accepted
  spellings: `Mod4/super/cmd/D-` → Super, `Control/ctrl/C-/<C-…>` → Ctrl,
  `Mod1/alt/M-/<M-…>/<A-…>` → Alt, `shift/S-` → Shift.
- Friendly names: `Return/enter/CR` Enter, `space` Space, `Escape/esc` Esc,
  `Left/Right/Up/Down` ←/→/↑/↓, `Tab`, `BackSpace/BS` Bksp, `Page_Up/
  PageUp` PgUp, `Page_Down` PgDn, `comma` `,`, `period` `.`, `less` `<`,
  `asciicircum` `^`, `XF86Audio*`/`XF86MonBrightness*` as in the awesome
  sheet, `#10…#18` 1…9. A single letter is upper-cased when the combo has a
  modifier (`Ctrl+T`) and keeps its case otherwise (vim/yazi `g` ≠ `G`).
- Display: combo keys joined by muted `+`, sequence steps by muted `›`,
  alternatives by muted `/`.

## Sources and merge rules

`trait Source { fn app(&self) -> &str; fn installed(&self) -> bool;
fn load(&self) -> Loaded }` where `Loaded { sections, origin: Live|Defaults|
Mixed, note: Option<String> }`.

- Installed = binary on `PATH` (awesome: `awesome`).
- Layers: bundled defaults (curated, ~15–40 useful bindings per app,
  grouped), then the live layer. A live binding replaces any default on the
  same sequence; an unbind removes it. Sources marked "no defaults" below use
  only the live layer when it succeeds.
- Actions without a description get a readable name from a small per-source
  table (`new_tab` → "New tab", `split-window -h` → "Split right"); unknown
  actions show the raw action text.
- Any parse or command failure: fall back to defaults and set `note =
  "config: <reason>"`, shown muted under the app's first title with ⚠.
- Commands run with stdin closed, a 1 s timeout, all sources loaded in
  parallel threads.

| App | Live layer | Defaults merged |
|---|---|---|
| awesome | `awesome-client` script that serialises `awful.key.hotkeys` (mods, key, description, group) one per line; groups mapped to the Mod+i sections (Windows, Tags, Layout, Apps, Media & volume, System, Other). Fallback when awesome is not running: scan `~/.config/awesome/keys.lua` for `key({mods}, "k", …, { description = …, group = … })`. Entries without description skipped. | no |
| kitty | `map` / `unmap` lines in `~/.config/kitty/kitty.conf` and `include`d files; `kitty_mod` expanded (default `ctrl+shift`); `clear_all_shortcuts yes` drops defaults; `>` sequences supported. | yes, unless cleared |
| wezterm | `wezterm show-keys` output (key table sections become groups). | no |
| alacritty | `[[keyboard.bindings]]` in `alacritty.toml` and `general.import` files; `action = "None"`/`"ReceiveChar"` = unbind. | yes |
| tmux | If a server is running: `tmux list-keys` (tables → groups, prefix resolved from `show -g prefix`, `-N` notes used as descriptions). Else parse `~/.tmux.conf` (following symlink and `source-file`) for `bind`/`unbind`/`set -g prefix`. | only when parsing the file |
| zellij | `keybinds` node of `~/.config/zellij/config.kdl`: modes → groups, `bind "…" { Action …; }`, `unbind`, `shared_except/shared_among`; `clear-defaults=true` on `keybinds` or a mode drops those defaults. Minimal KDL reader for this subset. | yes, unless cleared |
| nvim | `nvim --headless -u <user init> +'lua …' +q` printing normal-mode mappings that have a `desc` (leader expanded). | yes (core motions) |
| yazi | `~/.config/yazi/keymap.toml` `[mgr]` (or `[manager]`) `keymap`/`prepend_keymap`/`append_keymap`; `desc` used. | yes |
| lazygit | `keybinding:` in `~/.config/lazygit/config.yml` (flat `section.action: key` subset). | yes |
| fzf | `--bind` / `--bind=` in `$FZF_DEFAULT_OPTS` and the file in `$FZF_DEFAULT_OPTS_FILE`. | yes |

Known config facts on this machine: zellij uses `clear-defaults=true`;
tmux is oh-my-tmux (`~/.tmux.conf` → `~/.tmux/.tmux.conf`); kitty has no
`map` lines; no yazi keymap or lazygit config exist (defaults only).

## Theme (`theme.rs`)

- Name: first line of `~/.cache/awesome/theme` if it names an existing
  `~/.config/awesome/themes/<name>/theme.lua`; otherwise
  `config.default_theme = "<name>"` from `~/.config/awesome/config.lua`;
  otherwise built-in catppuccin.
- Resolve assignments `theme.<path> = "#rrggbb"` or `= theme.<path>` from
  `theme.lua` (comments ignored, references followed up to 8 steps).
- Colours: bg = `infocard_bg` ‖ `bg_normal`; fg = `infocard_fg` ‖
  `fg_normal`; muted = `infocard_fg_muted` ‖ fg blended 60 % over bg (as
  `components/infocard/render.lua` does with alpha `99`); accent =
  `infocard_accent` ‖ `menu_accent` ‖ `colors.base0D`; track (key cap
  background) = `infocard_border_color` ‖ `colors.base03`. Any colour still
  missing comes from built-in catppuccin.
- Truecolor when `COLORTERM` is `truecolor`/`24bit`, else nearest xterm-256.
  `NO_COLOR` set → no colours, key caps shown as `[Ctrl]`.

## UI

Alternate screen, raw mode, hidden cursor; restored on exit, on panic (hook)
and on Ctrl+C.

```
 Keyboard shortcuts                                   10 apps · catppuccin
 Type to filter…
 
 AWESOME · WINDOWS                  KITTY · TABS                  ZELLIJ · PANE
 Close            Super+Shift+C     New tab        Ctrl+Shift+T   New pane     Alt+N
 …
```

- Whole screen filled with bg. Header: bold "Keyboard shortcuts", right:
  `<n> apps · <theme>` muted (or `focus: <app>`). Filter line: muted
  placeholder, or the query in fg with `▏`; `No matches` muted when empty.
- Columns: n = clamp(width / 44, 1, 4); gap 3; column width = (width − 2 −
  gaps) / n. Row = description (left, truncated with `…`) and the key caps
  right-aligned; if the keys alone exceed the column, the alternatives after
  the first are dropped and `…` added.
- Key cap: ` Name ` with track background and fg text.
- Section title: accent, bold, uppercase; one blank line between sections.
  `⚠ note` muted under the title when set.
- Filter dims (muted colour, caps without background) non-matching rows and
  titles of sections with no match; layout does not change while typing.
- Sheet taller than the screen: vertical scroll; muted `↓ more` / `↑ more`
  in the bottom/top right when content is hidden.
- Redraw only on input or resize; whole-frame diff against the previous
  frame to avoid flicker.

Input:

| Key | Effect |
|---|---|
| printable | append to filter |
| Backspace | delete last char |
| Ctrl+U | clear filter |
| Esc | clear filter; if already empty, quit |
| Ctrl+C, Ctrl+D | quit |
| Tab / Shift+Tab | focus next/previous app (only its sections laid out); past the last → all apps |
| ↑ ↓ PgUp PgDn Home End | scroll |
| resize | reflow |

CLI: `cst` (all installed apps), `cst <app>` (start focused; unknown or
not installed → message listing installed apps, exit 2), `cst --list`
(one line per app: installed?, origin live/defaults/mixed, note),
`cst --help`, `cst --version`.

## Error handling

- No app installed / no sections: the sheet shows "No supported apps found".
- Terminal smaller than 20×5: show "Terminal too small".
- Source panics are caught per thread and turned into a note.

## Testing

- `keys`: normalisation and friendly names table-driven.
- `model`: merge (override, unbind, duplicate description), filter, columns.
- Each source: fixture inputs in `tests/fixtures/<app>/` (configs and
  captured command outputs) → expected sections; commands abstracted
  behind a `Runner` so tests feed captured output. Includes the real zellij
  `clear-defaults` file and oh-my-tmux config.
- `theme`: the real catppuccin `theme.lua` fixture resolves to
  bg `#313244`, fg `#cdd6f4`, muted `#8f94ae`, accent `#89b4fa`,
  track `#45475a`.
- UI: `layout` + `render` into a text buffer at 120×40 and 60×20, compared
  with snapshot files; input sequences (type "split", Backspace, Tab, Esc)
  checked against state.
- Manual: run `cst` in kitty, wezterm and alacritty; check startup time with
  `hyperfine 'cst --list'` (< 100 ms with awesome and tmux running).

## Install

`cargo build --release`, copy `target/release/cst` to `~/.local/bin/`.
Build needs a Rust toolchain (`pacman -S rustup && rustup default stable`).

## Out of scope

Editing or running bindings, print/pipe mode, mouse, apps beyond the ten
listed, per-user extra sheets.
