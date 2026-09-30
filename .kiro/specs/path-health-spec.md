# Feature Specification: PATH Health Diagnostics

## 1. Overview

PATH Health Diagnostics は、OS の環境変数 `PATH` を走査して、存在しない無効なパス（Dead Path）や重複して登録されているパス（Duplicate Path）を高速検出する診断機能である。`shell-doctor` の新しいサブコマンド `path-health` として提供し、既存の `demo` コマンドと同じテーブル形式で結果を出力する。

## 2. Requirements (EARS Syntax)

### REQ-001 — OS ごとの PATH セパレータによる安全な分割

- **Category:** Ubiquitous (常時適用)

```
THE SYSTEM SHALL split the PATH environment variable using the OS-native separator:
  ';' on Windows (cfg(target_os = "windows")),
  ':' on all other platforms (Unix / macOS).
```

- **補足制約:**
  - 分割後の各エントリは先頭・末尾のホワイトスペースをトリムしてから後続処理に渡すこと。
  - `std::env::var("PATH")` が `Err` を返した場合（変数未定義）は、空のエントリリストを返し、エラーで中断しないこと。
  - 分割ロジックは純粋関数として実装し、プロセス I/O から独立させること（アーキテクチャ設計規則 §2 "Pure Functional Core" に準拠）。

---

### REQ-002 — 存在しないディレクトリの検出

- **Category:** Event-Driven

```
WHEN each PATH entry is evaluated after splitting,
THE SYSTEM SHALL call std::fs::metadata on the resolved path and, if the call returns Err
or the resulting Metadata is not a directory, SHALL classify that entry as a Dead Path.
```

- **補足制約:**
  - `std::fs::metadata` の呼び出しは並列化可能であるが、出力の順序はオリジナルの PATH 出現順を保持すること。
  - Dead Path と判定されたエントリは `DeadPathEntry { index: usize, raw: String }` として記録すること。
  - ファイルとして存在するパス（ディレクトリではないもの）も Dead Path に分類し、その旨を出力時に区別できる `reason` フィールドを持つこと（`NotFound` / `NotADirectory`）。

---

### REQ-003 — 重複登録されているパスの出現順序とカウント

- **Category:** Event-Driven

```
WHEN the full list of split and trimmed PATH entries is processed,
THE SYSTEM SHALL detect duplicate entries by canonical path comparison and, for each
duplicated value, SHALL record the first occurrence index, all subsequent occurrence
indices, and the total count.
```

- **補足制約:**
  - 正規化には `std::path::Path::canonicalize` を **試みる** が、対象パスが存在しない場合（Dead Path）は正規化をスキップし、トリム済み文字列の大文字・小文字を無視した比較（`to_lowercase()`）にフォールバックすること。
  - 重複と判定されたエントリは `DuplicateEntry { canonical: String, occurrences: Vec<usize> }` として記録すること。`occurrences` の先頭要素が最初の出現インデックスとなる。
  - 出力テーブルでは初回出現を "first" として緑色、後続の重複を "duplicate" として黄色で色分けすること。

---

### REQ-004 — 不正な文字・空エントリの安全なスキップ

- **Category:** Unwanted Behavior / State-Driven

```
IF a PATH entry is empty after trimming, or contains any null byte ('\0'),
THEN THE SYSTEM SHALL silently skip the entry without emitting an error and SHALL
increment an internal skipped_count for summary reporting.
```

- **補足制約:**
  - `skipped_count` は最終サマリー行に `"Skipped (invalid): N entries"` として表示すること。
  - スキップ処理は分割直後・トリム直後に行い、REQ-002 と REQ-003 のロジックにそのエントリが渡らないよう保証すること。
  - 本要件の実装は純粋関数内のガード節として実装し、パニックや `unwrap()` を使用しないこと。

---

## 3. Output Format

`path-health` サブコマンドの標準出力は以下の 2 テーブルで構成する。

### 3-1. Dead Paths Table

| # | Path | Reason |
|---|------|--------|
| 3 | /usr/local/nonexistent | Not Found |
| 7 | /etc/hosts | Not a Directory |

- 「Reason」列は `NotFound` / `NotADirectory` の 2 値を取る。
- 存在しないパスは赤色（`colored::Red`）で強調する。

### 3-2. Duplicate Paths Table

| # | Path | Status | Count |
|---|------|--------|-------|
| 1 | /usr/bin | first | 3 |
| 5 | /usr/bin | duplicate | — |
| 9 | /usr/bin | duplicate | — |

- `first` 行は緑色、`duplicate` 行は黄色で表示する。
- 重複がない場合はテーブルを省略し `"No duplicates found."` を表示する。

### 3-3. Summary Line

```
⚡ PATH Health Summary: N entries | D dead | U duplicates | S skipped
```

---

## 4. Data Structures (Reference)

実装の参照用。確定的な API は実装フェーズで調整可。

```rust
pub enum DeadReason {
    NotFound,
    NotADirectory,
}

pub struct DeadPathEntry {
    pub index: usize,
    pub raw: String,
    pub reason: DeadReason,
}

pub struct DuplicateEntry {
    pub canonical: String,      // 正規化済み、またはフォールバック文字列
    pub occurrences: Vec<usize>, // 出現順インデックス（先頭が first）
}

pub struct PathHealthReport {
    pub total: usize,
    pub skipped_count: usize,
    pub dead_paths: Vec<DeadPathEntry>,
    pub duplicates: Vec<DuplicateEntry>,
}
```

---

## 5. Architectural Compliance

| 設計規則 | 準拠方針 |
|----------|----------|
| Dynamic Runtime Trace | PATH の走査は実行時に `std::env::var` で取得し静的解析は行わない |
| Pure Functional Core | `split_path_entries`, `detect_dead_paths`, `detect_duplicates` は副作用なし純粋関数として実装 |
| Property-Based Testing | `proptest` を用いて任意のバイト列を含む PATH 文字列でスキップ・分割ロジックをファジング |

---

## 6. Out of Scope

- PATH エントリの自動修復・削除提案（将来の `path-fix` コマンドとして別スペック化）
- シンボリックリンクの解決深度制限（標準の `canonicalize` に委譲）
- Windows の短いパス名（8.3 形式）の展開
