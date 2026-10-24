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

const EMPTY: Telemetry = { phase: "stopped", rxBps: 0, txBps: 0, rxPackets: 0, txPackets: 0, dropped: 0 };

function push(list: number[], v: number): number[] {
  return [...list, v].slice(-HISTORY);
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
    return (localStorage.getItem("lavinia_lang") as Lang) || "tr";
  });

  const [theme, setTheme] = useState<string>(() => {
    return localStorage.getItem("lavinia_theme") || "default";
  });

  const t = DICTIONARY[lang] || DICTIONARY.tr;

  const handleLangChange = (l: Lang) => {
    setLang(l);
    localStorage.setItem("lavinia_lang", l);
  };

  const handleThemeChange = (th: string) => {
    setTheme(th);
    localStorage.setItem("lavinia_theme", th);
    document.documentElement.setAttribute("data-theme", th);
  };

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

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
      .catch((e) => setError(String(e)))
      .finally(() => {
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
    const unlisten = ipc.onTelemetry((tel) => {
      setLatest(tel);
      setPhase(tel.phase);
      setRxHistory((h) => push(h, tel.rxBps));
      setTxHistory((h) => push(h, tel.txBps));
    });
    return () => {
      unlisten.then((fn) => fn()).catch((e) => console.error("unlisten failed", e));
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
    } catch (e: any) {
      const errStr = String(e);
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
    const engaged = phase === "running" || phase === "degraded";
    if (engaged) {
      setBusy(true);
      setError(null);
      try {
        const s = await ipc.stopTunnel();
        setPhase(s.phase);
        setInfo({ link: s.link, udp: s.udp });
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    } else {
      await startWithTarget();
    }
  }, [phase, startWithTarget]);

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
      setError(String(e));
      setGranting(false);
      setModal(false);
    }
  }, []);

  return (
    <main className="shell">
      <Header phase={phase} t={t} onOpenSettings={() => setSettingsOpen(true)} />
      <PowerButton phase={phase} busy={busy} t={t} onToggle={toggle} />
      <p className={error ? "error show" : "error"} role="alert">{error ?? "\u00a0"}</p>
      <TargetForm
        host={host}
        port={port}
        disabled={phase === "running" || phase === "degraded"}
        t={t}
        onChange={(n) => {
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
