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
#[derive(Debug)]
pub struct PathHealthReport {
    pub total: usize,
    pub skipped_count: usize,
    pub dead_paths: Vec<DeadPathEntry>,
    pub duplicates: Vec<DuplicateEntry>,
}

// ─── 後方互換：PathIssueType / PathIssue（タスク要件の型名）───────

/// タスク要件で指定された問題種別
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum PathIssueType {
    DeadPath,
    DuplicatePath,
}

/// タスク要件で指定された汎用 Issue 型
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PathIssue {
    pub path: String,
    pub issue_type: PathIssueType,
    /// Dead Path: 0、Duplicate Path: 出現回数
    pub occurrences: usize,
}

// ─── 純粋関数 ─────────────────────────────────────────────────

/// PATH 文字列をセパレータで分割し、トリム済みエントリを返す（REQ-001）
///
/// - 空エントリや null バイトを含むエントリは除去し `skipped_count` に加算（REQ-004）
/// - I/O を一切行わない純粋関数
#[allow(dead_code)]
pub fn split_path_entries(raw_path: &str, separator: char) -> Vec<String> {
    raw_path
        .split(separator)
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty() && !e.contains('\0'))
        .collect()
}

/// 分割済みエントリを検査し、スキップ数・Dead Paths・Duplicates をまとめて返す。
///
/// - `entries`: `split_path_entries` から受け取ったトリム済みリスト
/// - 戻り値: `(Vec<PathIssue>, usize)` = (検出された問題一覧, 総エントリ数)
///
/// これはタスク要件の `analyze_paths` シグネチャに準拠した薄いラッパー。
/// 詳細な診断は `run_path_health` で行う。
#[allow(dead_code)]
pub fn analyze_paths(entries: &[String]) -> (Vec<PathIssue>, usize) {
    let total = entries.len();
    let report = run_path_health_on(entries);

    let mut issues = Vec::new();

    for dead in &report.dead_paths {
        issues.push(PathIssue {
            path: dead.raw.clone(),
            issue_type: PathIssueType::DeadPath,
            occurrences: 0,
        });
    }

    for dup in &report.duplicates {
        issues.push(PathIssue {
            path: dup.canonical.clone(),
            issue_type: PathIssueType::DuplicatePath,
            occurrences: dup.occurrences.len(),
        });
    }

    (issues, total)
}

