use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvCore, RiscvHartRunState};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, SchedulerError};
use rem6_memory::{AccessSize, Address, AddressRange, TranslationAddressSpaceId};

use crate::{RiscvSystemRunDriver, SystemError};

const SBI_SUCCESS: u64 = 0;
const SBI_ERR_NOT_SUPPORTED: u64 = (-2_i64) as u64;
const SBI_ERR_INVALID_PARAM: u64 = (-3_i64) as u64;
const SBI_ERR_ALREADY_AVAILABLE: u64 = (-6_i64) as u64;
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
const SBI_HSM_HART_STOP: u64 = 1;
const SBI_HSM_HART_GET_STATUS: u64 = 2;
const SBI_HSM_HART_SUSPEND: u64 = 3;
const SBI_HSM_DEFAULT_RETENTIVE_SUSPEND: u64 = 0;
const SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND: u64 = 0x8000_0000;
const SBI_HSM_HART_STARTED: u64 = 0;
const SBI_HSM_HART_STOPPED: u64 = 1;
const SBI_HSM_HART_START_PENDING: u64 = 2;
const SBI_HSM_HART_STOP_PENDING: u64 = 3;
const SBI_HSM_HART_SUSPENDED: u64 = 4;
const SBI_HSM_HART_SUSPEND_PENDING: u64 = 5;
const SBI_HSM_HART_RESUME_PENDING: u64 = 6;
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
    Stopped,
    Resumed,
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

    pub const fn already_available() -> Self {
        Self::Return {
            error: SBI_ERR_ALREADY_AVAILABLE,
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
            (SBI_HSM_EXTENSION, SBI_HSM_HART_START) => {
                let outcome = self.hart_start(request);
                if let RiscvSbiOutcome::Return {
                    error: SBI_SUCCESS, ..
                } = outcome
                {
                    self.schedule_hart_start(scheduler, core, request, parallel)
                        .map_err(SystemError::Scheduler)?;
                }
                outcome
            }
            (SBI_HSM_EXTENSION, SBI_HSM_HART_STOP) => {
                let outcome = self.hart_stop(core);
                if outcome == RiscvSbiOutcome::Stopped {
                    self.schedule_hart_stop(scheduler, core, parallel)
                        .map_err(SystemError::Scheduler)?;
                }
                outcome
            }
            (SBI_HSM_EXTENSION, SBI_HSM_HART_GET_STATUS) => self.hart_get_status(request),
            (SBI_HSM_EXTENSION, SBI_HSM_HART_SUSPEND) => {
                let outcome = self.hart_suspend(core, request);
                if outcome == RiscvSbiOutcome::success(0)
                    && request.arg0() == SBI_HSM_DEFAULT_RETENTIVE_SUSPEND
                {
                    self.schedule_hart_suspend(scheduler, core, parallel)
                        .map_err(SystemError::Scheduler)?;
                } else if outcome == RiscvSbiOutcome::Resumed
                    && request.arg0() == SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND
                {
                    self.schedule_nonretentive_hart_resume(
                        scheduler,
                        core,
                        Address::new(request.arg1()),
                        request.arg2(),
                        parallel,
                    )
                    .map_err(SystemError::Scheduler)?;
                }
                outcome
            }
            (SBI_IPI_EXTENSION, SBI_IPI_SEND_IPI) => self.send_ipi(request),
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_FENCE_I) => self.remote_fence_i(request),
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_SFENCE_VMA)
            | (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_SFENCE_VMA_ASID) => self
                .remote_sfence_vma(scheduler, core, request, parallel)
                .map_err(SystemError::Scheduler)?,
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
        let Some(target) = self
            .cores
            .lock()
            .expect("RISC-V SBI core registry lock")
            .get(&request.arg0())
            .cloned()
        else {
            return RiscvSbiOutcome::invalid_param();
        };
        match target.hart_run_state() {
            RiscvHartRunState::Stopped => {}
            RiscvHartRunState::Started => return RiscvSbiOutcome::already_available(),
            _ => return RiscvSbiOutcome::invalid_param(),
        }
        if request.arg1() & 0x1 != 0 {
            return RiscvSbiOutcome::invalid_address();
        }

        target.set_hart_start_pending();
        RiscvSbiOutcome::success(0)
    }

    fn schedule_hart_start(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        request: RiscvSbiRequest,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        let Some(target) = self
            .cores
            .lock()
            .expect("RISC-V SBI core registry lock")
            .get(&request.arg0())
            .cloned()
        else {
            return Ok(());
        };
        let source_now = scheduler.partition_now(source.partition())?;
        let target_now = scheduler.partition_now(target.partition())?;
        let delay = scheduler.min_remote_delay();
        let deadline = source_now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow {
                now: source_now,
                delay,
            })?
            .max(target_now);
        let entry = Address::new(request.arg1());
        let opaque = request.arg2();
        if parallel {
            scheduler.schedule_parallel_at(target.partition(), deadline, move |_context| {
                target.complete_pending_supervisor_hart_start(entry, opaque);
            })?;
        } else {
            scheduler.schedule_at(target.partition(), deadline, move |_context| {
                target.complete_pending_supervisor_hart_start(entry, opaque);
            })?;
        }
        Ok(())
    }

    fn hart_stop(&self, core: &RiscvCore) -> RiscvSbiOutcome {
        core.set_hart_stop_pending();
        RiscvSbiOutcome::Stopped
    }

    fn schedule_hart_stop(
        &self,
        scheduler: &mut PartitionedScheduler,
        core: &RiscvCore,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        let partition = core.partition();
        let now = scheduler.partition_now(partition)?;
        let delay = scheduler.min_remote_delay();
        let deadline = now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow { now, delay })?;
        let stopped_core = core.clone();
        if parallel {
            scheduler.schedule_parallel_at(partition, deadline, move |_context| {
                stopped_core.complete_pending_hart_stop();
            })?;
        } else {
            scheduler.schedule_at(partition, deadline, move |_context| {
                stopped_core.complete_pending_hart_stop();
            })?;
        }
        Ok(())
    }

    fn hart_suspend(&self, core: &RiscvCore, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        match request.arg0() {
            SBI_HSM_DEFAULT_RETENTIVE_SUSPEND => {
                core.set_hart_suspend_pending();
                RiscvSbiOutcome::success(0)
            }
            SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND => {
                if request.arg1() & 0x1 != 0 {
                    return RiscvSbiOutcome::invalid_address();
                }
                core.set_hart_resume_pending();
                RiscvSbiOutcome::Resumed
            }
            _ => RiscvSbiOutcome::invalid_param(),
        }
    }

    fn schedule_hart_suspend(
        &self,
        scheduler: &mut PartitionedScheduler,
        core: &RiscvCore,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        let partition = core.partition();
        let now = scheduler.partition_now(partition)?;
        let delay = scheduler.min_remote_delay();
        let deadline = now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow { now, delay })?;
        let suspended_core = core.clone();
        if parallel {
            scheduler.schedule_parallel_at(partition, deadline, move |_context| {
                suspended_core.complete_pending_hart_suspend();
            })?;
        } else {
            scheduler.schedule_at(partition, deadline, move |_context| {
                suspended_core.complete_pending_hart_suspend();
            })?;
        }
        Ok(())
    }

    fn schedule_nonretentive_hart_resume(
        &self,
        scheduler: &mut PartitionedScheduler,
        core: &RiscvCore,
        entry: Address,
        opaque: u64,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        let partition = core.partition();
        let now = scheduler.partition_now(partition)?;
        let delay = scheduler.min_remote_delay();
        let deadline = now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow { now, delay })?;
        let suspended_core = core.clone();
        if parallel {
            scheduler.schedule_parallel_at(partition, deadline, move |_context| {
                suspended_core.resume_pending_nonretentive_supervisor_hart(entry, opaque);
            })?;
        } else {
            scheduler.schedule_at(partition, deadline, move |_context| {
                suspended_core.resume_pending_nonretentive_supervisor_hart(entry, opaque);
            })?;
        }
        Ok(())
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
        RiscvSbiOutcome::success(match target.hart_run_state() {
            RiscvHartRunState::Started => SBI_HSM_HART_STARTED,
            RiscvHartRunState::StartPending => SBI_HSM_HART_START_PENDING,
            RiscvHartRunState::StopPending => SBI_HSM_HART_STOP_PENDING,
            RiscvHartRunState::SuspendPending => SBI_HSM_HART_SUSPEND_PENDING,
            RiscvHartRunState::ResumePending => SBI_HSM_HART_RESUME_PENDING,
            RiscvHartRunState::Stopped => SBI_HSM_HART_STOPPED,
            RiscvHartRunState::Suspended => SBI_HSM_HART_SUSPENDED,
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

    fn remote_sfence_vma(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        request: RiscvSbiRequest,
        parallel: bool,
    ) -> Result<RiscvSbiOutcome, SchedulerError> {
        if !valid_rfence_range(request.arg2(), request.arg3()) {
            return Ok(RiscvSbiOutcome::invalid_address());
        }
        let Some(address_space) = rfence_address_space(request) else {
            return Ok(RiscvSbiOutcome::invalid_param());
        };
        let Some(targets) = self.hart_mask_targets(request.arg0(), request.arg1()) else {
            return Ok(RiscvSbiOutcome::invalid_param());
        };
        let Some(virtual_range) = rfence_virtual_range(request.arg2(), request.arg3()) else {
            return Ok(RiscvSbiOutcome::invalid_address());
        };

        self.schedule_remote_sfence_vma(
            scheduler,
            source,
            targets,
            virtual_range,
            address_space,
            parallel,
        )?;
        Ok(RiscvSbiOutcome::success(0))
    }

    fn schedule_remote_sfence_vma(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        targets: Vec<RiscvCore>,
        virtual_range: Option<AddressRange>,
        address_space: Option<TranslationAddressSpaceId>,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        let source_now = scheduler.partition_now(source.partition())?;
        let delay = scheduler.min_remote_delay();
        let source_deadline =
            source_now
                .checked_add(delay)
                .ok_or(SchedulerError::TickOverflow {
                    now: source_now,
                    delay,
                })?;
        for target in targets {
            let target_now = scheduler.partition_now(target.partition())?;
            let deadline = source_deadline.max(target_now);
            if parallel {
                scheduler.schedule_parallel_at(target.partition(), deadline, move |_context| {
                    target.flush_data_translation_tlb_range(virtual_range, address_space);
                })?;
            } else {
                scheduler.schedule_at(target.partition(), deadline, move |_context| {
                    target.flush_data_translation_tlb_range(virtual_range, address_space);
                })?;
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use rem6_boot::BootImage;
    use rem6_cpu::{
        CpuCore, CpuDataConfig, CpuFetchConfig, CpuResetState, CpuTranslationFrontend,
        CpuTranslationFrontendSnapshot,
    };
    use rem6_kernel::{PartitionId, SchedulerContext};
    use rem6_memory::{
        AccessSize, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
        TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
        TranslationQueueSnapshot, TranslationTlbConfig, TranslationTlbEntrySnapshot,
        TranslationTlbSnapshot, TranslationTlbStats,
    };
    use rem6_transport::{
        MemoryRoute, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
        TransportEndpointId,
    };

    use super::*;

    const TEST_HART_START_PENDING: u64 = 2;
    const TEST_HART_STOP_PENDING: u64 = 3;
    const TEST_HART_SUSPEND_PENDING: u64 = 5;
    const TEST_HART_RESUME_PENDING: u64 = 6;

    fn endpoint(name: &str) -> TransportEndpointId {
        TransportEndpointId::new(name).expect("valid test endpoint")
    }

    fn test_core(
        cpu: u32,
        partition: u32,
        agent: u32,
        fetch_endpoint: &str,
        route: rem6_transport::MemoryRouteId,
        pc: u64,
    ) -> RiscvCore {
        RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(cpu),
                    PartitionId::new(partition),
                    AgentId::new(agent),
                    Address::new(pc),
                ),
                CpuFetchConfig::new(
                    endpoint(fetch_endpoint),
                    route,
                    CacheLineLayout::new(16).expect("valid cache line size"),
                    AccessSize::new(4).expect("valid fetch size"),
                ),
            )
            .expect("valid CPU core"),
        )
    }

    struct TranslatedTestCoreSpec<'a> {
        cpu: u32,
        partition: u32,
        agent: u32,
        fetch_endpoint: &'a str,
        fetch_route: rem6_transport::MemoryRouteId,
        data_endpoint: &'a str,
        data_route: rem6_transport::MemoryRouteId,
        pc: u64,
    }

    fn translated_test_core(
        spec: TranslatedTestCoreSpec<'_>,
        tlb_entries: Vec<TranslationTlbEntrySnapshot>,
    ) -> RiscvCore {
        let core = CpuCore::new(
            CpuResetState::new(
                CpuId::new(spec.cpu),
                PartitionId::new(spec.partition),
                AgentId::new(spec.agent),
                Address::new(spec.pc),
            ),
            CpuFetchConfig::new(
                endpoint(spec.fetch_endpoint),
                spec.fetch_route,
                CacheLineLayout::new(16).expect("valid cache line size"),
                AccessSize::new(4).expect("valid fetch size"),
            ),
        )
        .expect("valid CPU core");
        let queue_config = TranslationQueueConfig::new(4, 0).expect("valid translation queue");
        let tlb_config = TranslationTlbConfig::new(4).expect("valid translation TLB");
        let frontend =
            CpuTranslationFrontend::from_snapshot(&CpuTranslationFrontendSnapshot::new_with_tlb(
                TranslationQueueSnapshot::new(queue_config, Vec::new(), 0),
                Vec::new(),
                TranslationTlbSnapshot::new(
                    tlb_config,
                    tlb_entries,
                    8,
                    TranslationTlbStats::default(),
                ),
            ))
            .expect("valid translated frontend snapshot");

        RiscvCore::with_data_translation(
            core,
            CpuDataConfig::new(
                endpoint(spec.data_endpoint),
                spec.data_route,
                CacheLineLayout::new(16).expect("valid cache line size"),
            ),
            frontend,
        )
    }

    fn rfence_tlb_entry(virtual_page: u64, physical_page: u64) -> TranslationTlbEntrySnapshot {
        TranslationTlbEntrySnapshot::new(
            Address::new(virtual_page),
            Address::new(physical_page),
            TranslationPageSize::new(4096).expect("valid page size"),
            TranslationPagePermissions::read_write_execute(),
            4,
        )
    }

    fn hsm_request(function: u64, arg0: u64, arg1: u64, arg2: u64) -> RiscvSbiRequest {
        RiscvSbiRequest {
            extension: SBI_HSM_EXTENSION,
            function,
            arg0,
            arg1,
            arg2,
            arg3: 0,
            arg4: 0,
        }
    }

    fn rfence_request(
        function: u64,
        hart_mask: u64,
        hart_mask_base: u64,
        start_addr: u64,
        size: u64,
        asid: u64,
    ) -> RiscvSbiRequest {
        RiscvSbiRequest {
            extension: SBI_RFENCE_EXTENSION,
            function,
            arg0: hart_mask,
            arg1: hart_mask_base,
            arg2: start_addr,
            arg3: size,
            arg4: asid,
        }
    }

    fn ecall_store(address: u64) -> Arc<Mutex<PartitionedMemoryStore>> {
        let target = MemoryTargetId::new(0);
        let layout = CacheLineLayout::new(16).expect("valid cache line size");
        let mut store = PartitionedMemoryStore::new();
        store.add_partition(target, layout).expect("memory target");
        store
            .map_region(
                target,
                Address::new(0x8000),
                AccessSize::new(0x2000).expect("valid mapped size"),
            )
            .expect("mapped test memory");
        BootImage::new(Address::new(address))
            .add_segment(
                Address::new(address),
                0x0000_0073_u32.to_le_bytes().to_vec(),
            )
            .expect("ecall segment")
            .load_into_partitioned_store(&mut store, target)
            .expect("loaded ecall");
        Arc::new(Mutex::new(store))
    }

    fn responder(
        store: Arc<Mutex<PartitionedMemoryStore>>,
    ) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static
    {
        move |delivery, _context| {
            let response = store
                .lock()
                .expect("test memory lock")
                .respond(delivery.request())
                .expect("memory response")
                .response()
                .cloned()
                .expect("completed memory response");
            TargetOutcome::Respond(response)
        }
    }

    fn registered_hsm_pair() -> (
        PartitionedScheduler,
        MemoryTransport,
        RiscvSbiFirmware,
        RiscvCore,
        RiscvCore,
    ) {
        let scheduler =
            PartitionedScheduler::with_min_remote_delay(4, 2).expect("valid test scheduler");
        let mut transport = MemoryTransport::new();
        let cpu0_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu0.ifetch"),
                    PartitionId::new(0),
                    endpoint("l1i"),
                    PartitionId::new(2),
                    2,
                    3,
                )
                .expect("valid CPU 0 route"),
            )
            .expect("registered CPU 0 route");
        let cpu1_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu1.ifetch"),
                    PartitionId::new(1),
                    endpoint("l1i"),
                    PartitionId::new(2),
                    2,
                    3,
                )
                .expect("valid CPU 1 route"),
            )
            .expect("registered CPU 1 route");
        let core0 = test_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
        let core1 = test_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x8800);
        core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).expect("valid cluster");
        let firmware = RiscvSbiFirmware::new();
        firmware
            .register_cluster(&cluster)
            .expect("cluster registers with SBI firmware");
        (scheduler, transport, firmware, core0, core1)
    }

    fn registered_rfence_pair() -> (
        PartitionedScheduler,
        MemoryTransport,
        RiscvSbiFirmware,
        RiscvCore,
        RiscvCore,
    ) {
        let scheduler =
            PartitionedScheduler::with_min_remote_delay(4, 2).expect("valid test scheduler");
        let mut transport = MemoryTransport::new();
        let cpu0_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu0.ifetch"),
                    PartitionId::new(0),
                    endpoint("l1i"),
                    PartitionId::new(2),
                    2,
                    3,
                )
                .expect("valid CPU 0 route"),
            )
            .expect("registered CPU 0 route");
        let cpu1_fetch_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu1.ifetch"),
                    PartitionId::new(1),
                    endpoint("l1i"),
                    PartitionId::new(2),
                    2,
                    3,
                )
                .expect("valid CPU 1 fetch route"),
            )
            .expect("registered CPU 1 fetch route");
        let cpu1_data_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu1.dmem"),
                    PartitionId::new(1),
                    endpoint("l1d"),
                    PartitionId::new(2),
                    2,
                    3,
                )
                .expect("valid CPU 1 data route"),
            )
            .expect("registered CPU 1 data route");
        let core0 = test_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
        let core1 = translated_test_core(
            TranslatedTestCoreSpec {
                cpu: 1,
                partition: 1,
                agent: 8,
                fetch_endpoint: "cpu1.ifetch",
                fetch_route: cpu1_fetch_route,
                data_endpoint: "cpu1.dmem",
                data_route: cpu1_data_route,
                pc: 0x8800,
            },
            vec![rfence_tlb_entry(0x4000, 0x9000)],
        );
        core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).expect("valid cluster");
        let firmware = RiscvSbiFirmware::new();
        firmware
            .register_cluster(&cluster)
            .expect("cluster registers with SBI firmware");
        (scheduler, transport, firmware, core0, core1)
    }

    fn execute_sbi_ecall(
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        core: &RiscvCore,
        extension: u64,
        function: u64,
        args: [u64; 5],
    ) {
        core.write_register(register(17), extension);
        core.write_register(register(16), function);
        for (offset, value) in args.into_iter().enumerate() {
            core.write_register(register(10 + offset as u8), value);
        }
        core.issue_next_fetch(
            scheduler,
            transport,
            MemoryTrace::new(),
            responder(ecall_store(0x8000)),
        )
        .expect("issued ecall fetch");
        scheduler.run_until_idle_conservative();
        core.execute_next_completed_fetch()
            .expect("executed ecall")
            .expect("ecall execution event");
        assert!(core.has_pending_trap());
    }

    fn execute_hsm_ecall(
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        core: &RiscvCore,
        function: u64,
        arg0: u64,
        arg1: u64,
        arg2: u64,
    ) {
        execute_sbi_ecall(
            scheduler,
            transport,
            core,
            SBI_HSM_EXTENSION,
            function,
            [arg0, arg1, arg2, 0, 0],
        );
    }

    fn execute_rfence_ecall(
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        core: &RiscvCore,
        request: RiscvSbiRequest,
    ) {
        execute_sbi_ecall(
            scheduler,
            transport,
            core,
            request.extension(),
            request.function(),
            [
                request.arg0(),
                request.arg1(),
                request.arg2(),
                request.arg3(),
                request.arg4(),
            ],
        );
    }

    fn execute_hsm_hart_start_ecall(
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        core: &RiscvCore,
    ) {
        execute_hsm_ecall(
            scheduler,
            transport,
            core,
            SBI_HSM_HART_START,
            1,
            0x9000,
            0x55,
        );
    }

    #[test]
    fn remote_sfence_vma_flushes_target_tlb_when_completion_event_runs() {
        let (mut scheduler, transport, firmware, core0, core1) = registered_rfence_pair();
        assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
        assert_eq!(
            core1.data_translation_tlb_contains_entry(
                TranslationAddressSpaceId::global(),
                Address::new(0x4000),
            ),
            Some(true)
        );

        execute_rfence_ecall(
            &mut scheduler,
            &transport,
            &core0,
            rfence_request(SBI_RFENCE_REMOTE_SFENCE_VMA, 0b10, 0, 0, 0, 0),
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::success(0));
        assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));

        scheduler.run_until_idle_conservative();

        assert_eq!(core1.data_translation_tlb_entry_count(), Some(0));
    }

    #[test]
    fn hart_start_reports_start_pending_before_entry_event_runs() {
        let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();

        let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

        assert_eq!(start, RiscvSbiOutcome::success(0));
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_START_PENDING)
        );
        assert_eq!(core1.pc(), Address::new(0x8800));
    }

    #[test]
    fn hart_start_reports_already_available_for_started_target() {
        let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();
        core1.set_hart_started();

        let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

        assert_eq!(start, RiscvSbiOutcome::already_available());
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
        assert_eq!(core1.pc(), Address::new(0x8800));
    }

    #[test]
    fn hart_start_reports_invalid_param_for_suspended_target() {
        let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();
        core1.set_hart_suspended();

        let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

        assert_eq!(start, RiscvSbiOutcome::invalid_param());
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_SUSPENDED)
        );
        assert_eq!(core1.pc(), Address::new(0x8800));
    }

    #[test]
    fn handle_pending_core_trap_schedules_hart_start_completion() {
        let (mut scheduler, transport, firmware, core0, core1) = registered_hsm_pair();
        execute_hsm_hart_start_ecall(&mut scheduler, &transport, &core0);

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::success(0));
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_START_PENDING)
        );
        assert_eq!(core1.pc(), Address::new(0x8800));

        scheduler.run_until_idle_conservative();

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
        assert_eq!(core1.pc(), Address::new(0x9000));
        assert_eq!(core1.read_register(register(10)), 1);
        assert_eq!(core1.read_register(register(11)), 0x55);
    }

    fn assert_hart_stop_pending_until_event_runs(parallel: bool) {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_STOP,
            44,
            55,
            0,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, parallel)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::Stopped);
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_STOP_PENDING)
        );
        assert_eq!(core0.read_register(register(10)), 44);
        assert_eq!(core0.read_register(register(11)), 55);

        if parallel {
            scheduler
                .run_until_idle_parallel()
                .expect("parallel stop event");
        } else {
            scheduler.run_until_idle_conservative();
        }

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STOPPED)
        );
        assert_eq!(core0.read_register(register(31)), 0);
    }

    #[test]
    fn handle_pending_core_trap_reports_hart_stop_pending_until_event_runs() {
        assert_hart_stop_pending_until_event_runs(false);
    }

    #[test]
    fn parallel_handle_pending_core_trap_reports_hart_stop_pending_until_event_runs() {
        assert_hart_stop_pending_until_event_runs(true);
    }

    #[test]
    fn scheduled_hart_stop_does_not_complete_after_state_changes() {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_STOP,
            44,
            55,
            0,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::Stopped);
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_STOP_PENDING)
        );
        core0.set_hart_started();
        scheduler.run_until_idle_conservative();

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
    }

    fn assert_retentive_suspend_pending_until_event_runs(parallel: bool) {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_SUSPEND,
            SBI_HSM_DEFAULT_RETENTIVE_SUSPEND,
            0x9000,
            0x55,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, parallel)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::success(0));
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_SUSPEND_PENDING)
        );
        assert_eq!(
            core0.read_register(register(10)),
            SBI_HSM_DEFAULT_RETENTIVE_SUSPEND
        );
        assert_eq!(core0.read_register(register(11)), 0x9000);

        if parallel {
            scheduler
                .run_until_idle_parallel()
                .expect("parallel suspend event");
        } else {
            scheduler.run_until_idle_conservative();
        }

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_SUSPENDED)
        );
        assert_eq!(core0.read_register(register(31)), 0);
    }

    #[test]
    fn handle_pending_core_trap_reports_retentive_suspend_pending_until_event_runs() {
        assert_retentive_suspend_pending_until_event_runs(false);
    }

    #[test]
    fn parallel_handle_pending_core_trap_reports_retentive_suspend_pending_until_event_runs() {
        assert_retentive_suspend_pending_until_event_runs(true);
    }

    #[test]
    fn scheduled_hart_suspend_does_not_complete_after_state_changes() {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_SUSPEND,
            SBI_HSM_DEFAULT_RETENTIVE_SUSPEND,
            0x9000,
            0x55,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::success(0));
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_SUSPEND_PENDING)
        );
        core0.set_hart_started();
        scheduler.run_until_idle_conservative();

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
    }

    fn assert_nonretentive_resume_pending_until_resume_event_runs(parallel: bool) {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_SUSPEND,
            SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND,
            0x9000,
            0x55,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, parallel)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::Resumed);
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_RESUME_PENDING)
        );
        assert_ne!(core0.pc(), Address::new(0x9000));

        if parallel {
            scheduler
                .run_until_idle_parallel()
                .expect("parallel non-retentive resume event");
        } else {
            scheduler.run_until_idle_conservative();
        }

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
        assert_eq!(core0.pc(), Address::new(0x9000));
        assert_eq!(core0.read_register(register(10)), 0);
        assert_eq!(core0.read_register(register(11)), 0x55);
    }

    #[test]
    fn handle_pending_core_trap_reports_nonretentive_resume_pending_until_resume_event_runs() {
        assert_nonretentive_resume_pending_until_resume_event_runs(false);
    }

    #[test]
    fn parallel_handle_pending_core_trap_reports_nonretentive_resume_pending_until_resume_event_runs(
    ) {
        assert_nonretentive_resume_pending_until_resume_event_runs(true);
    }

    #[test]
    fn scheduled_nonretentive_resume_does_not_complete_after_state_changes() {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_SUSPEND,
            SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND,
            0x9000,
            0x55,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::Resumed);
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_RESUME_PENDING)
        );
        core0.set_hart_started();
        scheduler.run_until_idle_conservative();

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
        assert_ne!(core0.pc(), Address::new(0x9000));
    }

    #[test]
    fn pending_interrupt_wakes_retentive_suspend_before_suspend_event_completes() {
        let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
        execute_hsm_ecall(
            &mut scheduler,
            &transport,
            &core0,
            SBI_HSM_HART_SUSPEND,
            SBI_HSM_DEFAULT_RETENTIVE_SUSPEND,
            0x9000,
            0x55,
        );

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::success(0));
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(TEST_HART_SUSPEND_PENDING)
        );
        core0.set_machine_interrupt_pending_bits(SSIP);
        scheduler.run_until_idle_conservative();

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
        );
        assert_eq!(core0.machine_interrupt_pending() & SSIP, SSIP);
    }

    #[test]
    fn scheduled_hart_start_does_not_complete_after_state_changes() {
        let (mut scheduler, transport, firmware, core0, core1) = registered_hsm_pair();
        execute_hsm_hart_start_ecall(&mut scheduler, &transport, &core0);

        let outcome = firmware
            .handle_pending_core_trap(&mut scheduler, &core0, false)
            .expect("handled SBI trap")
            .expect("SBI outcome");

        assert_eq!(outcome, RiscvSbiOutcome::success(0));
        core1.set_hart_stopped();
        scheduler.run_until_idle_conservative();

        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(SBI_HSM_HART_STOPPED)
        );
        assert_eq!(core1.pc(), Address::new(0x8800));
    }
}
