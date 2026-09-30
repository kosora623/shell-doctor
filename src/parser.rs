use regex::Regex;
use std::sync::OnceLock;

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
        Regex::new(r"^\+([0-9]+\.[0-9]+)\s+([^:]+):([0-9]+):\s+(.*)$").unwrap()
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