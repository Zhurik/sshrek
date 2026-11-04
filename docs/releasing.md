# Releasing

Releases are cut by [release-please](https://github.com/googleapis/release-please)
from the [conventional commits](../CONTRIBUTING.md#commit-messages) on `main`.
Nothing has to be bumped or tagged by hand.

## The release PR

Every push to `main` updates a single open pull request titled
`chore(main): release X.Y.Z`. It bumps `version` in `Cargo.toml` and
`Cargo.lock` and adds the new section to `CHANGELOG.md`. The next version
follows from the commits since the last tag:

| Commits since the last release | `0.x` today | after `1.0` |
|---|---|---|
| `fix:`, `perf:` (Renovate's `fix(deps):` included) | patch | patch |
| `feat:` | minor | minor |
| `!` or a `BREAKING CHANGE:` footer | minor | major |
| only `chore:`, `docs:`, `ci:`, … | no release PR | no release PR |

Merging that PR is the release. release-please tags the merge commit, publishes
a GitHub release with the changelog section as its notes, and then calls
`release.yml`, which builds four targets — Linux x86_64 and aarch64 against
musl, macOS aarch64 and x86_64 — and attaches a `.tar.gz` plus a `.sha256` for
each, and a `SHA256SUMS` file.

The release PR is opened with the workflow token, and GitHub runs no checks on
pull requests opened that way. That is fine: it only touches the version and
the changelog, and everything it releases has already passed CI on `main`.

`sshrek --version` prints the `version` from `Cargo.toml`, so the binaries of a
release always report the release they belong to.

## Homebrew

The last job of `release.yml` asks [Zhurik/homebrew-tap](https://github.com/Zhurik/homebrew-tap)
to re-render the formula for the new tag, after which `brew upgrade sshrek`
picks it up. Pre-releases are skipped.

It needs a repository secret, `TAP_DISPATCH_TOKEN`: a fine-grained token with
access to `Zhurik/homebrew-tap` only and the **Actions: read and write**
permission. Without it the release still succeeds, the job only leaves a
warning, and the formula stays on the previous version.

## By hand

Pushing a `v*` tag still runs `release.yml` on its own, for a pre-release
(`v0.3.0-rc1`, anything with a `-`) or when release-please is not an option.
The tag and the `version` in `Cargo.toml` must be identical, otherwise the
`verify` job fails before anything is built.

```sh
# bump the version in Cargo.toml, then refresh Cargo.lock (builds use --locked)
cargo build
git commit -am "chore: release v0.3.0-rc1"
git tag v0.3.0-rc1
git push && git push origin v0.3.0-rc1
```

Bump `.release-please-manifest.json` along with it when the version is a final
one, or release-please will propose it a second time.

## The Intel macOS build

The x86_64 macOS binary is cross-compiled on an arm64 runner, since GitHub
retired the x86_64 image. CI builds that target on every pull request for
exactly that reason: a link error should surface there and not at tag time.
