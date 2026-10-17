//! A thin shell around the library: argument parsing and command routing.

use std::collections::HashMap;

use chrono::Utc;
use clap::{CommandFactory, Parser};
use clap_complete::Shell;

use sshrek::alias;
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

/// Rust mutes SIGPIPE on startup, which makes `sshrek ls | head` die with a
/// panic instead of exiting quietly. Restore the behaviour expected of a tool
/// in a pipeline — the same one `cat` or `ls` has.
fn restore_default_sigpipe() {
    // SAFETY: the only signal(2) call, made before any thread is spawned.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

struct Context {
    hosts: Vec<HostEntry>,
    stats: HashMap<String, HostStats>,
    store: Store,
    /// The latest start event: computed here so the log is not read twice.
    last_host: Option<String>,
}

fn load(verbose: bool) -> Context {
    let (hosts, config_error) = config::parse_file(&paths::ssh_config());
    if let Some(e) = config_error {
        eprintln!("sshrek: could not read the config: {e}");
    }
    let store = Store::new(paths::sessions_log());
    let read = store.read_all();
    // A read error is always printed: otherwise a whole history that became
    // unreadable would look just like a missing one. Only the counters of
    // skipped lines hide behind --verbose.
    if let Some(e) = &read.error {
        eprintln!("sshrek: could not read the history: {e}");
    }
    if verbose && (read.malformed > 0 || read.unknown_version > 0) {
        eprintln!(
            "sshrek: lines skipped in the log: {} malformed, {} with another schema version",
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

/// Hosts present in the history but gone from the config: they cannot be
/// connected to, yet `ls` is a good place to learn they exist.
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

/// Absolute path to this binary, so a generated snippet keeps working for a
/// binary that is not in `PATH`.
fn self_exe() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "sshrek".to_string())
}

fn emit_completions(shell: Shell, name: Option<&str>) -> i32 {
    let mut cmd = Cli::command();
    let bin = match name {
        Some(n) => {
            if let Err(e) = alias::check_name(n) {
                eprintln!("sshrek: {e}");
                return 1;
            }
            n.to_string()
        }
        None => cmd.get_name().to_string(),
    };
    clap_complete::generate(shell, &mut cmd, bin, &mut std::io::stdout());
    0
}

/// The alias definition plus the completion script generated under that same
/// name, so one `eval` makes the alias complete like `sshrek` itself.
fn emit_alias(name: &str, shell: Option<Shell>) -> i32 {
    let Some(shell) = shell.or_else(Shell::from_env) else {
        eprintln!(
            "sshrek: could not tell the shell from $SHELL — name it: sshrek alias {name} --shell zsh"
        );
        return 1;
    };
    match alias::definition(name, shell, &self_exe()) {
        Ok(definition) => {
            print!("{definition}");
            emit_completions(shell, Some(name))
        }
        Err(e) => {
            eprintln!("sshrek: {e}");
            1
        }
    }
}

fn run(args: Cli) -> i32 {
    match &args.command {
        Some(Command::Completions { shell, name }) => emit_completions(*shell, name.as_deref()),
        Some(Command::Alias { name, shell }) => emit_alias(name, *shell),
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
            // The picker format is the only one that must not carry history-only
            // hosts: there is no way to connect to what the config does not list.
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
                eprintln!("sshrek: no history yet — run sshrek with no arguments and pick a host");
                return 1;
            }
        },
        Some(explicit) => explicit.to_string(),
        None => {
            let hosts = sorted_hosts(&ctx, args.sorting.key());
            if hosts.is_empty() {
                eprintln!(
                    "sshrek: not a single host in {}",
                    paths::ssh_config().display()
                );
                return 1;
            }
            let picker_env = std::env::var("SSHREK_PICKER").ok();
            let args_env = std::env::var("SSHREK_PICKER_ARGS").ok();
            let rows = render::picker_rows(&hosts, &ctx.stats, Utc::now());
            let self_exe = self_exe();
            // Check for a terminal only for the default picker: without a TTY fzf
            // does not fail, it hangs forever. A custom picker was chosen by the
            // user — we do not know its requirements and have no right to interfere.
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
                eprintln!("sshrek: picker {} {:?}", cmd.program, cmd.args);
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
