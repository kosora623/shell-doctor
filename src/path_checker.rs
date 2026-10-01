use rayon::prelude::*;
use std::collections::HashMap;
use std::io;
use std::path::Path;

// ─── データ構造 ───────────────────────────────────────────────

/// Dead Path の原因区分（REQ-002）
#[derive(Debug, Clone, PartialEq)]
pub enum DeadReason {
    /// ファイルシステム上に存在しない
    NotFound,
    /// 存在するがディレクトリではない（ファイルなど）
    NotADirectory,
    /// アクセス権限がなく metadata を取得できない
    PermissionDenied,
}

/// Dead Path エントリ（REQ-002）
#[derive(Debug, Clone)]
pub struct DeadPathEntry {
    /// 元の PATH における 0-indexed 位置
    pub index: usize,
    /// トリム済みの生文字列
    pub raw: String,
    /// Dead と判定された理由
    pub reason: DeadReason,
}

/// 重複エントリ（REQ-003）
#[derive(Debug, Clone)]
pub struct DuplicateEntry {
    /// 正規化済みパス、またはフォールバックとして lowercase 文字列
    pub canonical: String,
    /// 出現順インデックスのリスト（先頭が最初の出現）
    pub occurrences: Vec<usize>,
}

/// PATH 診断結果全体（仕様書 §4）
#[derive(Debug, Clone)]
pub struct PathHealthReport {
    pub total: usize,
    pub skipped_count: usize,
    pub dead_paths: Vec<DeadPathEntry>,
    pub duplicates: Vec<DuplicateEntry>,
}

// ─── 純粋関数 ─────────────────────────────────────────────────

/// PATH 文字列をセパレータで分割し、トリム済みの有効エントリを返す（REQ-001）
///
/// - 空エントリや null バイトを含むエントリは除去する（REQ-004）
/// - I/O を一切行わない純粋関数
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn split_path_entries(raw_path: &str, separator: char) -> Vec<String> {
    raw_path
        .split(separator)
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty() && !e.contains('\0'))
        .collect()
}

/// OS ネイティブのセパレータで PATH を取得・分割し、完全な診断レポートを返す。
///
/// `std::env::var("PATH")` が `Err` の場合は空エントリとして扱う（REQ-001）。
/// `run_path_health_from(None)` の便利ラッパー。
#[allow(dead_code)]
pub(crate) fn run_path_health() -> PathHealthReport {
    run_path_health_from(None)
}

/// カスタム PATH 文字列を受け取って診断する。
///
/// `custom_path` が `Some` のときはその文字列を、`None` のときは環境変数 `PATH` を使う。
/// CI スクリプトや `--path` オプションからの呼び出し向け。
pub fn run_path_health_from(custom_path: Option<&str>) -> PathHealthReport {
    #[cfg(target_os = "windows")]
    let separator = ';';
    #[cfg(not(target_os = "windows"))]
    let separator = ':';

    let raw = match custom_path {
        Some(p) => p.to_string(),
        None => std::env::var("PATH").unwrap_or_default(),
    };

    let all_raw: Vec<String> = raw.split(separator).map(|e| e.trim().to_string()).collect();

    // REQ-004: 空エントリ・null バイト含みをスキップ
    let mut skipped_count = 0usize;
    let mut valid_entries: Vec<String> = Vec::new();
    for entry in &all_raw {
        if entry.is_empty() || entry.contains('\0') {
            skipped_count += 1;
        } else {
            valid_entries.push(entry.clone());
        }
    }

    let mut report = run_path_health_on(&valid_entries);
    report.skipped_count = skipped_count;
    report
}

