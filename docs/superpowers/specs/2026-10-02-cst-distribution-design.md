# cst — distribution (repo, CI, releases, AUR, installer, README)

Date: 2026-10-02 · Sub-project A of the 2026-10-02 request (B: shells + picker)

## Goal

`cst` is public on GitHub as `13/cst`, every push is checked by CI, and
pushing a `vX.Y.Z` tag publishes a release (static x86_64 binary + pacman
package) and updates the AUR package `cst-bin`. People install with the AUR,
`pacman -U`, or a one-line `install.sh` (which also uninstalls). The README
is a polished landing page with a real screenshot.

Success: `scripts/release.sh 0.1.0` produces, with no manual step except the
one-time AUR key setup, a GitHub release `v0.1.0` with
`cst-x86_64-linux.tar.gz`, its `.sha256`, `cst-bin-0.1.0-1-x86_64.pkg.tar.zst`
and the PKGBUILD/`cst-bin.SRCINFO` (GitHub renames dot-files), and (if `AUR_SSH_KEY` is set) AUR `cst-bin`
at 0.1.0; `curl -fsSL …/install.sh | bash` installs a working `cst`, and
`… | bash -s -- --uninstall` removes it.

## Decisions (from the user)

- Public repo named `cst` under the logged-in account (`13`).
- x86_64 only.
- AUR package `cst-bin` (prebuilt binary), pushed by CI.
- One script for install and uninstall.
- New version published when a new tag is pushed.

Defaults chosen in the design (user approved): MIT licence; target
`x86_64-unknown-linux-musl` (static); no `cargo fmt` gate.

## Repository

- `gh repo create cst --public --source . --remote origin --push`
  (description: "Keyboard shortcut cheatsheet for your terminal apps, in the
  style of an awesome WM popup"; topics: cheatsheet, keybindings, tui, rust,
  awesomewm, tmux, zellij, kitty, wezterm).
- New files: `LICENSE` (MIT, holder "ben", 2026), `.github/workflows/ci.yml`,
  `.github/workflows/release.yml`, `packaging/aur/PKGBUILD.in`,
  `scripts/release.sh`, `scripts/screenshot.sh`, `scripts/ansi2svg.py`,
  `install.sh`, `docs/RELEASING.md`, `docs/screenshot.svg`, `README.md`.
- `Cargo.toml` gains `description`, `license = "MIT"`, `repository`,
  `readme`, `keywords`, `categories`.

## CI — `.github/workflows/ci.yml`

