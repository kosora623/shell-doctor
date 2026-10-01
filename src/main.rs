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
use path_checker::{run_path_health, DeadReason};

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
    PathHealth,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo { file, spawn } => run_demo(file, spawn)?,
        Commands::PathHealth => run_path_health_cmd(),
    }

    Ok(())
}

// ─── Demo コマンド ────────────────────────────────────────────

fn run_demo(file: Option<String>, spawn: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // モード選択: --spawn > --file > サンプルデータ
    if let Some(profile_path) = spawn {
        run_demo_spawn(&profile_path)
    } else if let Some(trace_path) = file {
        run_demo_from_file(&trace_path)
    } else {
        run_demo_sample()
    }
}

/// --spawn: headless シェルを起動してリアルタイムにトレースを取得する
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

/// 指定プロファイルを headless シェルで実行し、PS4 タイムスタンプ付きトレースを返す
fn spawn_shell_trace(
    profile_path: &str,
    kind: &ProfileKind,
) -> Result<String, Box<dyn std::error::Error>> {
    let output = match kind {
        ProfileKind::Zsh => {
            // zsh: XTRACE + PS4 の高精度タイムスタンプ形式でプロファイルをソース
            Command::new("zsh")
                .args([
                    "-c",
                    &format!(
                        "PS4='+${{EPOCHREALTIME}} ${{(%):-%x}}:${{LINENO}}: ' zsh --no-rcs -o xtrace -i -c 'source {profile_path}' 2>&1"
                    ),
                ])
                .output()
        }
        ProfileKind::Bash => {
            // bash: PS4 フォーマット + xtrace でプロファイルをソース
            Command::new("bash")
                .args([
                    "-c",
                    &format!(
                        "PS4='+$(date +%s.%N) ${{BASH_SOURCE}}:${{LINENO}}: ' bash --norc -x -c 'source {profile_path}' 2>&1"
                    ),
                ])
                .output()
        }
        ProfileKind::PowerShell => {
            // PowerShell: Set-PSDebug -Trace でスクリプトをトレース
            Command::new("pwsh")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!(
                        "Set-PSDebug -Trace 2; . '{profile_path}'; Set-PSDebug -Off"
                    ),
                ])
                .output()
        }
        ProfileKind::Other(_) => {
            // 拡張子不明: sh として試みる
            Command::new("sh")
                .args([
                    "-c",
                    &format!(
                        "PS4='+$(date +%s.%6N) ${{0}}:${{LINENO}}: ' sh -x '{profile_path}' 2>&1"
                    ),
                ])
                .output()
        }
    };

    match output {
        Ok(out) => {
            if !out.status.success() && out.stdout.is_empty() && out.stderr.is_empty() {
                return Err(format!(
                    "Shell process exited with status: {}",
                    out.status
                )
                .into());
            }
            // stdout + stderr を結合（トレースは stderr に出ることが多い）
            let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            Ok(combined)
        }
        Err(e) => Err(format!("Failed to spawn shell: {e}").into()),
    }
}

/// --file: トレースログファイルを読み込んで解析する
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

    // ファイル名からサフィックスを推定。--file はトレースログなので
    // ファイル名そのものではなくログ内のパスが基準。
    // フォールバックとして Path から末尾のファイル名コンポーネントを使う。
    let suffix = Path::new(trace_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(kind.suffix());

    print_trace_results(&trace_output, suffix, kind.display_name());
    Ok(())
}

/// サンプルデータ（引数なし実行時のデモ）
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

    // Zsh / Bash / PowerShell のサンプルをまとめて表示
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
            format!(
                "  ✅ No bottlenecks detected in {shell_name} profile (all commands < 1ms)."
            )
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

fn run_path_health_cmd() {
    println!(
        "{}",
        "🩺 Diagnosing PATH environment variable...".cyan().bold()
    );
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
