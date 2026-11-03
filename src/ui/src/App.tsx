import { useCallback, useEffect, useState } from "react";
import { ipc, type Phase, type Status, type Telemetry } from "./ipc";
import { DICTIONARY, type Lang } from "./lib/i18n";
import { LinkInfo } from "./components/LinkInfo";
import { PermissionModal } from "./components/PermissionModal";
import { AuthModal } from "./components/AuthModal";
import { SettingsModal } from "./components/SettingsModal";
import { Header } from "./components/Header";
import { PowerButton } from "./components/PowerButton";
import { HISTORY, Stats } from "./components/Stats";
import { TargetForm, toTarget } from "./components/TargetForm";

const EMPTY: Telemetry = { phase: "stopped", rxBps: 0, txBps: 0, rxPackets: 0, txPackets: 0, dropped: 0, rttMs: 0, jitterMs: 0 };

function push(list: number[], v: number): number[] {
  return [...list, v].slice(-HISTORY);
}

const FRIENDLY_ERRORS: Array<[RegExp, string]> = [
  [/authorization was cancelled or denied/i, "Yetkilendirme iptal edildi veya reddedildi."],
  [/connection refused/i, "Telefondaki proxy bağlantıyı reddetti. Uygulamanın açık olduğundan emin olun."],
  [/(timed out|timeout)/i, "Telefona ulaşılamadı (zaman aşımı). IP ve portu kontrol edin."],
  [/(network is unreachable|no route to host)/i, "Telefona ağ yolu yok. Wi-Fi veya USB bağlantısını kontrol edin."],
  [/auth/i, "Proxy kimlik doğrulaması başarısız. Kullanıcı adı ve şifreyi kontrol edin."],
  [/firewall setup failed/i, "Güvenlik duvarı kuralları uygulanamadı. Yönetici yetkisi gerekiyor."],
];

function friendlyError(raw: string): string {
  for (const [re, msg] of FRIENDLY_ERRORS) {
    if (re.test(raw)) return msg;
  }
  return raw;
}

function formatError(e: unknown): string {
  return friendlyError(rawError(e));
}

function rawError(e: unknown): string {
  if (typeof e === "string") {
    return e;
  }
  if (e && typeof e === "object") {
    if ("message" in e && typeof (e as { message: unknown }).message === "string") {
      return (e as { message: string }).message;
    }
    try {
      const json = JSON.stringify(e);
      if (json !== "{}") return json;
    } catch {
      // ignore json error
    }
  }
  return String(e);
}

