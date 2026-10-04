//! Тонкая обёртка над библиотекой: разбор аргументов и маршрутизация команд.

use std::collections::HashMap;

use chrono::Utc;
use clap::{CommandFactory, Parser};

use sshrek::cli::{Cli, Command};
use sshrek::config::{self, HostEntry};
use sshrek::history::{self, HostStats, Store};
use sshrek::picker::{self, PickOutcome};
use sshrek::sort::{self, SortKey};
use sshrek::{paths, render, runner};

fn main() {
    restore_default_sigpipe();
    let args = Cli::parse();
    std::process::exit(run(args));
}

/// Rust глушит SIGPIPE при старте, из-за чего `sshrek ls | head` роняет
/// программу паникой вместо тихого завершения. Возвращаем поведение,
/// которого ждут от инструмента в конвейере — как у `cat` или `ls`.
fn restore_default_sigpipe() {
    // SAFETY: единственный вызов signal(2) до создания потоков.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

struct Context {
    hosts: Vec<HostEntry>,
    stats: HashMap<String, HostStats>,
    store: Store,
    /// Последний start-эвент: считается здесь же, чтобы не читать лог второй раз.
    last_host: Option<String>,
}

fn load(verbose: bool) -> Context {
    let (hosts, config_error) = config::parse_file(&paths::ssh_config());
    if let Some(e) = config_error {
        eprintln!("sshrek: конфиг не прочитан: {e}");
    }
    let store = Store::new(paths::sessions_log());
    let read = store.read_all();
    // Ошибка чтения печатается всегда: иначе целая история, ставшая
    // недоступной, выглядит как её отсутствие. Под --verbose прячутся
    // только счётчики пропущенных строк.
    if let Some(e) = &read.error {
        eprintln!("sshrek: история не прочитана: {e}");
    }
    if verbose && (read.malformed > 0 || read.unknown_version > 0) {
        eprintln!(
            "sshrek: в логе пропущено строк: битых {}, с другой версией схемы {}",
            read.malformed, read.unknown_version
        );
    }
    let stats = history::aggregate(&read.records);
    let last_host = history::last_host(&read.records).map(str::to_string);
    Context {
        hosts,
        stats,
        store,
        last_host,
    }
}

fn ssh_bin() -> String {
    std::env::var("SSHREK_SSH_BIN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "ssh".to_string())
}

fn sorted_hosts(ctx: &Context, key: SortKey) -> Vec<HostEntry> {
    let mut hosts = ctx.hosts.clone();
    sort::sort_hosts(&mut hosts, &ctx.stats, key);
    hosts
}

/// Хосты, которые есть в истории, но исчезли из config: подключиться к ним
/// нельзя, но в `ls` о них полезно знать.
fn orphans<'a>(ctx: &'a Context, hosts: &[HostEntry]) -> Vec<&'a HostStats> {
    let known: std::collections::HashSet<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    let mut out: Vec<&HostStats> = ctx
        .stats
        .values()
        .filter(|s| !known.contains(s.host.as_str()))
        .collect();
    out.sort_by(|a, b| {
        b.last_connected
            .cmp(&a.last_connected)
            .then(a.host.cmp(&b.host))
    });
    out
}

fn run(args: Cli) -> i32 {
    match &args.command {
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(*shell, &mut cmd, name, &mut std::io::stdout());
            0
        }
        Some(Command::Preview { host }) => {
            let ctx = load(args.verbose);
            let entry = ctx.hosts.iter().find(|h| &h.alias == host);
            print!(
                "{}",
                render::preview(host, entry, ctx.stats.get(host), Utc::now())
            );
            0
        }
        Some(Command::Ls {
            porcelain,
            picker: picker_format,
            sorting,
        }) => {
            let ctx = load(args.verbose);
            let hosts = sorted_hosts(&ctx, sorting.key());
            let now = Utc::now();
            // Формат пикера — единственный, где хостов из истории быть не должно:
            // подключиться к тому, чего нет в config, всё равно нельзя.
            let gone = if *picker_format {
                Vec::new()
            } else {
                orphans(&ctx, &hosts)
            };
            if *porcelain {
                let rows = if *picker_format {
                    render::picker_rows(&hosts, &ctx.stats, now)
                } else {
                    render::porcelain_rows(&hosts, &ctx.stats, &gone, now)
                };
                for row in rows {
                    println!("{row}");
                }
            } else {
                println!("{}", render::ls_table(&hosts, &ctx.stats, &gone, now));
            }
            0
        }
        None => connect_flow(&args),
    }
}

fn connect_flow(args: &Cli) -> i32 {
    let ctx = load(args.verbose);

    let host = match args.host.as_deref() {
        Some("-") => match ctx.last_host.clone() {
            Some(h) => h,
            None => {
                eprintln!("sshrek: истории пока нет, запусти sshrek без аргументов и выбери хост");
                return 1;
            }
        },
        Some(explicit) => explicit.to_string(),
        None => {
            let hosts = sorted_hosts(&ctx, args.sorting.key());
            if hosts.is_empty() {
                eprintln!(
                    "sshrek: в {} нет ни одного хоста",
                    paths::ssh_config().display()
                );
                return 1;
            }
            let picker_env = std::env::var("SSHREK_PICKER").ok();
            let args_env = std::env::var("SSHREK_PICKER_ARGS").ok();
            let rows = render::picker_rows(&hosts, &ctx.stats, Utc::now());
            let self_exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "sshrek".to_string());
            // Проверяем терминал только для пикера по умолчанию: fzf без TTY
            // не падает, а виснет навсегда. Свой пикер пользователь задал сам —
            // его требования нам неизвестны, и мешать ему мы не вправе.
            let terminal_ok = picker_env.is_some() || picker::has_controlling_terminal();
            if let Err(e) = picker::ensure_terminal(terminal_ok) {
                eprintln!("sshrek: {e}");
                return 1;
            }
            let cmd = match picker::build_command(
                picker_env.as_deref(),
                args_env.as_deref(),
                &self_exe,
            ) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("sshrek: {e}");
                    return 1;
                }
            };
            if args.verbose {
                eprintln!("sshrek: пикер {} {:?}", cmd.program, cmd.args);
            }
            match picker::run(&cmd, &rows) {
                Ok(PickOutcome::Selected(alias)) => alias,
                Ok(PickOutcome::Cancelled) => return 130,
                Err(e) => {
                    eprintln!("sshrek: {e}");
                    return 1;
                }
            }
        }
    };

    if args.verbose {
        eprintln!("sshrek: {} {} {:?}", ssh_bin(), host, args.extra);
    }
    runner::connect(&ctx.store, &ssh_bin(), &host, &args.extra)
}
