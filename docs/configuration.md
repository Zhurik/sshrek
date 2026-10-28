# Configuration

Nothing to configure for the common case: sshrek reads `~/.ssh/config`, keeps
its history under `$XDG_STATE_HOME`, and drives `fzf`. Five environment
variables move those pieces.

| Variable | Default | Purpose |
|---|---|---|
| `SSHREK_PICKER` | `fzf` | picker command, for example `fzf-tmux -p` |
| `SSHREK_PICKER_ARGS` | — | replaces the flags passed to the picker entirely |
| `SSHREK_SSH_BIN` | `ssh` | what to connect with |
| `SSHREK_SSH_CONFIG` | `~/.ssh/config` | where to read hosts from |
| `SSHREK_STATE_DIR` | `$XDG_STATE_HOME/sshrek` | where the history is kept |

## The picker

`SSHREK_PICKER` is a command line, not just a program name, so flags belong in
it: `SSHREK_PICKER='fzf-tmux -p'` works because `fzf-tmux` passes everything it
does not recognise through to fzf.

`SSHREK_PICKER_ARGS` is the escape hatch, and it *replaces* the default flags
rather than adding to them. Those defaults are what make the picker more than a
list, so expect to lose them:

- `--no-sort --layout=reverse --height=40%` and the `ssh > ` prompt
- `--preview 'sshrek preview {1}'` in a right-hand window, the card for the
  highlighted host
- `ctrl-o` / `ctrl-r` / `ctrl-f`, each rebinding the list to
  `sshrek ls --porcelain --picker --sort=…`

Whatever you put there has to accept the rows on stdin and print the chosen one
back. Only the first whitespace-separated field of that line is read, so a
picker that returns the whole row is fine.

With a picker other than the default, sshrek stops checking for a terminal
first: the check exists because fzf hangs instead of failing when there is no
tty, and a picker you chose yourself is not ours to second-guess.

## What is read from the ssh config

Enough of the format to build the list, and no more:

- `Host` blocks give the aliases. Patterns (`Host *`, `Host web-?`) are skipped
  — they are not hosts you can connect to by name.
- `HostName`, `User` and `Port` inside a block make up the `TARGET` column. As
  in ssh itself, the first value wins when a key repeats.
- `Include` is followed, relative to the file doing the including, up to 16
  levels deep. A cycle stops instead of looping.
- `Match` blocks are skipped entirely: their conditions are evaluated at
  connection time, so no fixed list of hosts can be derived from them.
- A missing config is not an error — the list is simply empty, and
  `sshrek <host>` still works.

Nothing else is interpreted, and nothing is ever written back: `ssh` remains
the authority on what your config means.

## The history

An append-only JSONL file at `$SSHREK_STATE_DIR/sessions.jsonl`, two lines per
session:

```json
{"v":1,"event":"start","id":"18e279ecceaca758-e37a","host":"prod-db","at":"2026-10-27T08:14:02.511304Z"}
{"v":1,"event":"end","id":"18e279ecceaca758-e37a","at":"2026-10-27T09:01:48.222811Z","exit":0}
```

The `id` pairs the two events — the start time in nanoseconds and the pid, both
in hex — and `v` is the schema version. A line from a future version is skipped
rather than treated as corrupt, so an older sshrek keeps working against a
newer file. Malformed lines are counted and ignored; `--verbose` says how many
of each were passed over.

Grep it, cut it, delete the lines you would rather forget. Nothing else keeps
state, so removing the file resets the history and loses nothing else.

## Completing hosts from your ssh config

Out of the box zsh completes the `HOST` argument from its own standard host
list (`/etc/hosts`, `known_hosts`). To make it offer the aliases from your
config instead, add this to `.zshrc`:

```sh
zstyle ':completion:*:sshrek:*' hosts ${(f)"$(sshrek ls --porcelain --picker | awk '{print $1}')"}
```

The context in the pattern is the command you actually type, so repeat the line
for every name you use — see
[Running it under another name](alias.md#host-completion-is-per-command-name).
