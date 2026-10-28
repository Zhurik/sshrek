# Usage

Every command in one table, then the details worth knowing.

| Command | What it does |
|---|---|
| `sshrek` | pick a host from `~/.ssh/config` with fzf |
| `sshrek -` | connect to the last host |
| `sshrek <host>` | connect directly |
| `sshrek <host> -- <ssh args>` | everything after `--` goes to `ssh` |
| `sshrek ls` | table of hosts and history |
| `sshrek ls -r` / `-f` | sort by recency / frequency |
| `sshrek preview <host>` | the card the picker shows on the right |
| `sshrek completions <shell>` | completion script |
| `sshrek alias cssh` | shell snippet binding the tool to another name |

`--verbose` adds diagnostics on stderr, and works on every command.

## Connecting

`sshrek <host>` hands the name straight to `ssh`, so a host that is not in your
config works just as well as one that is. Anything after `--` is appended
untouched:

```sh
sshrek prod-db -- -L 8080:localhost:80
sshrek staging -- -N
```

The exit code is `ssh`'s own, which keeps `sshrek host && echo done` honest.
Ctrl-C reaches `ssh` rather than killing sshrek out from under it — the session
is still written to the history when you interrupt it.

`sshrek -` is the shortcut worth building a habit around: it reads the last
`start` event from the history and reconnects. With no history yet it says so
instead of guessing.

## The picker

Running `sshrek` with no arguments lists the config in fzf:

```text
prod-db  2h ago   6×  deploy@10.0.4.12
staging  55m ago  3×  deploy@staging.example.com:2222
nas      18m ago  2×  192.168.1.9
jump     never    0×  ops@jump.example.com
```

| Key | Order |
|---|---|
| `ctrl-o` | as declared in `~/.ssh/config` |
| `ctrl-r` | most recently used first |
| `ctrl-f` | most frequently used first |

The right-hand pane is `sshrek preview <host>`: the target, the last
connection with how long it lasted, the totals.

A picker needs a terminal to talk to. Started without one — from cron, from a
daemon — sshrek fails immediately rather than hanging on input that will never
arrive, so name the host explicitly in that context. The check applies to the
default picker only; one you chose yourself is not ours to second-guess.

## Listing

`sshrek ls` prints the table; `-r` and `-f` (or `--sort recent|frequent|config`)
reorder it:

```console
$ sshrek ls -r
HOST        LAST         COUNT  TOTAL  TARGET
nas         18m ago         2×     5m  192.168.1.9
staging     55m ago         3×    54m  deploy@staging.example.com:2222
prod-db     2h ago          6×  4h 6m  deploy@10.0.4.12
jump        never           0×     0s  ops@jump.example.com
old-laptop  3 weeks ago     1×     7m  (not in config)
```

`COUNT` and `TOTAL` are sessions and time spent connected. A host that appears
only in the history — `old-laptop` above, dropped from the config at some point
— is still listed, marked `(not in config)`: you cannot connect to it by name
any more, but knowing it was there is the point of keeping a history.

### For scripts

`--porcelain` prints the same five columns as tab-separated fields, with no
padding and no header:

```sh
sshrek ls --porcelain | cut -f1,3    # host and how many sessions
```

Adding `--picker` narrows that to what the picker is fed — hosts from the
config only, no orphans, and without the `TOTAL` column. These rows are the
ones fzf displays, so they are space-padded rather than tabbed:

```sh
sshrek ls --porcelain --picker | awk '{ print $1 }'    # just the names
```

`--picker` on its own does nothing; it only refines `--porcelain`.

In either form the cells are the table's, not raw numbers: the count keeps its
`×` and times stay human (`2h ago`, `4h 6m`).

## Shell completion

```sh
sshrek completions zsh > ~/.zfunc/_sshrek       # a directory on your fpath
sshrek completions bash > /etc/bash_completion.d/sshrek
sshrek completions fish > ~/.config/fish/completions/sshrek.fish
```

`--name` generates the same script for a different command name, which is what
makes a custom alias complete like the original.

Out of the box zsh completes the `HOST` argument from its own standard host
list (`/etc/hosts`, `known_hosts`) — see
[Configuration](configuration.md#completing-hosts-from-your-ssh-config) for
making it offer your config aliases instead.

To run the tool under a different name with completion included, see
[Running it under another name](alias.md).
