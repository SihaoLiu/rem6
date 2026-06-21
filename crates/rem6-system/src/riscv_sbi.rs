use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvCore, RiscvHartRunState};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, SchedulerError};
use rem6_memory::{AccessSize, Address, AddressRange, TranslationAddressSpaceId};

use crate::{
    riscv_syscall::{RiscvGuestMemoryReader, RiscvGuestMemoryWriter},
    RiscvSystemRunDriver, SystemError,
};

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
const SBI_DEBUG_CONSOLE_EXTENSION: u64 = 0x4442_434e;
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
const SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID: u64 = 3;
const SBI_RFENCE_REMOTE_HFENCE_GVMA: u64 = 4;
const SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID: u64 = 5;
const SBI_RFENCE_REMOTE_HFENCE_VVMA: u64 = 6;
const SBI_SRST_SYSTEM_RESET: u64 = 0;
const SBI_DEBUG_CONSOLE_WRITE: u64 = 0;
const SBI_DEBUG_CONSOLE_READ: u64 = 1;
const SBI_DEBUG_CONSOLE_WRITE_BYTE: u64 = 2;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;
const REM6_SBI_IMPL_ID: u64 = 0x7265_6d36;
const REM6_SBI_IMPL_VERSION: u64 = 0;
const SBI_ERR_INVALID_ADDRESS: u64 = (-5_i64) as u64;
const RISCV64_ASID_MAX: u64 = u16::MAX as u64;
const RISCV64_HYPERVISOR_VMID_MAX: u64 = (1 << 14) - 1;
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
    hsm: Arc<Mutex<Vec<RiscvSbiHsmRecord>>>,
    ipis: Arc<Mutex<Vec<RiscvSbiIpiRecord>>>,
    rfences: Arc<Mutex<Vec<RiscvSbiRfenceRecord>>>,
    resets: Arc<Mutex<Vec<RiscvSbiResetRecord>>>,
    debug_console: Arc<Mutex<Vec<u8>>>,
    debug_console_input: Arc<Mutex<VecDeque<u8>>>,
    functional_guest_memory_reader: Option<RiscvGuestMemoryReader>,
    functional_guest_memory_writer: Option<RiscvGuestMemoryWriter>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RiscvSbiTimerState {
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
pub struct RiscvSbiResetRecord {
    cpu: CpuId,
    reset_type: u32,
    reset_reason: u32,
    code: i32,
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

    fn cancel(&mut self, cpu: CpuId) {
        let generation = self
            .generations
            .get(&cpu)
            .copied()
            .unwrap_or_default()
            .wrapping_add(1);
        self.generations.insert(cpu, generation);
        self.deadlines.remove(&cpu);
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
            hsm: Arc::new(Mutex::new(Vec::new())),
            ipis: Arc::new(Mutex::new(Vec::new())),
            rfences: Arc::new(Mutex::new(Vec::new())),
            resets: Arc::new(Mutex::new(Vec::new())),
            debug_console: Arc::new(Mutex::new(Vec::new())),
            debug_console_input: Arc::new(Mutex::new(VecDeque::new())),
            functional_guest_memory_reader: None,
            functional_guest_memory_writer: None,
        }
    }

    pub fn with_functional_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        self.functional_guest_memory_reader = Some(RiscvGuestMemoryReader::new(read));
        self
    }

    pub fn with_functional_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        self.functional_guest_memory_writer = Some(RiscvGuestMemoryWriter::new(write));
        self
    }

    pub fn with_functional_guest_memory_writer_object(
        mut self,
        writer: RiscvGuestMemoryWriter,
    ) -> Self {
        self.functional_guest_memory_writer = Some(writer);
        self
    }

    pub fn with_debug_console_input(mut self, input: Vec<u8>) -> Self {
        self.debug_console_input = Arc::new(Mutex::new(VecDeque::from(input)));
        self
    }

    pub(crate) fn register_cluster(&self, cluster: &RiscvCluster) -> Result<(), SystemError> {
        self.debug_console
            .lock()
            .expect("RISC-V SBI debug console lock")
            .clear();
        self.hsm.lock().expect("RISC-V SBI HSM record lock").clear();
        self.ipis
            .lock()
            .expect("RISC-V SBI IPI record lock")
            .clear();
        self.rfences
            .lock()
            .expect("RISC-V SBI RFENCE record lock")
            .clear();
        self.resets
            .lock()
            .expect("RISC-V SBI reset record lock")
            .clear();
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

    pub fn debug_console_bytes(&self) -> Vec<u8> {
        self.debug_console
            .lock()
            .expect("RISC-V SBI debug console lock")
            .clone()
    }

    pub fn hsm_records(&self) -> Vec<RiscvSbiHsmRecord> {
        self.hsm.lock().expect("RISC-V SBI HSM record lock").clone()
    }

    pub fn ipi_records(&self) -> Vec<RiscvSbiIpiRecord> {
        self.ipis
            .lock()
            .expect("RISC-V SBI IPI record lock")
            .clone()
    }

    pub fn rfence_records(&self) -> Vec<RiscvSbiRfenceRecord> {
        self.rfences
            .lock()
            .expect("RISC-V SBI RFENCE record lock")
            .clone()
    }

    pub fn reset_records(&self) -> Vec<RiscvSbiResetRecord> {
        self.resets
            .lock()
            .expect("RISC-V SBI reset record lock")
            .clone()
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
                RiscvSbiOutcome::success(SBI_SPEC_VERSION_2_0)
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
                    || request.arg0() == SBI_SRST_EXTENSION
                    || (request.arg0() == SBI_DEBUG_CONSOLE_EXTENSION
                        && self.supports_debug_console_extension()),
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
                    self.record_hsm_start(core.id(), request);
                }
                outcome
            }
            (SBI_HSM_EXTENSION, SBI_HSM_HART_STOP) => {
                let outcome = self.hart_stop(core);
                if outcome == RiscvSbiOutcome::Stopped {
                    self.schedule_hart_stop(scheduler, core, parallel)
                        .map_err(SystemError::Scheduler)?;
                    self.record_hsm_stop(core);
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
                    self.record_hsm_suspend(core.id(), request);
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
                    self.record_hsm_suspend(core.id(), request);
                }
                outcome
            }
            (SBI_IPI_EXTENSION, SBI_IPI_SEND_IPI) => self
                .send_ipi(scheduler, core, request, parallel)
                .map_err(SystemError::Scheduler)?,
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_FENCE_I) => self
                .remote_fence_i(scheduler, core, request, parallel)
                .map_err(SystemError::Scheduler)?,
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_SFENCE_VMA)
            | (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_SFENCE_VMA_ASID) => self
                .remote_sfence_vma(scheduler, core, request, parallel)
                .map_err(SystemError::Scheduler)?,
            (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID)
            | (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_HFENCE_GVMA)
            | (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID)
            | (SBI_RFENCE_EXTENSION, SBI_RFENCE_REMOTE_HFENCE_VVMA) => self
                .remote_hfence(scheduler, core, request, parallel)
                .map_err(SystemError::Scheduler)?,
            (SBI_SRST_EXTENSION, SBI_SRST_SYSTEM_RESET) => self.system_reset(core, request),
            (SBI_DEBUG_CONSOLE_EXTENSION, SBI_DEBUG_CONSOLE_WRITE) => {
                self.debug_console_write(request)
            }
            (SBI_DEBUG_CONSOLE_EXTENSION, SBI_DEBUG_CONSOLE_READ) => {
                self.debug_console_read(request)
            }
            (SBI_DEBUG_CONSOLE_EXTENSION, SBI_DEBUG_CONSOLE_WRITE_BYTE) => {
                self.debug_console_write_byte(request)
            }
            _ => RiscvSbiOutcome::not_supported(),
        }))
    }

    fn supports_debug_console_extension(&self) -> bool {
        self.functional_guest_memory_reader.is_some()
            && self.functional_guest_memory_writer.is_some()
    }

    fn debug_console_write(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        let bytes = match self.read_debug_console_payload(request) {
            Ok(bytes) => bytes,
            Err(outcome) => return outcome,
        };
        let written = bytes.len() as u64;
        self.debug_console
            .lock()
            .expect("RISC-V SBI debug console lock")
            .extend_from_slice(&bytes);
        RiscvSbiOutcome::success(written)
    }

    fn read_debug_console_payload(
        &self,
        request: RiscvSbiRequest,
    ) -> Result<Vec<u8>, RiscvSbiOutcome> {
        let (bytes, address) = debug_console_shared_memory_request(request)?;
        if bytes == 0 {
            return Ok(Vec::new());
        }
        let Some(reader) = &self.functional_guest_memory_reader else {
            return Err(RiscvSbiOutcome::invalid_address());
        };
        let Some(payload) = reader.read(address, bytes) else {
            return Err(RiscvSbiOutcome::invalid_address());
        };
        if payload.len() != bytes {
            return Err(RiscvSbiOutcome::invalid_address());
        }
        Ok(payload)
    }

    fn debug_console_read(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        let (bytes, address) = match debug_console_shared_memory_request(request) {
            Ok(range) => range,
            Err(outcome) => return outcome,
        };
        if bytes == 0 {
            return RiscvSbiOutcome::success(0);
        }
        let Some(writer) = &self.functional_guest_memory_writer else {
            return RiscvSbiOutcome::invalid_address();
        };
        if !writer.can_write(address, bytes) {
            return RiscvSbiOutcome::invalid_address();
        }
        let mut input = self
            .debug_console_input
            .lock()
            .expect("RISC-V SBI debug console input lock");
        let readable = bytes.min(input.len());
        if readable == 0 {
            return RiscvSbiOutcome::success(0);
        }
        let payload = input.iter().take(readable).copied().collect::<Vec<_>>();
        if !writer.write(address, &payload) {
            return RiscvSbiOutcome::invalid_address();
        }
        input.drain(..readable);
        RiscvSbiOutcome::success(readable as u64)
    }

    fn debug_console_write_byte(&self, request: RiscvSbiRequest) -> RiscvSbiOutcome {
        let byte = request.arg0() as u8;
        self.debug_console
            .lock()
            .expect("RISC-V SBI debug console lock")
            .push(byte);
        RiscvSbiOutcome::success(0)
    }

    fn system_reset(&self, core: &RiscvCore, request: RiscvSbiRequest) -> RiscvSbiOutcome {
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

        let code = i32::from(reset_reason == SBI_RESET_REASON_SYSTEM_FAILURE);
        self.record_reset(core.id(), reset_type, reset_reason, code);
        RiscvSbiOutcome::SystemReset {
            reset_type,
            reset_reason,
            code,
        }
    }

    fn record_reset(&self, cpu: CpuId, reset_type: u32, reset_reason: u32, code: i32) {
        self.resets
            .lock()
            .expect("RISC-V SBI reset record lock")
            .push(RiscvSbiResetRecord::new(
                cpu,
                reset_type,
                reset_reason,
                code,
            ));
    }

    fn send_ipi(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        request: RiscvSbiRequest,
        parallel: bool,
    ) -> Result<RiscvSbiOutcome, SchedulerError> {
        let Some(targets) = self.hart_mask_targets(request.arg0(), request.arg1()) else {
            return Ok(RiscvSbiOutcome::invalid_param());
        };

        let target_harts = targets.iter().map(RiscvCore::hart_id).collect();
        self.schedule_remote_ipi(scheduler, source, targets, parallel)?;
        self.record_ipi(source.id(), request.arg0(), request.arg1(), target_harts);
        Ok(RiscvSbiOutcome::success(0))
    }

    fn record_ipi(
        &self,
        source_cpu: CpuId,
        hart_mask: u64,
        hart_mask_base: u64,
        targets: Vec<u64>,
    ) {
        self.ipis
            .lock()
            .expect("RISC-V SBI IPI record lock")
            .push(RiscvSbiIpiRecord::new(
                source_cpu,
                hart_mask,
                hart_mask_base,
                targets,
            ));
    }

    fn record_hsm_start(&self, source_cpu: CpuId, request: RiscvSbiRequest) {
        self.hsm
            .lock()
            .expect("RISC-V SBI HSM record lock")
            .push(RiscvSbiHsmRecord::new(
                source_cpu,
                request.function(),
                request.arg0(),
                request.arg1(),
                request.arg2(),
            ));
    }

    fn record_hsm_stop(&self, core: &RiscvCore) {
        self.hsm
            .lock()
            .expect("RISC-V SBI HSM record lock")
            .push(RiscvSbiHsmRecord::new(
                core.id(),
                SBI_HSM_HART_STOP,
                core.hart_id(),
                0,
                0,
            ));
    }

    fn record_hsm_suspend(&self, source_cpu: CpuId, request: RiscvSbiRequest) {
        self.hsm
            .lock()
            .expect("RISC-V SBI HSM record lock")
            .push(RiscvSbiHsmRecord::new(
                source_cpu,
                request.function(),
                request.arg0(),
                request.arg1(),
                request.arg2(),
            ));
    }

    fn schedule_remote_ipi(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        targets: Vec<RiscvCore>,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        for (target, deadline) in remote_target_deadlines(scheduler, source, targets)? {
            if parallel {
                scheduler.schedule_parallel_at(target.partition(), deadline, move |_context| {
                    target.set_machine_interrupt_pending_bits(SSIP);
                })?;
            } else {
                scheduler.schedule_at(target.partition(), deadline, move |_context| {
                    target.set_machine_interrupt_pending_bits(SSIP);
                })?;
            }
        }
        Ok(())
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
            RiscvHartRunState::Started | RiscvHartRunState::Suspended => {
                return RiscvSbiOutcome::already_available();
            }
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

    fn remote_fence_i(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        request: RiscvSbiRequest,
        parallel: bool,
    ) -> Result<RiscvSbiOutcome, SchedulerError> {
        let Some(targets) = self.hart_mask_targets(request.arg0(), request.arg1()) else {
            return Ok(RiscvSbiOutcome::invalid_param());
        };
        let target_harts = targets.iter().map(RiscvCore::hart_id).collect();
        self.schedule_remote_instruction_fence(scheduler, source, targets, parallel)?;
        self.record_rfence(source.id(), request, target_harts);
        Ok(RiscvSbiOutcome::success(0))
    }

    fn schedule_remote_instruction_fence(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        targets: Vec<RiscvCore>,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        for (target, deadline) in remote_target_deadlines(scheduler, source, targets)? {
            if parallel {
                scheduler.schedule_parallel_at(target.partition(), deadline, move |_context| {
                    target.reset_instruction_fetch_stream();
                })?;
            } else {
                scheduler.schedule_at(target.partition(), deadline, move |_context| {
                    target.reset_instruction_fetch_stream();
                })?;
            }
        }
        Ok(())
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

        let target_harts = targets.iter().map(RiscvCore::hart_id).collect();
        self.schedule_remote_data_tlb_flush(
            scheduler,
            source,
            targets,
            virtual_range,
            address_space,
            parallel,
        )?;
        self.record_rfence(source.id(), request, target_harts);
        Ok(RiscvSbiOutcome::success(0))
    }

    fn remote_hfence(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        request: RiscvSbiRequest,
        parallel: bool,
    ) -> Result<RiscvSbiOutcome, SchedulerError> {
        if !valid_rfence_range(request.arg2(), request.arg3()) {
            return Ok(RiscvSbiOutcome::invalid_address());
        }
        if !valid_hfence_address_space(request) {
            return Ok(RiscvSbiOutcome::invalid_param());
        }
        let Some(targets) = self.hart_mask_targets(request.arg0(), request.arg1()) else {
            return Ok(RiscvSbiOutcome::invalid_param());
        };
        let Some((virtual_range, address_space)) = hfence_flush_scope(request) else {
            return Ok(RiscvSbiOutcome::invalid_address());
        };

        let target_harts = targets.iter().map(RiscvCore::hart_id).collect();
        self.schedule_remote_data_tlb_flush(
            scheduler,
            source,
            targets,
            virtual_range,
            address_space,
            parallel,
        )?;
        self.record_rfence(source.id(), request, target_harts);
        Ok(RiscvSbiOutcome::success(0))
    }

    fn record_rfence(&self, source_cpu: CpuId, request: RiscvSbiRequest, targets: Vec<u64>) {
        let (start_addr, size, address_space) = if request.function() == SBI_RFENCE_REMOTE_FENCE_I {
            (0, 0, None)
        } else {
            (
                request.arg2(),
                request.arg3(),
                rfence_record_address_space(request),
            )
        };
        self.rfences
            .lock()
            .expect("RISC-V SBI RFENCE record lock")
            .push(RiscvSbiRfenceRecord::new(
                source_cpu,
                request.function(),
                request.arg0(),
                request.arg1(),
                start_addr,
                size,
                address_space,
                targets,
            ));
    }

    fn schedule_remote_data_tlb_flush(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: &RiscvCore,
        targets: Vec<RiscvCore>,
        virtual_range: Option<AddressRange>,
        address_space: Option<TranslationAddressSpaceId>,
        parallel: bool,
    ) -> Result<(), SchedulerError> {
        for (target, deadline) in remote_target_deadlines(scheduler, source, targets)? {
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
        core.clear_machine_interrupt_pending_bits(STIP);
        if deadline == u64::MAX {
            self.timer
                .lock()
                .expect("RISC-V SBI timer state lock")
                .cancel(cpu);
            return Ok(());
        }

        let generation = self
            .timer
            .lock()
            .expect("RISC-V SBI timer state lock")
            .program(cpu, deadline);

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

fn remote_target_deadlines(
    scheduler: &PartitionedScheduler,
    source: &RiscvCore,
    targets: Vec<RiscvCore>,
) -> Result<Vec<(RiscvCore, u64)>, SchedulerError> {
    let source_now = scheduler.partition_now(source.partition())?;
    let delay = scheduler.min_remote_delay();
    let source_deadline = source_now
        .checked_add(delay)
        .ok_or(SchedulerError::TickOverflow {
            now: source_now,
            delay,
        })?;
    targets
        .into_iter()
        .map(|target| {
            let target_now = scheduler.partition_now(target.partition())?;
            Ok((target, source_deadline.max(target_now)))
        })
        .collect()
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

fn rfence_record_address_space(request: RiscvSbiRequest) -> Option<u64> {
    match request.function() {
        SBI_RFENCE_REMOTE_SFENCE_VMA_ASID
        | SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID
        | SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID => Some(request.arg4()),
        _ => None,
    }
}

fn valid_hfence_address_space(request: RiscvSbiRequest) -> bool {
    match request.function() {
        SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID => request.arg4() <= RISCV64_HYPERVISOR_VMID_MAX,
        SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID => request.arg4() <= RISCV64_ASID_MAX,
        SBI_RFENCE_REMOTE_HFENCE_GVMA | SBI_RFENCE_REMOTE_HFENCE_VVMA => true,
        _ => false,
    }
}

fn hfence_flush_scope(
    request: RiscvSbiRequest,
) -> Option<(Option<AddressRange>, Option<TranslationAddressSpaceId>)> {
    match request.function() {
        SBI_RFENCE_REMOTE_HFENCE_GVMA | SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID => {
            rfence_virtual_range(request.arg2(), request.arg3())?;
            // The modeled core TLB has ASID-tagged data translations but no
            // VMID or G-stage address tag, so G-stage fences over-invalidate.
            Some((None, None))
        }
        SBI_RFENCE_REMOTE_HFENCE_VVMA => {
            Some((rfence_virtual_range(request.arg2(), request.arg3())?, None))
        }
        SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID => Some((
            rfence_virtual_range(request.arg2(), request.arg3())?,
            Some(TranslationAddressSpaceId::new(
                u16::try_from(request.arg4()).ok()?,
            )),
        )),
        _ => None,
    }
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

fn debug_console_shared_memory_request(
    request: RiscvSbiRequest,
) -> Result<(usize, u64), RiscvSbiOutcome> {
    let bytes = usize::try_from(request.arg0()).map_err(|_| RiscvSbiOutcome::invalid_param())?;
    if request.arg2() != 0 {
        return Err(RiscvSbiOutcome::invalid_param());
    }
    if bytes != 0 {
        let last_offset = u64::try_from(bytes)
            .ok()
            .and_then(|bytes| bytes.checked_sub(1))
            .ok_or_else(RiscvSbiOutcome::invalid_param)?;
        request
            .arg1()
            .checked_add(last_offset)
            .ok_or_else(RiscvSbiOutcome::invalid_param)?;
    }
    Ok((bytes, request.arg1()))
}

impl RiscvSystemRunDriver {
    pub fn with_riscv_sbi_firmware(mut self) -> Self {
        self.riscv_sbi_firmware = Some(RiscvSbiFirmware::new());
        self
    }

    pub fn with_riscv_sbi_firmware_and_functional_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        let firmware = self
            .riscv_sbi_firmware
            .take()
            .unwrap_or_default()
            .with_functional_guest_memory_reader(read);
        self.riscv_sbi_firmware = Some(firmware);
        self
    }

    pub fn with_riscv_sbi_firmware_and_functional_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        let firmware = self
            .riscv_sbi_firmware
            .take()
            .unwrap_or_default()
            .with_functional_guest_memory_writer(write);
        self.riscv_sbi_firmware = Some(firmware);
        self
    }

    pub fn with_riscv_sbi_firmware_and_functional_guest_memory_writer_object(
        mut self,
        writer: RiscvGuestMemoryWriter,
    ) -> Self {
        let firmware = self
            .riscv_sbi_firmware
            .take()
            .unwrap_or_default()
            .with_functional_guest_memory_writer_object(writer);
        self.riscv_sbi_firmware = Some(firmware);
        self
    }

    pub fn with_riscv_sbi_debug_console_input(mut self, input: Vec<u8>) -> Self {
        let firmware = self
            .riscv_sbi_firmware
            .take()
            .unwrap_or_default()
            .with_debug_console_input(input);
        self.riscv_sbi_firmware = Some(firmware);
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
mod tests;
