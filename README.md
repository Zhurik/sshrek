# sshrek

A wrapper around `ssh`: pick a host with fzf, jump straight back to the last
one, keep a history of your connections.

## Installation

```sh
cargo install --path .
```

Or download the archive for your platform from [Releases](../../releases) and
drop the binary into your `PATH`.

## Usage

| Command | What it does |
|---|---|
| `sshrek` | pick a host from `~/.ssh/config` with fzf |
| `sshrek -` | connect to the last host |
| `sshrek <host>` | connect directly |
| `sshrek <host> -- -L 8080:localhost:80` | everything after `--` goes to `ssh` |
| `sshrek ls` | table of hosts and history |
| `sshrek ls -r` / `-f` | sort by recency / frequency |
| `sshrek completions zsh` | completion script |

Inside the picker: `ctrl-o` — config order, `ctrl-r` — most recent first,
`ctrl-f` — most frequent first.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `SSHREK_PICKER` | `fzf` | picker command, for example `fzf-tmux -p` |
| `SSHREK_PICKER_ARGS` | — | replaces the flags passed to the picker entirely |
| `SSHREK_SSH_BIN` | `ssh` | what to connect with |
| `SSHREK_SSH_CONFIG` | `~/.ssh/config` | where to read hosts from |
| `SSHREK_STATE_DIR` | `$XDG_STATE_HOME/sshrek` | where the history is kept |

Completion for zsh:

```sh
sshrek completions zsh > ~/.zfunc/_sshrek
```

Out of the box zsh completes the `HOST` argument from its own standard host
list (`/etc/hosts`, `known_hosts`). To make it offer the aliases from
`~/.ssh/config` instead, add this to `.zshrc`:

```sh
zstyle ':completion:*:sshrek:*' hosts ${(f)"$(sshrek ls --porcelain --picker | awk '{print $1}')"}
```

The history is an append-only JSONL file at `$SSHREK_STATE_DIR/sessions.jsonl`:
grep it, cut it, edit it by hand.

macOS and Linux are supported.

## Releasing

A new version reaches [Releases](../../releases) only when a matching tag is
pushed — nothing is published from `main` on its own. The tag and the `version`
in `Cargo.toml` must be identical, otherwise the `verify` job fails the release
before anything is built.

```sh
# 1. bump the version in Cargo.toml, e.g. 0.1.0 -> 0.2.0
# 2. refresh Cargo.lock (the release build runs with --locked)
cargo build
# 3. commit both files
git commit -am "chore: release v0.2.0"
# 4. tag with the very same version, prefixed with v
git tag v0.2.0
# 5. push the branch and the tag
git push && git push origin v0.2.0
```

The tag is what triggers the workflow, so pushing the commit alone does nothing.
It then builds the three targets (Linux x86_64 and aarch64 against musl, macOS
aarch64), uploads a `.tar.gz` plus a `.sha256` for each, and publishes a release
with generated notes and a `SHA256SUMS` file. A tag containing a `-`
(`v0.2.0-rc1`) is published as a pre-release.

## License

MIT
