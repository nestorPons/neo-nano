# Packaging

Distribution is Arch/AUR first, as decided in `AGENTS.md`; the repository only
carries the recipe, the server never sees it.

## `nnano-git`

Built from `main`, no checksum needed while there is no release tarball.

```sh
cd packaging/nnano-git
makepkg -si          # build, run the test suite, install
makepkg --printsrcinfo > .SRCINFO
```

Publishing to the AUR (after creating the AUR repository):

```sh
git clone ssh://aur@aur.archlinux.org/nnano-git.git
cp packaging/nnano-git/{PKGBUILD,.SRCINFO} nnano-git/
cd nnano-git && git add . && git commit -m "0.1.0" && git push
```

## Stable `nnano` (after the first tagged release)

The only differences from the `-git` recipe:

```diff
-pkgname=nnano-git
+pkgname=nnano
-pkgver=0.1.0
+pkgver=0.1.0
-source=("$pkgname::git+https://github.com/nestorPons/neo-nano.git")
-sha256sums=('SKIP')
+source=("$pkgname-$pkgver.tar.gz::https://github.com/nestorPons/neo-nano/archive/v$pkgver.tar.gz")
+sha256sums=('...')
```

Drop `git` from `makedepends`, keep `cargo`, and regenerate the checksum with
`updpkgsums` once `v0.1.0` exists on GitHub. `prepare()`/`build()`/`check()` and
`package()` stay as they are.

## What is installed

| Path | Content |
|---|---|
| `/usr/bin/nnano` | the binary (no `nano` shim, ever) |
| `/usr/share/licenses/nnano/LICENSE` | MIT |
| `/usr/share/doc/nnano/README.md`, `CHANGELOG.md` | project docs |
| `/usr/share/doc/nnano/shortcuts.<lang>.md` | the ten keymap references |

A man page is not shipped yet; `nnano --help` and `Ctrl+H` are the reference.
