use regex::Regex;
use std::sync::OnceLock;

/// サポートするシェルプロファイルの種別
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileKind {
    Zsh,
    Bash,
    PowerShell,
    /// ファイル拡張子から推定できなかった場合。suffix を直接指定する。
    Other(String),
}

impl ProfileKind {
    /// ファイルパスの末尾から ProfileKind を推定する
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_lowercase();
        if lower.ends_with(".zshrc")
            || lower.ends_with(".zprofile")
            || lower.ends_with(".zshenv")
        {
            ProfileKind::Zsh
        } else if lower.ends_with(".bashrc")
            || lower.ends_with(".bash_profile")
            || lower.ends_with(".profile")
        {
            ProfileKind::Bash
        } else if lower.ends_with(".ps1")
            || lower.ends_with("profile.ps1")
            || lower.ends_with("microsoft.powershell_profile.ps1")
        {
            ProfileKind::PowerShell
        } else {
            // ファイル名そのもの（例: "init"）をサフィックスとして使う
            let suffix = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string();
            ProfileKind::Other(suffix)
        }
    }

    /// `analyze_trace` に渡す `target_file_suffix` 文字列を返す
    pub fn suffix(&self) -> &str {
        match self {
            ProfileKind::Zsh => ".zshrc",
            ProfileKind::Bash => ".bashrc",
            ProfileKind::PowerShell => "profile.ps1",
            ProfileKind::Other(s) => s.as_str(),
        }
    }

    /// ユーザー向け表示名
    pub fn display_name(&self) -> &str {
        match self {
            ProfileKind::Zsh => "Zsh",
            ProfileKind::Bash => "Bash",
            ProfileKind::PowerShell => "PowerShell",
            ProfileKind::Other(_) => "Shell",
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
        // ファイルパスのキャプチャグループを `([^:]+(?::[^:]+)?)` に変更することで
        // Windows のドライブレター（例: C:/path/to/file）を含むパスも正しく解析できる。
        // 具体的には `C:` の後に `/` が続く場合のみドライブレターとして許容する。
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

    // Lesson 4: Property-based testing
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
    }
}