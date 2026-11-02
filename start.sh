#!/usr/bin/env bash
# Tek komutla dev modu: tek sudo doğrulaması hem UDP izni (setcap) hem güvenlik duvarı (nft) için kullanılır.
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

export PATH="$HOME/.cargo/bin:$PATH"
NODE_BIN="$(ls -d "$HOME"/.nvm/versions/node/*/bin 2>/dev/null | sort -V | tail -n1)"
[ -n "$NODE_BIN" ] && export PATH="$NODE_BIN:$PATH"
export LD_LIBRARY_PATH="$HOME/.local/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Tek parola istemi; arka planda sudo zaman damgası canlı tutulur.
echo "🔐 Yönetici doğrulaması (UDP izni + güvenlik duvarı için tek sefer)..."
sudo -v
( while kill -0 "$$" 2>/dev/null; do sudo -n -v 2>/dev/null; sleep 50; done ) &
KEEPALIVE=$!
trap 'kill "$KEEPALIVE" 2>/dev/null' EXIT

# Her derlemeden sonra binary'ye CAP_NET_ADMIN verip çalıştırır.
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="$DIR/scripts/dev-runner.sh"

echo "🚀 Lavinia Client başlatılıyor..."
./node_modules/.bin/tauri dev
