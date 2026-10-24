use std::net::IpAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};

use super::dto::{Phase, StartOptions, Status, TunnelError};
use super::dto::Target;
use crate::engine::{Counters, Engine};
use crate::engine::UdpMode;
use crate::platform::Uplink;

struct Inner {
    phase: Phase,
    target: Target,
    engine: Option<Engine>,
    link: Option<(Uplink, UdpMode)>,
}

#[derive(Clone)]
pub struct TunnelState {
    inner: Arc<Mutex<Inner>>,
    pub counters: Arc<Counters>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                phase: Phase::Stopped,
                target: Target::default_phone(),
                engine: None,
                link: None,
            })),
            counters: Arc::new(Counters::default()),
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
        {
            let mut g = self.lock();
            if g.phase != Phase::Stopped {
                return Err(TunnelError::Conflict("tunnel is already running"));
            }
            g.phase = Phase::Starting;
            g.target = target;
        }
        let started = Engine::start(phone, target.port, auth, resolver, self.counters.clone());
        let mut g = self.lock();
        match started {
            Ok(engine) => {
                g.link = Some((engine.uplink().clone(), engine.udp_mode()));
                g.engine = Some(engine);
                g.phase = Phase::Running;
                Ok(Self::snapshot(&g, g.phase))
            }
            Err(e) => {
                g.phase = Phase::Stopped;
                Err(TunnelError::from(e))
            }
        }
    }

    /// Blocking: waits for the watchdog to remove the firewall rules.
    pub fn stop(&self) -> Status {
        let engine = {
            let mut g = self.lock();
            if g.phase == Phase::Starting {
                return Self::snapshot(&g, g.phase);
            }
            g.phase = Phase::Stopped;
            g.link = None;
            g.engine.take()
        };
        if let Some(engine) = engine {
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
