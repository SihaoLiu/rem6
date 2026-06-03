use std::cmp::Reverse;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlicOutputLine {
    line: InterruptLineId,
    priority: InterruptPriority,
}

impl PlicOutputLine {
    pub const fn new(line: InterruptLineId, priority: InterruptPriority) -> Self {
        Self { line, priority }
    }

    pub const fn line(self) -> InterruptLineId {
        self.line
    }

    pub const fn priority(self) -> InterruptPriority {
        self.priority
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlicContextOutputState {
    context: u64,
    target: InterruptTargetId,
    target_partition: PartitionId,
    output: Option<PlicOutputLine>,
}

impl PlicContextOutputState {
    pub const fn new(
        context: u64,
        target: InterruptTargetId,
        target_partition: PartitionId,
        output: Option<PlicOutputLine>,
    ) -> Self {
        Self {
            context,
            target,
            target_partition,
            output,
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

    pub const fn output(&self) -> Option<PlicOutputLine> {
        self.output
    }

    pub const fn is_asserted(&self) -> bool {
        self.output.is_some()
    }

    pub const fn line(&self) -> Option<InterruptLineId> {
        match self.output {
            Some(output) => Some(output.line()),
            None => None,
        }
    }

    pub const fn priority(&self) -> Option<InterruptPriority> {
        match self.output {
            Some(output) => Some(output.priority()),
            None => None,
        }
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

    fn read_enable_word(&self, key: PlicContextKey, word: u64, mask: u32) -> u32 {
        let Some(lines) = self.enabled.get(&key) else {
            return 0;
        };
        lines
            .iter()
            .filter(|line| line.get() / 32 == word)
            .fold(0u32, |bits, line| bits | (1u32 << (line.get() % 32)))
            & mask
    }

    fn write_enable_word(&mut self, key: PlicContextKey, word: u64, value: u32, mask: u32) {
        let lines = self.enabled.entry(key).or_default();
        let value = value & mask;
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
    source_count: Option<u64>,
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
        Self::with_contexts_inner(
            controller,
            base,
            [PlicContextRoute::new(0, target, target_partition)],
            None,
        )
    }

    pub fn with_source_count(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source_count: u32,
    ) -> Self {
        Self::with_contexts_inner(
            controller,
            base,
            [PlicContextRoute::new(0, target, target_partition)],
            Some(u64::from(source_count)),
        )
    }

    pub fn with_contexts(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        contexts: impl IntoIterator<Item = PlicContextRoute>,
    ) -> Self {
        Self::with_contexts_inner(controller, base, contexts, None)
    }

    pub fn with_contexts_and_source_count(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        contexts: impl IntoIterator<Item = PlicContextRoute>,
        source_count: u32,
    ) -> Self {
        Self::with_contexts_inner(controller, base, contexts, Some(u64::from(source_count)))
    }

    fn with_contexts_inner(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        contexts: impl IntoIterator<Item = PlicContextRoute>,
        source_count: Option<u64>,
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
            source_count,
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

    pub const fn source_count(&self) -> Option<u64> {
        self.source_count
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

    pub fn context_output_states(&self) -> Vec<PlicContextOutputState> {
        let state = self.state.lock().expect("plic state lock");
        let controller = self.controller.lock().expect("interrupt controller lock");
        let claimed_contexts = controller
            .claimed()
            .into_iter()
            .map(|claim| (claim.target(), claim.target_partition()))
            .collect::<BTreeSet<_>>();

        self.context_routes()
            .into_iter()
            .map(|route| {
                PlicContextOutputState::new(
                    route.context(),
                    route.target(),
                    route.target_partition(),
                    self.context_output_line(route, &state, &controller, &claimed_contexts),
                )
            })
            .collect()
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
        for context in snapshot.contexts() {
            for line in context.enabled() {
                if !self.source_in_range(*line) {
                    return Err(PlicError::SnapshotSourceOutOfRange {
                        line: *line,
                        source_count: self.source_count,
                    });
                }
            }
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

    fn context_output_line(
        &self,
        route: PlicContextRoute,
        state: &PlicMmioState,
        controller: &InterruptController,
        claimed_contexts: &BTreeSet<PlicContextKey>,
    ) -> Option<PlicOutputLine> {
        let key = route.key();
        if claimed_contexts.contains(&key) {
            return None;
        }

        let threshold = state.threshold(key);
        controller
            .pending()
            .into_iter()
            .filter(|pending| {
                pending.target() == route.target()
                    && pending.target_partition() == route.target_partition()
            })
            .filter(|pending| self.source_in_range(pending.line()))
            .filter(|pending| state.enabled(key, pending.line()))
            .filter_map(|pending| {
                let priority = controller.priority(pending.line()).ok()?;
                (priority > threshold).then_some(PlicOutputLine::new(pending.line(), priority))
            })
            .min_by_key(|output| (Reverse(output.priority()), output.line()))
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
        if !self.source_in_range(line) {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }
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
        if !self.source_word_in_range(word) {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }
        match request.operation() {
            MmioOperation::Read => {
                let bits = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .pending()
                    .into_iter()
                    .filter(|pending| self.source_in_range(pending.line()))
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
        if !self.source_word_in_range(word) {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }
        let mask = self.source_word_mask(word);
        match request.operation() {
            MmioOperation::Read => {
                let bits = self
                    .state
                    .lock()
                    .expect("plic state lock")
                    .read_enable_word(key, word, mask);
                Ok(MmioResponse::completed(request.id(), Some(le32(bits))))
            }
            MmioOperation::Write => {
                let value = self.u32_from_write(request)?;
                self.state
                    .lock()
                    .expect("plic state lock")
                    .write_enable_word(key, word, value, mask);
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
                        self.source_in_range(pending.line())
                            && state.enabled(key, pending.line())
                            && priority > state.threshold(key)
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
                if !self.source_in_range(line) {
                    return Err(MmioError::DeviceError {
                        request: request.id(),
                        message: self.source_out_of_range_message(line),
                    });
                }
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

    fn source_in_range(&self, line: InterruptLineId) -> bool {
        self.source_count
            .map(|source_count| line.get() <= source_count)
            .unwrap_or(true)
    }

    fn source_word_in_range(&self, word: u64) -> bool {
        self.source_count
            .map(|source_count| word <= source_count / 32)
            .unwrap_or(true)
    }

    fn source_word_mask(&self, word: u64) -> u32 {
        let Some(source_count) = self.source_count else {
            return u32::MAX;
        };
        if word < source_count / 32 {
            return u32::MAX;
        }
        if word > source_count / 32 {
            return 0;
        }
        let valid_bits = (source_count % 32) + 1;
        if valid_bits == 32 {
            u32::MAX
        } else {
            (1u32 << valid_bits) - 1
        }
    }

    fn source_out_of_range_message(&self, line: InterruptLineId) -> String {
        match self.source_count {
            Some(source_count) => format!(
                "PLIC source {} exceeds declared source count {source_count}",
                line.get()
            ),
            None => format!("PLIC source {} is out of range", line.get()),
        }
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
    SnapshotSourceOutOfRange {
        line: InterruptLineId,
        source_count: Option<u64>,
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
            Self::SnapshotSourceOutOfRange { line, source_count } => match source_count {
                Some(source_count) => write!(
                    formatter,
                    "PLIC snapshot source {} exceeds declared source count {source_count}",
                    line.get()
                ),
                None => write!(
                    formatter,
                    "PLIC snapshot source {} is out of range",
                    line.get()
                ),
            },
        }
    }
}

impl Error for PlicError {}
