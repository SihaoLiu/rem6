use std::collections::{btree_map::Entry, BTreeMap};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address};

use crate::CpuId;

pub const DEFAULT_RISCV_SC_DIAGNOSTIC_THRESHOLD: u64 = 10_000;

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
        }
    }
}

impl Error for RiscvStoreConditionalProgressError {}
