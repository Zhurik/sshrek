# Contributing

## The loop

```sh
cargo test --all        # unit tests next to the code, CLI tests in tests/
cargo clippy --all-targets -- -D warnings
cargo fmt
```

That is also what CI runs, on Linux and macOS both, plus a cross-build of the
Intel macOS target.

To try the real thing without touching your own history or config, point the
environment variables somewhere disposable:

```sh
SSHREK_SSH_CONFIG=tests/fixtures/ssh_config_history \
SSHREK_STATE_DIR=$(mktemp -d) \
  cargo run -- ls
```

## Hooks

The checks above are wired up as [pre-commit](https://pre-commit.com) hooks, so
a push cannot carry what CI will reject:

```sh
brew install pre-commit        # or: pip install pre-commit
pre-commit install --install-hooks
```

That one `install` registers all three stages the config asks for:

| Stage | Hooks |
|---|---|
| commit | whitespace and line endings, YAML and TOML syntax, `cargo fmt`, `cargo clippy` |
| commit message | the convention below |
| push | `cargo test`, `cargo doc` with warnings denied |

The slow pair waits for the push on purpose: committing stays cheap, and
nothing reaches the remote unchecked. To run everything by hand:

```sh
pre-commit run --all-files                      # the commit-stage hooks
pre-commit run --all-files --hook-stage pre-push
```

`cargo fmt` fixes files rather than complaining about them, which makes the
hook fail with the formatting already applied — `git add` and commit again.

Hooks are a convenience, not a gate: CI runs the same checks, and
`git commit --no-verify` is there when you need it.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org), which is what the
history already looks like:

```
feat(picker): remember the last query
fix: write the end event when ssh is killed
chore: release v0.2.1
```

Types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`,
`revert`, `style`, `test`. A `!` before the colon marks a breaking change.
Merge, revert and `--fixup`/`--squash` subjects are left alone.

## Shape of the code

The logic lives in the library and `main.rs` only wires commands together, so
every module stays a public API that can be tested on its own. Keep it that
way: a new feature is a function in a module with tests beside it, plus a line
in `main.rs`.

Comments explain *why*, as the existing ones do — the mechanics are in the code
already.

Releasing is [its own document](docs/releasing.md).
