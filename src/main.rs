use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "sshrek", version, about = "ssh с историей подключений")]
struct Cli {
    /// Хост из ~/.ssh/config; `-` — последний, к которому подключался
    #[arg(value_name = "HOST", allow_hyphen_values = true)]
    host: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    println!("{cli:?}");
}
