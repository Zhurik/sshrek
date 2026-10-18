# sshrek

A wrapper around `ssh`: pick a host with fzf, jump straight back to the last
one, keep a history of your connections.

## Installation

```sh
brew install Zhurik/tap/sshrek
```

Homebrew covers macOS on Apple silicon and Intel, and Linux on x86_64 and arm64.
Otherwise build it yourself:

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
| `sshrek alias cssh` | shell snippet binding the tool to another name |

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

## Running it under another name

`sshrek alias` prints a shell snippet that binds the tool to a name of your
choosing, completion included. Add one line to your shell's startup file:

```sh
eval "$(sshrek alias cssh)"
```

That is all: `cssh`, `cssh -`, `cssh ls`, `cssh <host>` and tab completion all
work exactly like the `sshrek` originals. The name is just an argument, so pick
whatever you like — `eval "$(sshrek alias s)"` gives you a one-letter `s`. With
no argument the name defaults to `cssh`.

The shell is taken from `$SHELL`; pass `--shell` when that is wrong or unset:

```sh
eval "$(sshrek alias cssh --shell bash)"
```

Per shell, the line to add is:

| Shell | File | Line |
|---|---|---|
| zsh | `.zshrc` | `eval "$(sshrek alias cssh)"` — **after** your `compinit` |
| bash | `.bashrc` | `eval "$(sshrek alias cssh)"` |
| fish | `config.fish` | `sshrek alias cssh --shell fish \| source` |

Two details worth knowing:

- **zsh: put it after `compinit`.** The snippet registers completion with
  `compdef`, which does not exist until `compinit` has run. Too early and zsh
  prints `command not found: compdef`.
- **The host-completion `zstyle` is per command name.** If you set up the
  `~/.ssh/config` host list above, repeat it for the new name — the context in
  the pattern is the command you actually type:

  ```sh
  zstyle ':completion:*:cssh:*' hosts ${(f)"$(cssh ls --porcelain --picker | awk '{print $1}')"}
  ```

If you would rather not run `sshrek` on every shell startup, install a
completion file under the new name instead and define the function yourself:

```sh
sshrek completions zsh --name cssh > ~/.zfunc/_cssh
# in .zshrc:
cssh() { command sshrek "$@"; }
```

What `sshrek alias` emits is a shell *function*, not an `alias`. bash expands no
aliases at all in a non-interactive shell, and both bash and zsh expand them
when a command is parsed rather than when it runs — an alias would therefore be
invisible to the rest of the same script. A function has neither limit, so
`cssh` also works from scripts, while `complete`/`compdef` bind to it just the
same.

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
