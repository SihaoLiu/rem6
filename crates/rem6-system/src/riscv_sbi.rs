use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, SchedulerError};
use rem6_memory::{AccessSize, Address, AddressRange, TranslationAddressSpaceId};

use crate::{RiscvSystemRunDriver, SystemError};

const SBI_SUCCESS: u64 = 0;
const SBI_ERR_NOT_SUPPORTED: u64 = (-2_i64) as u64;
const SBI_ERR_INVALID_PARAM: u64 = (-3_i64) as u64;
const SBI_BASE_EXTENSION: u64 = 0x10;
const SBI_TIME_EXTENSION: u64 = 0x5449_4d45;
const SBI_HSM_EXTENSION: u64 = 0x0048_534d;
const SBI_IPI_EXTENSION: u64 = 0x0073_5049;
const SBI_RFENCE_EXTENSION: u64 = 0x5246_4e43;
const SBI_SRST_EXTENSION: u64 = 0x5352_5354;
const SBI_BASE_GET_SPEC_VERSION: u64 = 0;
const SBI_BASE_GET_IMPL_ID: u64 = 1;
const SBI_BASE_GET_IMPL_VERSION: u64 = 2;
const SBI_BASE_PROBE_EXTENSION: u64 = 3;
const SBI_BASE_GET_MVENDORID: u64 = 4;
const SBI_BASE_GET_MARCHID: u64 = 5;
const SBI_BASE_GET_MIMPID: u64 = 6;
const SBI_TIME_SET_TIMER: u64 = 0;
const SBI_HSM_HART_START: u64 = 0;
const SBI_HSM_HART_GET_STATUS: u64 = 2;
const SBI_HSM_HART_STARTED: u64 = 0;
const SBI_HSM_HART_STOPPED: u64 = 1;
const SBI_IPI_SEND_IPI: u64 = 0;
const SBI_RFENCE_REMOTE_FENCE_I: u64 = 0;
const SBI_RFENCE_REMOTE_SFENCE_VMA: u64 = 1;
const SBI_RFENCE_REMOTE_SFENCE_VMA_ASID: u64 = 2;
const SBI_SRST_SYSTEM_RESET: u64 = 0;
const SBI_SPEC_VERSION_0_3: u64 = 3;
const REM6_SBI_IMPL_ID: u64 = 0x7265_6d36;
const REM6_SBI_IMPL_VERSION: u64 = 0;
const SBI_ERR_INVALID_ADDRESS: u64 = (-5_i64) as u64;
const SBI_RESET_TYPE_SHUTDOWN: u32 = 0;
const SBI_RESET_TYPE_COLD_REBOOT: u32 = 1;
const SBI_RESET_TYPE_WARM_REBOOT: u32 = 2;
const SBI_RESET_REASON_NONE: u32 = 0;
const SBI_RESET_REASON_SYSTEM_FAILURE: u32 = 1;
const SSIP: u64 = 1 << 1;
const STIP: u64 = 1 << 5;

#[derive(Clone, Debug, Default)]
pub struct RiscvSbiFirmware {
    cores: Arc<Mutex<BTreeMap<u64, RiscvCore>>>,
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
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
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
            arg1: core.read_register(register(11)),
            arg2: core.read_register(register(12)),
            arg3: core.read_register(register(13)),
            arg4: core.read_register(register(14)),
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

    pub const fn arg1(self) -> u64 {
        self.arg1
    }

    pub const fn arg2(self) -> u64 {
        self.arg2
    }

    pub const fn arg3(self) -> u64 {
        self.arg3
    }