On push to any branch and on pull requests: one job on `ubuntu-latest`:
checkout, stable Rust (`dtolnay/rust-toolchain@stable` with clippy), cache
(`Swatinem/rust-cache@v2`), `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `shellcheck install.sh scripts/*.sh`.

## Release — `.github/workflows/release.yml`

Trigger: push of tag `v*`. Permissions: `contents: write`.

1. **build** (ubuntu-latest): fail unless `${GITHUB_REF_NAME#v}` equals the
   `Cargo.toml` version; `cargo test`; `rustup target add
   x86_64-unknown-linux-musl`, `sudo apt-get install -y musl-tools`,
   `cargo build --release --target x86_64-unknown-linux-musl`; stage
   `cst-x86_64-linux/{cst,README.md,LICENSE}` → `cst-x86_64-linux.tar.gz`
   and `cst-x86_64-linux.tar.gz.sha256` (`sha256sum` format); create the
   release with `gh release create "$GITHUB_REF_NAME" --generate-notes`
   uploading both files. Asset names carry no version so
   `releases/latest/download/<name>` always works.
2. **pacman** (needs build; container `archlinux:base-devel`): render
   `packaging/aur/PKGBUILD.in` → `PKGBUILD` (version, sha256 of the tarball
   downloaded from the release), run `makepkg` as a non-root user,
   `makepkg --printsrcinfo > .SRCINFO`, upload
   `cst-bin-<ver>-1-x86_64.pkg.tar.zst`, `PKGBUILD`, `.SRCINFO` to the release.
3. **aur** (needs pacman): if secret `AUR_SSH_KEY` is empty → `::warning::`
   and exit 0. Else write the key, `ssh-keyscan aur.archlinux.org`, clone
   `ssh://aur@aur.archlinux.org/cst-bin.git`, copy PKGBUILD/.SRCINFO, commit
   "cst-bin <ver>" as `cst release bot`, push.

### PKGBUILD (`packaging/aur/PKGBUILD.in`)

```
pkgname=cst-bin
pkgver=@VERSION@
pkgrel=1
pkgdesc="Keyboard shortcut cheatsheet for your terminal apps (awesome, tmux, zellij, kitty, …)"
arch=('x86_64')
url="https://github.com/13/cst"
license=('MIT')
provides=('cst')
conflicts=('cst')
source=("cst-x86_64-linux-$pkgver.tar.gz::$url/releases/download/v$pkgver/cst-x86_64-linux.tar.gz")
sha256sums=('@SHA256@')
package() {
  install -Dm755 cst-x86_64-linux/cst "$pkgdir/usr/bin/cst"
  install -Dm644 cst-x86_64-linux/LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 cst-x86_64-linux/README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
```

## Cutting a version — `scripts/release.sh X.Y.Z`

Refuses unless: argument is `X.Y.Z`, on `main`, tree clean, tag `vX.Y.Z`
absent, `cargo test` passes. Then sets `version` in `Cargo.toml`, runs
`cargo check` (updates `Cargo.lock`), commits "release vX.Y.Z", tags
annotated `vX.Y.Z`, `git push origin main vX.Y.Z`. `--dry-run` prints the
steps without changing anything.

## Installer — `install.sh`

`curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash`
(`bash -s -- <flags>` for flags). POSIX-ish bash, `set -euo pipefail`.

- Flags: `--version vX.Y.Z` (default latest), `--system` (install to
  `/usr/local/bin` via sudo; default `~/.local/bin`), `--uninstall`,
  `--help`.
- Refuses on non-Linux or non-x86_64 (`uname -s`/`uname -m`) with exit 1.
- On Arch (`/etc/arch-release`): prints a hint about `yay -S cst-bin` /
  the release `.pkg.tar.zst`, then continues.
- Needs `curl` (or `wget`), `tar`, `sha256sum`; checks for them first.
- Downloads tarball + `.sha256` to a temp dir (removed on exit), verifies,
  installs with `install -Dm755`, warns if the target dir is not on `PATH`,
  prints the installed version (`cst --version`).
- `--uninstall`: removes `~/.local/bin/cst` and/or `/usr/local/bin/cst`
  (sudo for the latter) if present; if `cst` is pacman-owned
  (`pacman -Qo`), tells the user to use pacman instead and leaves it.

## README

Sections: title + one-line pitch; badges (CI workflow, latest release,
AUR version, licence); screenshot (`docs/screenshot.svg`); Features;
Supported apps table (app · where keys come from · notes); Install (AUR,
pacman package from releases, install.sh, from source with cargo);
Usage (`cst`, `cst <app>`, `cst --list`, key table); Theming (follows the
active awesome theme, `NO_COLOR`, truecolor); How it works (live config
over bundled defaults, ⚠ notes, 1 s command timeout); Contributing
(tests, adding a source: one file + defaults + fixture); Releasing (link
to docs/RELEASING.md); Licence.

### Screenshot — `scripts/screenshot.sh`

Runs the release binary in a private tmux server (`tmux -L cstshot`,
`-x 140 -y 40`, `COLORTERM=truecolor`), waits, `capture-pane -e -p`, and
pipes it to `scripts/ansi2svg.py` (stdlib only: parses SGR 38/48;2 colours
and bold, emits one `<text>` per run on a monospace grid with the theme
background) → `docs/screenshot.svg`.

## docs/RELEASING.md

One-time: create an AUR account, add an SSH key, `git clone
ssh://aur@aur.archlinux.org/cst-bin.git` once to register the name, store the
private key as repo secret `AUR_SSH_KEY` (`gh secret set AUR_SSH_KEY <
key`). Each release: `scripts/release.sh X.Y.Z`; what the workflow does; how
to re-run a failed job.

## Testing

- CI itself on the first push (green run is the check).
- `shellcheck` clean for all scripts.
- `install.sh`: a `scripts/test-install.sh` that runs it against a local
  tarball (`CST_INSTALL_BASE_URL=file://…` override) into a temp `HOME`,
  checks `cst --version`, then `--uninstall` and checks removal; run in CI.
- `ansi2svg.py`: a doctest-style self-check (`python3 scripts/ansi2svg.py
  --selftest`) run in CI.
- Release pipeline: verified for real by tagging `v0.1.0` and checking the
  assets (and `pacman -U` of the built package on this machine).

## Out of scope

Other architectures/OSes, Homebrew/Nix/crates.io, signing, a changelog
generator beyond GitHub's generated notes.
