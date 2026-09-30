use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Row, Table};

mod parser;
mod path_checker;

use parser::analyze_trace;
use path_checker::{run_path_health, DeadReason};

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
    /// PATH 環境変数の健全性を診断する（Dead Path・重複エントリの検出）
    PathHealth,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo => run_demo(),
        Commands::PathHealth => run_path_health_cmd(),
    }
}

// ─── Demo コマンド ────────────────────────────────────────────

fn run_demo() {
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

// ─── PathHealth コマンド ──────────────────────────────────────

fn run_path_health_cmd() {
    println!("{}", "🩺 Diagnosing PATH environment variable...".cyan().bold());
    println!();

    let report = run_path_health();

    // ── Dead Paths テーブル ──────────────────────────────────
    println!("{}", "Dead Paths".red().bold());
    if report.dead_paths.is_empty() {
        println!("{}\n", "  ✅ No dead paths found.".green());
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("#").add_attribute(Attribute::Bold),
                Cell::new("Path").add_attribute(Attribute::Bold),
                Cell::new("Reason").add_attribute(Attribute::Bold),
            ]);

        for dead in &report.dead_paths {
            let reason_str = match dead.reason {
                DeadReason::NotFound => "Not Found",
                DeadReason::NotADirectory => "Not a Directory",
            };
            table.add_row(Row::from(vec![
                Cell::new(dead.index + 1),
                Cell::new(&dead.raw).fg(Color::Red),
                Cell::new(reason_str).fg(Color::Red),
            ]));
        }

        println!("{table}\n");
    }

    // ── Duplicate Paths テーブル ─────────────────────────────
    println!("{}", "Duplicate Paths".yellow().bold());
    if report.duplicates.is_empty() {
        println!("{}\n", "  ✅ No duplicates found.".green());
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("#").add_attribute(Attribute::Bold),
                Cell::new("Path").add_attribute(Attribute::Bold),
                Cell::new("Status").add_attribute(Attribute::Bold),
                Cell::new("Count").add_attribute(Attribute::Bold),
            ]);

        for dup in &report.duplicates {
            for (i, &occ_idx) in dup.occurrences.iter().enumerate() {
                let is_first = i == 0;
                let status = if is_first { "first" } else { "duplicate" };
                let color = if is_first { Color::Green } else { Color::Yellow };
                let count_cell = if is_first {
                    Cell::new(dup.occurrences.len()).fg(color)
                } else {
                    Cell::new("—").fg(color)
                };

                table.add_row(Row::from(vec![
                    Cell::new(occ_idx + 1).fg(color),
                    Cell::new(&dup.canonical).fg(color),
                    Cell::new(status).fg(color),
                    count_cell,
                ]));
            }
        }

        println!("{table}\n");
    }

    // ── Health Score ─────────────────────────────────────────
    let issue_count = report.dead_paths.len() + report.duplicates.len();
    let score = if report.total == 0 {
        100u32
    } else {
        let penalty = (issue_count * 100) / report.total;
        100u32.saturating_sub(penalty as u32)
    };

    let score_colored = match score {
        90..=100 => score.to_string().green().bold(),
        60..=89 => score.to_string().yellow().bold(),
        _ => score.to_string().red().bold(),
    };

    // ── サマリー行（仕様書 §3-3）─────────────────────────────
    println!(
        "⚡ {} {} entries | {} dead | {} duplicates | {} skipped",
        "PATH Health Summary:".bold(),
        report.total.to_string().cyan(),
        report.dead_paths.len().to_string().red(),
        report.duplicates.len().to_string().yellow(),
        report.skipped_count.to_string().white(),
    );
    println!("🏥 Health Score: {}/100", score_colored);
}