    pub const fn arg4(self) -> u64 {
        self.arg4
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSbiOutcome {
    Return {
        error: u64,
        value: u64,
    },
    SystemReset {
        reset_type: u32,
        reset_reason: u32,
        code: i32,
    },
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

    pub const fn invalid_param() -> Self {
        Self::Return {
            error: SBI_ERR_INVALID_PARAM,
            value: 0,
        }
    }

    pub const fn invalid_address() -> Self {
        Self::Return {
            error: SBI_ERR_INVALID_ADDRESS,
            value: 0,
        }
    }
}

impl RiscvSbiFirmware {
    pub fn new() -> Self {
        Self {
            cores: Arc::new(Mutex::new(BTreeMap::new())),
            timer: Arc::new(Mutex::new(RiscvSbiTimerState {
                generations: BTreeMap::new(),
                deadlines: BTreeMap::new(),
            })),
        }
    }

    pub(crate) fn register_cluster(&self, cluster: &RiscvCluster) -> Result<(), SystemError> {
        let mut cores = self.cores.lock().expect("RISC-V SBI core registry lock");
        cores.clear();
        for cpu in cluster.core_ids() {
            let core = cluster.core(cpu).map_err(SystemError::RiscvCluster)?;
            cores.insert(core.hart_id(), core);
        }
        let boot_hart = cores.keys().next().copied();
        for (hart, core) in cores.iter() {
            if core.has_explicit_hart_run_state() {
                continue;
            }
            if Some(*hart) == boot_hart {
                core.set_hart_started();
            } else {
                core.set_hart_stopped();
            }
        }
        Ok(())
    }

    pub fn timer_deadline(&self, cpu: CpuId) -> Option<u64> {
        self.timer
            .lock()
            .expect("RISC-V SBI timer state lock")
            .deadline(cpu)
    }

    pub(crate) fn handle_pending_core_trap(
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
                RiscvSbiOutcome::success(SBI_SPEC_VERSION_0_3)
            }
            (SBI_BASE_EXTENSION, SBI_BASE_GET_IMPL_ID) => {
                RiscvSbiOutcome::success(REM6_SBI_IMPL_ID)
            }
            (SBI_BASE_EXTENSION, SBI_BASE_GET_IMPL_VERSION) => {
                RiscvSbiOutcome::success(REM6_SBI_IMPL_VERSION)
            }
            (SBI_BASE_EXTENSION, SBI_BASE_PROBE_EXTENSION) => RiscvSbiOutcome::success(u64::from(
                request.arg0() == SBI_BASE_EXTENSION
                    || request.arg0() == SBI_TIME_EXTENSION
                    || request.arg0() == SBI_HSM_EXTENSION
                    || request.arg0() == SBI_IPI_EXTENSION
                    || request.arg0() == SBI_RFENCE_EXTENSION
                    || request.arg0() == SBI_SRST_EXTENSION,
            )),
            (SBI_BASE_EXTENSION, SBI_BASE_GET_MVENDORID)
            | (SBI_BASE_EXTENSION, SBI_BASE_GET_MARCHID)
            | (SBI_BASE_EXTENSION, SBI_BASE_GET_MIMPID) => RiscvSbiOutcome::success(0),
            (SBI_TIME_EXTENSION, SBI_TIME_SET_TIMER) => {
                self.program_timer(scheduler, core, request.arg0(), parallel)
                    .map_err(SystemError::Scheduler)?;
                RiscvSbiOutcome::success(0)
            }
            (SBI_HSM_EXTENSION, SBI_HSM_HART_START) => self.hart_start(request),
            (SBI_HSM_EXTENSION, SBI_HSM_HART_GET_STATUS) => self.hart_get_status(request),
            (SBI_IPI_EXTENSION, SBI_IPI_SEND_IPI) => self.send_ipi(request),
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_FENCE_I) => self.remote_fence_i(request),
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_SFENCE_VMA)
            | (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_SFENCE_VMA_ASID) => {
                self.remote_sfence_vma(request)
            }
            (SBI_SRST_EXTENSION, SBI_SRST_SYSTEM_RESET) => self.system_reset(request),
            _ => RiscvSbiOutcome::not_supported(),
        }))
    }

    fn system_reset(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        let Some(reset_type) = u32::try_from(request.arg0()).ok() else {
            return RiscvSbiOutcome::invalid_param();
        };
        let Some(reset_reason) = u32::try_from(request.arg1()).ok() else {
            return RiscvSbiOutcome::invalid_param();
        };
        if !matches!(
            reset_type,
            SBI_RESET_TYPE_SHUTDOWN | SBI_RESET_TYPE_COLD_REBOOT | SBI_RESET_TYPE_WARM_REBOOT
        ) || !matches!(
            reset_reason,
            SBI_RESET_REASON_NONE | SBI_RESET_REASON_SYSTEM_FAILURE
        ) {
            return RiscvSbiOutcome::invalid_param();
        }

        RiscvSbiOutcome::SystemReset {
            reset_type,
            reset_reason,
            code: i32::from(reset_reason == SBI_RESET_REASON_SYSTEM_FAILURE),
        }
    }

    fn send_ipi(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        let Some(targets) = self.hart_mask_targets(request.arg0(), request.arg1()) else {
            return RiscvSbiOutcome::invalid_param();
        };

        for target in targets {
            target.set_machine_interrupt_pending_bits(SSIP);
        }
        RiscvSbiOutcome::success(0)
    }

    fn hart_start(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        if request.arg1() & 0x1 != 0 {
            return RiscvSbiOutcome::invalid_address();
        }
        let Some(target) = self
            .cores
            .lock()
            .expect("RISC-V SBI core registry lock")
            .get(&request.arg0())
            .cloned()
        else {
            return RiscvSbiOutcome::invalid_param();
        };
        if target.is_hart_started() {
            return RiscvSbiOutcome::invalid_param();
        }

        target.start_supervisor_hart(Address::new(request.arg1()), request.arg2());
        RiscvSbiOutcome::success(0)
    }

    fn hart_get_status(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        let Some(target) = self
            .cores
            .lock()
            .expect("RISC-V SBI core registry lock")
            .get(&request.arg0())
            .cloned()
        else {
            return RiscvSbiOutcome::invalid_param();
        };
        RiscvSbiOutcome::success(if target.is_hart_started() {
            SBI_HSM_HART_STARTED
        } else {
            SBI_HSM_HART_STOPPED
        })
    }

    fn remote_fence_i(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        if self
            .hart_mask_targets(request.arg0(), request.arg1())
            .is_none()
        {
            return RiscvSbiOutcome::invalid_param();
        }
        RiscvSbiOutcome::success(0)
    }

    fn remote_sfence_vma(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        if !valid_rfence_range(request.arg2(), request.arg3()) {
            return RiscvSbiOutcome::invalid_address();
        }
        let Some(address_space) = rfence_address_space(request) else {
            return RiscvSbiOutcome::invalid_param();
        };
        let Some(targets) = self.hart_mask_targets(request.arg0(), request.arg1()) else {
            return RiscvSbiOutcome::invalid_param();
        };
        let Some(virtual_range) = rfence_virtual_range(request.arg2(), request.arg3()) else {
            return RiscvSbiOutcome::invalid_address();
        };

        for target in targets {
            target.flush_data_translation_tlb_range(virtual_range, address_space);
        }
        RiscvSbiOutcome::success(0)
    }

    fn hart_mask_targets(&self, hart_mask: u64, hart_mask_base: u64) -> Option<Vec<RiscvCore>> {
        let cores = self.cores.lock().expect("RISC-V SBI core registry lock");
        if hart_mask_base == u64::MAX {
            return Some(cores.values().cloned().collect());
        }

        let mut targets = Vec::new();
        let mut remaining = hart_mask;
        while remaining != 0 {
            let bit = u64::from(remaining.trailing_zeros());
            let hart = hart_mask_base.checked_add(bit)?;
            targets.push(cores.get(&hart)?.clone());
            remaining &= remaining - 1;
        }
        Some(targets)
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

fn valid_rfence_range(start_addr: u64, size: u64) -> bool {
    (start_addr == 0 && size == 0)
        || size == u64::MAX
        || (size != 0 && start_addr.checked_add(size).is_some())
}

fn rfence_address_space(request: RiscvSbiRequest) -> Option<Option<TranslationAddressSpaceId>> {
    if request.function() == SBI_RFENCE_REMOTE_SFENCE_VMA_ASID {
        return u16::try_from(request.arg4())
            .ok()
            .map(TranslationAddressSpaceId::new)
            .map(Some);
    }
    Some(None)
}

fn rfence_virtual_range(start_addr: u64, size: u64) -> Option<Option<AddressRange>> {
    if (start_addr == 0 && size == 0) || size == u64::MAX {
        Some(None)
    } else {
        Some(Some(
            AddressRange::new(Address::new(start_addr), AccessSize::new(size).ok()?).ok()?,
        ))
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
