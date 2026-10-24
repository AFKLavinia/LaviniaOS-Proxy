# maps.md

```
.
├── .gitignore                      # Excludes target/, node_modules, Wintun DLLs, secrets, artifacts.
├── src-tauri/                      # Tauri host crate: Rust orchestration, async tun2socks, firewall/watchdog, IPC host.
│   ├── Cargo.toml                  # Crate manifest.
│   ├── build.rs                    # Compiles ../src/c per target_os + tauri_build.
│   ├── tauri.conf.json             # Tauri config; main window starts hidden (visible:false).
│   ├── icons/icon.png              # Tray/window icon.
│   ├── capabilities/default.json   # Main-window IPC permissions.
│   └── src/
│       ├── lib.rs                  # Crate root; exports `app`, `engine`, `platform`, `tun`, `tunnel`.
│       ├── main.rs                 # Binary entry; calls app::run().
│       ├── app/
│       │   ├── mod.rs              # Tauri builder, hide-on-close (entry: run).
│       │   ├── tray.rs             # System tray icon + Show/Quit menu; Wi-Fi/USB state icon + tooltip.
│       │   ├── glyph.rs            # Procedural Wi-Fi/USB tray glyphs.
│       │   └── window.rs           # show_main + `ui_ready` command.
│       ├── engine/
│       │   ├── mod.rs              # Tokio engine lifecycle (entry: Engine, Counters, EngineError).
│       │   ├── counters.rs         # Atomic packet/byte counters.
│       │   ├── tcp.rs              # Redirected TCP listener relayed via SOCKS5.
│       │   ├── dns.rs              # Local UDP DNS relay over SOCKS5.
│       │   ├── udp.rs              # Transparent UDP relay via SOCKS5 UDP ASSOCIATE (TPROXY, spoofed replies).
│       │   └── socks5.rs           # SOCKS5 client handshake.
│       ├── platform/
│       │   ├── mod.rs              # PAL facade (entry: check_uplink, apply_firewall, FirewallGuard).
│       │   ├── linux/              # Firewall rules, /proc lookup, network checks.
│       │   └── windows/            # Windows adapter: Wintun routing, PowerShell uplink check, autostart registry, UAC elevation.
│       │       ├── mod.rs          # Windows adapter exports and socket stubs.
│       │       ├── autostart.rs    # Registry CurrentVersion\Run autostart.
│       │       ├── firewall.rs     # Route table management and background PowerShell watchdog.
│       │       ├── network.rs      # Route and adapter classification via Get-NetRoute/Get-NetAdapter.
│       │       └── proc.rs         # Unprivileged and UAC-elevated PowerShell execution.
│       ├── tunnel/
│       │   ├── mod.rs          # Tunnel facade + 1 Hz telemetry emitter (entry: TunnelState, spawn_telemetry).
│       │   ├── dto.rs          # Ingress TargetDto validation, domain Target, IPC payloads, TunnelError.
│       │   ├── store.rs        # Last-used endpoint persistence.
│       │   ├── state.rs        # Phase/target state + atomic engine counters.
│       │   └── commands.rs     # Skinny IPC handlers: get_status/start_tunnel/stop_tunnel.
│       └── tun/
│           ├── ffi.rs              # Raw extern "C" declarations (private).
│           └── mod.rs              # Safe RAII wrapper (entry point: TunDevice, TunError).

├── log.md                          # Append-only change log.
├── maps.md                         # This file map.
└── src/
    ├── c/                          # Kernel/driver-level L3 packet I/O only; zero business logic.
    │   ├── include/
    │   │   └── lavinia_tun.h       # Public stable C-ABI (entry point: lv_tun_open/read/write/close).
    │   ├── ring_buffer.{h,c}       # Lock-free SPSC zero-alloc packet slot ring.
    │   ├── tun_core.c              # Platform-agnostic C-ABI implementation over the platform interface.
    │   └── platform/
    │       ├── platform.h          # Internal platform contract (open/write/close + RX pump).
    │       ├── linux/
    │       │   └── tun_linux.c     # /dev/net/tun backend with poll-based RX pump thread.
    │       └── windows/
    │           └── tun_windows.c   # wintun.dll dynamic-loaded backend with RX pump thread.
    └── ui/                         # React/Vite display+control only (no syscalls/FFI).
        ├── package.json            # UI deps/scripts.
        ├── vite.config.ts          # Vite build config (dev port 5173).
        ├── tsconfig.json           # TS strict config.
        ├── index.html              # Dark inline bg to prevent flash.
        └── src/
            ├── main.tsx            # React entry.
            ├── App.tsx             # Root view; signals ui_ready after 2 frames.
            ├── ipc.ts              # Sole IPC gateway to Rust (typed commands + telemetry event).
            ├── lib/                # Formatters and i18n dictionary (TR/EN).
            │   ├── format.ts       # Rate/count formatters.
            │   └── i18n.ts         # Multilingual dictionaries and language type definitions.
            ├── components/         # Header, PowerButton, TargetForm, LinkInfo, PermissionModal, AuthModal, SettingsModal, Stats.
            └── styles.css          # Theme styles, glassmorphic layout, animations, theme accent variables.
```
