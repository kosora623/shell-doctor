# 🩺 shell-doctor

Lightning-fast shell runtime doctor written in Rust, built with Kiro.

Diagnose shell startup bottlenecks and PATH health issues in milliseconds.

> 🇯🇵 [日本語版ドキュメントはこちら](README.ja.md)

---

## 📋 Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
  - [demo — Trace Bottleneck Analysis](#demo--trace-bottleneck-analysis)
  - [path-health — PATH Diagnostics](#path-health--path-diagnostics)
- [Optimizing Your Shell Profile](#-optimizing-your-shell-profile)
- [Testing](#testing)
- [Kiro University Challenge](#-kiro-university-challenge-implementation-matrix)

---

## Requirements

- Rust 1.75+
- For `--spawn` / `--shell zsh`: `zsh` must be in `$PATH`
- For `--spawn` / `--shell bash`: `bash` must be in `$PATH`
- For `--spawn` / `--shell fish`: `fish` must be in `$PATH`
- For `--spawn` / `--shell powershell`: `pwsh` must be in `$PATH`

---

## Installation

```bash
git clone https://github.com/your-name/shell-doctor
cd shell-doctor
cargo build --release
# Optionally add to PATH:
# cp target/release/shell-doctor ~/.local/bin/
```

---

## Usage

```
shell-doctor <COMMAND>

Commands:
  demo         Diagnose shell startup bottlenecks
  path-health  Diagnose PATH environment variable health
  help         Print help
```

### `demo` — Trace Bottleneck Analysis

Identify slow commands in your shell profile by measuring per-line execution time.

#### Run the built-in demo (no setup required)

Displays sample traces for Zsh, Bash, Fish, and PowerShell side by side.

```bash
cargo run -- demo
```

#### Analyze a captured trace file (`--file`)

Pass a pre-captured `set -x` trace log directly.

```bash
# Capture a zsh trace first:
PS4='+${EPOCHREALTIME} ${(%):-%x}:${LINENO}: ' zsh --no-rcs -o xtrace -i -c 'source ~/.zshrc' 2> /tmp/zsh_trace.log

# Then analyze it:
cargo run -- demo --file /tmp/zsh_trace.log

# If auto-detection picks the wrong shell, use --shell to override:
cargo run -- demo --file /tmp/zsh_trace.log --shell zsh
```

#### Spawn a headless shell and capture live (`--spawn`)

Let shell-doctor launch the shell, capture the trace, and report — all in one step.

```bash
# Zsh
cargo run -- demo --spawn ~/.zshrc

# Bash
cargo run -- demo --spawn ~/.bashrc

# Fish
cargo run -- demo --spawn ~/.config/fish/config.fish

# PowerShell
cargo run -- demo --spawn $PROFILE

# Override detected shell kind explicitly
cargo run -- demo --spawn ~/my-init-script --shell zsh
```

**`--shell` option values:**

| Value | Shell |
|---|---|
| `zsh` | Zsh |
| `bash` or `sh` | Bash |
| `fish` | Fish |
| `powershell`, `pwsh`, or `ps` | PowerShell |

**Example output:**

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

### `path-health` — PATH Diagnostics

Scan your `PATH` environment variable for dead entries and duplicates.

```bash
# Diagnose current $PATH
cargo run -- path-health

# Diagnose a custom PATH string (useful for CI or testing)
cargo run -- path-health --path "/usr/bin:/usr/local/bin:/nonexistent"

# Exit with non-zero code if any issues are found (CI integration)
cargo run -- path-health --fail-on-issues
```

**Options:**

| Option | Description |
|---|---|
| `--path <PATH_STRING>` | Diagnose a custom PATH string instead of `$PATH` |
| `--fail-on-issues` | Exit with code 1 if any dead paths or duplicates are found |

**Example output:**

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

**Dead Path reasons:**

| Reason | Description |
|---|---|
| `Not Found` | Path does not exist on the filesystem |
| `Not a Directory` | Path exists but is a file, not a directory |
| `Permission Denied` | Path exists but cannot be accessed due to permissions |

**Health Score:**

| Score | Status |
|---|---|
| 90–100 (green) | Healthy |
| 60–89 (yellow) | Minor issues |
| 0–59 (red) | Needs attention |

> **Scoring:** Dead paths incur a penalty of 2 points each; duplicates incur 1 point each,
> reflecting that missing paths are more harmful than redundant ones.

---

## 🔧 Optimizing Your Shell Profile

After identifying bottlenecks with `shell-doctor demo`, apply the included patch to your
`.zshrc` as a starting point for optimization.

```bash
# Preview the optimizations
cat zshrc-optimizations.patch

# Apply to your .zshrc (dry-run first)
patch --dry-run ~/.zshrc zshrc-optimizations.patch

# Apply for real
patch ~/.zshrc zshrc-optimizations.patch
```

**What the patch does:**

| Original | Optimization | Typical saving |
|---|---|---|
| `eval "$(brew shellenv)"` | Static cache — regenerated only when Homebrew updates | ~185 ms → ~0 ms |
| `eval "$(pyenv init -)"` | Lazy-loading wrapper — deferred until first `python`/`pyenv` call | ~220 ms → ~0 ms |

The sample profiles in `init` (Zsh) and `login` (Bash) also demonstrate these patterns
with inline comments.

---

## Testing

```bash
# Unit tests + property-based tests
cargo test
```

---

## 🚀 Kiro University Challenge Implementation Matrix

| Lesson | Focus Area | Implementation Path |
| :--- | :--- | :--- |
| **Lesson 1** | Spec-driven dev (EARS) | `.kiro/specs/trace-parser-spec.md` |
| **Lesson 2** | Steering documents | `.kiro/steering/architecture.md` |
| **Lesson 3** | Automation Hooks | `.kiro/hooks/hooks.json` |
| **Lesson 4** | Property-Based Testing | `src/parser.rs` (tested with `proptest`) |
| **Lesson 5** | Powers | `.kiro/powers/profile-analyzer.json` |
| **Lesson 6** | MCP Configuration | `.kiro/mcp.json` |
| **Lesson 7** | Custom Agents | `.kiro/agents/profile-optimizer.json` |
