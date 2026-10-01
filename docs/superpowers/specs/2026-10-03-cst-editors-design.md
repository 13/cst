# cst — editor sheets (nano, vi, vim, nvim fix, NvChad, micro, emacs)

Date: 2026-10-03 · Ships as v0.3.0.

## Goal

Sheets for the editors people use in a terminal: nano, vi, vim, neovim,
NvChad (its own app), micro and emacs — live bindings where the editor can
report them, curated defaults otherwise. Fix the nvim sheet missing every
mapping registered after startup (NvChad's included: 47 → 93 on the dev
machine).

## Decisions (from the user)

- Editors: nano, vi, vim, nvim, NvChad, plus micro and emacs.
- NvChad is its own app with sections from its mapping descriptions.
- vim/vi: curated defaults plus the user's own mappings (labelled by what
  they run).

## App order

awesome, kitty, wezterm, alacritty, tmux, zellij, zsh, bash, fish, nushell,
readline, nano, vi, vim, nvim, nvchad, micro, emacs, yazi, lazygit, fzf.

## Sources

### nano

- Shown when `nano` is on PATH.
- Live layer: `bind <key> <function> <menu>` / `unbind <key> <menu>` lines in
  `/etc/nanorc`, `~/.nanorc`, `$XDG_CONFIG_HOME/nano/nanorc` (in that order,
  later wins; `include` globs not followed). Only menus `main` and `all`
  are shown. Keys: `^X` → Ctrl+X, `M-X` → Alt+X, `Sh-M-X` → Alt+Shift+X,
  `F1`…, names (`Ins`, `Del`, `Home`, `PgUp`…). A `bind` to a quoted string
  is a macro (`Macro: …`). Functions → descriptions by table, else humanize.
- Defaults: nano's main-menu shortcuts (~40), grouped File, Edit, Search,
  Navigation, Other.

### vi

- Shown when `vi` is on PATH and does not resolve (symlinks followed) to a
  vim or nvim binary. On the dev machine `/usr/bin/vi` is ex-vi.
- No live layer: defaults only (~35), grouped Motion, Editing, Search,
  Ex commands.

### vim

- Shown when `vim` is on PATH.
- Live layer: `vim -Es -c 'redir! > /dev/stdout | silent map | silent imap |
  redir END' -c 'qa!'` (1 s timeout). Lines `<mode>  <lhs>  [*&@ ]<rhs>`;
  `<Plug>`/`<SNR>` lhs and `<Plug>` / `<Nop>` rhs dropped. Description:
  rhs `:Cmd<CR>` / `<Cmd>Cmd<CR>` → `Run :Cmd`; other rhs → `Keys: <rhs>`.
  Section `Your mappings` (normal) / `Your insert mappings`.
- Defaults (~45): Motion, Editing, Search, Windows, Files.

### nvim (changed)

- The dump runs inside `vim.schedule`, after the startup event-loop tick,
  and flushes stdout before `qa!`, so mappings registered by plugins and
  distributions after init are included (28 ms on the dev machine).
- When NvChad is detected, mappings whose description appears in NvChad's
  `lua/nvchad/mappings.lua` go to the nvchad sheet instead.

### nvchad (new)

- Shown when `$XDG_DATA_HOME/nvim/lazy/NvChad/lua/nvchad/mappings.lua`
  exists (data dir default `~/.local/share`).
- Source of truth for which mappings are NvChad's: the `desc = "…"`
  strings in that file. Live layer: the same nvim dump (normal, insert,
  visual modes), filtered to those descriptions.
- Sections from the description's first word when it is one of
  general, toggle, telescope, terminal, whichkey, nvimtree, buffer, lsp,
  comment, format, blankline, nvcheatsheet → that word capitalised
  (`telescope find files` → section Telescope, row `Find files`); insert-mode
  mappings without such a word → `Insert`; others → `Other`.
- Defaults: none (live only; failure → note).

### micro

- Shown when `micro` is on PATH (not installed on the dev machine: fixtures).
- Live layer: `$XDG_CONFIG_HOME/micro/bindings.json`, a JSON object of
  `"Key": "Action"` (or `"Action,Action"`); read with a small JSON-object
  reader (strings only; nested objects for other modes ignored). Keys
  `Ctrl-s`, `Alt-x`, `CtrlShift-Left`, `F1`…; actions humanized
  (`SaveAs` → "Save as"); `lua:…` → the function name; `None` unbinds.
- Defaults (~35): File, Edit, Search, Navigation, Tabs & splits.

### emacs

- Shown when `emacs` is on PATH (not installed on the dev machine).
- Defaults only (~45): Files, Movement, Editing, Search, Windows & buffers,
  Help. Notation `C-x C-f` (sequence of `C-`/`M-` combos).

## Shared

- `keys::parse_vim` reused by vi, vim, nvim, nvchad.
- `keys::parse_emacs("C-x C-f")` → sequence (also used by nano's `^X`/`M-X`
  via a small nano-specific mapper).
- `sources::group_by_prefix(desc, known) -> (group, rest)` for NvChad.

## Testing

- Fixtures: nanorc with bind/unbind/macro; a captured `vim -Es` map dump
  (plus synthetic user maps); an nvim dump captured from the real NvChad
  setup; NvChad's real `mappings.lua`; a micro `bindings.json`.
- vi detection: symlink to vim → hidden; ex-vi binary → shown (temp dirs on
  PATH).
- nvim/nvchad split: NvChad descriptions only in the nvchad sheet; a user
  mapping with its own description stays in nvim.
- `cst --list` on the dev machine: nano, vi, vim, nvim, nvchad live/mixed;
  micro, emacs not installed.

## Out of scope

LazyVim/AstroNvim/LunarVim detection, helix, kakoune, live emacs keymaps,
nano menus other than main.
