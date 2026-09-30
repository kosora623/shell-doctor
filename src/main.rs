use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Row, Table};

mod parser;
use parser::analyze_trace;

#[derive(Parser)]
#[command(name = "shell-doctor", version = "0.1.0", about = "Lightning-fast shell runtime doctor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// デモ用のトレース解析を実行
    Demo,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo => {
            println!("{}", "🩺 Analyzing shell startup bottlenecks...".cyan().bold());

            let sample_trace = r#"
+1727670000.000000 /home/user/.zshrc:1: export PATH=/usr/local/bin:$PATH
+1727670000.005000 /home/user/.zshrc:5: alias g=git
+1727670000.010000 /home/user/.zshrc:12: eval "$(brew shellenv)"
+1727670000.195000 /home/user/.zshrc:24: eval "$(pyenv init -)"
+1727670000.415000 /home/user/.zshrc:40: source ~/.fzf.zsh
+1727670000.440000 /home/user/.zshrc:50: echo 'Ready!'
"#;

            let profiles = analyze_trace(sample_trace, ".zshrc");

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec!["Line", "Duration", "Command"]);

            let mut total_ms = 0;
            for p in &profiles {
                total_ms += p.duration_ms;
                let color = if p.duration_ms > 100 { Color::Red } else { Color::Yellow };
                table.add_row(Row::from(vec![
                    Cell::new(p.line.to_string()),
                    Cell::new(format!("{} ms", p.duration_ms)).fg(color),
                    Cell::new(&p.command),
                ]));
            }

            println!("{table}");
            println!("⚡ Total bottleneck delay: {} ms\n", total_ms.to_string().red().bold());
        }
    }
}