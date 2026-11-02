//! Userspace FQ-CoDel (Fair Queueing with Controlled Delay - RFC 8290 & RFC 8289)
//! Provides zero-latency queue isolation for gaming, voice, and DNS traffic during bulk downloads.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::time::{Duration, Instant};
use bytes::Bytes;

const TARGET: Duration = Duration::from_millis(5);     // 5ms target queue delay
const INTERVAL: Duration = Duration::from_millis(100);  // 100ms tolerance interval
const QUANTUM: usize = 1514;                           // 1 MTU quantum for DRR
const NUM_BUCKETS: usize = 1024;                       // 1024 flow buckets

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FlowKey {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
}

pub struct Packet {
    pub key: FlowKey,
    pub payload: Bytes,
    pub enqueue_time: Instant,
    pub len: usize,
}

pub struct FlowBucket {
    pub queue: VecDeque<Packet>,
    pub deficit: i32,
    pub dropping: bool,
    pub first_above_time: Option<Instant>,
    pub drop_next: Instant,
    pub drop_count: u32,
}

impl FlowBucket {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(32),
            deficit: 0,
            dropping: false,
            first_above_time: None,
            drop_next: Instant::now(),
            drop_count: 0,
        }
    }
}

impl Default for FlowBucket {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FqCodelQueue {
    buckets: Vec<FlowBucket>,
    new_flows: VecDeque<usize>,
    old_flows: VecDeque<usize>,
    total_bytes: usize,
    max_bytes: usize,
}

impl FqCodelQueue {
    pub fn new(max_bytes: usize) -> Self {
        let mut buckets = Vec::with_capacity(NUM_BUCKETS);
        for _ in 0..NUM_BUCKETS {
            buckets.push(FlowBucket::new());
        }
        Self {
            buckets,
            new_flows: VecDeque::new(),
            old_flows: VecDeque::new(),
            total_bytes: 0,
            max_bytes,
        }
    }

    #[inline]
    fn hash_flow(&self, key: &FlowKey) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) & (NUM_BUCKETS - 1)
    }

    /// Enqueues a packet into its flow bucket. Returns false if the queue is hard-full.
    pub fn enqueue(&mut self, pkt: Packet) -> bool {
        if self.total_bytes + pkt.len > self.max_bytes {
            return false;
        }

        let idx = self.hash_flow(&pkt.key);
        let bucket = &mut self.buckets[idx];

        if bucket.queue.is_empty() {
            bucket.deficit = QUANTUM as i32;
            self.new_flows.push_back(idx);
        }

        self.total_bytes += pkt.len;
        bucket.queue.push_back(pkt);
        true
    }

    /// CoDel drop state-machine check during dequeue.
    fn codel_should_drop(&mut self, idx: usize, now: Instant) -> (Option<Packet>, bool) {
        let bucket = &mut self.buckets[idx];
        let pkt = match bucket.queue.pop_front() {
            Some(p) => p,
            None => return (None, false),
        };

        self.total_bytes = self.total_bytes.saturating_sub(pkt.len);
        let sojourn = now.saturating_duration_since(pkt.enqueue_time);

        if sojourn < TARGET || bucket.queue.len() <= 1 {
            bucket.first_above_time = None;
            bucket.dropping = false;
            return (Some(pkt), false);
        }

        if let Some(first_above) = bucket.first_above_time {
            if now.saturating_duration_since(first_above) >= INTERVAL {
                if !bucket.dropping {
                    bucket.dropping = true;
                    bucket.drop_count = 1;
                    bucket.drop_next = Self::control_law(now, bucket.drop_count);
                    return (Some(pkt), true); // Drop packet to signal backpressure
                } else if now >= bucket.drop_next {
                    bucket.drop_count = bucket.drop_count.saturating_add(1);
                    bucket.drop_next = Self::control_law(now, bucket.drop_count);
                    return (Some(pkt), true); // Sustained drop
                }
            }
        } else {
            bucket.first_above_time = Some(now);
        }

        (Some(pkt), false)
    }

    #[inline]
    fn control_law(now: Instant, count: u32) -> Instant {
        let inv_sqrt = 1.0 / (count as f64).sqrt();
        let step = Duration::from_secs_f64(INTERVAL.as_secs_f64() * inv_sqrt);
        now + step
    }

    /// Dequeues the next packet using Deficit Round Robin across flows.
    pub fn dequeue(&mut self) -> Option<Packet> {
        let now = Instant::now();

        while !self.new_flows.is_empty() || !self.old_flows.is_empty() {
            let (idx, from_new) = if let Some(&i) = self.new_flows.front() {
                (i, true)
            } else {
                (*self.old_flows.front().unwrap(), false)
            };

            let bucket = &mut self.buckets[idx];
            if bucket.deficit <= 0 {
                bucket.deficit += QUANTUM as i32;
                if from_new {
                    self.new_flows.pop_front();
                    self.old_flows.push_back(idx);
                } else {
                    self.old_flows.rotate_left(1);
                }
                continue;
            }

            loop {
                let (pkt_opt, dropped) = self.codel_should_drop(idx, now);
                match (pkt_opt, dropped) {
                    (Some(pkt), false) => {
                        self.buckets[idx].deficit -= pkt.len as i32;
                        return Some(pkt);
                    }
                    (Some(_dropped_pkt), true) => {
                        // Drop and try the next packet in this flow
                        continue;
                    }
                    (None, _) => {
                        if from_new {
                            self.new_flows.pop_front();
                        } else {
                            self.old_flows.pop_front();
                        }
                        break;
                    }
                }
            }
        }
        None
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}
