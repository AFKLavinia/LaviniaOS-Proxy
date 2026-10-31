import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Phase = "stopped" | "starting" | "running" | "degraded";

export interface Target {
  host: string;
  port: number;
  username?: string;
  password?: string;
  dns?: string;
}

export type LinkKind = "wifi" | "usb" | "ethernet" | "bluetooth" | "cellular" | "other";
export type UdpMode = "active" | "noPrivilege" | "unsupported";

export interface Uplink {
  device: string;
  kind: LinkKind;
  name: string;
  hardware: string | null;
}

export interface Status {
  phase: Phase;
  target: Target;
  link: Uplink | null;
  udp: UdpMode | null;
}

export interface Telemetry {
  phase: Phase;
  rxBps: number;
  txBps: number;
  rxPackets: number;
  txPackets: number;
  dropped: number;
}

/** Single IPC gateway: the UI performs no system calls or FFI. */
export const ipc = {
  uiReady: (): Promise<void> => invoke<void>("ui_ready"),
  getStatus: (): Promise<Status> => invoke<Status>("get_status"),
  startTunnel: (target: Target): Promise<Status> => invoke<Status>("start_tunnel", { target }),
  grantUdpPrivilege: (): Promise<void> => invoke<void>("grant_udp_privilege"),
  stopTunnel: (): Promise<Status> => invoke<Status>("stop_tunnel"),
  getAutostart: (): Promise<boolean> => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean): Promise<boolean> => invoke<boolean>("set_autostart", { enabled }),
  onTelemetry: (cb: (t: Telemetry) => void): Promise<UnlistenFn> =>
    listen<Telemetry>("telemetry", (e) => cb(e.payload)),
};
