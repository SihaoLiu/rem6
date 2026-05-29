use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

use crate::{InterruptController, InterruptLineId, InterruptPriority, InterruptTargetId};

pub const PLIC_MMIO_REGISTER_BYTES: u64 = 4;
pub const PLIC_MMIO_PRIORITY_STRIDE: u64 = PLIC_MMIO_REGISTER_BYTES;
pub const PLIC_MMIO_PENDING_BASE_OFFSET: u64 = 0x1000;
pub const PLIC_MMIO_ENABLE_BASE_OFFSET: u64 = 0x2000;
pub const PLIC_MMIO_ENABLE_CONTEXT_STRIDE: u64 = 0x80;
pub const PLIC_MMIO_CONTEXT_BASE_OFFSET: u64 = 0x20_0000;
pub const PLIC_MMIO_CONTEXT_STRIDE: u64 = 0x1000;
pub const PLIC_MMIO_THRESHOLD_OFFSET: u64 = 0;
pub const PLIC_MMIO_CLAIM_COMPLETE_OFFSET: u64 = 4;

type PlicContextKey = (InterruptTargetId, PartitionId);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PlicContextRoute {
    context: u64,
    target: InterruptTargetId,
    target_partition: PartitionId,
}

impl PlicContextRoute {
    pub const fn new(
        context: u64,
        target: InterruptTargetId,
        target_partition: PartitionId,
    ) -> Self {
        Self {
            context,
            target,
            target_partition,
        }
    }

    pub const fn context(self) -> u64 {
        self.context
    }