export function App() {
  const [phase, setPhase] = useState<Phase>("stopped");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const [auth, setAuth] = useState<{ username?: string; password?: string }>({});
  const [info, setInfo] = useState<Pick<Status, "link" | "udp">>({ link: null, udp: null });
  const [modal, setModal] = useState(false);
  const [authModal, setAuthModal] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [granting, setGranting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [latest, setLatest] = useState<Telemetry>(EMPTY);
  const [rxHistory, setRxHistory] = useState<number[]>([]);
  const [txHistory, setTxHistory] = useState<number[]>([]);

  const [lang, setLang] = useState<Lang>(() => {
    try {
      return (localStorage.getItem("lavinia_lang") as Lang) || "tr";
    } catch {
      return "tr";
    }
  });

  const [theme, setTheme] = useState<string>(() => {
    try {
      return localStorage.getItem("lavinia_theme") || "default";
    } catch {
      return "default";
    }
  });

  const t = DICTIONARY[lang] || DICTIONARY.tr;

  const handleLangChange = (l: Lang) => {
    setLang(l);
    try {
      localStorage.setItem("lavinia_lang", l);
    } catch (e) {
      console.warn("Failed to save lang to localStorage", e);
    }
  };

  const handleThemeChange = (th: string) => {
    setTheme(th);
    try {
      localStorage.setItem("lavinia_theme", th);
    } catch (e) {
      console.warn("Failed to save theme to localStorage", e);
    }
    document.documentElement.setAttribute("data-theme", th);
  };

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  // Initial state from Rust (single source of truth), then reveal the window after first paint.
  useEffect(() => {
    let cancelled = false;
    ipc
      .getStatus()
      .then((s) => {
        if (cancelled) return;
        setPhase(s.phase);
        setInfo({ link: s.link, udp: s.udp });
        setHost(s.target.host);
        setPort(String(s.target.port));
      })
      .catch((e) => {
        if (cancelled) return;
        setError(formatError(e));
      })
      .finally(() => {
        if (cancelled) return;
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            ipc.uiReady().catch((e) => console.error("ui_ready failed", e));
          }),
        );
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let active = true;
    let unlistenFn: (() => void) | null = null;

    ipc
      .onTelemetry((tel) => {
        if (!active) return;
        setLatest(tel);
        setPhase(tel.phase);
        setRxHistory((h) => push(h, tel.rxBps));
        setTxHistory((h) => push(h, tel.txBps));
      })
      .then((fn) => {
        if (!active) {
          fn();
        } else {
          unlistenFn = fn;
        }
      })
      .catch((e) => console.error("telemetry listen failed", e));

    return () => {
      active = false;
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, []);

  const startWithTarget = useCallback(async (targetAuth?: { username?: string; password?: string }) => {
    setBusy(true);
    setError(null);
    try {
      const target = {
        ...toTarget(host, port),
        username: targetAuth?.username ?? auth.username,
        password: targetAuth?.password ?? auth.password,
      };
      const s = await ipc.startTunnel(target);
      setPhase(s.phase);
      setInfo({ link: s.link, udp: s.udp });
      if (s.udp === "noPrivilege") setModal(true);
    } catch (e: unknown) {
      const errStr = formatError(e);
      if (
        errStr.toLowerCase().includes("username and password") ||
        errStr.toLowerCase().includes("authentication") ||
        errStr.toLowerCase().includes("requires a username")
      ) {
        setAuthModal(true);
      } else {
        setError(errStr);
      }
    } finally {
      setBusy(false);
    }
  }, [host, port, auth]);

  const toggle = useCallback(async () => {
    if (busy || phase === "starting") return;
    const engaged = phase === "running" || phase === "degraded";
    if (engaged) {
      setBusy(true);
      setError(null);
      try {
        const s = await ipc.stopTunnel();
        setPhase(s.phase);
        setInfo({ link: s.link, udp: s.udp });
      } catch (e) {
        setError(formatError(e));
      } finally {
        setBusy(false);
      }
    } else {
      await startWithTarget();
    }
  }, [busy, phase, startWithTarget]);

  const handleAuthConfirm = useCallback((user: string, pass: string) => {
    setAuth({ username: user, password: pass });
    setAuthModal(false);
    startWithTarget({ username: user, password: pass });
  }, [startWithTarget]);

  const grant = useCallback(async () => {
    setGranting(true);
    setError(null);
    try {
      await ipc.grantUdpPrivilege();
    } catch (e) {
      setError(formatError(e));
      setGranting(false);
      setModal(false);
    }
  }, []);

  return (
    <main className="shell">
      <Header phase={phase} t={t} onOpenSettings={() => setSettingsOpen(true)} />
      <PowerButton phase={phase} busy={busy} t={t} onToggle={toggle} />
      <p className={`error ${error ? "show" : ""}`} role={error ? "alert" : undefined} aria-live="assertive">{error ?? "\u00a0"}</p>
      <TargetForm
        host={host}
        port={port}
        disabled={busy || phase === "starting" || phase === "running" || phase === "degraded"}
        t={t}
        onChange={(n) => {
          if (n.host !== host || n.port !== port) {
            setAuth({});
          }
          setHost(n.host);
          setPort(n.port);
        }}
      />
      <LinkInfo
        status={info}
        target={{ host, port: Number(port) }}
        t={t}
        onFixUdp={() => setModal(true)}
      />
      <Stats latest={latest} rxHistory={rxHistory} txHistory={txHistory} t={t} />

      {modal && (
        <PermissionModal
          busy={granting}
          t={t}
          onGrant={grant}
          onDismiss={() => setModal(false)}
        />
      )}

      {authModal && (
        <AuthModal
          host={host}
          port={port}
          t={t}
          onConfirm={handleAuthConfirm}
          onDismiss={() => setAuthModal(false)}
        />
      )}

      {settingsOpen && (
        <SettingsModal
          t={t}
          lang={lang}
          theme={theme}
          onLangChange={handleLangChange}
          onThemeChange={handleThemeChange}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </main>
  );
}
