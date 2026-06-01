use std::collections::{btree_map::Entry, BTreeMap};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address};

use crate::CpuId;

pub const DEFAULT_RISCV_SC_DIAGNOSTIC_THRESHOLD: u64 = 10_000;
const SC_CHECKPOINT_MAGIC: [u8; 4] = *b"RSCP";
const SC_CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const SC_CHECKPOINT_HEADER_BYTES: usize =
    SC_CHECKPOINT_MAGIC.len() + 1 + U64_BYTES + U32_BYTES + U32_BYTES;
const SC_CHECKPOINT_STREAK_BYTES: usize = U32_BYTES + U64_BYTES * 5;
const SC_CHECKPOINT_DIAGNOSTIC_BYTES: usize = SC_CHECKPOINT_STREAK_BYTES + U64_BYTES;
const SC_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvStoreConditionalProgressConfig {
    diagnostic_threshold: u64,
}

impl RiscvStoreConditionalProgressConfig {
    pub fn new(diagnostic_threshold: u64) -> Result<Self, RiscvStoreConditionalProgressError> {
        if diagnostic_threshold == 0 {
            return Err(RiscvStoreConditionalProgressError::ZeroDiagnosticThreshold);
        }
        Ok(Self {
            diagnostic_threshold,
        })
    }

    pub const fn diagnostic_threshold(self) -> u64 {
        self.diagnostic_threshold
    }
}

