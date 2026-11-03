use std::net::IpAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use super::dto::{Phase, StartOptions, Status, Target, TunnelError};
use crate::engine::{Counters, Engine, UdpMode};
use crate::platform::Uplink;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InternalPhase {
    Stopped,
    Starting,
    Running,
    Stopping,
}

struct Inner {
    phase: InternalPhase,
    target: Target,
    engine: Option<Engine>,
    link: Option<(Uplink, UdpMode)>,
    cancel_requested: bool,
}

#[derive(Clone)]
pub struct TunnelState {
    inner: Arc<Mutex<Inner>>,
    phase_cv: Arc<Condvar>,
    pub counters: Arc<Counters>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                phase: InternalPhase::Stopped,
                target: Target::default_phone(),
                engine: None,
                link: None,
                cancel_requested: false,
            })),
            phase_cv: Arc::new(Condvar::new()),
            counters: Arc::new(Counters::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn status(&self) -> Status {
        let g = self.lock();
        let phase = match g.phase {
            InternalPhase::Stopped => Phase::Stopped,
            InternalPhase::Starting => Phase::Starting,
            InternalPhase::Stopping => Phase::Stopped,
            InternalPhase::Running => {
                if !self.counters.link_up.load(Ordering::Relaxed) {
                    Phase::Degraded
                } else {
                    Phase::Running
                }
            }
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

    pub fn preset_target(&self, target: Target) {
        let mut g = self.lock();
        if g.phase == InternalPhase::Stopped {
            g.target = target;
        }
    }

    pub fn start(&self, opts: StartOptions) -> Result<Status, TunnelError> {
        let StartOptions { target, auth, resolver } = opts;
        let IpAddr::V4(phone) = target.host else {
            return Err(TunnelError::Validation("only IPv4 proxy addresses are supported"));
        };

        // Transition to Starting; wait if currently Stopping
        {
            let mut g = self.lock();
            while g.phase == InternalPhase::Stopping {
                g = self.phase_cv.wait(g).unwrap_or_else(|p| p.into_inner());
            }
            if g.phase != InternalPhase::Stopped {
                return Err(TunnelError::Conflict("tunnel is already running or starting"));
            }
            g.phase = InternalPhase::Starting;
            g.cancel_requested = false;
            g.target = target;
            g.link = None;
        }

        // Reset telemetry counters cleanly (including rtt and jitter)
        self.counters.reset();

        let started = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Engine::start(phone, target.port, auth, resolver, self.counters.clone())
        }));

        let engine_result = match started {
            Ok(res) => res,
            Err(payload) => {
                let mut g = self.lock();
                g.phase = InternalPhase::Stopped;
                g.cancel_requested = false;
                g.link = None;
                self.phase_cv.notify_all();
                std::panic::resume_unwind(payload);
            }
        };

        match engine_result {
            Ok(engine) => {
                let mut g = self.lock();
                if g.cancel_requested || g.phase != InternalPhase::Starting {
                    g.phase = InternalPhase::Stopping;
                    self.phase_cv.notify_all();
                    drop(g);

                    engine.stop();

                    let mut g = self.lock();
                    g.phase = InternalPhase::Stopped;
                    g.cancel_requested = false;
                    g.link = None;
                    self.phase_cv.notify_all();
                    return Err(TunnelError::Conflict("tunnel startup was cancelled"));
                }

                let link_info = (engine.uplink().clone(), engine.udp_mode());
                g.link = Some(link_info);
                g.engine = Some(engine);
                g.phase = InternalPhase::Running;
                self.phase_cv.notify_all();

                let phase = if !self.counters.link_up.load(Ordering::Relaxed) {
                    Phase::Degraded
                } else {
                    Phase::Running
                };
                Ok(Self::snapshot(&g, phase))
            }
            Err(e) => {
                let mut g = self.lock();
                g.phase = InternalPhase::Stopped;
                g.cancel_requested = false;
                g.link = None;
                self.phase_cv.notify_all();
                Err(TunnelError::from(e))
            }
        }
    }

    pub fn stop(&self) -> Status {
        let mut g = self.lock();

        if g.phase == InternalPhase::Starting {
            g.cancel_requested = true;
            while g.phase == InternalPhase::Starting {
                g = self.phase_cv.wait(g).unwrap_or_else(|p| p.into_inner());
            }
        }

        while g.phase == InternalPhase::Stopping {
            g = self.phase_cv.wait(g).unwrap_or_else(|p| p.into_inner());
        }

        if g.phase == InternalPhase::Stopped {
            return Self::snapshot(&g, Phase::Stopped);
        }

        g.phase = InternalPhase::Stopping;
        g.link = None;
        let engine = g.engine.take();
        drop(g);

        if let Some(engine) = engine {
            engine.stop();
        }

        let mut g = self.lock();
        g.phase = InternalPhase::Stopped;
        self.phase_cv.notify_all();
        Self::snapshot(&g, Phase::Stopped)
    }
}

impl Default for TunnelState {
    fn default() -> Self {
        Self::new()
    }
}
