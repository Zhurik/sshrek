use clap::{Parser, Subcommand, ValueHint};
use clap_complete::Shell;

use crate::sort::SortKey;

#[derive(Parser, Debug)]
#[command(
    name = "sshrek",
    version,
    about = "ssh with connection history and a one-keystroke jump back to the last host",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// Host from ~/.ssh/config; `-` means the last one you connected to
    #[arg(
        value_name = "HOST",
        allow_hyphen_values = true,
        value_hint = ValueHint::Hostname
    )]
    pub host: Option<String>,

    /// Extra arguments passed on to ssh after `--`
    #[arg(last = true, value_name = "SSH_ARGS")]
    pub extra: Vec<String>,

    #[command(flatten)]
    pub sorting: Sorting,

    /// Verbose diagnostics on stderr
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(clap::Args, Debug)]
pub struct Sorting {
    /// Order of the list
    #[arg(long, value_enum, value_name = "KEY")]
    pub sort: Option<SortKey>,

    /// Same as --sort=recent
    #[arg(short = 'r', long, conflicts_with_all = ["frequent", "sort"])]
    pub recent: bool,

    /// Same as --sort=frequent
    #[arg(short = 'f', long, conflicts_with_all = ["recent", "sort"])]
    pub frequent: bool,
}

impl Sorting {
    pub fn key(&self) -> SortKey {
        if self.recent {
            SortKey::Recent
        } else if self.frequent {
            SortKey::Frequent
        } else {
            self.sort.unwrap_or(SortKey::Config)
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Table of hosts and connection history
    Ls {
        /// TSV with no padding and no header
        #[arg(long)]
        porcelain: bool,

        /// Only hosts from the config, formatted as picker rows
        #[arg(long)]
        picker: bool,

        #[command(flatten)]
        sorting: Sorting,
    },

    /// Card for a single host (used by the picker's --preview)
    Preview {
        #[arg(value_name = "HOST", value_hint = ValueHint::Hostname)]
        host: String,
    },

    /// Shell completion script
    Completions {
        #[arg(value_name = "SHELL")]
        shell: Shell,
    },
}
