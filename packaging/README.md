# Packaging

Distribution is Arch/AUR first, as decided in `AGENTS.md`; the repository only
carries the recipes, the server never sees them.

| Directory | Package | Source |
|---|---|---|
| `nnano/` | `nnano` (stable) | the `v0.1.0` tag tarball, checksummed |
| `nnano-git/` | `nnano-git` | `main`, `pkgver()` from the commit count |

Both run the test suite in `check()`, strip `$srcdir` out of the binary with
`--remap-path-prefix`, and install the same things:

| Path | Content |
|---|---|
| `/usr/bin/nnano` | the binary (no `nano` shim, ever) |
| `/usr/share/licenses/nnano/LICENSE` | MIT |
| `/usr/share/doc/nnano/README.md`, `CHANGELOG.md` | project docs |
| `/usr/share/doc/nnano/shortcuts.<lang>.md` | the ten keymap references |

A man page is not shipped yet; `nnano --help` and `Ctrl+H` are the reference.

## Build locally

```sh
cd packaging/nnano        # or packaging/nnano-git
makepkg -si               # build, run the tests, install
makepkg --printsrcinfo > .SRCINFO
```

## Publish to the AUR

```sh
git clone ssh://aur@aur.archlinux.org/nnano.git
cp packaging/nnano/{PKGBUILD,.SRCINFO} nnano/
cd nnano && git add . && git commit -m "0.1.0" && git push
```

Same for `nnano-git`.

## The stable recipe, step by step

1. Tag and push `vX.Y.Z`, then create the GitHub release (the release notes are
   the matching `CHANGELOG.md` section).
2. Bump `pkgver` and `pkgrel` in `nnano/PKGBUILD`.
3. `updpkgsums` — it downloads the new tag tarball and rewrites `sha256sums`.
4. `makepkg --printsrcinfo > .SRCINFO`, commit, push.

The archive extracts to `neo-nano-<tag>/`, not `nnano-<pkgver>/`; `_srcname`
carries that difference so the functions can `cd` into it.
