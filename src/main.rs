use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Row, Table};
use std::path::Path;
use std::process::Command;

mod parser;
mod path_checker;

use parser::{analyze_trace, ProfileKind};
use path_checker::{compute_health_score, run_path_health_from, DeadReason};

#[derive(Parser)]
#[command(
    name = "shell-doctor",
    version = "0.1.0",
    about = "Lightning-fast shell runtime doctor",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// シェルの起動ボトルネックを診断する
    ///
    /// --file でトレースログファイルを指定するか、
    /// --spawn でシェルプロファイルを直接指定して自動計測します。
    /// どちらも省略するとデモデータで動作します。
    Demo {
        /// 解析するトレースログファイルのパス（zsh/bash の `set -x` 出力など）
        #[arg(long, short = 'f', value_name = "FILE")]
        file: Option<String>,

        /// headless シェルを起動してトレースを取得するプロファイルファイルのパス
        /// （例: ~/.zshrc, ~/.bashrc）
        #[arg(long, short = 's', value_name = "PROFILE")]
        spawn: Option<String>,
    },
    /// PATH 環境変数の健全性を診断する（Dead Path・重複エントリの検出）
    PathHealth {
        /// 診断する PATH 文字列を直接指定する（省略時は環境変数 PATH を使用）
        /// 例: --path "/usr/bin:/usr/local/bin:/nonexistent"
        #[arg(long, value_name = "PATH_STRING")]
        path: Option<String>,

        /// 問題が 1 件以上検出されたとき非ゼロ終了コードで終了する（CI 向け）
        #[arg(long)]
        fail_on_issues: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo { file, spawn } => run_demo(file, spawn)?,
        Commands::PathHealth {
            path,
            fail_on_issues,
        } => run_path_health_cmd(path.as_deref(), fail_on_issues)?,
    }

    Ok(())
}

// ─── Demo コマンド ────────────────────────────────────────────

fn run_demo(file: Option<String>, spawn: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(profile_path) = spawn {
        run_demo_spawn(&profile_path)
    } else if let Some(trace_path) = file {
        run_demo_from_file(&trace_path)
    } else {
        run_demo_sample()
    }
}

fn run_demo_spawn(profile_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let kind = ProfileKind::from_path(profile_path);
    println!(
        "{}",
        format!(
            "🩺 Spawning {} to capture real startup trace: {}",
            kind.display_name(),
            profile_path
        )
        .cyan()
        .bold()
    );

    let trace_output = spawn_shell_trace(profile_path, &kind)?;
    if trace_output.trim().is_empty() {
        eprintln!(
            "{}",
            "⚠️  No trace output captured. Make sure the profile contains commands.".yellow()
        );
        return Ok(());
    }
    print_trace_results(&trace_output, kind.suffix(), kind.display_name());
    Ok(())
}

fn spawn_shell_trace(
    profile_path: &str,
    kind: &ProfileKind,
) -> Result<String, Box<dyn std::error::Error>> {
    let output = match kind {
        ProfileKind::Zsh => Command::new("zsh")
            .args([
                "-c",
                &format!(
                    "PS4='+${{EPOCHREALTIME}} ${{(%):-%x}}:${{LINENO}}: ' zsh --no-rcs -o xtrace -i -c 'source {profile_path}' 2>&1"
                ),
            ])
            .output(),
        ProfileKind::Bash => Command::new("bash")
            .args([
                "-c",
                &format!(
                    "PS4='+$(date +%s.%N) ${{BASH_SOURCE}}:${{LINENO}}: ' bash --norc -x -c 'source {profile_path}' 2>&1"
                ),
            ])
            .output(),
        ProfileKind::PowerShell => Command::new("pwsh")
            .args([
                "-NoProfile",
                "-Command",
                &format!("Set-PSDebug -Trace 2; . '{profile_path}'; Set-PSDebug -Off"),
            ])
            .output(),
        ProfileKind::Other(_) => Command::new("sh")
            .args([
                "-c",
                &format!(
                    "PS4='+$(date +%s.%6N) ${{0}}:${{LINENO}}: ' sh -x '{profile_path}' 2>&1"
                ),
            ])
            .output(),
    };

    match output {
        Ok(out) => {
            if !out.status.success() && out.stdout.is_empty() && out.stderr.is_empty() {
                return Err(format!("Shell process exited with status: {}", out.status).into());
            }
            let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            Ok(combined)
        }
        Err(e) => Err(format!("Failed to spawn shell: {e}").into()),
    }
}

