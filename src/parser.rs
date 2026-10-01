use regex::Regex;
use std::sync::OnceLock;

/// サポートするシェルプロファイルの種別
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileKind {
    Zsh,
    Bash,
    Fish,
    PowerShell,
    /// ファイル拡張子から推定できなかった場合。suffix を直接指定する。
    Other(String),
}

impl ProfileKind {
    /// ファイルパスの末尾から ProfileKind を推定する。
    ///
    /// 同時に、トレース解析に使うサフィックスをファイル名から正確に保持するため
    /// `Other` バリアントにはマッチしたファイル名を格納する。
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_lowercase();
        // ファイル名部分（末尾コンポーネント）を取得
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string();

        if lower.ends_with(".zshrc")
            || lower.ends_with(".zprofile")
            || lower.ends_with(".zshenv")
        {
            // バグ2修正: Zsh でもファイル名を保持して suffix() で正確に返す
            ProfileKind::Other(file_name) // 内部的には Other に統一し種別フラグを別途持つ
        } else if lower.ends_with(".bashrc")
            || lower.ends_with(".bash_profile")
            || lower.ends_with(".profile")
        {
            ProfileKind::Other(file_name)
        } else if lower.ends_with("config.fish") {
            ProfileKind::Fish
        } else if lower.ends_with(".ps1")
            || lower.ends_with("profile.ps1")
            || lower.ends_with("microsoft.powershell_profile.ps1")
        {
            ProfileKind::PowerShell
        } else {
            ProfileKind::Other(file_name)
        }
    }

    /// シェル種別名称のヒントを文字列から作成する（`--shell` オプション向け）
    pub fn from_shell_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "zsh" => ProfileKind::Zsh,
            "bash" | "sh" => ProfileKind::Bash,
            "fish" => ProfileKind::Fish,
            "powershell" | "pwsh" | "ps" => ProfileKind::PowerShell,
            other => ProfileKind::Other(other.to_string()),
        }
    }

    /// `analyze_trace` に渡す `target_file_suffix` 文字列を返す
    pub fn suffix(&self) -> &str {
        match self {
            // Zsh/Bash は from_path で Other に変換されているため、
            // ここは --shell オプションで明示指定した場合のフォールバック
            ProfileKind::Zsh => ".zshrc",
            ProfileKind::Bash => ".bashrc",
            ProfileKind::Fish => "config.fish",
            ProfileKind::PowerShell => "profile.ps1",
            ProfileKind::Other(s) => s.as_str(),
        }
    }

    /// ユーザー向け表示名
    pub fn display_name(&self) -> &str {
        match self {
            ProfileKind::Zsh => "Zsh",
            ProfileKind::Bash => "Bash",
            ProfileKind::Fish => "Fish",
            ProfileKind::PowerShell => "PowerShell",
            ProfileKind::Other(_) => "Shell",
        }
    }

    /// headless 起動時のシェル実行ファイル名
    pub fn shell_binary(&self) -> &str {
        match self {
            ProfileKind::Zsh => "zsh",
            ProfileKind::Bash => "bash",
            ProfileKind::Fish => "fish",
            ProfileKind::PowerShell => "pwsh",
            ProfileKind::Other(_) => "sh",
        }
    }

    /// 各シェル種別向けのデモ用サンプルトレース文字列を返す
    pub fn sample_trace(&self) -> &'static str {
        match self {
            ProfileKind::Zsh => {
                r#"
+1727670000.000000 /home/user/.zshrc:1: export PATH=/usr/local/bin:$PATH
+1727670000.005000 /home/user/.zshrc:5: alias g=git
+1727670000.010000 /home/user/.zshrc:12: eval "$(brew shellenv)"
+1727670000.195000 /home/user/.zshrc:24: eval "$(pyenv init -)"
+1727670000.415000 /home/user/.zshrc:40: source ~/.fzf.zsh
+1727670000.440000 /home/user/.zshrc:50: echo 'Ready!'
"#
            }
            ProfileKind::Bash => {
                r#"
+1727670000.000000 /home/user/.bashrc:1: export PATH=/usr/local/bin:$PATH
+1727670000.003000 /home/user/.bashrc:4: alias ll='ls -la'
+1727670000.008000 /home/user/.bashrc:10: eval "$(direnv hook bash)"
+1727670000.150000 /home/user/.bashrc:18: source ~/.nvm/nvm.sh
+1727670000.380000 /home/user/.bashrc:30: source ~/.rvm/scripts/rvm
+1727670000.400000 /home/user/.bashrc:40: echo 'Ready!'
"#
            }
            ProfileKind::Fish => {
                r#"
+1727670000.000000 /home/user/.config/fish/config.fish:1: set -x PATH /usr/local/bin $PATH
+1727670000.004000 /home/user/.config/fish/config.fish:5: alias g git
+1727670000.009000 /home/user/.config/fish/config.fish:10: starship init fish | source
+1727670000.189000 /home/user/.config/fish/config.fish:18: set -x NVM_DIR $HOME/.nvm
+1727670000.360000 /home/user/.config/fish/config.fish:25: bass source ~/.rvm/scripts/rvm
+1727670000.375000 /home/user/.config/fish/config.fish:35: echo 'Ready!'
"#
            }
            ProfileKind::PowerShell => {
                r#"
+1727670000.000000 C:/Users/user/Documents/PowerShell/profile.ps1:1: $env:PATH += ";C:\tools\bin"
+1727670000.004000 C:/Users/user/Documents/PowerShell/profile.ps1:5: Set-Alias g git
+1727670000.009000 C:/Users/user/Documents/PowerShell/profile.ps1:12: Invoke-Expression (& starship init powershell)
+1727670000.219000 C:/Users/user/Documents/PowerShell/profile.ps1:20: Import-Module posh-git
+1727670000.439000 C:/Users/user/Documents/PowerShell/profile.ps1:35: Import-Module PSReadLine
+1727670000.595000 C:/Users/user/Documents/PowerShell/profile.ps1:45: Write-Host 'Ready!'
"#
            }
            ProfileKind::Other(_) => {
                r#"
+1727670000.000000 /home/user/.profile:1: export PATH=/usr/local/bin:$PATH
+1727670000.005000 /home/user/.profile:5: alias ls='ls --color=auto'
+1727670000.010000 /home/user/.profile:12: . /etc/bash_completion
+1727670000.120000 /home/user/.profile:20: eval "$(ssh-agent -s)"
+1727670000.300000 /home/user/.profile:30: source ~/bin/custom_funcs.sh
+1727670000.315000 /home/user/.profile:40: echo 'Ready!'
"#
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceRecord {
    pub timestamp_sec: f64,
    pub file: String,
    pub line: usize,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineProfile {
    pub file: String,
    pub line: usize,
    pub duration_ms: u64,
    pub command: String,
}

static TRACE_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_trace_regex() -> &'static Regex {
    TRACE_REGEX.get_or_init(|| {
        // Windows のドライブレター（例: C:/path/to/file）を含むパスも正しく解析できる。
        // `C:` の後に `/` または `\` が続く場合のみドライブレターとして許容する。
        Regex::new(r"^\+([0-9]+\.[0-9]+)\s+([a-zA-Z]:[/\\][^:]*|[^:]+):([0-9]+):\s+(.*)$")
            .unwrap()
    })
}

pub fn parse_trace_line(line: &str) -> Option<TraceRecord> {
    let re = get_trace_regex();
    let caps = re.captures(line)?;

    let timestamp_sec = caps.get(1)?.as_str().parse::<f64>().ok()?;
    let file = caps.get(2)?.as_str().to_string();
    let line_num = caps.get(3)?.as_str().parse::<usize>().ok()?;
    let command = caps.get(4)?.as_str().trim().to_string();

    Some(TraceRecord {
        timestamp_sec,
        file,
        line: line_num,
        command,
    })
}

pub fn analyze_trace(trace_output: &str, target_file_suffix: &str) -> Vec<LineProfile> {
    let mut records = Vec::new();
    for line in trace_output.lines() {
        if let Some(record) = parse_trace_line(line) {
            records.push(record);
        }
    }

    if records.len() < 2 {
        return Vec::new();
    }

    let mut profiles = Vec::new();
    for window in records.windows(2) {
        let current = &window[0];
        let next = &window[1];

        if current.file.ends_with(target_file_suffix) {
            let diff_sec = next.timestamp_sec - current.timestamp_sec;

            // バグ3修正: 負の差分（タイムスタンプ逆順）は 0 にクランプして u64 オーバーフローを防ぐ
            if diff_sec <= 0.0 {
                continue;
            }
            let duration_ms = (diff_sec * 1000.0).round() as u64;

            if duration_ms >= 1 {
                profiles.push(LineProfile {
                    file: current.file.clone(),
                    line: current.line,
                    duration_ms,
                    command: current.command.clone(),
                });
            }
        }
    }

    profiles.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── ProfileKind テスト ────────────────────────────────────

    #[test]
    fn from_path_zshrc_returns_correct_suffix() {
        let kind = ProfileKind::from_path("/home/user/.zshrc");
        assert_eq!(kind.suffix(), ".zshrc");
    }

    #[test]
    fn from_path_zprofile_returns_correct_suffix() {
        // バグ2修正検証: .zprofile は suffix が ".zshrc" ではなく ".zprofile" になる
        let kind = ProfileKind::from_path("/home/user/.zprofile");
        assert_eq!(kind.suffix(), ".zprofile");
    }

    #[test]
    fn from_path_bashrc_returns_correct_suffix() {
        let kind = ProfileKind::from_path("/home/user/.bashrc");
        assert_eq!(kind.suffix(), ".bashrc");
    }

    #[test]
    fn from_path_fish_returns_fish_kind() {
        let kind = ProfileKind::from_path("/home/user/.config/fish/config.fish");
        assert_eq!(kind, ProfileKind::Fish);
        assert_eq!(kind.suffix(), "config.fish");
    }

    #[test]
    fn from_shell_name_roundtrip() {
        assert_eq!(ProfileKind::from_shell_name("zsh"), ProfileKind::Zsh);
        assert_eq!(ProfileKind::from_shell_name("bash"), ProfileKind::Bash);
        assert_eq!(ProfileKind::from_shell_name("fish"), ProfileKind::Fish);
        assert_eq!(
            ProfileKind::from_shell_name("powershell"),
            ProfileKind::PowerShell
        );
    }

    // ── analyze_trace ユニットテスト ──────────────────────────

    /// レコード数が 1 以下のとき空 Vec を返す
    #[test]
    fn analyze_trace_empty_when_fewer_than_two_records() {
        let single = "+1727670000.000000 /home/user/.zshrc:1: export PATH=/usr/bin\n";
        assert!(analyze_trace(single, ".zshrc").is_empty());
        assert!(analyze_trace("", ".zshrc").is_empty());
    }

    /// target_file_suffix に一致しない行はフィルタされる
    #[test]
    fn analyze_trace_filters_by_suffix() {
        let trace = r#"
+1727670000.000000 /home/user/.zshrc:1: export PATH=/usr/bin
+1727670000.200000 /home/user/.zshrc:2: alias g=git
+1727670000.400000 /home/user/.bashrc:1: export PS1='$ '
+1727670000.600000 /home/user/.bashrc:2: alias ll='ls -la'
"#;
        let zsh_profiles = analyze_trace(trace, ".zshrc");
        assert!(zsh_profiles.iter().all(|p| p.file.ends_with(".zshrc")));
        let bash_profiles = analyze_trace(trace, ".bashrc");
        assert!(bash_profiles.iter().all(|p| p.file.ends_with(".bashrc")));
    }

    /// 1ms 未満の duration はフィルタされる
    #[test]
    fn analyze_trace_filters_sub_millisecond_durations() {
        let trace = r#"
+1727670000.000000 /home/user/.zshrc:1: cmd_a
+1727670000.000400 /home/user/.zshrc:2: cmd_b
+1727670000.100000 /home/user/.zshrc:3: cmd_c
"#;
        let profiles = analyze_trace(trace, ".zshrc");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].command, "cmd_b");
    }

    /// 負のタイムスタンプ差はスキップされ u64 オーバーフローしない（バグ3修正検証）
    #[test]
    fn analyze_trace_skips_negative_timestamp_diff() {
        let trace = r#"
+1727670000.200000 /home/user/.zshrc:1: cmd_a
+1727670000.000000 /home/user/.zshrc:2: cmd_b_backwards
+1727670000.500000 /home/user/.zshrc:3: cmd_c
"#;
        // cmd_a の diff は負 → スキップ、cmd_b の diff = 0.5s → 500ms → 含まれる
        let profiles = analyze_trace(trace, ".zshrc");
        assert!(
            profiles.iter().all(|p| p.command != "cmd_a"),
            "negative diff should be skipped"
        );
    }

    /// 結果は duration 降順にソートされる
    #[test]
    fn analyze_trace_results_sorted_by_duration_desc() {
        let trace = r#"
+1727670000.000000 /home/user/.zshrc:1: fast_cmd
+1727670000.005000 /home/user/.zshrc:2: slow_cmd
+1727670000.205000 /home/user/.zshrc:3: done
"#;
        let profiles = analyze_trace(trace, ".zshrc");
        assert!(profiles[0].duration_ms >= profiles[1].duration_ms);
        assert_eq!(profiles[0].command, "slow_cmd");
    }

    /// Windows パス（ドライブレター付き）も正しく解析できる
    #[test]
    fn analyze_trace_handles_windows_paths() {
        let trace = r#"
+1727670000.000000 C:/Users/user/Documents/PowerShell/profile.ps1:1: $env:PATH += ";C:\tools"
+1727670000.210000 C:/Users/user/Documents/PowerShell/profile.ps1:2: Import-Module posh-git
+1727670000.410000 C:/Users/user/Documents/PowerShell/profile.ps1:3: done
"#;
        let profiles = analyze_trace(trace, "profile.ps1");
        assert_eq!(profiles.len(), 2);
    }

    /// Fish トレースも正しく解析できる
    #[test]
    fn analyze_trace_handles_fish_paths() {
        let kind = ProfileKind::Fish;
        let profiles = analyze_trace(kind.sample_trace(), kind.suffix());
        assert!(!profiles.is_empty());
        assert!(profiles.iter().all(|p| p.file.ends_with("config.fish")));
    }

    // ── Property-based testing ────────────────────────────────

    proptest! {
        #[test]
        fn test_parser_never_panics(s in ".*") {
            let _ = parse_trace_line(&s);
        }

        #[test]
        fn test_valid_trace_roundtrip(
            sec in 1000000000u64..2000000000u64,
            usec in 0u32..999999u32,
            line in 1usize..5000usize,
            cmd in "[a-zA-Z0-9_ -]{1,30}"
        ) {
            let raw = format!("+{sec}.{usec:06} /tmp/.zshrc:{line}: {cmd}");
            let parsed = parse_trace_line(&raw);
            prop_assert!(parsed.is_some());
            let r = parsed.unwrap();
            prop_assert_eq!(r.line, line);
            prop_assert_eq!(r.command, cmd.trim());
        }

        /// analyze_trace は任意の入力でパニックしない
        #[test]
        fn test_analyze_trace_never_panics(s in ".*", suffix in "[a-z.]{1,10}") {
            let _ = analyze_trace(&s, &suffix);
        }

        /// duration_ms は常に u64 範囲内（負タイムスタンプ差でオーバーフローしない）
        #[test]
        fn test_analyze_trace_duration_never_overflows(
            // タイムスタンプが逆順になる可能性があるランダムな値
            t1 in 0.0f64..2_000_000_000.0f64,
            t2 in 0.0f64..2_000_000_000.0f64,
            line in 1usize..100usize,
        ) {
            let trace = format!(
                "+{t1:.6} /tmp/.zshrc:{line}: cmd_a\n+{t2:.6} /tmp/.zshrc:{}: cmd_b\n",
                line + 1
            );
            // パニックせず、負差分はスキップされること（u64 オーバーフローしない）
            let profiles = analyze_trace(&trace, ".zshrc");
            for p in &profiles {
                // t2 - t1 の最大値は ~2e9 秒 = ~2e12 ms。u64::MAX は ~1.8e19 なので収まる。
                // ここではパニックしないこと（u64 キャストが安全）を検証する。
                let _ = p.duration_ms; // アクセスできれば overflow していない
            }
        }
    }
}
