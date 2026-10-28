# Releasing

A new version reaches [Releases](https://github.com/Zhurik/sshrek/releases)
only when a matching tag is pushed — nothing is published from `main` on its
own. The tag and the `version` in `Cargo.toml` must be identical, otherwise the
`verify` job fails the release before anything is built.

```sh
# 1. bump the version in Cargo.toml, e.g. 0.2.1 -> 0.3.0
# 2. refresh Cargo.lock (the release build runs with --locked)
cargo build
# 3. commit both files
git commit -am "chore: release v0.3.0"
# 4. tag with the very same version, prefixed with v
git tag v0.3.0
# 5. push the branch and the tag
git push && git push origin v0.3.0
```

The tag is what triggers the workflow, so pushing the commit alone does
nothing. It then builds four targets — Linux x86_64 and aarch64 against musl,
macOS aarch64 and x86_64 — uploads a `.tar.gz` plus a `.sha256` for each, and
publishes a release with generated notes and a `SHA256SUMS` file.

A tag containing a `-` (`v0.3.0-rc1`) is published as a pre-release.

The Intel macOS binary is cross-compiled on an arm64 runner, since GitHub
retired the x86_64 image. CI builds that target on every pull request for
exactly that reason: a link error should surface there and not at tag time.
