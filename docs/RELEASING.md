# Releasing cst

## One-time setup (AUR)

1. Create an account on https://aur.archlinux.org and add an SSH public key
   to it (My Account → SSH Public Key). Use a dedicated key:
   `ssh-keygen -t ed25519 -f ~/.ssh/aur -C cst-aur -N ''`.
2. Register the package name once: `git -c core.sshCommand='ssh -i ~/.ssh/aur'
   clone ssh://aur@aur.archlinux.org/cst-bin.git` (an empty repo is fine).
3. Store the private key as a repository secret:
   `gh secret set AUR_SSH_KEY < ~/.ssh/aur`.

Without the secret, releases still work; the `aur` job only warns.

## Every release

    scripts/release.sh X.Y.Z          # --dry-run to preview

The script refuses unless you are on a clean `main` with passing tests and
an unused tag. It bumps `Cargo.toml`, commits `release vX.Y.Z`, tags
`vX.Y.Z` and pushes. The tag starts `.github/workflows/release.yml`:

1. **build** — checks tag = Cargo version, tests, builds the static musl
   binary, creates the GitHub release with `cst-x86_64-linux.tar.gz` and its
   `.sha256`.
2. **pacman** — builds `cst-bin-X.Y.Z-1-x86_64.pkg.tar.zst` from
   `packaging/aur/PKGBUILD.in` in an Arch container and attaches it, the
   PKGBUILD and `.SRCINFO` to the release.
3. **aur** — pushes PKGBUILD and `.SRCINFO` to `cst-bin` on the AUR.

A failed job can be re-run from the Actions tab (`gh run rerun <id>
--failed`); jobs are safe to re-run except `build`'s `gh release create`
(delete the release first: `gh release delete vX.Y.Z --yes`).
