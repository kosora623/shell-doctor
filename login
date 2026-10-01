#!/usr/bin/env bash
# =============================================================================
# login — shell-doctor サンプル Bash ログインスクリプト
#
# 使い方:
#   1. このファイルを ~/.bash_profile にコピーして使う
#   2. shell-doctor でトレース計測する:
#        shell-doctor demo --spawn ~/.bash_profile
#   3. ボトルネックを特定して最適化する
# =============================================================================

# ── PATH 設定 ─────────────────────────────────────────────────
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"

# ── .bashrc を読み込む（インタラクティブシェル向け設定を統合）──
if [ -f "$HOME/.bashrc" ]; then
    source "$HOME/.bashrc"
fi

# ── 基本エイリアス ─────────────────────────────────────────────
alias g="git"
alias ll="ls -la"
alias ..="cd .."

# ── direnv（重い場合は遅延ロードを検討）──────────────────────
# eval "$(direnv hook bash)"

# ── nvm（遅延ロードの例）──────────────────────────────────────
# export NVM_DIR="$HOME/.nvm"
# [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"

# ── rvm ───────────────────────────────────────────────────────
# [ -s "$HOME/.rvm/scripts/rvm" ] && source "$HOME/.rvm/scripts/rvm"

echo "✅ Login shell initialized"
