## [2026-10-23 13:55] - [CHORE]
- Description: Project architecture established (C/Rust/TSX client-only L3 TUN→SOCKS5 engine); Milestone 1 started.
- Affected Paths: `.gitignore`, `maps.md`, `log.md`, `Cargo.toml`, `build.rs`
- Boundary/Security: Secrets, Wintun binaries, and build outputs excluded from VCS.

## [2026-10-23 13:56] - [FEAT]
- Description: Milestone 1: C TUN driver abstraction (Linux /dev/net/tun, Windows wintun.dll), lock-free SPSC packet ring, and safe Rust FFI wrapper.
- Affected Paths: `src/c/`, `src/rust/lib.rs`, `src/rust/tun/`
- Boundary/Security: Ingress packets bounded to 2048B slots; write length validated; backpressure by counted drop; wintun loaded from app dir/System32 only; FFI errors mapped to typed `TunError`.

## [2026-10-23 14:12] - [FEAT]
- Description: Tauri v2 tray-resident shell with hidden-at-start window revealed by `ui_ready` IPC after first paint; React/Vite UI scaffold.
- Affected Paths: `Cargo.toml`, `build.rs`, `tauri.conf.json`, `icons/`, `src/rust/app/`, `src/rust/main.rs`, `src/ui/`
- Boundary/Security: UI limited to a single IPC gateway (`ipc.ts`); strict CSP; window close hides instead of exiting.