    pub const fn target(self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(self) -> PartitionId {
        self.target_partition
    }

    const fn key(self) -> PlicContextKey {
        (self.target, self.target_partition)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlicContextSnapshot {
    context: u64,
    target: InterruptTargetId,
    target_partition: PartitionId,
    enabled: Vec<InterruptLineId>,
    threshold: InterruptPriority,
}

impl PlicContextSnapshot {
    pub fn new(
        context: u64,
        target: InterruptTargetId,
        target_partition: PartitionId,
        mut enabled: Vec<InterruptLineId>,
        threshold: InterruptPriority,
    ) -> Self {
        enabled.sort();
        enabled.dedup();
        Self {
            context,
            target,
            target_partition,
            enabled,
            threshold,
        }
    }

    pub const fn context(&self) -> u64 {
        self.context
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub fn enabled(&self) -> &[InterruptLineId] {
        &self.enabled
    }

    pub const fn threshold(&self) -> InterruptPriority {
        self.threshold
    }

    const fn route(&self) -> PlicContextRoute {
        PlicContextRoute::new(self.context, self.target, self.target_partition)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlicSnapshot {
    base: Address,
    contexts: Vec<PlicContextSnapshot>,
}

impl PlicSnapshot {
    pub fn new(base: Address, mut contexts: Vec<PlicContextSnapshot>) -> Self {
        contexts.sort_by_key(|context| context.context());
        Self { base, contexts }
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn contexts(&self) -> &[PlicContextSnapshot] {
        &self.contexts
    }

    fn routes(&self) -> Vec<PlicContextRoute> {
        self.contexts
            .iter()
            .map(PlicContextSnapshot::route)
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
struct PlicMmioState {
    enabled: BTreeMap<PlicContextKey, BTreeSet<InterruptLineId>>,
    thresholds: BTreeMap<PlicContextKey, InterruptPriority>,
}

impl PlicMmioState {
    fn enabled(&self, key: PlicContextKey, line: InterruptLineId) -> bool {
        self.enabled
            .get(&key)
            .is_some_and(|lines| lines.contains(&line))
    }

    fn threshold(&self, key: PlicContextKey) -> InterruptPriority {
        self.thresholds
            .get(&key)
            .copied()
            .unwrap_or(InterruptPriority::ZERO)
    }

    fn read_enable_word(&self, key: PlicContextKey, word: u64) -> u32 {
        let Some(lines) = self.enabled.get(&key) else {
            return 0;
        };
        lines
            .iter()
            .filter(|line| line.get() / 32 == word)
            .fold(0u32, |bits, line| bits | (1u32 << (line.get() % 32)))
    }

    fn write_enable_word(&mut self, key: PlicContextKey, word: u64, value: u32) {
        let lines = self.enabled.entry(key).or_default();
        lines.retain(|line| line.get() / 32 != word);
        for bit in 0..32 {
            if value & (1u32 << bit) != 0 {
                lines.insert(InterruptLineId::new(word * 32 + bit));
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlicMmioDevice {
    controller: Arc<Mutex<InterruptController>>,
    base: Address,
    target: InterruptTargetId,
    target_partition: PartitionId,
    contexts: Arc<BTreeMap<u64, PlicContextKey>>,
    state: Arc<Mutex<PlicMmioState>>,
}

impl PlicMmioDevice {
    pub fn new(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        target: InterruptTargetId,
        target_partition: PartitionId,
    ) -> Self {
        Self::with_contexts(
            controller,
            base,
            [PlicContextRoute::new(0, target, target_partition)],
        )
    }

    pub fn with_contexts(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        contexts: impl IntoIterator<Item = PlicContextRoute>,
    ) -> Self {
        let routes = contexts.into_iter().collect::<Vec<_>>();
        let primary = routes.first().copied().unwrap_or(PlicContextRoute::new(
            0,
            InterruptTargetId::new(0),
            PartitionId::new(0),
        ));
        Self {
            controller,
            base,
            target: primary.target(),
            target_partition: primary.target_partition(),
            contexts: Arc::new(
                routes
                    .into_iter()
                    .map(|route| (route.context(), route.key()))
                    .collect(),
            ),
            state: Arc::new(Mutex::new(PlicMmioState::default())),
        }
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.controller)
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub fn snapshot(&self) -> PlicSnapshot {
        let state = self.state.lock().expect("plic state lock");
        let contexts = self
            .context_routes()
            .into_iter()
            .map(|route| {
                let key = route.key();
                let enabled = state
                    .enabled
                    .get(&key)
                    .map(|lines| lines.iter().copied().collect())
                    .unwrap_or_default();
                PlicContextSnapshot::new(
                    route.context(),
                    route.target(),
                    route.target_partition(),
                    enabled,
                    state.threshold(key),
                )
            })
            .collect();
        PlicSnapshot::new(self.base, contexts)
    }

    pub fn validate_snapshot(&self, snapshot: &PlicSnapshot) -> Result<(), PlicError> {
        if snapshot.base() != self.base {
            return Err(PlicError::SnapshotBaseMismatch {
                expected: self.base,
                actual: snapshot.base(),
            });
        }

        let expected = self.context_routes();
        let actual = snapshot.routes();
        if let Some(context) = duplicate_context(&actual) {
            return Err(PlicError::DuplicateSnapshotContext { context });
        }
        if actual != expected {
            return Err(PlicError::SnapshotContextMismatch { expected, actual });
        }

        Ok(())
    }

    pub fn restore(&self, snapshot: &PlicSnapshot) -> Result<(), PlicError> {
        self.validate_snapshot(snapshot)?;
        let mut state = PlicMmioState::default();
        for context in snapshot.contexts() {
            let key = (context.target(), context.target_partition());
            if !context.enabled().is_empty() {
                state
                    .enabled
                    .insert(key, context.enabled().iter().copied().collect());
            }
            if context.threshold() != InterruptPriority::ZERO {
                state.thresholds.insert(key, context.threshold());
            }
        }
        *self.state.lock().expect("plic state lock") = state;
        Ok(())
    }

    fn context_routes(&self) -> Vec<PlicContextRoute> {
        self.contexts
            .iter()
            .map(|(context, (target, target_partition))| {
                PlicContextRoute::new(*context, *target, *target_partition)
            })
            .collect()
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_at_tick(context.now(), request)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_at_tick(context.now(), request)
    }

    fn respond_at_tick(
        &self,
        tick: Tick,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        if offset < PLIC_MMIO_PENDING_BASE_OFFSET {
            return self.respond_priority(request, InterruptLineId::new(offset / 4));
        }
        if offset < PLIC_MMIO_ENABLE_BASE_OFFSET {
            return self.respond_pending(request, (offset - PLIC_MMIO_PENDING_BASE_OFFSET) / 4);
        }
        if offset < PLIC_MMIO_CONTEXT_BASE_OFFSET {
            let window = offset - PLIC_MMIO_ENABLE_BASE_OFFSET;
            let context = window / PLIC_MMIO_ENABLE_CONTEXT_STRIDE;
            let word_offset = window % PLIC_MMIO_ENABLE_CONTEXT_STRIDE;
            if word_offset.is_multiple_of(4) {
                let key = self.context_key(context, request)?;
                return self.respond_enable(request, key, word_offset / 4);
            }
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }

        let window = offset - PLIC_MMIO_CONTEXT_BASE_OFFSET;
        let context = window / PLIC_MMIO_CONTEXT_STRIDE;
        let context_offset = window % PLIC_MMIO_CONTEXT_STRIDE;
        let key = self.context_key(context, request)?;
        match context_offset {
            PLIC_MMIO_THRESHOLD_OFFSET => self.respond_threshold(request, key),
            PLIC_MMIO_CLAIM_COMPLETE_OFFSET => self.respond_claim_complete(tick, request, key),
            _ => Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            }),
        }
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != PLIC_MMIO_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: PLIC_MMIO_REGISTER_BYTES,
                actual: request.size().bytes(),
            });
        }
        Ok(())
    }

    fn offset(&self, request: &MmioRequest) -> Result<u64, MmioError> {
        let offset = request
            .range()
            .start()
            .get()
            .checked_sub(self.base.get())
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })?;
        if !offset.is_multiple_of(PLIC_MMIO_REGISTER_BYTES) {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }
        Ok(offset)
    }

    fn context_key(
        &self,
        context: u64,
        request: &MmioRequest,
    ) -> Result<PlicContextKey, MmioError> {
        self.contexts
            .get(&context)
            .copied()
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })
    }

    fn respond_priority(
        &self,
        request: &MmioRequest,
        line: InterruptLineId,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => {
                let priority = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .priority(line)
                    .map_err(|error| MmioError::DeviceError {
                        request: request.id(),
                        message: error.to_string(),
                    })?;
                Ok(MmioResponse::completed(
                    request.id(),
                    Some(le32(priority.get())),
                ))
            }
            MmioOperation::Write => {
                let priority = InterruptPriority::new(self.u32_from_write(request)?);
                self.controller
                    .lock()
                    .expect("interrupt controller lock")
                    .set_priority(line, priority)
                    .map_err(|error| MmioError::DeviceError {
                        request: request.id(),
                        message: error.to_string(),
                    })?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_pending(&self, request: &MmioRequest, word: u64) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => {
                let bits = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .pending()
                    .into_iter()
                    .filter(|pending| pending.line().get() / 32 == word)
                    .fold(0u32, |bits, pending| {
                        bits | (1u32 << (pending.line().get() % 32))
                    });
                Ok(MmioResponse::completed(request.id(), Some(le32(bits))))
            }
            MmioOperation::Write => Err(MmioError::AccessDenied {
                request: request.id(),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            }),
        }
    }

    fn respond_enable(
        &self,
        request: &MmioRequest,
        key: PlicContextKey,
        word: u64,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => {
                let bits = self
                    .state
                    .lock()
                    .expect("plic state lock")
                    .read_enable_word(key, word);
                Ok(MmioResponse::completed(request.id(), Some(le32(bits))))
            }
            MmioOperation::Write => {
                let value = self.u32_from_write(request)?;
                self.state
                    .lock()
                    .expect("plic state lock")
                    .write_enable_word(key, word, value);
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_threshold(
        &self,
        request: &MmioRequest,
        key: PlicContextKey,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => {
                let threshold = self.state.lock().expect("plic state lock").threshold(key);
                Ok(MmioResponse::completed(
                    request.id(),
                    Some(le32(threshold.get())),
                ))
            }
            MmioOperation::Write => {
                let threshold = InterruptPriority::new(self.u32_from_write(request)?);
                self.state
                    .lock()
                    .expect("plic state lock")
                    .thresholds
                    .insert(key, threshold);
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_claim_complete(
        &self,
        tick: Tick,
        request: &MmioRequest,
        key: PlicContextKey,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => {
                let state = self.state.lock().expect("plic state lock");
                let line = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .claim_filtered(key.0, key.1, tick, |pending, priority| {
                        state.enabled(key, pending.line()) && priority > state.threshold(key)
                    })
                    .map(|claim| u32::try_from(claim.line().get()))
                    .transpose()
                    .map_err(|_| MmioError::DeviceError {
                        request: request.id(),
                        message: "PLIC claim line does not fit u32".to_string(),
                    })?
                    .unwrap_or_default();
                Ok(MmioResponse::completed(request.id(), Some(le32(line))))
            }
            MmioOperation::Write => {
                let line = InterruptLineId::new(u64::from(self.u32_from_write(request)?));
                self.controller
                    .lock()
                    .expect("interrupt controller lock")
                    .complete(key.0, key.1, line, tick)
                    .map_err(|error| MmioError::DeviceError {
                        request: request.id(),
                        message: error.to_string(),
                    })?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn u32_from_write(&self, request: &MmioRequest) -> Result<u32, MmioError> {
        let data = request.data().ok_or(MmioError::MissingWriteData {
            request: request.id(),
        })?;
        if data.len() as u64 != PLIC_MMIO_REGISTER_BYTES {
            return Err(MmioError::PayloadSizeMismatch {
                request: request.id(),
                expected: PLIC_MMIO_REGISTER_BYTES,
                actual: data.len() as u64,
            });
        }
        let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
            request: request.id(),
        })?;
        validate_plic_mmio_mask(request, mask)?;

        let mut bytes = [0; 4];
        for (index, byte) in data.iter().enumerate() {
            if mask.bits()[index] {
                bytes[index] = *byte;
            }
        }
        Ok(u32::from_le_bytes(bytes))
    }
}

impl MmioDevice for PlicMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        PlicMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        PlicMmioDevice::respond_parallel(self, context, request)
    }
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn validate_plic_mmio_mask(request: &MmioRequest, mask: &ByteMask) -> Result<(), MmioError> {
    if mask.len() != PLIC_MMIO_REGISTER_BYTES {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: PLIC_MMIO_REGISTER_BYTES,
            actual: mask.len(),
        });
    }
    Ok(())
}

fn duplicate_context(routes: &[PlicContextRoute]) -> Option<u64> {
    let mut seen = BTreeSet::new();
    routes
        .iter()
        .find_map(|route| (!seen.insert(route.context())).then_some(route.context()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlicError {
    SnapshotBaseMismatch {
        expected: Address,
        actual: Address,
    },
    SnapshotContextMismatch {
        expected: Vec<PlicContextRoute>,
        actual: Vec<PlicContextRoute>,
    },
    DuplicateSnapshotContext {
        context: u64,
    },
}

impl fmt::Display for PlicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SnapshotBaseMismatch { expected, actual } => write!(
                formatter,
                "PLIC snapshot base mismatch: expected {}, actual {}",
                expected.get(),
                actual.get()
            ),
            Self::SnapshotContextMismatch { .. } => {
                write!(
                    formatter,
                    "PLIC snapshot context routes do not match device"
                )
            }
            Self::DuplicateSnapshotContext { context } => {
                write!(
                    formatter,
                    "PLIC snapshot contains duplicate context {context}"
                )
            }
        }
    }
}

impl Error for PlicError {}
