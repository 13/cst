# cst — shell cheatsheets and app picker

Date: 2026-10-02 · Sub-project B (after A: distribution). Ships as v0.2.0.

## Goal

1. Sheets for zsh, bash, fish, nushell and readline (inputrc), read from
   the live shell where possible, over bundled defaults — same rules as
   the existing sources.
2. Plain `cst` opens an app picker; Enter opens the chosen app's sheet
   (or all apps); Esc goes back.

## Decisions (from the user)

- Picker on plain `cst`; `cst <app>` still opens that app directly.
- Shells: zsh, bash, fish, readline (inputrc), nushell.

## Sources

App order becomes: awesome, kitty, wezterm, alacritty, tmux, zellij, zsh,
bash, fish, nushell, readline, nvim, yazi, lazygit, fzf.

`Source` gains `fn installed(&self, env: &Env) -> bool { env.which(self.binary()) }`;
`load_installed` uses it.

### Common key notation (`keys::parse_term`)

Shell key strings are terminal byte sequences. `parse_term(s, style)` →
`Seq`, `style` ∈ {Zsh (`^X`, `^[`, `^?`), Readline (`\C-x`, `\M-x`, `\e`,
`\\`, `\"`, `C-x`, `Control-x`, `Meta-x`, `M-x` names), Fish3 (`\cx`, `\e`)}:

- Control: `^X`/`\C-x`/`\cx`/`C-x`/`Control-x` → `Ctrl+X`; `^?` → `Bksp`;
  `^I` → `Tab`, `^M`/`^J` → `Enter`, `^[` alone → `Esc`.
- Escape-prefixed: known CSI/SS3 sequences → names: `[A`/`OA` ↑, `[B`/`OB` ↓,
  `[C`/`OC` →, `[D`/`OD` ←, `[H`/`OH`/`[1~` Home, `[F`/`OF`/`[4~` End,
  `[2~` Ins, `[3~` Del, `[5~` PgUp, `[6~` PgDn, `[Z` ⇧Tab; `[1;5X` → Ctrl+arrow,
  `[1;3X` → Alt+arrow, `[1;2X` → Shift+arrow, `[3;5~` Ctrl+Del. Any other
  ESC + char → `Alt+<char>`; an unrecognised sequence is shown verbatim.
- Multi-key bytes become a sequence (`^X^E` → `Ctrl+X › Ctrl+E`).

### zsh

- Live: `zsh -ic 'bindkey -lL main; print -- ---; bindkey -M main; print -- ---; bindkey -M vicmd'`.
- `bindkey -lL main` → `bindkey -A viins main` (vi) or `… emacs main`.
- Lines `"<keys>" <widget>` and ranges `"^A"-"^C" <widget>` (ranges skipped
  unless the widget is meaningful — ranges are almost always self-insert).
- Sections: `Insert` (main keymap; title `ZSH · INSERT` in vi mode,
  `ZSH · EMACS` otherwise); `Normal` (vicmd) only in vi mode.
- Dropped widgets: `self-insert`, `self-insert-unmeta`, `undefined-key`,
  `digit-argument`, `neg-argument`, `vi-digit-or-beginning-of-line` is kept.
- Description: table of ~40 friendly names (`accept-line` → "Run command",
  `history-incremental-search-backward` → "Search history backward",
  `edit-command-line` → "Edit command in $EDITOR", …), else `humanize`.
- Defaults (fallback only): zsh emacs keymap essentials (~25).

### bash

- Live: `bash -ic 'bind -p; echo ---; bind -v | grep editing-mode'`.
- Lines `"<keys>": <function>`; `# … (not bound)` lines skipped; same drop list
  (`self-insert`, `do-lowercase-version`, `digit-argument`, …).
- Section `BASH · EMACS` or `BASH · VI` from `editing-mode`.
- Friendly-name table shared with readline (readline function names).
- Defaults: readline emacs essentials (~30).

### fish

- Live: `fish -c bind` (all modes; `bind --preset` included).
- Lines `bind [--preset] [-M mode] [-m newmode] <key> <command…>`;
  key in fish 4 names (`ctrl-x`, `alt-left`, `ctrl-x,ctrl-e` sequence) or
  fish 3 escapes (`\cx`, `\e\[A`, `-k up` terminfo names).
