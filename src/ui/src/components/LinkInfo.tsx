import type { LinkKind, Status, Target, UdpMode } from "../ipc";
import type { Translations } from "../lib/i18n";

function KindIcon({ kind }: { kind: LinkKind }) {
  const common = { width: 22, height: 22, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, "aria-hidden": true };
  if (kind === "usb") {
    return (
      <svg {...common}>
        <path d="M12 3v14" /><circle cx="12" cy="19" r="2" /><path d="M12 9l-4-2v-2M12 12l4-2V8" />
        <rect x="6.5" y="3.5" width="3" height="3" /><circle cx="16" cy="7" r="1.5" />
      </svg>
    );
  }
  if (kind === "wifi") {
    return (
      <svg {...common}>
        <path d="M2 9a15 15 0 0 1 20 0M5.5 12.5a10 10 0 0 1 13 0M9 16a5 5 0 0 1 6 0" /><circle cx="12" cy="19.5" r=".8" />
      </svg>
    );
  }
  return (
    <svg {...common}>
      <rect x="3" y="6" width="18" height="12" rx="2" /><path d="M7 10h.01M11 10h.01M7 14h10" />
    </svg>
  );
}

interface Props {
  status: Pick<Status, "link" | "udp">;
  target: Target;
  t: Translations;
  onFixUdp: () => void;
}

export function LinkInfo({ status, target, t, onFixUdp }: Props) {
  const { link, udp } = status;
  if (!link) return null;

  const getKindLabel = (k: LinkKind): string => {
    switch (k) {
      case "wifi": return t.linkWifi;
      case "usb": return t.linkUsb;
      case "ethernet": return t.linkEthernet;
      case "bluetooth": return t.linkBluetooth;
      case "cellular": return t.linkCellular;
      default: return t.linkOther;
    }
  };

  const getUdpChip = (mode: UdpMode) => {
    switch (mode) {
      case "active":
        return { cls: "chip chip-ok", text: t.udpOn, hint: t.udpRelayed };
      case "noPrivilege":
        return { cls: "chip chip-warn", text: t.udpOff, hint: t.udpNeedPerm };
      case "unsupported":
        return { cls: "chip chip-bad", text: t.udpOff, hint: t.udpUnsupported };
    }
  };

  const chip = udp ? getUdpChip(udp) : null;

  return (
    <section className="card link" aria-label={t.connection}>
      <div className="link-head">
        <span className={`link-icon kind-${link.kind}`}><KindIcon kind={link.kind} /></span>
        <div className="link-main">
          <strong id="link-kind">{getKindLabel(link.kind)}</strong>
          <span className="link-sub" id="link-name">{link.hardware ?? (link.name || t.unnamedConn)}</span>
        </div>
        {chip && <span className={chip.cls} id="udp-mode">{chip.text}</span>}
      </div>
      <dl className="link-grid">
        <div><dt>{t.interface}</dt><dd>{link.device}</dd></div>
        <div><dt>{t.proxy}</dt><dd>{target.host}:{target.port}</dd></div>
        {link.hardware && link.name && <div><dt>{t.connection}</dt><dd>{link.name}</dd></div>}
        {chip && <div><dt>{t.udp}</dt><dd>{chip.hint}</dd></div>}
      </dl>
      {udp === "noPrivilege" && (
        <button type="button" className="link-fix" id="udp-fix" onClick={onFixUdp}>
          {t.enableUdp}
        </button>
      )}
    </section>
  );
}
