# 🩺 shell-doctor

Rust 製の高速シェル診断ツールです。シェルの起動ボトルネックと PATH の問題をミリ秒単位で検出します。

> 🇺🇸 [English documentation](README.md)

---

## 📋 目次

- [動作環境](#動作環境)
- [インストール](#インストール)
- [使い方](#使い方)
  - [demo — 起動ボトルネック解析](#demo--起動ボトルネック解析)
  - [path-health — PATH 診断](#path-health--path-診断)
- [シェルプロファイルの最適化](#-シェルプロファイルの最適化)
- [テスト](#テスト)

---

## 動作環境

- Rust 1.75 以上
- `--spawn` / `--shell zsh` を使う場合: `zsh` が `$PATH` に存在すること
- `--spawn` / `--shell bash` を使う場合: `bash` が `$PATH` に存在すること
- `--spawn` / `--shell fish` を使う場合: `fish` が `$PATH` に存在すること
- `--spawn` / `--shell powershell` を使う場合: `pwsh` が `$PATH` に存在すること

---

## インストール

```bash
git clone https://github.com/your-name/shell-doctor
cd shell-doctor
cargo build --release
# 任意でパスに追加:
# cp target/release/shell-doctor ~/.local/bin/
```

---

## 使い方

```
shell-doctor <COMMAND>

Commands:
  demo         シェルの起動ボトルネックを診断する
  path-health  PATH 環境変数の健全性を診断する
  help         ヘルプを表示する
```

### `demo` — 起動ボトルネック解析

シェルプロファイルの各行ごとの実行時間を計測し、遅いコマンドを特定します。

#### 引数なし — デモデータで動作確認

Zsh・Bash・Fish・PowerShell のサンプルトレースをまとめて表示します。セットアップ不要で即試せます。

```bash
cargo run -- demo
```

#### `--file` — トレースファイルを指定して解析

事前に取得した `set -x` のトレースログファイルを直接渡して解析します。

```bash
# zsh のトレースをファイルに保存:
PS4='+${EPOCHREALTIME} ${(%):-%x}:${LINENO}: ' zsh --no-rcs -o xtrace -i -c 'source ~/.zshrc' 2> /tmp/zsh_trace.log

# 解析:
cargo run -- demo --file /tmp/zsh_trace.log

# シェル種別が自動判定されない場合は --shell で明示:
cargo run -- demo --file /tmp/zsh_trace.log --shell zsh
```

#### `--spawn` — シェルを直接起動してリアルタイム計測

shell-doctor がシェルを headless で起動し、トレース取得から解析まで一括実行します。

```bash
# Zsh の場合
cargo run -- demo --spawn ~/.zshrc

# Bash の場合
cargo run -- demo --spawn ~/.bashrc

# Fish の場合
cargo run -- demo --spawn ~/.config/fish/config.fish

# PowerShell の場合
cargo run -- demo --spawn $PROFILE

# シェル種別を明示指定する場合
cargo run -- demo --spawn ~/my-init-script --shell zsh
```

**`--shell` オプションの値:**

| 値 | シェル |
|---|---|
| `zsh` | Zsh |
| `bash` または `sh` | Bash |
| `fish` | Fish |
| `powershell`、`pwsh`、または `ps` | PowerShell |

**出力例:**

```
🩺 Spawning Zsh to capture real startup trace: /home/user/.zshrc

╭──────┬──────────┬──────────────────────────────╮
│ Line │ Duration │ Command                      │
╞══════╪══════════╪══════════════════════════════╡
│ 24   │ 220 ms   │ eval "$(pyenv init -)"       │
│ 12   │ 185 ms   │ eval "$(brew shellenv)"      │
│ 40   │  25 ms   │ source ~/.fzf.zsh            │
╰──────┴──────────┴──────────────────────────────╯
⚡ Total bottleneck delay: 430 ms
```

---

### `path-health` — PATH 診断

`PATH` 環境変数を走査し、存在しないパス（Dead Path）と重複登録（Duplicate）を検出します。

```bash
# 現在の $PATH を診断
cargo run -- path-health

# カスタム PATH 文字列を直接渡して診断（CI やテストに便利）
cargo run -- path-health --path "/usr/bin:/usr/local/bin:/nonexistent"

# 問題が1件でもあれば非ゼロ終了（CI 統合向け）
cargo run -- path-health --fail-on-issues
```

**オプション一覧:**

| オプション | 説明 |
|---|---|
| `--path <PATH_STRING>` | `$PATH` の代わりに指定した PATH 文字列を診断する |
| `--fail-on-issues` | Dead Path または Duplicate が1件でもあれば終了コード 1 で終了する |

**出力例:**

```
🩺 Diagnosing PATH environment variable...

Dead Paths
╭───┬───────────────────────────┬──────────────────╮
│ # │ Path                      │ Reason           │
╞═══╪═══════════════════════════╪══════════════════╡
│ 3 │ /usr/local/nonexistent    │ Not Found        │
│ 7 │ /etc/hosts                │ Not a Directory  │
╰───┴───────────────────────────┴──────────────────╯

Duplicate Paths
╭───┬──────────┬───────────┬───────╮
│ # │ Path     │ Status    │ Count │
╞═══╪══════════╪═══════════╪═══════╡
│ 1 │ /usr/bin │ first     │ 3     │
│ 5 │ /usr/bin │ duplicate │ —     │
│ 9 │ /usr/bin │ duplicate │ —     │
╰───┴──────────┴───────────┴───────╯

⚡ PATH Health Summary: 12 entries | 2 dead | 1 duplicates | Skipped (invalid): 0 entries
🏥 Health Score: 75/100
```

**Dead Path の判定基準:**

| 表示 | 意味 |
|---|---|
| `Not Found` | ファイルシステム上に存在しないパス |
| `Not a Directory` | 存在するがディレクトリではなくファイルなど |
| `Permission Denied` | 存在するがアクセス権限がないパス |

**Health Score の見方:**

| スコア | 状態 |
|---|---|
| 90〜100（緑） | 健全 |
| 60〜89（黄） | 軽微な問題あり |
| 0〜59（赤） | 要対処 |

> **スコアの計算:** Dead Path は 2 ポイント、Duplicate は 1 ポイントのペナルティを与えます。
> 存在しないパスの方が実害が大きいため、より重く扱われます。

---

## 🔧 シェルプロファイルの最適化

`shell-doctor demo` でボトルネックを特定したら、同梱の `zshrc-optimizations.patch` を参考に `.zshrc` を最適化できます。

```bash
# パッチの内容を確認
cat zshrc-optimizations.patch

# ドライランで影響範囲を確認
patch --dry-run ~/.zshrc zshrc-optimizations.patch

# 実際に適用
patch ~/.zshrc zshrc-optimizations.patch
```

**パッチの内容:**

| 元の記述 | 最適化手法 | 削減効果の目安 |
|---|---|---|
| `eval "$(brew shellenv)"` | 静的キャッシュ（Homebrew 更新時のみ再生成） | 約 185 ms → 約 0 ms |
| `eval "$(pyenv init -)"` | 遅延ロード（`python`/`pyenv` 初回呼び出し時に初期化） | 約 220 ms → 約 0 ms |

`init`（Zsh 向け）および `login`（Bash 向け）のサンプルスクリプトにも同様のパターンがコメント付きで記載されています。

---

## テスト

```bash
# ユニットテスト + プロパティベーステスト
cargo test
```
