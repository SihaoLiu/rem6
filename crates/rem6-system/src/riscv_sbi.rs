use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, SchedulerError};

use crate::{RiscvSystemRunDriver, SystemError};

const SBI_SUCCESS: u64 = 0;
const SBI_ERR_NOT_SUPPORTED: u64 = (-2_i64) as u64;
const SBI_BASE_EXTENSION: u64 = 0x10;
const SBI_TIME_EXTENSION: u64 = 0x5449_4d45;
const SBI_BASE_GET_SPEC_VERSION: u64 = 0;
const SBI_BASE_GET_IMPL_ID: u64 = 1;
const SBI_BASE_GET_IMPL_VERSION: u64 = 2;
const SBI_BASE_PROBE_EXTENSION: u64 = 3;
const SBI_BASE_GET_MVENDORID: u64 = 4;
const SBI_BASE_GET_MARCHID: u64 = 5;
const SBI_BASE_GET_MIMPID: u64 = 6;
const SBI_TIME_SET_TIMER: u64 = 0;
const SBI_SPEC_VERSION_0_2: u64 = 2;
const REM6_SBI_IMPL_ID: u64 = 0x7265_6d36;
const REM6_SBI_IMPL_VERSION: u64 = 0;
const STIP: u64 = 1 << 5;

#[derive(Clone, Debug, Default)]
pub struct RiscvSbiFirmware {
    timer: Arc<Mutex<RiscvSbiTimerState>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RiscvSbiTimerState {
    generations: BTreeMap<CpuId, u64>,
    deadlines: BTreeMap<CpuId, u64>,
}

impl RiscvSbiTimerState {
    fn program(&mut self, cpu: CpuId, deadline: u64) -> u64 {
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

    fn generation_matches(&self, cpu: CpuId, generation: u64) -> bool {
        self.generations.get(&cpu).copied() == Some(generation)
    }

    fn deadline(&self, cpu: CpuId) -> Option<u64> {
        self.deadlines.get(&cpu).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSbiRequest {
    extension: u64,
    function: u64,
    arg0: u64,
}

impl RiscvSbiRequest {
    pub fn from_pending_core_trap(core: &RiscvCore) -> Option<Self> {
        let trap = core.pending_trap()?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        if core.privilege_mode() != RiscvPrivilegeMode::Machine {
            return None;
        }
        if core.pending_trap_return_privilege_mode()? != RiscvPrivilegeMode::Supervisor {
            return None;
        }

        Some(Self {
            extension: core.read_register(register(17)),
            function: core.read_register(register(16)),
            arg0: core.read_register(register(10)),
        })
    }

    pub const fn extension(self) -> u64 {
        self.extension
    }

    pub const fn function(self) -> u64 {
        self.function
    }

    pub const fn arg0(self) -> u64 {
        self.arg0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSbiOutcome {
    Return { error: u64, value: u64 },
}

impl RiscvSbiOutcome {
    pub const fn success(value: u64) -> Self {
        Self::Return {
            error: SBI_SUCCESS,
            value,
        }
    }

    pub const fn not_supported() -> Self {
        Self::Return {
            error: SBI_ERR_NOT_SUPPORTED,
            value: 0,
        }
    }
}

impl RiscvSbiFirmware {
    pub fn new() -> Self {
        Self {
            timer: Arc::new(Mutex::new(RiscvSbiTimerState {
                generations: BTreeMap::new(),
                deadlines: BTreeMap::new(),
            })),
        }
    }

    pub fn timer_deadline(&self, cpu: CpuId) -> Option<u64> {
        self.timer
            .lock()
            .expect("RISC-V SBI timer state lock")
            .deadline(cpu)
    }

    pub fn handle_pending_core_trap(
        &self,
        scheduler: &mut PartitionedScheduler,
        core: &RiscvCore,
        parallel: bool,
    ) -> Result<Option<RiscvSbiOutcome>, SystemError> {
        let Some(request) = RiscvSbiRequest::from_pending_core_trap(core) else {
            return Ok(None);
        };
        Ok(Some(match (request.extension(), request.function()) {
            (SBI_BASE_EXTENSION, SBI_BASE_GET_SPEC_VERSION) => {
                RiscvSbiOutcome::success(SBI_SPEC_VERSION_0_2)
            }
            (SBI_BASE_EXTENSION, SBI_BASE_GET_IMPL_ID) => {
                RiscvSbiOutcome::success(REM6_SBI_IMPL_ID)
            }
            (SBI_BASE_EXTENSION, SBI_BASE_GET_IMPL_VERSION) => {
                RiscvSbiOutcome::success(REM6_SBI_IMPL_VERSION)
            }
            (SBI_BASE_EXTENSION, SBI_BASE_PROBE_EXTENSION) => RiscvSbiOutcome::success(u64::from(
                request.arg0() == SBI_BASE_EXTENSION || request.arg0() == SBI_TIME_EXTENSION,
            )),
            (SBI_BASE_EXTENSION, SBI_BASE_GET_MVENDORID)
            | (SBI_BASE_EXTENSION, SBI_BASE_GET_MARCHID)
            | (SBI_BASE_EXTENSION, SBI_BASE_GET_MIMPID) => RiscvSbiOutcome::success(0),
            (SBI_TIME_EXTENSION, SBI_TIME_SET_TIMER) => {
                self.program_timer(scheduler, core, request.arg0(), parallel)
                    .map_err(SystemError::Scheduler)?;
                RiscvSbiOutcome::success(0)
            }
            _ => RiscvSbiOutcome::not_supported(),
        }))
    }

    fn program_timer(
        &self,
        scheduler: &mut PartitionedScheduler,
        core: &RiscvCore,
        deadline: u64,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        let cpu = core.id();
        let generation = self
            .timer
            .lock()
            .expect("RISC-V SBI timer state lock")
            .program(cpu, deadline);
        core.clear_machine_interrupt_pending_bits(STIP);

        let partition = core.partition();
        let now = scheduler.partition_now(partition)?;
        if deadline <= now {
            core.set_machine_interrupt_pending_bits(STIP);
            return Ok(());
        }

        let timer = Arc::clone(&self.timer);
        let timer_core = core.clone();
        if parallel {
            scheduler.schedule_parallel_at(partition, deadline, move |_context| {
                if timer
                    .lock()
                    .expect("RISC-V SBI timer state lock")
                    .generation_matches(cpu, generation)
                {
                    timer_core.set_machine_interrupt_pending_bits(STIP);
                }
            })?;
        } else {
            scheduler.schedule_at(partition, deadline, move |_context| {
                if timer
                    .lock()
                    .expect("RISC-V SBI timer state lock")
                    .generation_matches(cpu, generation)
                {
                    timer_core.set_machine_interrupt_pending_bits(STIP);
                }
            })?;
        }
        Ok(())
    }
}

impl RiscvSystemRunDriver {
    pub fn with_riscv_sbi_firmware(mut self) -> Self {
        self.riscv_sbi_firmware = Some(RiscvSbiFirmware::new());
        self
    }

    pub const fn riscv_sbi_firmware(&self) -> Option<&RiscvSbiFirmware> {
        self.riscv_sbi_firmware.as_ref()
    }
}

fn register(index: u8) -> Register {
    Register::new(index).expect("valid RISC-V integer register")
}