fn run_demo_from_file(trace_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let kind = ProfileKind::from_path(trace_path);
    println!(
        "{}",
        format!("🩺 Analyzing trace file: {}", trace_path).cyan().bold()
    );

    let trace_output = std::fs::read_to_string(trace_path)
        .map_err(|e| format!("Failed to read trace file '{trace_path}': {e}"))?;

    if trace_output.trim().is_empty() {
        eprintln!("{}", "⚠️  Trace file is empty.".yellow());
        return Ok(());
    }

    let suffix = Path::new(trace_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(kind.suffix());

    print_trace_results(&trace_output, suffix, kind.display_name());
    Ok(())
}

fn run_demo_sample() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        "🩺 Running demo with sample trace data (use --file or --spawn for real analysis)"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "   Tip: shell-doctor demo --file <trace.log>  | Read a captured trace file".dimmed()
    );
    println!(
        "{}",
        "        shell-doctor demo --spawn ~/.zshrc     | Spawn shell and capture live".dimmed()
    );
    println!();

    for kind in [ProfileKind::Zsh, ProfileKind::Bash, ProfileKind::PowerShell] {
        println!(
            "{}",
            format!("── {} Sample ──────────────────────", kind.display_name())
                .bold()
                .dimmed()
        );
        print_trace_results(kind.sample_trace(), kind.suffix(), kind.display_name());
    }
    Ok(())
}

/// トレース文字列を解析してテーブルを表示する共通ロジック
fn print_trace_results(trace_output: &str, suffix: &str, shell_name: &str) {
    let profiles = analyze_trace(trace_output, suffix);

    if profiles.is_empty() {
        println!(
            "{}",
            format!("  ✅ No bottlenecks detected in {shell_name} profile (all commands < 1ms).")
                .green()
        );
        println!();
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Line").add_attribute(Attribute::Bold),
            Cell::new("Duration").add_attribute(Attribute::Bold),
            Cell::new("Command").add_attribute(Attribute::Bold),
        ]);

    let mut total_ms = 0u64;
    for p in &profiles {
        total_ms += p.duration_ms;
        let color = if p.duration_ms > 100 {
            Color::Red
        } else {
            Color::Yellow
        };
        table.add_row(Row::from(vec![
            Cell::new(p.line.to_string()),
            Cell::new(format!("{} ms", p.duration_ms)).fg(color),
            Cell::new(&p.command),
        ]));
    }

    println!("{table}");
    println!(
        "⚡ Total bottleneck delay: {} ms\n",
        total_ms.to_string().red().bold()
    );
}

// ─── PathHealth コマンド ──────────────────────────────────────

fn run_path_health_cmd(
    custom_path: Option<&str>,
    fail_on_issues: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // --path 指定の有無をユーザーに明示
    if let Some(p) = custom_path {
        println!(
            "{}",
            format!("🩺 Diagnosing custom PATH string: {}", p).cyan().bold()
        );
    } else {
        println!(
            "{}",
            "🩺 Diagnosing PATH environment variable...".cyan().bold()
        );
    }
    println!();

    let report = run_path_health_from(custom_path);

    // ── Dead Paths テーブル ──────────────────────────────────
    println!("{}", "Dead Paths".red().bold());
    if report.dead_paths.is_empty() {
        println!("{}\n", "  ✅ No dead paths found.".green());
    } else {
        println!("{}\n", build_dead_paths_table(&report.dead_paths));
    }

    // ── Duplicate Paths テーブル ─────────────────────────────
    println!("{}", "Duplicate Paths".yellow().bold());
    if report.duplicates.is_empty() {
        println!("{}\n", "  ✅ No duplicates found.".green());
    } else {
        println!("{}\n", build_duplicates_table(&report.duplicates));
    }

    // ── Health Score ─────────────────────────────────────────
    let score = compute_health_score(&report);
    let score_colored = match score {
        90..=100 => score.to_string().green().bold(),
        60..=89 => score.to_string().yellow().bold(),
        _ => score.to_string().red().bold(),
    };

    // ── サマリー行（仕様書 §3-3）─────────────────────────────
    println!(
        "⚡ {} {} entries | {} dead | {} duplicates | Skipped (invalid): {} entries",
        "PATH Health Summary:".bold(),
        report.total.to_string().cyan(),
        report.dead_paths.len().to_string().red(),
        report.duplicates.len().to_string().yellow(),
        report.skipped_count.to_string().white(),
    );
    println!("🏥 Health Score: {}/100", score_colored);

    // --fail-on-issues: 問題が1件でもあれば非ゼロで終了
    if fail_on_issues && (!report.dead_paths.is_empty() || !report.duplicates.is_empty()) {
        std::process::exit(1);
    }

    Ok(())
}

/// Dead Paths テーブルを構築して返す
fn build_dead_paths_table(dead_paths: &[path_checker::DeadPathEntry]) -> Table {
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

    for dead in dead_paths {
        let (reason_str, color) = match dead.reason {
            DeadReason::NotFound => ("Not Found", Color::Red),
            DeadReason::NotADirectory => ("Not a Directory", Color::Red),
            DeadReason::PermissionDenied => ("Permission Denied", Color::Magenta),
        };
        table.add_row(Row::from(vec![
            Cell::new(dead.index + 1),
            Cell::new(&dead.raw).fg(color),
            Cell::new(reason_str).fg(color),
        ]));
    }
    table
}

/// Duplicate Paths テーブルを構築して返す
fn build_duplicates_table(duplicates: &[path_checker::DuplicateEntry]) -> Table {
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

    for dup in duplicates {
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
    table
}