/// テスト・再利用向け：エントリスライスを直接受け取って診断レポートを生成する純粋関数。
///
/// REQ-002 の補足仕様に従い、`std::fs::metadata` の呼び出しを `rayon` で並列化する。
/// 出力の順序はオリジナルの PATH 出現順を保持する。
pub fn run_path_health_on(entries: &[String]) -> PathHealthReport {
    let total = entries.len();

    // REQ-002: Dead Path 検出（rayon で並列化、出現順を保持するため index 付き）
    let mut dead_paths: Vec<DeadPathEntry> = entries
        .par_iter()
        .enumerate()
        .filter_map(|(index, raw)| match std::fs::metadata(raw) {
            Ok(meta) if meta.is_dir() => None, // 正常なディレクトリ
            Ok(_) => Some(DeadPathEntry {
                index,
                raw: raw.clone(),
                reason: DeadReason::NotADirectory,
            }),
            Err(e) => {
                let reason = if e.kind() == io::ErrorKind::PermissionDenied {
                    DeadReason::PermissionDenied
                } else {
                    DeadReason::NotFound
                };
                Some(DeadPathEntry {
                    index,
                    raw: raw.clone(),
                    reason,
                })
            }
        })
        .collect();

    // 並列処理後に出現順（index 昇順）へ並び替え
    dead_paths.sort_by_key(|d| d.index);

    // REQ-003: Duplicate 検出（canonical キー → 出現インデックス Vec）
    let mut seen: HashMap<String, (String, Vec<usize>)> = HashMap::new();
    for (index, raw) in entries.iter().enumerate() {
        let key = normalize_key(raw);
        seen.entry(key.clone())
            .and_modify(|e| e.1.push(index))
            .or_insert_with(|| (key, vec![index]));
    }

    let mut duplicates: Vec<DuplicateEntry> = seen
        .into_values()
        .filter(|(_, occ)| occ.len() >= 2)
        .map(|(canonical, occurrences)| DuplicateEntry {
            canonical,
            occurrences,
        })
        .collect();

    // 最初の出現インデックス昇順でソート
    duplicates.sort_by_key(|d| d.occurrences[0]);

    PathHealthReport {
        total,
        skipped_count: 0,
        dead_paths,
        duplicates,
    }
}

/// Health Score を計算する。
///
/// Dead Path と Duplicate の重み付け:
///   - Dead Path: ペナルティ 2 ポイント（存在しないパスはより深刻）
///   - Duplicate:  ペナルティ 1 ポイント（冗長だが実害は少ない）
///
/// score = 100 - (dead * 2 + dup * 1) * 100 / (total * 2) を上限 100 で clamp。
/// total が 0 のとき 100 を返す。
pub fn compute_health_score(report: &PathHealthReport) -> u32 {
    if report.total == 0 {
        return 100;
    }
    // 最大ペナルティは全エントリが Dead の場合（= total * 2 ポイント）
    let max_penalty = report.total * 2;
    let penalty = report.dead_paths.len() * 2 + report.duplicates.len();
    let penalty_pct = (penalty * 100) / max_penalty;
    100u32.saturating_sub(penalty_pct as u32)
}

/// REQ-003: パスの正規化キーを生成する。
///
/// `canonicalize` が成功すればその文字列、失敗（存在しない等）なら lowercase フォールバック。
fn normalize_key(raw: &str) -> String {
    Path::new(raw)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_else(|_| raw.to_lowercase())
}

