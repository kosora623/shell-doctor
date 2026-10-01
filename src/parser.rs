use regex::Regex;
use std::sync::OnceLock;

/// サポートするシェルプロファイルの種別
///
/// `from_path` で生成した場合、各バリアントは実際のファイル名を内包し
/// `suffix()` でそのファイル名を返す。`from_shell_name` で生成した場合は
/// フォールバックの一般的なファイル名を返す。
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileKind {
    /// Zsh。`file_name` はトレース解析に使う実際のファイル名。
    Zsh { file_name: String },
    /// Bash。`file_name` はトレース解析に使う実際のファイル名。
    Bash { file_name: String },
    /// Fish shell。
    Fish,
    /// PowerShell。`file_name` はトレース解析に使う実際のファイル名。
    PowerShell { file_name: String },
    /// 拡張子から推定できなかった場合。`file_name` をサフィックスとして使う。
    Other { file_name: String },
}

impl ProfileKind {
    /// ファイルパスの末尾から ProfileKind を推定する。
    ///
    /// 各バリアントにファイル名を保持させることで、`suffix()` が正確な
    /// ファイル名（例: `.zprofile`）を返せるようにしている。
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_lowercase();
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string();

        if lower.ends_with(".zshrc")
            || lower.ends_with(".zprofile")
            || lower.ends_with(".zshenv")
        {
            ProfileKind::Zsh { file_name }
        } else if lower.ends_with(".bashrc")
            || lower.ends_with(".bash_profile")
            || lower.ends_with(".profile")
        {
            ProfileKind::Bash { file_name }
        } else if lower.ends_with("config.fish") {
            ProfileKind::Fish
        } else if lower.ends_with(".ps1")
            || lower.ends_with("profile.ps1")
            || lower.ends_with("microsoft.powershell_profile.ps1")
        {
            ProfileKind::PowerShell { file_name }
        } else {
            ProfileKind::Other { file_name }
        }
    }

    /// `--shell` オプションの文字列からシェル種別を生成する。
    ///
    /// ファイル名が不明なため、各バリアントには一般的なデフォルトファイル名を格納する。
    pub fn from_shell_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "zsh" => ProfileKind::Zsh {
                file_name: ".zshrc".into(),
            },
            "bash" | "sh" => ProfileKind::Bash {
                file_name: ".bashrc".into(),
            },
            "fish" => ProfileKind::Fish,
            "powershell" | "pwsh" | "ps" => ProfileKind::PowerShell {
                file_name: "profile.ps1".into(),
            },
            other => ProfileKind::Other {
                file_name: other.to_string(),
            },
        }
    }

    /// `analyze_trace` に渡す `target_file_suffix` 文字列を返す。
    ///
    /// `from_path` 経由の場合は実際のファイル名、`from_shell_name` 経由の場合は
    /// デフォルトのサフィックスを返す。
    pub fn suffix(&self) -> &str {
        match self {
            ProfileKind::Zsh { file_name } => file_name.as_str(),
            ProfileKind::Bash { file_name } => file_name.as_str(),
            ProfileKind::Fish => "config.fish",
            ProfileKind::PowerShell { file_name } => file_name.as_str(),
            ProfileKind::Other { file_name } => file_name.as_str(),
        }
    }

    /// ユーザー向け表示名
    pub fn display_name(&self) -> &str {
        match self {
            ProfileKind::Zsh { .. } => "Zsh",
            ProfileKind::Bash { .. } => "Bash",
            ProfileKind::Fish => "Fish",
            ProfileKind::PowerShell { .. } => "PowerShell",
            ProfileKind::Other { .. } => "Shell",
        }
    }

    /// headless 起動時のシェル実行ファイル名
    pub fn shell_binary(&self) -> &str {
        match self {
            ProfileKind::Zsh { .. } => "zsh",
            ProfileKind::Bash { .. } => "bash",
            ProfileKind::Fish => "fish",
            ProfileKind::PowerShell { .. } => "pwsh",
            ProfileKind::Other { .. } => "sh",
        }
    }

    /// 各シェル種別向けのデモ用サンプルトレース文字列を返す
    pub fn sample_trace(&self) -> &'static str {
        match self {
            ProfileKind::Zsh { .. } => {
                r#"
+1727670000.000000 /home/user/.zshrc:1: export PATH=/usr/local/bin:$PATH
+1727670000.005000 /home/user/.zshrc:5: alias g=git
+1727670000.010000 /home/user/.zshrc:12: eval "$(brew shellenv)"
+1727670000.195000 /home/user/.zshrc:24: eval "$(pyenv init -)"
+1727670000.415000 /home/user/.zshrc:40: source ~/.fzf.zsh
+1727670000.440000 /home/user/.zshrc:50: echo 'Ready!'
"#
            }
            ProfileKind::Bash { .. } => {
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
            ProfileKind::PowerShell { .. } => {
                r#"
+1727670000.000000 C:/Users/user/Documents/PowerShell/profile.ps1:1: $env:PATH += ";C:\tools\bin"
+1727670000.004000 C:/Users/user/Documents/PowerShell/profile.ps1:5: Set-Alias g git
+1727670000.009000 C:/Users/user/Documents/PowerShell/profile.ps1:12: Invoke-Expression (& starship init powershell)
+1727670000.219000 C:/Users/user/Documents/PowerShell/profile.ps1:20: Import-Module posh-git
+1727670000.439000 C:/Users/user/Documents/PowerShell/profile.ps1:35: Import-Module PSReadLine
+1727670000.595000 C:/Users/user/Documents/PowerShell/profile.ps1:45: Write-Host 'Ready!'
"#
            }
            ProfileKind::Other { .. } => {
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

            // 負の差分（タイムスタンプ逆順）はスキップして u64 オーバーフローを防ぐ
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
    fn from_path_zshrc_returns_zsh_with_correct_suffix() {
        let kind = ProfileKind::from_path("/home/user/.zshrc");
        assert!(matches!(kind, ProfileKind::Zsh { .. }));
        assert_eq!(kind.suffix(), ".zshrc");
        assert_eq!(kind.display_name(), "Zsh");
    }

    #[test]
    fn from_path_zprofile_returns_zsh_with_zprofile_suffix() {
        // バグ2修正検証: .zprofile は suffix が ".zprofile" になる（旧: ".zshrc" 固定）
        let kind = ProfileKind::from_path("/home/user/.zprofile");
        assert!(matches!(kind, ProfileKind::Zsh { .. }));
        assert_eq!(kind.suffix(), ".zprofile");
    }

    #[test]
    fn from_path_bashrc_returns_bash_with_correct_suffix() {
        let kind = ProfileKind::from_path("/home/user/.bashrc");
        assert!(matches!(kind, ProfileKind::Bash { .. }));
        assert_eq!(kind.suffix(), ".bashrc");
        assert_eq!(kind.display_name(), "Bash");
    }

    #[test]
    fn from_path_bash_profile_returns_bash_with_correct_suffix() {
        let kind = ProfileKind::from_path("/home/user/.bash_profile");
        assert!(matches!(kind, ProfileKind::Bash { .. }));
        assert_eq!(kind.suffix(), ".bash_profile");
    }

    #[test]
    fn from_path_fish_returns_fish_kind() {
        let kind = ProfileKind::from_path("/home/user/.config/fish/config.fish");
        assert_eq!(kind, ProfileKind::Fish);
        assert_eq!(kind.suffix(), "config.fish");
        assert_eq!(kind.display_name(), "Fish");
    }

    #[test]
    fn from_path_ps1_returns_powershell_with_correct_suffix() {
        let kind = ProfileKind::from_path("C:/Users/user/Documents/PowerShell/profile.ps1");
        assert!(matches!(kind, ProfileKind::PowerShell { .. }));
        assert_eq!(kind.suffix(), "profile.ps1");
        assert_eq!(kind.display_name(), "PowerShell");
    }

    #[test]
    fn from_shell_name_zsh_returns_zsh_variant() {
        let kind = ProfileKind::from_shell_name("zsh");
        assert!(matches!(kind, ProfileKind::Zsh { .. }));
        assert_eq!(kind.shell_binary(), "zsh");
    }

    #[test]
    fn from_shell_name_bash_returns_bash_variant() {
        let kind = ProfileKind::from_shell_name("bash");
        assert!(matches!(kind, ProfileKind::Bash { .. }));
    }

    #[test]
    fn from_shell_name_fish_returns_fish_variant() {
        assert_eq!(ProfileKind::from_shell_name("fish"), ProfileKind::Fish);
    }

    #[test]
    fn from_shell_name_powershell_variants() {
        for name in ["powershell", "pwsh", "ps"] {
            let kind = ProfileKind::from_shell_name(name);
            assert!(matches!(kind, ProfileKind::PowerShell { .. }), "{name}");
        }
    }

    // ── analyze_trace ユニットテスト ──────────────────────────

    #[test]
    fn analyze_trace_empty_when_fewer_than_two_records() {
        let single = "+1727670000.000000 /home/user/.zshrc:1: export PATH=/usr/bin\n";
        assert!(analyze_trace(single, ".zshrc").is_empty());
        assert!(analyze_trace("", ".zshrc").is_empty());
    }

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

    #[test]
    fn analyze_trace_skips_negative_timestamp_diff() {
        let trace = r#"
+1727670000.200000 /home/user/.zshrc:1: cmd_a
+1727670000.000000 /home/user/.zshrc:2: cmd_b_backwards
+1727670000.500000 /home/user/.zshrc:3: cmd_c
"#;
        let profiles = analyze_trace(trace, ".zshrc");
        assert!(profiles.iter().all(|p| p.command != "cmd_a"));
    }

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

        #[test]
        fn test_analyze_trace_never_panics(s in ".*", suffix in "[a-z.]{1,10}") {
            let _ = analyze_trace(&s, &suffix);
        }

        /// duration_ms は負タイムスタンプ差でオーバーフローしない
        #[test]
        fn test_analyze_trace_duration_never_overflows(
            t1 in 0.0f64..2_000_000_000.0f64,
            t2 in 0.0f64..2_000_000_000.0f64,
            line in 1usize..100usize,
        ) {
            let trace = format!(
                "+{t1:.6} /tmp/.zshrc:{line}: cmd_a\n+{t2:.6} /tmp/.zshrc:{}: cmd_b\n",
                line + 1
            );
            // パニックせず、負差分はスキップされること
            let _ = analyze_trace(&trace, ".zshrc");
        }
    }
}
