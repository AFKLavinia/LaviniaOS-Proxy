#!/usr/bin/env bash
# Cargo runner used by start.sh: grants CAP_NET_ADMIN (UDP) to the freshly built binary
# using the already-cached sudo credential, then runs it. Usage: dev-runner.sh <binary> [args...]
bin="$1"
if [ -n "$bin" ] && ! getcap "$bin" 2>/dev/null | grep -q cap_net_admin; then
    sudo -n setcap cap_net_admin+ep "$bin" 2>/dev/null || echo "uyari: setcap uygulanamadi, UDP kapali kalabilir" >&2
fi
exec "$@"