## [2026-10-23 14:19] - [REFACTOR]
- Description: Moved Cargo/Tauri host config into `src-tauri/` (standard Tauri layout) while keeping sources in `src/c`, `src/rust`, `src/ui`.
- Affected Paths: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/icons/`
- Boundary/Security: Path-only change; no behavior or IPC surface changes.

## [2026-10-23 14:27] - [REFACTOR]
- Description: Moved Rust sources from `src/rust` into `src-tauri/src` (Tauri-native layout); C layer remains in `src/c`.
- Affected Paths: `src-tauri/src/`, `src-tauri/Cargo.toml`
- Boundary/Security: Path-only change; no behavior change.

## [2026-10-23 14:40] - [FEAT]
- Description: Full dashboard UI (status, power toggle, target form, throughput sparklines, packet counters) with typed Rust IPC (`get_status`, `start_tunnel`, `stop_tunnel`, `telemetry` event); engine counters are wired but updated only once Milestone 2 lands.
- Affected Paths: `src/ui/src/`, `src-tauri/src/tunnel/`, `src-tauri/capabilities/default.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`
- Boundary/Security: Target DTO uses deny_unknown_fields; host parsed as IpAddr (non-unspecified/multicast), port 1-65535; errors typed as TunnelError; added serde (already a Tauri transitive dependency).

## [2026-10-23 19:02] - [FEAT]
- Description: Milestone 2: tokio transparent SOCKS5 engine (TCP redirect listener, DNS-over-SOCKS5 UDP relay, atomic counters) with Linux firewall/proc/network platform adapter and Windows stub, wired into TunnelState start/stop.
- Affected Paths: `src-tauri/src/engine/`, `src-tauri/src/platform/`, `src-tauri/src/tunnel/`, `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`
- Boundary/Security: Listeners bound to loopback only; uplink validated before rule install; firewall guard released on stop/drop; OS-specific code confined to `src-tauri/src/platform/`; `cargo check` passes.

## [2026-10-23 19:10] - [FIX]
- Description: Tunnel is now stopped on application exit (RunEvent::Exit) so firewall rules are released deterministically on tray Quit.
- Affected Paths: `src-tauri/src/app/mod.rs`
- Boundary/Security: No new ingress surface; watchdog remains the crash-safe fallback.

## [2026-10-23 19:15] - [FEAT]
- Description: Active upstream health monitoring (SOCKS5 probe every 2s, 2 failures = down) surfaced as a `degraded` phase in IPC/UI so the dashboard no longer reports "active" when the phone proxy is unreachable.
- Affected Paths: `src-tauri/src/engine/`, `src-tauri/src/tunnel/`, `src/ui/src/`
- Boundary/Security: Probe sends only method negotiation to the user-specified proxy, 2s timeout; firewall stays fail-closed while degraded.

## [2026-10-23 19:12] - [FIX]
- Description: Tauri before-dev/build commands now run from the frontend directory (cwd is src/ui), fixing the doubled path error.
- Affected Paths: `src-tauri/tauri.conf.json`
- Boundary/Security: Config-only change.

## [2026-10-23 20:58] - [FEAT]
- Description: Transparent UDP support (Discord voice etc.) via nftables TPROXY + fwmark policy routing and SOCKS5 UDP ASSOCIATE relay with spoofed-source replies.
- Affected Paths: `src-tauri/src/engine/udp.rs`, `src-tauri/src/engine/socks5.rs`, `src-tauri/src/engine/mod.rs`, `src-tauri/src/platform/`
- Boundary/Security: Relay datagrams parsed with length/fragment/ATYP checks; loopback destinations refused; session and queue counts bounded; routing rules removed by watchdog.

## [2026-10-23 21:15] - [FEAT]
- Description: Persist last proxy endpoint, detect uplink medium (Wi-Fi/USB + device name), show it in tray icon/tooltip and UI, gate UDP on privilege + upstream probe, fix UDP session idle expiry, add SOCKS5 UDP frame tests.
- Affected Paths: `src-tauri/src/tunnel/store.rs`, `src-tauri/src/app/glyph.rs`, `src-tauri/src/app/tray.rs`, `src-tauri/src/platform/`, `src-tauri/src/engine/`, `src/ui/src/components/LinkInfo.tsx`
- Boundary/Security: Stored endpoint re-validated through TargetDto; credentials never persisted; atomic file write.

## [2026-10-23 21:25] - [FEAT]
- Description: Redesigned connected-device card and added permission modal that grants CAP_NET_ADMIN via polkit (setcap) and restarts the app.
- Affected Paths: `src-tauri/src/tunnel/commands.rs`, `src-tauri/src/platform/`, `src/ui/src/components/LinkInfo.tsx`, `src/ui/src/components/PermissionModal.tsx`
- Boundary/Security: Executable path whitelisted before shell use; credentials only entered in the system polkit prompt, never in-app.

## [2026-10-24 01:05] - [FEAT]
- Description: Added AuthModal for RFC 1929 SOCKS5 user/pass input when required, wired frontend target auth, and overhauled UI layout sizing for full window fit without clipping.
- Affected Paths: `src/ui/src/components/AuthModal.tsx`, `src/ui/src/App.tsx`, `src/ui/src/ipc.ts`, `src/ui/src/styles.css`
- Boundary/Security: SOCKS5 credentials collected exclusively via interactive modal on auth failure, securely forwarded via IPC to backend, never logged or serialized to unencrypted files.

## [2026-10-24 01:28] - [FEAT]
- Description: Implemented Windows platform adapter (Wintun route orchestration, PowerShell uplink detection, registry autostart, UAC elevation, background watchdog).
- Affected Paths: `src-tauri/src/platform/windows/`, `maps.md`
- Boundary/Security: Phone IP host route pinned to physical uplink adapter; PowerShell script parameters escaped; flag-file backed background watchdog cleans up routes on crash/exit.

## [2026-10-24 01:54] - [FEAT]
- Description: Implemented multilingual i18n (TR/EN), settings modal with theme switching (Standard/Midnight/OLED), saved endpoint quick-selection, and polished glassmorphic UI aesthetics.
- Affected Paths: `src/ui/src/lib/i18n.ts`, `src/ui/src/components/SettingsModal.tsx`, `src/ui/src/components/Header.tsx`, `src/ui/src/components/PowerButton.tsx`, `src/ui/src/components/TargetForm.tsx`, `src/ui/src/components/LinkInfo.tsx`, `src/ui/src/components/Stats.tsx`, `src/ui/src/components/AuthModal.tsx`, `src/ui/src/components/PermissionModal.tsx`, `src/ui/src/App.tsx`, `src/ui/src/styles.css`
- Boundary/Security: Preferences and saved endpoints stored in local client-side storage; sanitized port number formatting maintained; strict IPC boundary untouched.
