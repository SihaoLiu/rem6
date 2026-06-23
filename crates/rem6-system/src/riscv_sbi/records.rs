use std::collections::BTreeMap;

use rem6_cpu::CpuId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvSbiTimerState {
    generations: BTreeMap<CpuId, u64>,
    deadlines: BTreeMap<CpuId, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSbiHsmRecord {
    source_cpu: CpuId,
    function: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSbiHsmWakeRecord {
    source_cpu: CpuId,
    target_hart: u64,
    interrupt_bits: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSbiIpiRecord {
    source_cpu: CpuId,
    hart_mask: u64,
    hart_mask_base: u64,
    targets: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSbiRfenceRecord {
    source_cpu: CpuId,
    function: u64,
    hart_mask: u64,
    hart_mask_base: u64,
    start_addr: u64,
    size: u64,
    address_space: Option<u64>,
    targets: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSbiRfenceCompletionRecord {
    source_cpu: CpuId,
    target_hart: u64,
    function: u64,
    start_addr: u64,
    size: u64,
    address_space: Option<u64>,
    completed_tick: u64,
    flushed_entries: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSbiResetRecord {
    cpu: CpuId,
    reset_type: u32,
    reset_reason: u32,
    code: i32,
}

impl RiscvSbiTimerState {
    pub(super) fn program(&mut self, cpu: CpuId, deadline: u64) -> u64 {
        let generation = self
            .generations
            .get(&cpu)
            .copied()
            .unwrap_or_default()
            .wrapping_add(1);
        self.generations.insert(cpu, generation);
        self.deadlines.insert(cpu, deadline);
        generation
    }

    pub(super) fn cancel(&mut self, cpu: CpuId) {
        let generation = self
            .generations
            .get(&cpu)
            .copied()
            .unwrap_or_default()
            .wrapping_add(1);
        self.generations.insert(cpu, generation);
        self.deadlines.remove(&cpu);
    }

    pub(super) fn generation_matches(&self, cpu: CpuId, generation: u64) -> bool {
        self.generations.get(&cpu).copied() == Some(generation)
    }

    pub(super) fn deadline(&self, cpu: CpuId) -> Option<u64> {
        self.deadlines.get(&cpu).copied()
    }
}

impl RiscvSbiResetRecord {
    pub const fn new(cpu: CpuId, reset_type: u32, reset_reason: u32, code: i32) -> Self {
        Self {
            cpu,
            reset_type,
            reset_reason,
            code,
        }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn reset_type(&self) -> u32 {
        self.reset_type
    }

    pub const fn reset_reason(&self) -> u32 {
        self.reset_reason
    }

    pub const fn code(&self) -> i32 {
        self.code
    }
}

impl RiscvSbiHsmRecord {
    pub const fn new(source_cpu: CpuId, function: u64, arg0: u64, arg1: u64, arg2: u64) -> Self {
        Self {
            source_cpu,
            function,
            arg0,
            arg1,
            arg2,
        }
    }

    pub const fn source_cpu(&self) -> CpuId {
        self.source_cpu
    }

    pub const fn function(&self) -> u64 {
        self.function
    }

    pub const fn arg0(&self) -> u64 {
        self.arg0
    }

    pub const fn arg1(&self) -> u64 {
        self.arg1
    }

    pub const fn arg2(&self) -> u64 {
        self.arg2
    }
}

impl RiscvSbiHsmWakeRecord {
    pub const fn new(source_cpu: CpuId, target_hart: u64, interrupt_bits: u64) -> Self {
        Self {
            source_cpu,
            target_hart,
            interrupt_bits,
        }
    }

    pub const fn source_cpu(&self) -> CpuId {
        self.source_cpu
    }

    pub const fn target_hart(&self) -> u64 {
        self.target_hart
    }

    pub const fn interrupt_bits(&self) -> u64 {
        self.interrupt_bits
    }
}

impl RiscvSbiIpiRecord {
    pub fn new(source_cpu: CpuId, hart_mask: u64, hart_mask_base: u64, targets: Vec<u64>) -> Self {
        Self {
            source_cpu,
            hart_mask,
            hart_mask_base,
            targets,
        }
    }

    pub const fn source_cpu(&self) -> CpuId {
        self.source_cpu
    }

    pub const fn hart_mask(&self) -> u64 {
        self.hart_mask
    }

    pub const fn hart_mask_base(&self) -> u64 {
        self.hart_mask_base
    }

    pub fn targets(&self) -> &[u64] {
        &self.targets
    }
}

impl RiscvSbiRfenceRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_cpu: CpuId,
        function: u64,
        hart_mask: u64,
        hart_mask_base: u64,
        start_addr: u64,
        size: u64,
        address_space: Option<u64>,
        targets: Vec<u64>,
    ) -> Self {
        Self {
            source_cpu,
            function,
            hart_mask,
            hart_mask_base,
            start_addr,
            size,
            address_space,
            targets,
        }
    }

    pub const fn source_cpu(&self) -> CpuId {
        self.source_cpu
    }

    pub const fn function(&self) -> u64 {
        self.function
    }

    pub const fn hart_mask(&self) -> u64 {
        self.hart_mask
    }

    pub const fn hart_mask_base(&self) -> u64 {
        self.hart_mask_base
    }

    pub const fn start_addr(&self) -> u64 {
        self.start_addr
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn address_space(&self) -> Option<u64> {
        self.address_space
    }

    pub fn targets(&self) -> &[u64] {
        &self.targets
    }
}

impl RiscvSbiRfenceCompletionRecord {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        source_cpu: CpuId,
        target_hart: u64,
        function: u64,
        start_addr: u64,
        size: u64,
        address_space: Option<u64>,
        completed_tick: u64,
        flushed_entries: Option<u64>,
    ) -> Self {
        Self {
            source_cpu,
            target_hart,
            function,
            start_addr,
            size,
            address_space,
            completed_tick,
            flushed_entries,
        }
    }

    pub const fn source_cpu(&self) -> CpuId {
        self.source_cpu
    }

    pub const fn target_hart(&self) -> u64 {
        self.target_hart
    }

    pub const fn function(&self) -> u64 {
        self.function
    }

    pub const fn start_addr(&self) -> u64 {
        self.start_addr
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn address_space(&self) -> Option<u64> {
        self.address_space
    }

    pub const fn completed_tick(&self) -> u64 {
        self.completed_tick
    }

    pub const fn flushed_entries(&self) -> Option<u64> {
        self.flushed_entries
    }
}
