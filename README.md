# sshrek

Обёртка над `ssh`: выбор хоста через fzf, мгновенный возврат к последнему,
история подключений.

## Установка

```sh
cargo install --path .
```

Либо скачать архив под свою платформу из [Releases](../../releases) и положить
бинарник в `PATH`.

## Использование

| Команда | Что делает |
|---|---|
| `sshrek` | выбор хоста из `~/.ssh/config` через fzf |
| `sshrek -` | подключение к последнему хосту |
| `sshrek <host>` | подключение напрямую |
| `sshrek <host> -- -L 8080:localhost:80` | всё после `--` уходит в `ssh` |
| `sshrek ls` | таблица хостов и истории |
| `sshrek ls -r` / `-f` | сортировка по свежести / частоте |
| `sshrek completions zsh` | скрипт автодополнения |

В пикере: `ctrl-o` — порядок из config, `ctrl-r` — свежие сверху,
`ctrl-f` — частые сверху.

## Настройка

| Переменная | Дефолт | Назначение |
|---|---|---|
| `SSHREK_PICKER` | `fzf` | команда пикера, например `fzf-tmux -p` |
| `SSHREK_PICKER_ARGS` | — | полностью заменяет флаги, передаваемые пикеру |
| `SSHREK_SSH_BIN` | `ssh` | чем подключаться |
| `SSHREK_SSH_CONFIG` | `~/.ssh/config` | откуда брать хосты |
| `SSHREK_STATE_DIR` | `$XDG_STATE_HOME/sshrek` | где хранится история |

Автодополнение для zsh:

```sh
sshrek completions zsh > ~/.zfunc/_sshrek
```

Из коробки zsh дополняет аргумент `HOST` своим стандартным списком хостов
(`/etc/hosts`, `known_hosts`). Чтобы он предлагал именно алиасы из
`~/.ssh/config`, добавь в `.zshrc`:

```sh
zstyle ':completion:*:sshrek:*' hosts ${(f)"$(sshrek ls --porcelain --picker | awk '{print $1}')"}
```

История — append-only JSONL в `$SSHREK_STATE_DIR/sessions.jsonl`: его можно
грепать, резать и править руками.

Поддерживаются macOS и Linux.

## Лицензия

MIT