- Sections by mode: `Default`, `Insert`, `Visual`, others by name.
- Description: command (first word if a fish function name like
  `history-search-backward`) via table/humanize; multi-command → first.
- Defaults: fish default emacs-style essentials (~20). Not installed on the
  dev machine: tested only through fixtures.

### nushell

- Live: `nu -c '{d: (keybindings default), u: $env.config.keybindings} | to json -r'`.
- `d` rows: `{mode, modifier, code, event}`; `u` rows:
  `{name, modifier, keycode, mode, event}` (mode may be a list); user rows
  override defaults on (mode, modifier, key).
- Keys: modifier `control`/`alt`/`shift`/`control_alt`…, keycode
  `char_a`/`up`/`enter`/`f1`… → combo.
- Event → description: `{send: X}` → humanize(X); `{edit: X}` → humanize(X);
  `{until: [..]}` → first; `{send: menu, name: N}` → humanize(N);
  `null` → unbind.
- Sections by mode: `Emacs`, `Vi insert`, `Vi normal`.
- Defaults: nushell emacs essentials (~20). Not installed on the dev machine.

### readline (inputrc)

- `installed`: `$INPUTRC` or `~/.inputrc` exists (no binary). Not shown on
  the dev machine (no inputrc).
- Parse the file over bundled readline emacs defaults: `"keys": function`,
  `Name: function` (`Control-x`, `Meta-x`, `C-x`, `M-DEL`, `TAB`, `RET`…),
  `"keys": "macro"` → "Macro: …", `set editing-mode vi` → section
  `READLINE · VI`, `$include path` followed (depth ≤ 8),
  `$if mode=emacs|vi` honoured against the editing mode, `$if term=…` taken,
  `$if <application>` blocks skipped, `$else`/`$endif` tracked.
- Comments `#`; CRLF handled by `Env::read`.

## App picker

### Behaviour

- `cst` (no argument): opens the picker. `cst <app>`: opens that sheet
  directly (as today; Esc with empty filter quits there).
- Picker screen: header as in the sheet (right side `<n> apps · <theme>`),
  filter line placeholder `Choose an app…`, then entries: first
  `All apps` (total bindings), then each installed app with origin
  (`live`/`defaults`/`mixed`) and binding count, and `⚠` when it has a
  note. Laid out in columns (same column count rule as the sheet, width
  ≥ 30 per entry), row-major order.
- Selected entry: `▸` marker + accent colour + bold.
- Keys: printable → filter (case-insensitive substring of the app name;
  `All apps` always kept first); Backspace, Ctrl+U as in the sheet;
  ↑/↓/←/→ and Tab/Shift+Tab move the selection (wrapping), Home/End;
  Enter opens the sheet (`focus = Some(i)` or `None` for All apps, query
  cleared, scroll 0); Esc clears the filter, else quits; Ctrl+C/Ctrl+D quit.
  Filter with no matches: `No matches`, Enter does nothing.
- In a sheet opened from the picker: Esc with an empty filter returns to
  the picker (selection kept); Tab/Shift+Tab still cycle apps.

### Code

- `ui::input::State` gains `screen: Screen` (`Picker` | `Sheet`),
  `from_picker: bool`, `picked: usize`, `pick_query: String`.
  `apply` dispatches on the screen; the sheet keeps its current behaviour
  except the Esc rule above.
- `ui::layout::picker_frame(view, entries, st, w, h) -> Frame` with
  `struct Entry { name, origin, count, warn }` built in `main.rs` from the
  loaded sources; header/filter line drawing shared with `frame`.
- `ui::run` draws `picker_frame` or `frame` by screen and passes the entry
  list for picker navigation (`apply` needs the visible entry count).

## Testing

- `keys::parse_term`: table-driven for all three styles.
- Each shell source: fixtures (real captured output for zsh and bash from
  this machine; fish 3, fish 4 and nushell JSON samples written from their
  documented formats; an inputrc with `$if`/`$include`/macros) → expected
  sections; failure → defaults + note.
- Picker: snapshot at 120×30 and 60×20; input tests (filter, wrap,
  Enter → sheet with focus, Esc back, Esc quit); `All apps` stays first.
- `cst --list` lists 15 apps; zsh and bash `live` on this machine.

## Out of scope

Per-plugin zsh sheets (oh-my-zsh plugin aliases), csh/tcsh/xonsh, mouse
in the picker.
