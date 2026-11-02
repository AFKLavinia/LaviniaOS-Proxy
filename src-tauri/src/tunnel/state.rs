use std::net::IpAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};

use super::dto::{Phase, StartOptions, Status, Target, TunnelError};
use crate::engine::{Counters, Engine, UdpMode};
use crate::platform::Uplink;

struct Inner {
    phase: Phase,
    target: Target,
    engine: Option<Engine>,
    link: Option<(Uplink, UdpMode)>,
    cancel_start: bool,
}

#[derive(Clone)]
pub struct TunnelState {
    inner: Arc<Mutex<Inner>>,
    pub counters: Arc<Counters>,
    teardown: Arc<Mutex<()>>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                phase: Phase::Stopped,
                target: Target::default_phone(),
                engine: None,
                link: None,
                cancel_start: false,
            })),
            counters: Arc::new(Counters::default()),
            teardown: Arc::new(Mutex::new(())),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn status(&self) -> Status {
        let g = self.lock();
        let phase = match g.phase {
            Phase::Running if !self.counters.link_up.load(Ordering::Relaxed) => Phase::Degraded,
            p => p,
        };
        Self::snapshot(&g, phase)
    }

    fn snapshot(g: &Inner, phase: Phase) -> Status {
        Status {
            phase,
            target: g.target,
            link: g.link.as_ref().map(|(l, _)| l.clone()),
            udp: g.link.as_ref().map(|(_, u)| *u),
        }
    }

    /// Seeds the endpoint (e.g. last used) while idle.
    pub fn preset_target(&self, target: Target) {
        let mut g = self.lock();
        if g.phase == Phase::Stopped {
            g.target = target;
        }
    }

    /// Blocking (may show an authorization prompt); the state lock is not held while starting.
    pub fn start(&self, opts: StartOptions) -> Result<Status, TunnelError> {
        let StartOptions { target, auth, resolver } = opts;
        let IpAddr::V4(phone) = target.host else {
            return Err(TunnelError::Validation("only IPv4 proxy addresses are supported"));
        };

        // Serialize starts and wait for any ongoing engine teardown to complete.
        let _teardown = self.teardown.lock().unwrap_or_else(|p| p.into_inner());

        {
            let mut g = self.lock();
            if g.phase != Phase::Stopped {
                return Err(TunnelError::Conflict("tunnel is already running"));
            }
            g.phase = Phase::Starting;
            g.cancel_start = false;
            g.target = target;
        }

        // Reset live counters for the new session
        self.counters.rx_bytes.store(0, Ordering::Relaxed);
        self.counters.tx_bytes.store(0, Ordering::Relaxed);
        self.counters.rx_packets.store(0, Ordering::Relaxed);
        self.counters.tx_packets.store(0, Ordering::Relaxed);
        self.counters.dropped.store(0, Ordering::Relaxed);
        self.counters.link_up.store(false, Ordering::Relaxed);

        // Catch panics during Engine::start so phase does not get stuck in Starting
        let started = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Engine::start(phone, target.port, auth, resolver, self.counters.clone())
        })) {
            Ok(res) => res,
            Err(payload) => {
                let mut g = self.lock();
                g.phase = Phase::Stopped;
                g.cancel_start = false;
                std::panic::resume_unwind(payload);
            }
        };

        match started {
            Ok(engine) => {
                let to_stop = {
                    let mut g = self.lock();
                    if g.cancel_start || g.phase != Phase::Starting {
                        g.phase = Phase::Stopped;
                        g.cancel_start = false;
                        Some(engine)
                    } else {
                        g.link = Some((engine.uplink().clone(), engine.udp_mode()));
                        g.engine = Some(engine);
                        g.phase = Phase::Running;
                        None
                    }
                };

                if let Some(orphan) = to_stop {
                    // Start was cancelled or stop was requested while Engine::start was running.
                    // Shut down the orphan engine immediately outside the state lock.
                    orphan.stop();
                    let g = self.lock();
                    return Ok(Self::snapshot(&g, Phase::Stopped));
                }

                let g = self.lock();
                Ok(Self::snapshot(&g, g.phase))
            }
            Err(e) => {
                let mut g = self.lock();
                g.phase = Phase::Stopped;
                g.cancel_start = false;
                Err(TunnelError::from(e))
            }
        }
    }

    /// Blocking: waits for the watchdog to remove the firewall rules.
    pub fn stop(&self) -> Status {
        let engine = {
            let mut g = self.lock();
            if g.phase == Phase::Starting {
                g.cancel_start = true;
                g.phase = Phase::Stopped;
                return Self::snapshot(&g, g.phase);
            }
            g.phase = Phase::Stopped;
            g.link = None;
            g.engine.take()
        };
        if let Some(engine) = engine {
            let _teardown = self.teardown.lock().unwrap_or_else(|p| p.into_inner());
            engine.stop();
        }
        self.status()
    }
}

impl Default for TunnelState {
    fn default() -> Self {
        Self::new()
    }
}
