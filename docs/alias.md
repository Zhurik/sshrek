# Running it under another name

`sshrek` is a mouthful to type twenty times a day, and the name of the command
is not part of the tool. `sshrek alias` prints a shell snippet that binds it to
a name of your choosing, completion included:

```sh
eval "$(sshrek alias cssh)"
```

One line in your shell's startup file is the whole setup. After it `cssh`,
`cssh -`, `cssh ls`, `cssh <host>` and tab completion all behave exactly like
the `sshrek` originals. The name is just an argument, so pick what you like —
`eval "$(sshrek alias s)"` gives you a one-letter `s`. With no argument the
name defaults to `cssh`.

| Shell | File | Line |
|---|---|---|
| zsh | `.zshrc` | `eval "$(sshrek alias cssh)"` — **after** your `compinit` |
| bash | `.bashrc` | `eval "$(sshrek alias cssh)"` |
| fish | `config.fish` | `sshrek alias cssh --shell fish \| source` |

The shell is taken from `$SHELL`; pass `--shell` when that is wrong or unset:

```sh
eval "$(sshrek alias cssh --shell bash)"
```

## zsh: put it after `compinit`

The snippet registers completion with `compdef`, which does not exist until
`compinit` has run. Too early and zsh greets you with
`command not found: compdef`.

## Host completion is per command name

If you set up the [`zstyle` for config
hosts](configuration.md#completing-hosts-from-your-ssh-config), repeat it for
the new name — the context in the pattern is the command you actually type:

```sh
zstyle ':completion:*:cssh:*' hosts ${(f)"$(cssh ls --porcelain --picker | awk '{print $1}')"}
```

## Without running sshrek on every shell startup

`eval` means one `sshrek` process per shell. To avoid that, install a
completion file under the new name and define the function yourself:

```sh
sshrek completions zsh --name cssh > ~/.zfunc/_cssh
# in .zshrc:
cssh() { command sshrek "$@"; }
```

## Why a function and not an alias

What `sshrek alias` emits is a shell *function*, despite the subcommand's name.
bash expands no aliases at all in a non-interactive shell, and both bash and
zsh expand them when a command is parsed rather than when it runs — an alias
would therefore be invisible to the rest of the same script. A function has
neither limit, so `cssh` also works from scripts, while `complete`/`compdef`
bind to it just the same. fish's own `alias` already defines a function, so
there it is both idiomatic and equivalent.
