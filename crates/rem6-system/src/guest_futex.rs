use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, Tick};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestFutexAddress(u64);

impl GuestFutexAddress {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestThreadGroupId(u64);

impl GuestThreadGroupId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestThreadId(u64);

impl GuestThreadId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestFutexKey {
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
}

impl GuestFutexKey {
    pub const fn new(address: GuestFutexAddress, thread_group: GuestThreadGroupId) -> Self {
        Self {
            address,
            thread_group,
        }
    }

    pub const fn address(self) -> GuestFutexAddress {
        self.address
    }

    pub const fn thread_group(self) -> GuestThreadGroupId {
        self.thread_group
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestFutexWaiter {
    key: GuestFutexKey,
    thread: GuestThreadId,
    partition: PartitionId,
    enqueued_tick: Tick,
    bitset: u32,
}

impl GuestFutexWaiter {
    pub const fn key(self) -> GuestFutexKey {
        self.key
    }

    pub const fn thread(self) -> GuestThreadId {
        self.thread
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn enqueued_tick(self) -> Tick {
        self.enqueued_tick
    }

    pub const fn bitset(self) -> u32 {
        self.bitset
    }

    pub const fn matches_bitset(self, wake_bitset: u32) -> bool {
        self.bitset & wake_bitset != 0
    }

    const fn with_key(mut self, key: GuestFutexKey) -> Self {
        self.key = key;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestFutexWaitRequest {
    key: GuestFutexKey,
    thread: GuestThreadId,
    partition: PartitionId,
    tick: Tick,
    expected: i32,
    observed: i32,
    bitset: u32,
}

impl GuestFutexWaitRequest {
    pub const fn new(
        key: GuestFutexKey,
        thread: GuestThreadId,
        partition: PartitionId,
        tick: Tick,
        expected: i32,
        observed: i32,
    ) -> Self {
        Self {
            key,
            thread,
            partition,
            tick,
            expected,
            observed,
            bitset: u32::MAX,
        }
    }

    pub const fn with_bitset(mut self, bitset: u32) -> Self {
        self.bitset = bitset;
        self
    }

    pub const fn key(self) -> GuestFutexKey {
        self.key
    }

    pub const fn thread(self) -> GuestThreadId {
        self.thread
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn expected(self) -> i32 {
        self.expected
    }

    pub const fn observed(self) -> i32 {
        self.observed
    }

    pub const fn bitset(self) -> u32 {
        self.bitset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestFutexWaitOutcome {
    Queued {
        thread: GuestThreadId,
        waiter_count: usize,
    },
    WouldBlock {
        expected: i32,
        observed: i32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestFutexWakeRecord {
    waiter: GuestFutexWaiter,
    wake_tick: Tick,
}

impl GuestFutexWakeRecord {
    const fn new(waiter: GuestFutexWaiter, wake_tick: Tick) -> Self {
        Self { waiter, wake_tick }
    }

    pub const fn waiter(self) -> GuestFutexWaiter {
        self.waiter
    }

    pub const fn wake_tick(self) -> Tick {
        self.wake_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFutexWakeOutcome {
    key: GuestFutexKey,
    requested_count: usize,
    wake_tick: Tick,
    woken: Vec<GuestFutexWakeRecord>,
}

impl GuestFutexWakeOutcome {
    const fn empty(key: GuestFutexKey, requested_count: usize, wake_tick: Tick) -> Self {
        Self {
            key,
            requested_count,
            wake_tick,
            woken: Vec::new(),
        }
    }

    pub const fn key(&self) -> GuestFutexKey {
        self.key
    }

    pub const fn requested_count(&self) -> usize {
        self.requested_count
    }

    pub const fn wake_tick(&self) -> Tick {
        self.wake_tick
    }

    pub fn records(&self) -> &[GuestFutexWakeRecord] {
        &self.woken
    }

    pub fn woken_count(&self) -> usize {
        self.woken.len()
    }

    pub fn woken_threads(&self) -> Vec<GuestThreadId> {
        self.woken
            .iter()
            .map(|record| record.waiter.thread())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestFutexRequeueRecord {
    waiter: GuestFutexWaiter,
    source_key: GuestFutexKey,
    target_key: GuestFutexKey,
    requeue_tick: Tick,
}

impl GuestFutexRequeueRecord {
    const fn new(
        waiter: GuestFutexWaiter,
        source_key: GuestFutexKey,
        target_key: GuestFutexKey,
        requeue_tick: Tick,
    ) -> Self {
        Self {
            waiter,
            source_key,
            target_key,
            requeue_tick,
        }
    }

    pub const fn waiter(self) -> GuestFutexWaiter {
        self.waiter
    }

    pub const fn source_key(self) -> GuestFutexKey {
        self.source_key
    }

    pub const fn target_key(self) -> GuestFutexKey {
        self.target_key
    }

    pub const fn requeue_tick(self) -> Tick {
        self.requeue_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFutexRequeueOutcome {
    source_key: GuestFutexKey,
    target_key: GuestFutexKey,
    requested_wake_count: usize,
    requested_requeue_count: usize,
    tick: Tick,
    woken: Vec<GuestFutexWakeRecord>,
    requeued: Vec<GuestFutexRequeueRecord>,
}

impl GuestFutexRequeueOutcome {
    const fn empty(
        source_key: GuestFutexKey,
        target_key: GuestFutexKey,
        requested_wake_count: usize,
        requested_requeue_count: usize,
        tick: Tick,
    ) -> Self {
        Self {
            source_key,
            target_key,
            requested_wake_count,
            requested_requeue_count,
            tick,
            woken: Vec::new(),
            requeued: Vec::new(),
        }
    }

    pub const fn source_key(&self) -> GuestFutexKey {
        self.source_key
    }

    pub const fn target_key(&self) -> GuestFutexKey {
        self.target_key
    }

    pub const fn requested_wake_count(&self) -> usize {
        self.requested_wake_count
    }

    pub const fn requested_requeue_count(&self) -> usize {
        self.requested_requeue_count
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn woken(&self) -> &[GuestFutexWakeRecord] {
        &self.woken
    }

    pub fn requeued(&self) -> &[GuestFutexRequeueRecord] {
        &self.requeued
    }

    pub fn woken_threads(&self) -> Vec<GuestThreadId> {
        self.woken
            .iter()
            .map(|record| record.waiter.thread())
            .collect()
    }

    pub fn requeued_threads(&self) -> Vec<GuestThreadId> {
        self.requeued
            .iter()
            .map(|record| record.waiter.thread())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestFutexError {
    DuplicateWaiter { thread: GuestThreadId },
    ZeroBitset { thread: GuestThreadId },
}

impl fmt::Display for GuestFutexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateWaiter { thread } => write!(
                formatter,
                "guest futex thread {} is already waiting",
                thread.get()
            ),
            Self::ZeroBitset { thread } => write!(
                formatter,
                "guest futex thread {} cannot wait with an empty bitset",
                thread.get()
            ),
        }
    }
}

impl Error for GuestFutexError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GuestFutexTable {
    waiters: BTreeMap<GuestFutexKey, VecDeque<GuestFutexWaiter>>,
    waiting_threads: BTreeMap<GuestThreadId, GuestFutexKey>,
}

impl GuestFutexTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.waiters.is_empty()
    }

    pub fn total_waiter_count(&self) -> usize {
        self.waiters.values().map(VecDeque::len).sum()
    }

    pub fn waiter_count(
        &self,
        address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
    ) -> usize {
        self.waiter_queue(GuestFutexKey::new(address, thread_group))
            .map_or(0, VecDeque::len)
    }

    pub fn waiter_threads(
        &self,
        address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
    ) -> Vec<GuestThreadId> {
        self.waiter_queue(GuestFutexKey::new(address, thread_group))
            .into_iter()
            .flat_map(|queue| queue.iter().map(|waiter| waiter.thread()))
            .collect()
    }

    pub fn is_waiting(&self, thread: GuestThreadId) -> bool {
        self.waiting_threads.contains_key(&thread)
    }

    pub fn wait(
        &mut self,
        request: GuestFutexWaitRequest,
    ) -> Result<GuestFutexWaitOutcome, GuestFutexError> {
        if request.bitset() == 0 {
            return Err(GuestFutexError::ZeroBitset {
                thread: request.thread(),
            });
        }
        if request.expected() != request.observed() {
            return Ok(GuestFutexWaitOutcome::WouldBlock {
                expected: request.expected(),
                observed: request.observed(),
            });
        }
        let thread = request.thread();
        if self.waiting_threads.contains_key(&thread) {
            return Err(GuestFutexError::DuplicateWaiter { thread });
        }

        let key = request.key();
        let waiter = GuestFutexWaiter {
            key,
            thread,
            partition: request.partition(),
            enqueued_tick: request.tick(),
            bitset: request.bitset(),
        };
        let queue = self.waiters.entry(key).or_default();
        queue.push_back(waiter);
        self.waiting_threads.insert(thread, key);

        Ok(GuestFutexWaitOutcome::Queued {
            thread,
            waiter_count: queue.len(),
        })
    }

    pub fn wake(
        &mut self,
        address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
        count: usize,
        tick: Tick,
    ) -> Result<GuestFutexWakeOutcome, GuestFutexError> {
        self.wake_bitset(address, thread_group, count, u32::MAX, tick)
    }

    pub fn wake_bitset(
        &mut self,
        address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
        count: usize,
        bitset: u32,
        tick: Tick,
    ) -> Result<GuestFutexWakeOutcome, GuestFutexError> {
        let key = GuestFutexKey::new(address, thread_group);
        let Some(mut queue) = self.waiters.remove(&key) else {
            return Ok(GuestFutexWakeOutcome::empty(key, count, tick));
        };

        let mut outcome = GuestFutexWakeOutcome::empty(key, count, tick);
        let mut remaining = VecDeque::new();
        while let Some(waiter) = queue.pop_front() {
            if outcome.woken.len() < count && waiter.matches_bitset(bitset) {
                self.waiting_threads.remove(&waiter.thread());
                outcome.woken.push(GuestFutexWakeRecord::new(waiter, tick));
            } else {
                remaining.push_back(waiter);
            }
        }
        self.restore_queue(key, remaining);

        Ok(outcome)
    }

    pub fn requeue(
        &mut self,
        source_address: GuestFutexAddress,
        target_address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
        wake_count: usize,
        requeue_count: usize,
        tick: Tick,
    ) -> Result<GuestFutexRequeueOutcome, GuestFutexError> {
        let source_key = GuestFutexKey::new(source_address, thread_group);
        let target_key = GuestFutexKey::new(target_address, thread_group);
        let Some(mut source_queue) = self.waiters.remove(&source_key) else {
            return Ok(GuestFutexRequeueOutcome::empty(
                source_key,
                target_key,
                wake_count,
                requeue_count,
                tick,
            ));
        };

        let mut outcome = GuestFutexRequeueOutcome::empty(
            source_key,
            target_key,
            wake_count,
            requeue_count,
            tick,
        );
        while outcome.woken.len() < wake_count {
            let Some(waiter) = source_queue.pop_front() else {
                break;
            };
            self.waiting_threads.remove(&waiter.thread());
            outcome.woken.push(GuestFutexWakeRecord::new(waiter, tick));
        }

        let mut moved = VecDeque::new();
        while outcome.requeued.len() < requeue_count {
            let Some(waiter) = source_queue.pop_front() else {
                break;
            };
            let waiter = waiter.with_key(target_key);
            self.waiting_threads.insert(waiter.thread(), target_key);
            outcome.requeued.push(GuestFutexRequeueRecord::new(
                waiter, source_key, target_key, tick,
            ));
            moved.push_back(waiter);
        }

        self.restore_queue(source_key, source_queue);
        if !moved.is_empty() {
            self.waiters.entry(target_key).or_default().extend(moved);
        }

        Ok(outcome)
    }

    fn waiter_queue(&self, key: GuestFutexKey) -> Option<&VecDeque<GuestFutexWaiter>> {
        self.waiters.get(&key)
    }

    fn restore_queue(&mut self, key: GuestFutexKey, queue: VecDeque<GuestFutexWaiter>) {
        if !queue.is_empty() {
            self.waiters.insert(key, queue);
        }
    }
}