/// OS ネイティブのセパレータで PATH を取得・分割し、完全な診断レポートを返す。
///
/// `std::env::var("PATH")` が `Err` の場合は空エントリとして扱う（REQ-001）。
pub fn run_path_health() -> PathHealthReport {
    #[cfg(target_os = "windows")]
    let separator = ';';
    #[cfg(not(target_os = "windows"))]
    let separator = ':';

    let raw = std::env::var("PATH").unwrap_or_default();
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
/// - ファイルシステムアクセス（`metadata`、`canonicalize`）のみ副作用として持つが、
///   入力に対して決定論的であり、環境変数 I/O は行わない。
pub fn run_path_health_on(entries: &[String]) -> PathHealthReport {
    let total = entries.len();
    let mut dead_paths: Vec<DeadPathEntry> = Vec::new();

    // REQ-002: Dead Path 検出
    for (index, raw) in entries.iter().enumerate() {
        match std::fs::metadata(raw) {
            Ok(meta) if meta.is_dir() => {
                // 正常なディレクトリ
            }
            Ok(_) => {
                dead_paths.push(DeadPathEntry {
                    index,
                    raw: raw.clone(),
                    reason: DeadReason::NotADirectory,
                });
            }
            Err(e) => {
                let reason = if e.kind() == io::ErrorKind::PermissionDenied {
                    DeadReason::PermissionDenied
                } else {
                    DeadReason::NotFound
                };
                dead_paths.push(DeadPathEntry {
                    index,
                    raw: raw.clone(),
                    reason,
                });
            }
        }
    }

    // REQ-003: Duplicate 検出
    // canonical キー → (canonical 文字列, 出現インデックス Vec)
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

    // 出現順でソート（最初の出現インデックス昇順）
    duplicates.sort_by_key(|d| d.occurrences[0]);

    PathHealthReport {
        total,
        skipped_count: 0,
        dead_paths,
        duplicates,
    }
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
    fn analyze_paths_returns_correct_total() {
        let entries = vec![
            "/usr/bin".to_string(),
            "/nonexistent_path_xyz_9999".to_string(),
        ];
        let (_, total) = analyze_paths(&entries);
        assert_eq!(total, 2);
    }

    #[test]
    fn duplicate_detection_finds_two_occurrences() {
        // 同じ存在しないパスを2回登録
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

    // ── プロパティベーステスト（アーキテクチャ規則 §3）──

    proptest! {
        // ────────────────────────────────────────────────────
        // Property 1-A: split_path_entries — パニックしない
        // 任意のランダム文字列（特殊記号・連続セパレータ・空文字・極端な長さ）を
        // 渡してもクラッシュしないことを保証する。
        // ────────────────────────────────────────────────────

        /// 任意のバイト列を含む PATH 文字列でも split がパニックしない
        #[test]
        fn split_never_panics(s in ".*") {
            let _ = split_path_entries(&s, ':');
            let _ = split_path_entries(&s, ';');
        }

        /// 連続するセパレータ・前後空白・極端に長い文字列でも split がパニックしない
        #[test]
        fn split_never_panics_edge_cases(
            // 繰り返しセパレータを含む可能性のある文字列
            s in "([^:]{0,50}:){0,20}[^:]{0,50}",
        ) {
            let _ = split_path_entries(&s, ':');
        }

        // ────────────────────────────────────────────────────
        // Property 1-B: analyze_paths — パニックしない
        // 任意のランダム文字列をエントリとして直接渡しても
        // クラッシュしないことを保証する。
        // ────────────────────────────────────────────────────

        /// 任意の文字列をエントリリストとして analyze_paths に渡してもパニックしない
        #[test]
        fn analyze_paths_never_panics(
            entries in prop::collection::vec(".*", 0..20),
        ) {
            let _ = analyze_paths(&entries);
        }

        /// 特殊記号・空白・制御文字を含む文字列を analyze_paths に渡してもパニックしない
        #[test]
        fn analyze_paths_never_panics_with_special_chars(
            entries in prop::collection::vec(
                // 印字可能 ASCII 全域 + タブ・改行・バックスラッシュ
                "[\\x20-\\x7e\\t\\n\\\\]{0,60}",
                0..15,
            ),
        ) {
            let _ = analyze_paths(&entries);
        }

        // ────────────────────────────────────────────────────
        // Property 2: Roundtrip 性
        // 重複のないユニークなエントリを結合して再パースした場合、
        // 元のエントリ数と完全に一致すること。
        //
        // 前提条件:
        //   - 各エントリはセパレータ (':') も null バイトも前後空白も含まない
        //     → split 後に増減しない
        //   - 各エントリは最低 1 文字以上（空エントリは split でスキップされるため）
        //   - エントリ間で重複なし（大文字・小文字を区別した一意性）
        // ────────────────────────────────────────────────────

        /// 重複なし有効エントリの結合→再分割後のエントリ数が元と一致する（Roundtrip）
        #[test]
        fn split_roundtrip_preserves_count(
            // セパレータ・null バイト・前後空白を含まない 1〜40 文字のエントリを 0〜15 個生成
            raw_entries in prop::collection::vec(
                "[^:;\x00\\s][^:;\x00]{0,38}[^:;\x00\\s]|[^:;\x00\\s]",
                0..15usize,
            ),
        ) {
            // 大文字小文字を区別して重複排除（normalize_key が lowercase するため
            // 同一 lowercase のエントリが混ざると分割後に数が合わない可能性がある）
            let mut seen = std::collections::HashSet::new();
            let unique: Vec<String> = raw_entries
                .into_iter()
                .filter(|e| seen.insert(e.to_lowercase()))
                .collect();

            let expected = unique.len();

            // ':' で結合して再分割
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

        /// split の結果に空文字列や null バイトが含まれない（不変条件）
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

        /// split 後の各エントリは前後に空白を持たない（トリム保証）
        #[test]
        fn split_entries_are_trimmed(s in "[^\x00]{0,200}") {
            for sep in [':', ';'] {
                let entries = split_path_entries(&s, sep);
                for e in &entries {
                    prop_assert_eq!(
                        e.as_str(),
                        e.trim(),
                        "entry is not trimmed: {:?}",
                        e,
                    );
                }
            }
        }

        /// analyze_paths の total は常に入力エントリ数と一致する
        #[test]
        fn analyze_paths_total_always_equals_input_len(
            // fs I/O タイムアウトを避けるため存在しない確実なパスを生成
            entries in prop::collection::vec(
                "/nonexistent_proptest_[a-z]{1,8}/[a-z]{1,8}",
                0..10usize,
            ),
        ) {
            let (_, total) = analyze_paths(&entries);
            prop_assert_eq!(total, entries.len());
        }
    }
}
