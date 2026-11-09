# Packaging

Distribution is Arch/AUR first, as decided in `AGENTS.md`; the repository only
carries the recipes, the server never sees them.

| Directory | Package | Source |
|---|---|---|
| `nnano/` | `nnano` (stable) | the `v0.1.1` tag tarball, checksummed |
| `nnano-git/` | `nnano-git` | `main`, `pkgver()` from the commit count |

Both run the test suite in `check()`, strip `$srcdir` out of the binary with
`--remap-path-prefix`, and install the same things:

| Path | Content |
|---|---|
| `/usr/bin/nnano` | the binary (no `nano` shim, ever) |
| `/usr/share/man/man1/nnano.1.gz` | the man page |
| `/usr/share/bash-completion/completions/nnano` | bash completion |
| `/usr/share/zsh/site-functions/_nnano` | zsh completion |
| `/usr/share/fish/vendor_completions.d/nnano.fish` | fish completion |
| `/usr/share/licenses/nnano/LICENSE` | MIT |
| `/usr/share/doc/nnano/README.md`, `CHANGELOG.md` | project docs |
| `/usr/share/doc/nnano/shortcuts.<lang>.md` | the ten keymap references |

## Build locally

```sh
cd packaging/nnano        # or packaging/nnano-git
makepkg -si               # build, run the tests, install (never as root)
makepkg --printsrcinfo > .SRCINFO
```

`src/` and `pkg/` are gitignored. If crates.io is slow, warm `src/cargo` by
copying `~/.cargo/registry/{cache,index}` and build with
`CARGO_NET_OFFLINE=true`.

Optional lint, before pushing:

```sh
namcap PKGBUILD
namcap nnano-0.1.1-1-x86_64.pkg.tar.xz
```

## Publish to the AUR

`yay` uploads nothing: it is a client that reads the AUR. Publishing means
pushing a git repository to `aur.archlinux.org`, and `yay -S nnano` works the
moment that push lands. There is no review queue.

### One-time setup

1. Account: <https://aur.archlinux.org/register/> — the AUR account is separate
   from Arch's GitLab/BBS login, even if the name and e-mail are the same.
2. Register a public key at <https://aur.archlinux.org/account/> ("SSH Public
   Key"). The one on this machine is `~/.ssh/id_ed25519.pub`; paste the whole
   line.
3. Check it:

   ```sh
   ssh aur@aur.archlinux.org        # "Hi <user>! You've successfully authenticated..."
   ```

   `Permission denied (publickey)` means step 2 has not propagated yet.

### First push

Both `nnano` and `nnano-git` were free on the AUR when this was written, so
cloning them creates nothing yet and prints
`warning: You appear to have cloned an empty repository` — the repository is
created by the first push.

```sh
git clone ssh://aur@aur.archlinux.org/nnano.git
cd nnano
cp ~/Proyectos/neo-nano/packaging/nnano/{PKGBUILD,.SRCINFO} .
git add PKGBUILD .SRCINFO
git commit -m "Initial import: nnano 0.1.1-1"
git push
```

Same flow for `nnano-git`, with its own message (`Initial import: nnano-git`).

Only `PKGBUILD`, `.SRCINFO`, an optional `.install` file and patches belong in
that repository — never a binary, a tarball, `src/` or `pkg/`.

### Every release

1. Tag and push `vX.Y.Z`, then create the GitHub release (its notes are the
   matching `CHANGELOG.md` section).
2. Bump `pkgver`/`pkgrel` in `nnano/PKGBUILD`, then `updpkgsums` (it downloads
   the new tag tarball and rewrites `sha256sums`).
3. `makepkg --printsrcinfo > .SRCINFO`, then `git add PKGBUILD .SRCINFO` — the
   two files go in the same commit. The AUR does not regenerate `.SRCINFO`; the
   web page and every helper read it, so a stale one is a broken package.
4. Commit as `<pkgname> <pkgver>-<pkgrel>` (e.g. `nnano 0.1.2-1`) and push.

`nnano-git` needs its `pkgver` refreshed the same way before each push, because
`makepkg --printsrcinfo` prints the value written in the PKGBUILD and does not
run `pkgver()`.

### Verify what the AUR serves

```sh
curl -s "https://aur.archlinux.org/rpc/v5/info?arg[]=nnano" | jq .
yay -Ss nnano
yay -S nnano
```

### Prove it in a clean chroot (recommended before a first push)

```sh
extra-x86_64-build        # devtools; from the package directory
```

The archive extracts to `neo-nano-<tag>/`, not `nnano-<pkgver>/`; `_srcname`
carries that difference so the functions can `cd` into it.
