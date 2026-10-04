use clap::{Parser, Subcommand};
use clap_complete::Shell;

use crate::sort::SortKey;

#[derive(Parser, Debug)]
#[command(
    name = "sshrek",
    version,
    about = "ssh с историей подключений и быстрым возвратом к последнему хосту",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// Хост из ~/.ssh/config; `-` — последний, к которому подключался
    #[arg(value_name = "HOST", allow_hyphen_values = true)]
    pub host: Option<String>,

    /// Дополнительные аргументы для ssh после `--`
    #[arg(last = true, value_name = "SSH_ARGS")]
    pub extra: Vec<String>,

    #[command(flatten)]
    pub sorting: Sorting,

    /// Подробная диагностика в stderr
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(clap::Args, Debug)]
pub struct Sorting {
    /// Порядок списка
    #[arg(long, value_enum, value_name = "KEY")]
    pub sort: Option<SortKey>,

    /// То же, что --sort=recent
    #[arg(short = 'r', long, conflicts_with_all = ["frequent", "sort"])]
    pub recent: bool,

    /// То же, что --sort=frequent
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
    /// Таблица хостов и истории подключений
    Ls {
        /// TSV без выравнивания и заголовка
        #[arg(long)]
        porcelain: bool,

        /// Только хосты из config, в формате строк пикера
        #[arg(long)]
        picker: bool,

        #[command(flatten)]
        sorting: Sorting,
    },

    /// Карточка одного хоста (используется в --preview пикера)
    Preview {
        #[arg(value_name = "HOST")]
        host: String,
    },

    /// Скрипт автодополнения для шелла
    Completions {
        #[arg(value_name = "SHELL")]
        shell: Shell,
    },
}