// ─── テスト ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── ユニットテスト ──

    #[test]
    fn split_removes_empty_and_whitespace() {
        let entries = split_path_entries("/usr/bin::/usr/local/bin: :/sbin", ':');
        assert_eq!(entries, vec!["/usr/bin", "/usr/local/bin", "/sbin"]);
    }

    #[test]
    fn split_removes_null_bytes() {
        let entries = split_path_entries("/usr/bin\0:/sbin", ':');
        assert_eq!(entries, vec!["/sbin"]);
    }

    #[test]
    fn split_windows_semicolon() {
        let entries = split_path_entries(r"C:\Windows\System32;C:\Windows", ';');
        assert_eq!(entries, vec![r"C:\Windows\System32", r"C:\Windows"]);
    }

    #[test]
    fn split_empty_path_returns_empty() {
        let entries = split_path_entries("", ':');
        assert!(entries.is_empty());
    }

    #[test]
    fn duplicate_detection_finds_two_occurrences() {
        let entries = vec![
            "/fake/path/abc".to_string(),
            "/fake/path/def".to_string(),
            "/fake/path/abc".to_string(),
        ];
        let report = run_path_health_on(&entries);
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].occurrences.len(), 2);
    }

    #[test]
    fn no_duplicates_when_all_unique() {
        let entries = vec![
            "/fake/path/aaa".to_string(),
            "/fake/path/bbb".to_string(),
            "/fake/path/ccc".to_string(),
        ];
        let report = run_path_health_on(&entries);
        assert!(report.duplicates.is_empty());
    }

    #[test]
    fn dead_paths_preserve_original_order() {
        // 並列処理後も index 昇順で返ることを確認
        let entries: Vec<String> = (0..10)
            .map(|i| format!("/nonexistent_rayon_test_{i}"))
            .collect();
        let report = run_path_health_on(&entries);
        let indices: Vec<usize> = report.dead_paths.iter().map(|d| d.index).collect();
        let mut sorted = indices.clone();
        sorted.sort();
        assert_eq!(indices, sorted, "dead_paths should be sorted by index");
    }

    #[test]
    fn compute_health_score_perfect_when_empty() {
        let report = PathHealthReport {
            total: 0,
            skipped_count: 0,
            dead_paths: vec![],
            duplicates: vec![],
        };
        assert_eq!(compute_health_score(&report), 100);
    }

    #[test]
    fn compute_health_score_dead_penalizes_more_than_dup() {
        let base = PathHealthReport {
            total: 10,
            skipped_count: 0,
            dead_paths: vec![],
            duplicates: vec![],
        };
        // dead 1 件
        let with_dead = PathHealthReport {
            dead_paths: vec![DeadPathEntry {
                index: 0,
                raw: "/x".into(),
                reason: DeadReason::NotFound,
            }],
            ..base.clone()
        };
        // dup 1 件
        let with_dup = PathHealthReport {
            duplicates: vec![DuplicateEntry {
                canonical: "/x".into(),
                occurrences: vec![0, 1],
            }],
            ..base
        };
        assert!(
            compute_health_score(&with_dead) < compute_health_score(&with_dup),
            "dead path should penalize more than duplicate"
        );
    }

    // ── プロパティベーステスト（アーキテクチャ規則 §3）──

    proptest! {
        #[test]
        fn split_never_panics(s in ".*") {
            let _ = split_path_entries(&s, ':');
            let _ = split_path_entries(&s, ';');
        }

        #[test]
        fn split_never_panics_edge_cases(s in "([^:]{0,50}:){0,20}[^:]{0,50}") {
            let _ = split_path_entries(&s, ':');
        }

        #[test]
        fn run_path_health_on_never_panics(
            entries in prop::collection::vec(".*", 0..20),
        ) {
            let _ = run_path_health_on(&entries);
        }

        #[test]
        fn run_path_health_on_never_panics_with_special_chars(
            entries in prop::collection::vec(
                "[\\x20-\\x7e\\t\\n\\\\]{0,60}",
                0..15,
            ),
        ) {
            let _ = run_path_health_on(&entries);
        }

        #[test]
        fn split_roundtrip_preserves_count(
            raw_entries in prop::collection::vec(
                "[^:;\x00\\s][^:;\x00]{0,38}[^:;\x00\\s]|[^:;\x00\\s]",
                0..15usize,
            ),
        ) {
            let mut seen = std::collections::HashSet::new();
            let unique: Vec<String> = raw_entries
                .into_iter()
                .filter(|e| seen.insert(e.to_lowercase()))
                .collect();
            let expected = unique.len();
            let joined = unique.join(":");
            let reparsed = split_path_entries(&joined, ':');
            prop_assert_eq!(
                reparsed.len(),
                expected,
                "roundtrip failed: joined={:?}, reparsed={:?}",
                joined,
                reparsed,
            );
        }

        #[test]
        fn split_result_has_no_empty_or_null(s in "[^\x00]{0,200}") {
            for sep in [':', ';'] {
                let entries = split_path_entries(&s, sep);
                for e in &entries {
                    prop_assert!(!e.is_empty(), "empty entry found");
                    prop_assert!(!e.contains('\0'), "null byte found");
                }
            }
        }

        #[test]
        fn split_entries_are_trimmed(s in "[^\x00]{0,200}") {
            for sep in [':', ';'] {
                let entries = split_path_entries(&s, sep);
                for e in &entries {
                    prop_assert_eq!(e.as_str(), e.trim(), "entry is not trimmed: {:?}", e);
                }
            }
        }

        #[test]
        fn health_score_always_in_range(
            entries in prop::collection::vec(
                "/nonexistent_proptest_[a-z]{1,8}/[a-z]{1,8}",
                0..10usize,
            ),
        ) {
            let report = run_path_health_on(&entries);
            let score = compute_health_score(&report);
            prop_assert!(score <= 100, "score out of range: {}", score);
        }
    }
}