impl Default for RiscvStoreConditionalProgressConfig {
    fn default() -> Self {
        Self {
            diagnostic_threshold: DEFAULT_RISCV_SC_DIAGNOSTIC_THRESHOLD,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvStoreConditionalProgress {
    config: RiscvStoreConditionalProgressConfig,
    streaks: BTreeMap<CpuId, RiscvStoreConditionalFailureStreak>,
    diagnostics: Vec<RiscvStoreConditionalFailureDiagnostic>,
}

impl RiscvStoreConditionalProgress {
    pub fn new(config: RiscvStoreConditionalProgressConfig) -> Self {
        Self {
            config,
            streaks: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub const fn config(&self) -> RiscvStoreConditionalProgressConfig {
        self.config
    }

    pub fn record_failure(
        &mut self,
        cpu: CpuId,
        tick: Tick,
        address: Address,
        size: AccessSize,
    ) -> Option<RiscvStoreConditionalFailureDiagnostic> {
        let threshold = self.config.diagnostic_threshold;
        let streak = match self.streaks.entry(cpu) {
            Entry::Occupied(mut entry) => {
                if entry.get().matches_region(address, size) {
                    entry.get_mut().record_failure(tick);
                } else {
                    entry.insert(RiscvStoreConditionalFailureStreak::new(
                        cpu, tick, address, size,
                    ));
                }
                *entry.get()
            }
            Entry::Vacant(entry) => *entry.insert(RiscvStoreConditionalFailureStreak::new(
                cpu, tick, address, size,
            )),
        };

        if streak.failure_count % threshold != 0 {
            return None;
        }

        let diagnostic = RiscvStoreConditionalFailureDiagnostic::new(streak, threshold);
        self.diagnostics.push(diagnostic);
        Some(diagnostic)
    }

    pub fn record_success(&mut self, cpu: CpuId) -> Option<RiscvStoreConditionalFailureStreak> {
        self.streaks.remove(&cpu)
    }

    pub fn streak(&self, cpu: CpuId) -> Option<&RiscvStoreConditionalFailureStreak> {
        self.streaks.get(&cpu)
    }

    pub fn streaks(&self) -> Vec<RiscvStoreConditionalFailureStreak> {
        self.streaks.values().copied().collect()
    }

    pub fn diagnostics(&self) -> &[RiscvStoreConditionalFailureDiagnostic] {
        &self.diagnostics
    }

    pub fn snapshot(&self) -> RiscvStoreConditionalProgressSnapshot {
        RiscvStoreConditionalProgressSnapshot {
            config: self.config,
            streaks: self.streaks(),
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &RiscvStoreConditionalProgressSnapshot,
    ) -> Result<(), RiscvStoreConditionalProgressError> {
        if self.config != snapshot.config {
            return Err(RiscvStoreConditionalProgressError::SnapshotConfigMismatch {
                expected: self.config,
                actual: snapshot.config,
            });
        }

        let mut streaks = BTreeMap::new();
        for streak in snapshot.streaks.iter().copied() {
            if streaks.insert(streak.cpu, streak).is_some() {
                return Err(
                    RiscvStoreConditionalProgressError::DuplicateSnapshotStreak { cpu: streak.cpu },
                );
            }
        }

        self.streaks = streaks;
        self.diagnostics = snapshot.diagnostics.clone();
        Ok(())
    }
}

impl Default for RiscvStoreConditionalProgress {
    fn default() -> Self {
        Self::new(RiscvStoreConditionalProgressConfig::default())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvStoreConditionalFailureStreak {
    cpu: CpuId,
    address: Address,
    size: AccessSize,
    first_failure_tick: Tick,
    last_failure_tick: Tick,
    failure_count: u64,
}

impl RiscvStoreConditionalFailureStreak {
    pub const fn new(cpu: CpuId, tick: Tick, address: Address, size: AccessSize) -> Self {
        Self {
            cpu,
            address,
            size,
            first_failure_tick: tick,
            last_failure_tick: tick,
            failure_count: 1,
        }
    }

    pub const fn cpu(self) -> CpuId {
        self.cpu
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }

    pub const fn first_failure_tick(self) -> Tick {
        self.first_failure_tick
    }

    pub const fn last_failure_tick(self) -> Tick {
        self.last_failure_tick
    }

    pub const fn failure_count(self) -> u64 {
        self.failure_count
    }

    fn matches_region(self, address: Address, size: AccessSize) -> bool {
        self.address == address && self.size == size
    }

    fn record_failure(&mut self, tick: Tick) {
        self.last_failure_tick = tick;
        self.failure_count += 1;
    }

    fn with_failure_window(mut self, last_failure_tick: Tick, failure_count: u64) -> Self {
        self.last_failure_tick = last_failure_tick;
        self.failure_count = failure_count;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvStoreConditionalFailureDiagnostic {
    cpu: CpuId,
    address: Address,
    size: AccessSize,
    first_failure_tick: Tick,
    last_failure_tick: Tick,
    failure_count: u64,
    diagnostic_threshold: u64,
}

impl RiscvStoreConditionalFailureDiagnostic {
    pub const fn new(
        streak: RiscvStoreConditionalFailureStreak,
        diagnostic_threshold: u64,
    ) -> Self {
        Self {
            cpu: streak.cpu,
            address: streak.address,
            size: streak.size,
            first_failure_tick: streak.first_failure_tick,
            last_failure_tick: streak.last_failure_tick,
            failure_count: streak.failure_count,
            diagnostic_threshold,
        }
    }

    pub const fn cpu(self) -> CpuId {
        self.cpu
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }

    pub const fn first_failure_tick(self) -> Tick {
        self.first_failure_tick
    }

    pub const fn last_failure_tick(self) -> Tick {
        self.last_failure_tick
    }

    pub const fn failure_count(self) -> u64 {
        self.failure_count
    }

    pub const fn diagnostic_threshold(self) -> u64 {
        self.diagnostic_threshold
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvStoreConditionalProgressSnapshot {
    config: RiscvStoreConditionalProgressConfig,
    streaks: Vec<RiscvStoreConditionalFailureStreak>,
    diagnostics: Vec<RiscvStoreConditionalFailureDiagnostic>,
}

impl RiscvStoreConditionalProgressSnapshot {
    pub const fn config(&self) -> RiscvStoreConditionalProgressConfig {
        self.config
    }

    pub fn streaks(&self) -> &[RiscvStoreConditionalFailureStreak] {
        &self.streaks
    }

    pub fn diagnostics(&self) -> &[RiscvStoreConditionalFailureDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvStoreConditionalProgressCheckpointPayload {
    snapshot: RiscvStoreConditionalProgressSnapshot,
}

impl RiscvStoreConditionalProgressCheckpointPayload {
    pub fn from_progress(progress: &RiscvStoreConditionalProgress) -> Self {
        Self {
            snapshot: progress.snapshot(),
        }
    }

    pub fn from_snapshot(
        snapshot: RiscvStoreConditionalProgressSnapshot,
    ) -> Result<Self, RiscvStoreConditionalProgressError> {
        validate_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, RiscvStoreConditionalProgressError> {
        if payload.len() < SC_CHECKPOINT_HEADER_BYTES {
            return Err(
                RiscvStoreConditionalProgressError::InvalidCheckpointPayloadSize {
                    expected: SC_CHECKPOINT_HEADER_BYTES,
                    actual: payload.len(),
                },
            );
        }
        if payload[0..SC_CHECKPOINT_MAGIC.len()] != SC_CHECKPOINT_MAGIC {
            return Err(RiscvStoreConditionalProgressError::InvalidCheckpointMagic);
        }

        let mut offset = SC_CHECKPOINT_MAGIC.len();
        let version = payload[offset];
        offset += 1;
        if version != SC_CHECKPOINT_VERSION {
            return Err(
                RiscvStoreConditionalProgressError::UnsupportedCheckpointVersion { version },
            );
        }

        let diagnostic_threshold = read_u64(payload, &mut offset);
        let config = RiscvStoreConditionalProgressConfig::new(diagnostic_threshold)?;
        let streak_count = read_u32(payload, &mut offset) as usize;
        let diagnostic_count = read_u32(payload, &mut offset) as usize;
        let expected = sc_checkpoint_payload_size(streak_count, diagnostic_count)?;
        if payload.len() != expected {
            return Err(
                RiscvStoreConditionalProgressError::InvalidCheckpointPayloadSize {
                    expected,
                    actual: payload.len(),
                },
            );
        }

        let mut streaks = Vec::with_capacity(streak_count);
        for _ in 0..streak_count {
            streaks.push(read_checkpoint_streak(payload, &mut offset)?);
        }

        let mut diagnostics = Vec::with_capacity(diagnostic_count);
        for _ in 0..diagnostic_count {
            diagnostics.push(read_checkpoint_diagnostic(payload, &mut offset)?);
        }

        Self::from_snapshot(RiscvStoreConditionalProgressSnapshot {
            config,
            streaks,
            diagnostics,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("RISC-V store-conditional checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, RiscvStoreConditionalProgressError> {
        let streak_count = encode_checkpoint_u32("streak count", self.snapshot.streaks().len())?;
        let diagnostic_count =
            encode_checkpoint_u32("diagnostic count", self.snapshot.diagnostics().len())?;
        let mut payload = Vec::with_capacity(sc_checkpoint_payload_size(
            self.snapshot.streaks().len(),
            self.snapshot.diagnostics().len(),
        )?);
        payload.extend_from_slice(&SC_CHECKPOINT_MAGIC);
        payload.push(SC_CHECKPOINT_VERSION);
        payload.extend_from_slice(&self.snapshot.config().diagnostic_threshold().to_le_bytes());
        payload.extend_from_slice(&streak_count.to_le_bytes());
        payload.extend_from_slice(&diagnostic_count.to_le_bytes());
        for streak in self.snapshot.streaks() {
            write_checkpoint_streak(&mut payload, *streak);
        }
        for diagnostic in self.snapshot.diagnostics() {
            write_checkpoint_diagnostic(&mut payload, *diagnostic);
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &RiscvStoreConditionalProgressSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> RiscvStoreConditionalProgressSnapshot {
        self.snapshot
    }
}

fn validate_snapshot(
    snapshot: &RiscvStoreConditionalProgressSnapshot,
) -> Result<(), RiscvStoreConditionalProgressError> {
    let mut progress = RiscvStoreConditionalProgress::new(snapshot.config());
    progress.restore(snapshot)
}

fn write_checkpoint_streak(payload: &mut Vec<u8>, streak: RiscvStoreConditionalFailureStreak) {
    payload.extend_from_slice(&streak.cpu().get().to_le_bytes());
    payload.extend_from_slice(&streak.address().get().to_le_bytes());
    payload.extend_from_slice(&streak.size().bytes().to_le_bytes());
    payload.extend_from_slice(&streak.first_failure_tick().to_le_bytes());
    payload.extend_from_slice(&streak.last_failure_tick().to_le_bytes());
    payload.extend_from_slice(&streak.failure_count().to_le_bytes());
}

fn write_checkpoint_diagnostic(
    payload: &mut Vec<u8>,
    diagnostic: RiscvStoreConditionalFailureDiagnostic,
) {
    write_checkpoint_streak(
        payload,
        RiscvStoreConditionalFailureStreak::new(
            diagnostic.cpu(),
            diagnostic.first_failure_tick(),
            diagnostic.address(),
            diagnostic.size(),
        )
        .with_failure_window(diagnostic.last_failure_tick(), diagnostic.failure_count()),
    );
    payload.extend_from_slice(&diagnostic.diagnostic_threshold().to_le_bytes());
}

fn read_checkpoint_streak(
    payload: &[u8],
    offset: &mut usize,
) -> Result<RiscvStoreConditionalFailureStreak, RiscvStoreConditionalProgressError> {
    let cpu = CpuId::new(read_u32(payload, offset));
    let address = Address::new(read_u64(payload, offset));
    let size = checkpoint_access_size(read_u64(payload, offset))?;
    let first_failure_tick = read_u64(payload, offset);
    let last_failure_tick = read_u64(payload, offset);
    let failure_count = read_u64(payload, offset);

    Ok(
        RiscvStoreConditionalFailureStreak::new(cpu, first_failure_tick, address, size)
            .with_failure_window(last_failure_tick, failure_count),
    )
}

fn read_checkpoint_diagnostic(
    payload: &[u8],
    offset: &mut usize,
) -> Result<RiscvStoreConditionalFailureDiagnostic, RiscvStoreConditionalProgressError> {
    let streak = read_checkpoint_streak(payload, offset)?;
    let diagnostic_threshold = read_u64(payload, offset);
    Ok(RiscvStoreConditionalFailureDiagnostic::new(
        streak,
        diagnostic_threshold,
    ))
}

fn checkpoint_access_size(bytes: u64) -> Result<AccessSize, RiscvStoreConditionalProgressError> {
    AccessSize::new(bytes)
        .map_err(|_| RiscvStoreConditionalProgressError::InvalidCheckpointAccessSize { bytes })
}

fn sc_checkpoint_payload_size(
    streak_count: usize,
    diagnostic_count: usize,
) -> Result<usize, RiscvStoreConditionalProgressError> {
    let streak_bytes = streak_count.checked_mul(SC_CHECKPOINT_STREAK_BYTES).ok_or(
        RiscvStoreConditionalProgressError::InvalidCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        },
    )?;
    let diagnostic_bytes = diagnostic_count
        .checked_mul(SC_CHECKPOINT_DIAGNOSTIC_BYTES)
        .ok_or(
            RiscvStoreConditionalProgressError::InvalidCheckpointPayloadSize {
                expected: usize::MAX,
                actual: 0,
            },
        )?;
    SC_CHECKPOINT_HEADER_BYTES
        .checked_add(streak_bytes)
        .and_then(|size| size.checked_add(diagnostic_bytes))
        .ok_or(
            RiscvStoreConditionalProgressError::InvalidCheckpointPayloadSize {
                expected: usize::MAX,
                actual: 0,
            },
        )
}

fn encode_checkpoint_u32(
    field: &'static str,
    value: usize,
) -> Result<u32, RiscvStoreConditionalProgressError> {
    u32::try_from(value).map_err(
        |_| RiscvStoreConditionalProgressError::CheckpointValueTooLarge {
            field,
            value,
            maximum: SC_CHECKPOINT_U32_MAX,
        },
    )
}

fn read_u32(payload: &[u8], offset: &mut usize) -> u32 {
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    u32::from_le_bytes(bytes)
}

fn read_u64(payload: &[u8], offset: &mut usize) -> u64 {
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    u64::from_le_bytes(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvStoreConditionalProgressError {
    ZeroDiagnosticThreshold,
    SnapshotConfigMismatch {
        expected: RiscvStoreConditionalProgressConfig,
        actual: RiscvStoreConditionalProgressConfig,
    },
    DuplicateSnapshotStreak {
        cpu: CpuId,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    InvalidCheckpointAccessSize {
        bytes: u64,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for RiscvStoreConditionalProgressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDiagnosticThreshold => write!(
                formatter,
                "RISC-V store-conditional diagnostic threshold must be nonzero"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "RISC-V store-conditional snapshot config mismatch: expected threshold {}, got {}",
                expected.diagnostic_threshold(),
                actual.diagnostic_threshold()
            ),
            Self::DuplicateSnapshotStreak { cpu } => write!(
                formatter,
                "RISC-V store-conditional snapshot contains duplicate streak for CPU {}",
                cpu.get()
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "RISC-V store-conditional checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => write!(
                formatter,
                "RISC-V store-conditional checkpoint payload has invalid magic"
            ),
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "RISC-V store-conditional checkpoint payload version {version} is not supported"
            ),
            Self::InvalidCheckpointAccessSize { bytes } => write!(
                formatter,
                "RISC-V store-conditional checkpoint payload has invalid access size {bytes}"
            ),
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "RISC-V store-conditional checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for RiscvStoreConditionalProgressError {}
