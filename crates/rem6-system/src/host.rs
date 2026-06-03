mod execution_mode_checkpoint;

use std::collections::BTreeMap;

use rem6_checkpoint::{
    CheckpointComponentId, CheckpointError, CheckpointManifest, CheckpointRegistry,
};
use rem6_kernel::Tick;
use rem6_stats::{StatDumpRecord, StatsRegistry, StatsResetRecord};

use crate::{
    AcceleratorCheckpointBank, ClintCheckpointBank, CpuLocalTimerCheckpointBank,
    DramMemoryCheckpointBank, ExecutionMode, ExecutionModeTarget, FabricCheckpointBank,
    GpuCheckpointBank, GuestEventDelivery, GuestEventId, GuestFdCheckpointBank,
    GuestFutexCheckpointBank, GuestHostCallResponse, GuestSourceId, GuestWaitCheckpointBank,
    HostAction, HostActionRecord, HostEventPolicy, IdeControllerCheckpointBank,
    IdeControllerCheckpointPort, InterruptControllerCheckpointBank, MemoryStoreCheckpointBank,
    MsiBankCheckpointBank, PciHostCheckpointBank, PciHostCheckpointPort,
    PciLegacyInterruptRouterCheckpointBank, PciLegacyInterruptRouterCheckpointPort,
    Pl011UartCheckpointBank, Pl031CheckpointBank, PlicCheckpointBank, RiscvCoreCheckpointBank,
    RtcCheckpointBank, SchedulerCheckpointBank, SinicFifoCheckpointBank, SinicFifoCheckpointPort,
    SinicRegisterCheckpointBank, SinicRegisterCheckpointPort, Sp804CheckpointBank,
    Sp805CheckpointBank, StopRequest, StorageImageCheckpointBank, StorageImageCheckpointPort,
    SystemError, TimerCheckpointBank, UartCheckpointBank, VirtioPciCommonCheckpointBank,
    VirtioPciCommonCheckpointPort, VirtioPciDeviceConfigCheckpointBank,
    VirtioPciDeviceConfigCheckpointPort, VirtioPciIsrCheckpointBank, VirtioPciIsrCheckpointPort,
    VirtioPciNotifyCheckpointBank, VirtioPciNotifyCheckpointPort, VirtioSplitQueueCheckpointBank,
    VirtioSplitQueueCheckpointPort,
};

pub use execution_mode_checkpoint::ExecutionModeCheckpointError;
use execution_mode_checkpoint::{
    decode_execution_modes, encode_execution_modes, execution_mode_checkpoint_component,
    manifest_has_execution_mode_checkpoint, EXECUTION_MODE_CHECKPOINT_CHUNK,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemActionOutcome {
    InjectedCommand {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        command: String,
    },
    GuestHostCall {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        selector: u64,
        arguments: Vec<u64>,
        payload: Vec<u8>,
        response: GuestHostCallResponse,
    },
    StatsReset(StatsResetRecord),
    StatsDump(StatDumpRecord),
    Checkpoint {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        manifest: CheckpointManifest,
    },
    CheckpointRestored {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        manifest: CheckpointManifest,
    },
    ExecutionModeSwitched {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        target: ExecutionModeTarget,
        previous_mode: Option<ExecutionMode>,
        mode: ExecutionMode,
        stats_epoch: u64,
        stats_reset_tick: Tick,
    },
    Stop(StopRequest),
}

#[derive(Clone, Debug)]
pub struct SystemActionExecutor {
    stats: StatsRegistry,
    checkpoints: CheckpointRegistry,
    captured_manifests: BTreeMap<String, CheckpointManifest>,
    accelerator_checkpoints: Option<AcceleratorCheckpointBank>,
    msi_bank_checkpoints: Option<MsiBankCheckpointBank>,
    fabric_checkpoints: Option<FabricCheckpointBank>,
    gpu_checkpoints: Option<GpuCheckpointBank>,
    riscv_checkpoints: Option<RiscvCoreCheckpointBank>,
    scheduler_checkpoints: Option<SchedulerCheckpointBank>,
    memory_checkpoints: Option<MemoryStoreCheckpointBank>,
    storage_image_checkpoints: Option<StorageImageCheckpointBank>,
    guest_fd_checkpoints: Option<GuestFdCheckpointBank>,
    guest_futex_checkpoints: Option<GuestFutexCheckpointBank>,
    guest_wait_checkpoints: Option<GuestWaitCheckpointBank>,
    ide_controller_checkpoints: Option<IdeControllerCheckpointBank>,
    sinic_register_checkpoints: Option<SinicRegisterCheckpointBank>,
    sinic_fifo_checkpoints: Option<SinicFifoCheckpointBank>,
    dram_memory_checkpoints: Option<DramMemoryCheckpointBank>,
    interrupt_controller_checkpoints: Option<InterruptControllerCheckpointBank>,
    clint_checkpoints: Option<ClintCheckpointBank>,
    timer_checkpoints: Option<TimerCheckpointBank>,
    uart_checkpoints: Option<UartCheckpointBank>,
    pl011_uart_checkpoints: Option<Pl011UartCheckpointBank>,
    plic_checkpoints: Option<PlicCheckpointBank>,
    pl031_checkpoints: Option<Pl031CheckpointBank>,
    sp804_checkpoints: Option<Sp804CheckpointBank>,
    sp805_checkpoints: Option<Sp805CheckpointBank>,
    cpu_local_timer_checkpoints: Option<CpuLocalTimerCheckpointBank>,
    rtc_checkpoints: Option<RtcCheckpointBank>,
    pci_host_checkpoints: Option<PciHostCheckpointBank>,
    pci_legacy_interrupt_router_checkpoints: Option<PciLegacyInterruptRouterCheckpointBank>,
    virtio_split_queue_checkpoints: Option<VirtioSplitQueueCheckpointBank>,
    virtio_pci_common_checkpoints: Option<VirtioPciCommonCheckpointBank>,
    virtio_pci_isr_checkpoints: Option<VirtioPciIsrCheckpointBank>,
    virtio_pci_notify_checkpoints: Option<VirtioPciNotifyCheckpointBank>,
    virtio_pci_device_config_checkpoints: Option<VirtioPciDeviceConfigCheckpointBank>,
    execution_modes: BTreeMap<ExecutionModeTarget, ExecutionMode>,
    guest_host_call_responses: BTreeMap<u64, GuestHostCallResponse>,
    execution_mode_checkpoint_registered: bool,
}

impl SystemActionExecutor {
    pub fn new(stats: StatsRegistry) -> Self {
        Self::with_checkpoint(stats, CheckpointRegistry::new())
    }

    pub fn with_checkpoint(stats: StatsRegistry, checkpoints: CheckpointRegistry) -> Self {
        Self {
            stats,
            checkpoints,
            captured_manifests: BTreeMap::new(),
            accelerator_checkpoints: None,
            msi_bank_checkpoints: None,
            fabric_checkpoints: None,
            gpu_checkpoints: None,
            riscv_checkpoints: None,
            scheduler_checkpoints: None,
            memory_checkpoints: None,
            storage_image_checkpoints: None,
            guest_fd_checkpoints: None,
            guest_futex_checkpoints: None,
            guest_wait_checkpoints: None,
            ide_controller_checkpoints: None,
            sinic_register_checkpoints: None,
            sinic_fifo_checkpoints: None,
            dram_memory_checkpoints: None,
            interrupt_controller_checkpoints: None,
            clint_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
            pl011_uart_checkpoints: None,
            plic_checkpoints: None,
            pl031_checkpoints: None,
            sp804_checkpoints: None,
            sp805_checkpoints: None,
            cpu_local_timer_checkpoints: None,
            rtc_checkpoints: None,
            pci_host_checkpoints: None,
            pci_legacy_interrupt_router_checkpoints: None,
            virtio_split_queue_checkpoints: None,
            virtio_pci_common_checkpoints: None,
            virtio_pci_isr_checkpoints: None,
            virtio_pci_notify_checkpoints: None,
            virtio_pci_device_config_checkpoints: None,
            execution_modes: BTreeMap::new(),
            guest_host_call_responses: BTreeMap::new(),
            execution_mode_checkpoint_registered: false,
        }
    }

    pub fn with_riscv_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        riscv_checkpoints: RiscvCoreCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.riscv_checkpoints = Some(riscv_checkpoints);
        executor
    }

    pub fn with_memory_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        memory_checkpoints: MemoryStoreCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.memory_checkpoints = Some(memory_checkpoints);
        executor
    }

    pub fn with_msi_bank_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        msi_bank_checkpoints: MsiBankCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.msi_bank_checkpoints = Some(msi_bank_checkpoints);
        executor
    }

    pub fn with_dram_memory_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        dram_memory_checkpoints: DramMemoryCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.dram_memory_checkpoints = Some(dram_memory_checkpoints);
        executor
    }

    pub fn with_uart_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        uart_checkpoints: UartCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.uart_checkpoints = Some(uart_checkpoints);
        executor
    }

    pub fn with_pl011_uart_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        pl011_uart_checkpoints: Pl011UartCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.pl011_uart_checkpoints = Some(pl011_uart_checkpoints);
        executor
    }

    pub fn with_pci_host_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        pci_host_checkpoints: PciHostCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.pci_host_checkpoints = Some(pci_host_checkpoints);
        executor
    }

    pub fn with_checkpoint_banks(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        riscv_checkpoints: RiscvCoreCheckpointBank,
        memory_checkpoints: MemoryStoreCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.riscv_checkpoints = Some(riscv_checkpoints);
        executor.memory_checkpoints = Some(memory_checkpoints);
        executor
    }

    pub fn with_riscv_and_dram_checkpoint_banks(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        riscv_checkpoints: RiscvCoreCheckpointBank,
        dram_memory_checkpoints: DramMemoryCheckpointBank,
    ) -> Self {
        let mut executor = Self::with_checkpoint(stats, checkpoints);
        executor.riscv_checkpoints = Some(riscv_checkpoints);
        executor.dram_memory_checkpoints = Some(dram_memory_checkpoints);
        executor
    }

    pub const fn stats(&self) -> &StatsRegistry {
        &self.stats
    }

    pub const fn stats_mut(&mut self) -> &mut StatsRegistry {
        &mut self.stats
    }

    pub const fn checkpoints(&self) -> &CheckpointRegistry {
        &self.checkpoints
    }

    pub const fn checkpoints_mut(&mut self) -> &mut CheckpointRegistry {
        &mut self.checkpoints
    }

    pub fn execution_mode(&self, target: &ExecutionModeTarget) -> Option<ExecutionMode> {
        self.execution_modes.get(target).copied()
    }

    pub fn execution_modes(&self) -> &BTreeMap<ExecutionModeTarget, ExecutionMode> {
        &self.execution_modes
    }

    pub fn register_guest_host_call_response(
        &mut self,
        selector: u64,
        response: GuestHostCallResponse,
    ) -> Option<GuestHostCallResponse> {
        self.guest_host_call_responses.insert(selector, response)
    }

    pub fn guest_host_call_response(&self, selector: u64) -> Option<&GuestHostCallResponse> {
        self.guest_host_call_responses.get(&selector)
    }

    fn resolve_guest_host_call_response(&self, selector: u64) -> GuestHostCallResponse {
        self.guest_host_call_responses
            .get(&selector)
            .cloned()
            .unwrap_or_else(GuestHostCallResponse::unhandled)
    }

    fn capture_execution_modes_into(
        &self,
        checkpoints: &mut CheckpointRegistry,
    ) -> Result<bool, SystemError> {
        if self.execution_modes.is_empty() && !self.execution_mode_checkpoint_registered {
            return Ok(false);
        }

        let component = execution_mode_checkpoint_component();
        match checkpoints.register(component.clone()) {
            Ok(()) | Err(CheckpointError::DuplicateComponent { .. }) => {}
            Err(error) => return Err(SystemError::Checkpoint(error)),
        }
        checkpoints
            .write_chunk(
                &component,
                EXECUTION_MODE_CHECKPOINT_CHUNK,
                encode_execution_modes(&self.execution_modes),
            )
            .map_err(SystemError::Checkpoint)
            .map(|()| true)
    }

    fn stage_execution_mode_checkpoint_restore(
        &self,
        checkpoints: &mut CheckpointRegistry,
        manifest: &CheckpointManifest,
    ) -> Result<(BTreeMap<ExecutionModeTarget, ExecutionMode>, bool), SystemError> {
        let component = execution_mode_checkpoint_component();
        if !manifest_has_execution_mode_checkpoint(manifest) {
            let modes = BTreeMap::new();
            if self.execution_mode_checkpoint_registered {
                checkpoints
                    .write_chunk(
                        &component,
                        EXECUTION_MODE_CHECKPOINT_CHUNK,
                        encode_execution_modes(&modes),
                    )
                    .map_err(SystemError::Checkpoint)?;
            }
            return Ok((modes, self.execution_mode_checkpoint_registered));
        }

        match checkpoints.register(component.clone()) {
            Ok(()) | Err(CheckpointError::DuplicateComponent { .. }) => {}
            Err(error) => return Err(SystemError::Checkpoint(error)),
        }

        let payload = checkpoints
            .chunk(&component, EXECUTION_MODE_CHECKPOINT_CHUNK)
            .ok_or_else(|| ExecutionModeCheckpointError::MissingChunk {
                component: component.clone(),
                name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
            })
            .map_err(SystemError::ExecutionModeCheckpoint)?;
        decode_execution_modes(&component, payload)
            .map(|modes| (modes, true))
            .map_err(SystemError::ExecutionModeCheckpoint)
    }

    pub fn attach_memory_checkpoint_bank(
        &mut self,
        memory_checkpoints: MemoryStoreCheckpointBank,
    ) -> Result<(), CheckpointError> {
        memory_checkpoints.register_all(&mut self.checkpoints)?;
        self.memory_checkpoints = Some(memory_checkpoints);
        Ok(())
    }

    pub fn attach_storage_image_checkpoint_bank(
        &mut self,
        storage_image_checkpoints: StorageImageCheckpointBank,
    ) -> Result<(), CheckpointError> {
        storage_image_checkpoints.register_all(&mut self.checkpoints)?;
        self.storage_image_checkpoints = Some(storage_image_checkpoints);
        Ok(())
    }

    pub fn attach_guest_fd_checkpoint_bank(
        &mut self,
        guest_fd_checkpoints: GuestFdCheckpointBank,
    ) -> Result<(), CheckpointError> {
        guest_fd_checkpoints.register_all(&mut self.checkpoints)?;
        self.guest_fd_checkpoints = Some(guest_fd_checkpoints);
        Ok(())
    }

    pub fn attach_guest_futex_checkpoint_bank(
        &mut self,
        guest_futex_checkpoints: GuestFutexCheckpointBank,
    ) -> Result<(), CheckpointError> {
        guest_futex_checkpoints.register_all(&mut self.checkpoints)?;
        self.guest_futex_checkpoints = Some(guest_futex_checkpoints);
        Ok(())
    }

    pub fn attach_guest_wait_checkpoint_bank(
        &mut self,
        guest_wait_checkpoints: GuestWaitCheckpointBank,
    ) -> Result<(), CheckpointError> {
        guest_wait_checkpoints.register_all(&mut self.checkpoints)?;
        self.guest_wait_checkpoints = Some(guest_wait_checkpoints);
        Ok(())
    }

    pub fn attach_ide_controller_checkpoint_bank(
        &mut self,
        ide_controller_checkpoints: IdeControllerCheckpointBank,
    ) -> Result<(), CheckpointError> {
        ide_controller_checkpoints.register_all(&mut self.checkpoints)?;
        self.ide_controller_checkpoints = Some(ide_controller_checkpoints);
        Ok(())
    }

    pub fn attach_sinic_register_checkpoint_bank(
        &mut self,
        sinic_register_checkpoints: SinicRegisterCheckpointBank,
    ) -> Result<(), CheckpointError> {
        sinic_register_checkpoints.register_all(&mut self.checkpoints)?;
        self.sinic_register_checkpoints = Some(sinic_register_checkpoints);
        Ok(())
    }

    pub fn attach_sinic_fifo_checkpoint_bank(
        &mut self,
        sinic_fifo_checkpoints: SinicFifoCheckpointBank,
    ) -> Result<(), CheckpointError> {
        sinic_fifo_checkpoints.register_all(&mut self.checkpoints)?;
        self.sinic_fifo_checkpoints = Some(sinic_fifo_checkpoints);
        Ok(())
    }

    pub fn attach_sinic_register_checkpoint_port(
        &mut self,
        port: SinicRegisterCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(sinic_register_checkpoints) = &mut self.sinic_register_checkpoints {
            sinic_register_checkpoints.insert_port(port)
        } else {
            self.sinic_register_checkpoints = Some(SinicRegisterCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_sinic_fifo_checkpoint_port(
        &mut self,
        port: SinicFifoCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(sinic_fifo_checkpoints) = &mut self.sinic_fifo_checkpoints {
            sinic_fifo_checkpoints.insert_port(port)
        } else {
            self.sinic_fifo_checkpoints = Some(SinicFifoCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_storage_image_checkpoint_port(
        &mut self,
        port: StorageImageCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(storage_image_checkpoints) = &mut self.storage_image_checkpoints {
            storage_image_checkpoints.insert_port(port)
        } else {
            self.storage_image_checkpoints = Some(StorageImageCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_ide_controller_checkpoint_port(
        &mut self,
        port: IdeControllerCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(ide_controller_checkpoints) = &mut self.ide_controller_checkpoints {
            ide_controller_checkpoints.insert_port(port)
        } else {
            self.ide_controller_checkpoints = Some(IdeControllerCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_accelerator_checkpoint_bank(
        &mut self,
        accelerator_checkpoints: AcceleratorCheckpointBank,
    ) -> Result<(), CheckpointError> {
        accelerator_checkpoints.register_all(&mut self.checkpoints)?;
        self.accelerator_checkpoints = Some(accelerator_checkpoints);
        Ok(())
    }

    pub fn attach_msi_bank_checkpoint_bank(
        &mut self,
        msi_bank_checkpoints: MsiBankCheckpointBank,
    ) -> Result<(), CheckpointError> {
        msi_bank_checkpoints.register_all(&mut self.checkpoints)?;
        self.msi_bank_checkpoints = Some(msi_bank_checkpoints);
        Ok(())
    }

    pub fn attach_fabric_checkpoint_bank(
        &mut self,
        fabric_checkpoints: FabricCheckpointBank,
    ) -> Result<(), CheckpointError> {
        fabric_checkpoints.register_all(&mut self.checkpoints)?;
        self.fabric_checkpoints = Some(fabric_checkpoints);
        Ok(())
    }

    pub fn attach_gpu_checkpoint_bank(
        &mut self,
        gpu_checkpoints: GpuCheckpointBank,
    ) -> Result<(), CheckpointError> {
        gpu_checkpoints.register_all(&mut self.checkpoints)?;
        self.gpu_checkpoints = Some(gpu_checkpoints);
        Ok(())
    }

    pub fn attach_riscv_checkpoint_bank(
        &mut self,
        riscv_checkpoints: RiscvCoreCheckpointBank,
    ) -> Result<(), CheckpointError> {
        riscv_checkpoints.register_all(&mut self.checkpoints)?;
        self.riscv_checkpoints = Some(riscv_checkpoints);
        Ok(())
    }

    pub fn attach_scheduler_checkpoint_bank(
        &mut self,
        scheduler_checkpoints: SchedulerCheckpointBank,
    ) -> Result<(), CheckpointError> {
        scheduler_checkpoints.register_all(&mut self.checkpoints)?;
        self.scheduler_checkpoints = Some(scheduler_checkpoints);
        Ok(())
    }

    pub fn attach_dram_memory_checkpoint_bank(
        &mut self,
        dram_memory_checkpoints: DramMemoryCheckpointBank,
    ) -> Result<(), CheckpointError> {
        dram_memory_checkpoints.register_all(&mut self.checkpoints)?;
        self.dram_memory_checkpoints = Some(dram_memory_checkpoints);
        Ok(())
    }

    pub fn attach_interrupt_controller_checkpoint_bank(
        &mut self,
        interrupt_controller_checkpoints: InterruptControllerCheckpointBank,
    ) -> Result<(), CheckpointError> {
        interrupt_controller_checkpoints.register_all(&mut self.checkpoints)?;
        self.interrupt_controller_checkpoints = Some(interrupt_controller_checkpoints);
        Ok(())
    }

    pub fn attach_clint_checkpoint_bank(
        &mut self,
        clint_checkpoints: ClintCheckpointBank,
    ) -> Result<(), CheckpointError> {
        clint_checkpoints.register_all(&mut self.checkpoints)?;
        self.clint_checkpoints = Some(clint_checkpoints);
        Ok(())
    }

    pub fn attach_uart_checkpoint_bank(
        &mut self,
        uart_checkpoints: UartCheckpointBank,
    ) -> Result<(), CheckpointError> {
        uart_checkpoints.register_all(&mut self.checkpoints)?;
        self.uart_checkpoints = Some(uart_checkpoints);
        Ok(())
    }

    pub fn attach_pl011_uart_checkpoint_bank(
        &mut self,
        pl011_uart_checkpoints: Pl011UartCheckpointBank,
    ) -> Result<(), CheckpointError> {
        pl011_uart_checkpoints.register_all(&mut self.checkpoints)?;
        self.pl011_uart_checkpoints = Some(pl011_uart_checkpoints);
        Ok(())
    }

    pub fn attach_plic_checkpoint_bank(
        &mut self,
        plic_checkpoints: PlicCheckpointBank,
    ) -> Result<(), CheckpointError> {
        plic_checkpoints.register_all(&mut self.checkpoints)?;
        self.plic_checkpoints = Some(plic_checkpoints);
        Ok(())
    }

    pub fn attach_pl031_checkpoint_bank(
        &mut self,
        pl031_checkpoints: Pl031CheckpointBank,
    ) -> Result<(), CheckpointError> {
        pl031_checkpoints.register_all(&mut self.checkpoints)?;
        self.pl031_checkpoints = Some(pl031_checkpoints);
        Ok(())
    }

    pub fn attach_sp804_checkpoint_bank(
        &mut self,
        sp804_checkpoints: Sp804CheckpointBank,
    ) -> Result<(), CheckpointError> {
        sp804_checkpoints.register_all(&mut self.checkpoints)?;
        self.sp804_checkpoints = Some(sp804_checkpoints);
        Ok(())
    }

    pub fn attach_sp805_checkpoint_bank(
        &mut self,
        sp805_checkpoints: Sp805CheckpointBank,
    ) -> Result<(), CheckpointError> {
        sp805_checkpoints.register_all(&mut self.checkpoints)?;
        self.sp805_checkpoints = Some(sp805_checkpoints);
        Ok(())
    }

    pub fn attach_cpu_local_timer_checkpoint_bank(
        &mut self,
        cpu_local_timer_checkpoints: CpuLocalTimerCheckpointBank,
    ) -> Result<(), CheckpointError> {
        cpu_local_timer_checkpoints.register_all(&mut self.checkpoints)?;
        self.cpu_local_timer_checkpoints = Some(cpu_local_timer_checkpoints);
        Ok(())
    }

    pub fn attach_rtc_checkpoint_bank(
        &mut self,
        rtc_checkpoints: RtcCheckpointBank,
    ) -> Result<(), CheckpointError> {
        rtc_checkpoints.register_all(&mut self.checkpoints)?;
        self.rtc_checkpoints = Some(rtc_checkpoints);
        Ok(())
    }

    pub fn attach_timer_checkpoint_bank(
        &mut self,
        timer_checkpoints: TimerCheckpointBank,
    ) -> Result<(), CheckpointError> {
        timer_checkpoints.register_all(&mut self.checkpoints)?;
        self.timer_checkpoints = Some(timer_checkpoints);
        Ok(())
    }

    pub fn attach_pci_host_checkpoint_bank(
        &mut self,
        pci_host_checkpoints: PciHostCheckpointBank,
    ) -> Result<(), CheckpointError> {
        pci_host_checkpoints.register_all(&mut self.checkpoints)?;
        self.pci_host_checkpoints = Some(pci_host_checkpoints);
        Ok(())
    }

    pub fn attach_pci_legacy_interrupt_router_checkpoint_bank(
        &mut self,
        pci_legacy_interrupt_router_checkpoints: PciLegacyInterruptRouterCheckpointBank,
    ) -> Result<(), CheckpointError> {
        pci_legacy_interrupt_router_checkpoints.register_all(&mut self.checkpoints)?;
        self.pci_legacy_interrupt_router_checkpoints =
            Some(pci_legacy_interrupt_router_checkpoints);
        Ok(())
    }

    pub fn attach_pci_host_checkpoint_port(
        &mut self,
        port: PciHostCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(pci_host_checkpoints) = &mut self.pci_host_checkpoints {
            pci_host_checkpoints.insert_port(port)
        } else {
            self.pci_host_checkpoints = Some(PciHostCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_pci_legacy_interrupt_router_checkpoint_port(
        &mut self,
        port: PciLegacyInterruptRouterCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(pci_legacy_interrupt_router_checkpoints) =
            &mut self.pci_legacy_interrupt_router_checkpoints
        {
            pci_legacy_interrupt_router_checkpoints.insert_port(port)
        } else {
            self.pci_legacy_interrupt_router_checkpoints =
                Some(PciLegacyInterruptRouterCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_virtio_split_queue_checkpoint_bank(
        &mut self,
        virtio_split_queue_checkpoints: VirtioSplitQueueCheckpointBank,
    ) -> Result<(), CheckpointError> {
        virtio_split_queue_checkpoints.register_all(&mut self.checkpoints)?;
        self.virtio_split_queue_checkpoints = Some(virtio_split_queue_checkpoints);
        Ok(())
    }

    pub fn attach_virtio_split_queue_checkpoint_port(
        &mut self,
        port: VirtioSplitQueueCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(virtio_split_queue_checkpoints) = &mut self.virtio_split_queue_checkpoints {
            virtio_split_queue_checkpoints.insert_port(port)
        } else {
            self.virtio_split_queue_checkpoints =
                Some(VirtioSplitQueueCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_virtio_pci_isr_checkpoint_bank(
        &mut self,
        virtio_pci_isr_checkpoints: VirtioPciIsrCheckpointBank,
    ) -> Result<(), CheckpointError> {
        virtio_pci_isr_checkpoints.register_all(&mut self.checkpoints)?;
        self.virtio_pci_isr_checkpoints = Some(virtio_pci_isr_checkpoints);
        Ok(())
    }

    pub fn attach_virtio_pci_common_checkpoint_bank(
        &mut self,
        virtio_pci_common_checkpoints: VirtioPciCommonCheckpointBank,
    ) -> Result<(), CheckpointError> {
        virtio_pci_common_checkpoints.register_all(&mut self.checkpoints)?;
        self.virtio_pci_common_checkpoints = Some(virtio_pci_common_checkpoints);
        Ok(())
    }

    pub fn attach_virtio_pci_notify_checkpoint_bank(
        &mut self,
        virtio_pci_notify_checkpoints: VirtioPciNotifyCheckpointBank,
    ) -> Result<(), CheckpointError> {
        virtio_pci_notify_checkpoints.register_all(&mut self.checkpoints)?;
        self.virtio_pci_notify_checkpoints = Some(virtio_pci_notify_checkpoints);
        Ok(())
    }

    pub fn attach_virtio_pci_device_config_checkpoint_bank(
        &mut self,
        virtio_pci_device_config_checkpoints: VirtioPciDeviceConfigCheckpointBank,
    ) -> Result<(), CheckpointError> {
        virtio_pci_device_config_checkpoints.register_all(&mut self.checkpoints)?;
        self.virtio_pci_device_config_checkpoints = Some(virtio_pci_device_config_checkpoints);
        Ok(())
    }

    pub fn attach_virtio_pci_common_checkpoint_port(
        &mut self,
        port: VirtioPciCommonCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(virtio_pci_common_checkpoints) = &mut self.virtio_pci_common_checkpoints {
            virtio_pci_common_checkpoints.insert_port(port)
        } else {
            self.virtio_pci_common_checkpoints = Some(VirtioPciCommonCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_virtio_pci_notify_checkpoint_port(
        &mut self,
        port: VirtioPciNotifyCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(virtio_pci_notify_checkpoints) = &mut self.virtio_pci_notify_checkpoints {
            virtio_pci_notify_checkpoints.insert_port(port)
        } else {
            self.virtio_pci_notify_checkpoints = Some(VirtioPciNotifyCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_virtio_pci_isr_checkpoint_port(
        &mut self,
        port: VirtioPciIsrCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(virtio_pci_isr_checkpoints) = &mut self.virtio_pci_isr_checkpoints {
            virtio_pci_isr_checkpoints.insert_port(port)
        } else {
            self.virtio_pci_isr_checkpoints = Some(VirtioPciIsrCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub fn attach_virtio_pci_device_config_checkpoint_port(
        &mut self,
        port: VirtioPciDeviceConfigCheckpointPort,
    ) -> Result<(), CheckpointError> {
        port.register(&mut self.checkpoints)?;
        if let Some(virtio_pci_device_config_checkpoints) =
            &mut self.virtio_pci_device_config_checkpoints
        {
            virtio_pci_device_config_checkpoints.insert_port(port)
        } else {
            self.virtio_pci_device_config_checkpoints =
                Some(VirtioPciDeviceConfigCheckpointBank::new([port])?);
            Ok(())
        }
    }

    pub const fn riscv_checkpoint_bank(&self) -> Option<&RiscvCoreCheckpointBank> {
        self.riscv_checkpoints.as_ref()
    }

    pub const fn scheduler_checkpoint_bank(&self) -> Option<&SchedulerCheckpointBank> {
        self.scheduler_checkpoints.as_ref()
    }

    pub const fn accelerator_checkpoint_bank(&self) -> Option<&AcceleratorCheckpointBank> {
        self.accelerator_checkpoints.as_ref()
    }

    pub const fn msi_bank_checkpoint_bank(&self) -> Option<&MsiBankCheckpointBank> {
        self.msi_bank_checkpoints.as_ref()
    }

    pub const fn fabric_checkpoint_bank(&self) -> Option<&FabricCheckpointBank> {
        self.fabric_checkpoints.as_ref()
    }

    pub const fn gpu_checkpoint_bank(&self) -> Option<&GpuCheckpointBank> {
        self.gpu_checkpoints.as_ref()
    }

    pub const fn memory_checkpoint_bank(&self) -> Option<&MemoryStoreCheckpointBank> {
        self.memory_checkpoints.as_ref()
    }

    pub const fn storage_image_checkpoint_bank(&self) -> Option<&StorageImageCheckpointBank> {
        self.storage_image_checkpoints.as_ref()
    }

    pub const fn guest_fd_checkpoint_bank(&self) -> Option<&GuestFdCheckpointBank> {
        self.guest_fd_checkpoints.as_ref()
    }

    pub const fn guest_futex_checkpoint_bank(&self) -> Option<&GuestFutexCheckpointBank> {
        self.guest_futex_checkpoints.as_ref()
    }

    pub const fn guest_wait_checkpoint_bank(&self) -> Option<&GuestWaitCheckpointBank> {
        self.guest_wait_checkpoints.as_ref()
    }

    pub const fn ide_controller_checkpoint_bank(&self) -> Option<&IdeControllerCheckpointBank> {
        self.ide_controller_checkpoints.as_ref()
    }

    pub const fn sinic_register_checkpoint_bank(&self) -> Option<&SinicRegisterCheckpointBank> {
        self.sinic_register_checkpoints.as_ref()
    }

    pub const fn sinic_fifo_checkpoint_bank(&self) -> Option<&SinicFifoCheckpointBank> {
        self.sinic_fifo_checkpoints.as_ref()
    }

    pub const fn dram_memory_checkpoint_bank(&self) -> Option<&DramMemoryCheckpointBank> {
        self.dram_memory_checkpoints.as_ref()
    }

    pub const fn interrupt_controller_checkpoint_bank(
        &self,
    ) -> Option<&InterruptControllerCheckpointBank> {
        self.interrupt_controller_checkpoints.as_ref()
    }

    pub const fn clint_checkpoint_bank(&self) -> Option<&ClintCheckpointBank> {
        self.clint_checkpoints.as_ref()
    }

    pub const fn timer_checkpoint_bank(&self) -> Option<&TimerCheckpointBank> {
        self.timer_checkpoints.as_ref()
    }

    pub const fn uart_checkpoint_bank(&self) -> Option<&UartCheckpointBank> {
        self.uart_checkpoints.as_ref()
    }

    pub const fn pl011_uart_checkpoint_bank(&self) -> Option<&Pl011UartCheckpointBank> {
        self.pl011_uart_checkpoints.as_ref()
    }

    pub const fn plic_checkpoint_bank(&self) -> Option<&PlicCheckpointBank> {
        self.plic_checkpoints.as_ref()
    }

    pub const fn pl031_checkpoint_bank(&self) -> Option<&Pl031CheckpointBank> {
        self.pl031_checkpoints.as_ref()
    }

    pub const fn sp804_checkpoint_bank(&self) -> Option<&Sp804CheckpointBank> {
        self.sp804_checkpoints.as_ref()
    }

    pub const fn sp805_checkpoint_bank(&self) -> Option<&Sp805CheckpointBank> {
        self.sp805_checkpoints.as_ref()
    }

    pub const fn cpu_local_timer_checkpoint_bank(&self) -> Option<&CpuLocalTimerCheckpointBank> {
        self.cpu_local_timer_checkpoints.as_ref()
    }

    pub const fn rtc_checkpoint_bank(&self) -> Option<&RtcCheckpointBank> {
        self.rtc_checkpoints.as_ref()
    }

    pub const fn pci_host_checkpoint_bank(&self) -> Option<&PciHostCheckpointBank> {
        self.pci_host_checkpoints.as_ref()
    }

    pub const fn pci_legacy_interrupt_router_checkpoint_bank(
        &self,
    ) -> Option<&PciLegacyInterruptRouterCheckpointBank> {
        self.pci_legacy_interrupt_router_checkpoints.as_ref()
    }

    pub fn has_checkpoint_component(&self, component: &CheckpointComponentId) -> bool {
        self.checkpoints.contains_component(component)
    }

    pub const fn virtio_split_queue_checkpoint_bank(
        &self,
    ) -> Option<&VirtioSplitQueueCheckpointBank> {
        self.virtio_split_queue_checkpoints.as_ref()
    }

    pub const fn virtio_pci_isr_checkpoint_bank(&self) -> Option<&VirtioPciIsrCheckpointBank> {
        self.virtio_pci_isr_checkpoints.as_ref()
    }

    pub const fn virtio_pci_common_checkpoint_bank(
        &self,
    ) -> Option<&VirtioPciCommonCheckpointBank> {
        self.virtio_pci_common_checkpoints.as_ref()
    }

    pub const fn virtio_pci_notify_checkpoint_bank(
        &self,
    ) -> Option<&VirtioPciNotifyCheckpointBank> {
        self.virtio_pci_notify_checkpoints.as_ref()
    }

    pub const fn virtio_pci_device_config_checkpoint_bank(
        &self,
    ) -> Option<&VirtioPciDeviceConfigCheckpointBank> {
        self.virtio_pci_device_config_checkpoints.as_ref()
    }

    fn restore_checkpoint_manifest(
        &mut self,
        manifest: &CheckpointManifest,
    ) -> Result<(), SystemError> {
        let mut staged_checkpoints = self.checkpoints.clone();
        if manifest_has_execution_mode_checkpoint(manifest) {
            let component = execution_mode_checkpoint_component();
            match staged_checkpoints.register(component) {
                Ok(()) | Err(CheckpointError::DuplicateComponent { .. }) => {}
                Err(error) => return Err(SystemError::Checkpoint(error)),
            }
        }
        staged_checkpoints
            .restore(manifest)
            .map_err(SystemError::Checkpoint)?;
        let (staged_execution_modes, staged_execution_mode_checkpoint_registered) =
            self.stage_execution_mode_checkpoint_restore(&mut staged_checkpoints, manifest)?;
        self.validate_checkpoint_banks(&staged_checkpoints)?;

        self.checkpoints = staged_checkpoints;
        self.execution_modes = staged_execution_modes;
        self.execution_mode_checkpoint_registered = staged_execution_mode_checkpoint_registered;
        self.restore_checkpoint_banks()
    }

    fn validate_checkpoint_banks(
        &self,
        checkpoints: &CheckpointRegistry,
    ) -> Result<(), SystemError> {
        if let Some(accelerator_checkpoints) = &self.accelerator_checkpoints {
            accelerator_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::AcceleratorCheckpoint)?;
        }
        if let Some(msi_bank_checkpoints) = &self.msi_bank_checkpoints {
            msi_bank_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::MsiBankCheckpoint)?;
        }
        if let Some(fabric_checkpoints) = &self.fabric_checkpoints {
            fabric_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::FabricCheckpoint)?;
        }
        if let Some(gpu_checkpoints) = &self.gpu_checkpoints {
            gpu_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::GpuCheckpoint)?;
        }
        if let Some(riscv_checkpoints) = &self.riscv_checkpoints {
            riscv_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::RiscvCheckpoint)?;
        }
        if let Some(scheduler_checkpoints) = &self.scheduler_checkpoints {
            scheduler_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::SchedulerCheckpoint)?;
        }
        if let Some(memory_checkpoints) = &self.memory_checkpoints {
            memory_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::MemoryCheckpoint)?;
        }
        if let Some(storage_image_checkpoints) = &self.storage_image_checkpoints {
            storage_image_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::StorageCheckpoint)?;
        }
        if let Some(guest_fd_checkpoints) = &self.guest_fd_checkpoints {
            guest_fd_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::GuestFdCheckpoint)?;
        }
        if let Some(guest_futex_checkpoints) = &self.guest_futex_checkpoints {
            guest_futex_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::GuestFutexCheckpoint)?;
        }
        if let Some(guest_wait_checkpoints) = &self.guest_wait_checkpoints {
            guest_wait_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::GuestWaitCheckpoint)?;
        }
        if let Some(ide_controller_checkpoints) = &self.ide_controller_checkpoints {
            ide_controller_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::StorageCheckpoint)?;
        }
        if let Some(sinic_register_checkpoints) = &self.sinic_register_checkpoints {
            sinic_register_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::SinicRegisterCheckpoint)?;
        }
        if let Some(sinic_fifo_checkpoints) = &self.sinic_fifo_checkpoints {
            sinic_fifo_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::SinicFifoCheckpoint)?;
        }
        if let Some(dram_memory_checkpoints) = &self.dram_memory_checkpoints {
            dram_memory_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::DramMemoryCheckpoint)?;
        }
        if let Some(interrupt_controller_checkpoints) = &self.interrupt_controller_checkpoints {
            interrupt_controller_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::InterruptControllerCheckpoint)?;
        }
        if let Some(clint_checkpoints) = &self.clint_checkpoints {
            clint_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::ClintCheckpoint)?;
        }
        if let Some(timer_checkpoints) = &self.timer_checkpoints {
            timer_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::TimerCheckpoint)?;
        }
        if let Some(uart_checkpoints) = &self.uart_checkpoints {
            uart_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::UartCheckpoint)?;
        }
        if let Some(pl011_uart_checkpoints) = &self.pl011_uart_checkpoints {
            pl011_uart_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::Pl011UartCheckpoint)?;
        }
        if let Some(plic_checkpoints) = &self.plic_checkpoints {
            plic_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::PlicCheckpoint)?;
        }
        if let Some(pl031_checkpoints) = &self.pl031_checkpoints {
            pl031_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::Pl031Checkpoint)?;
        }
        if let Some(sp804_checkpoints) = &self.sp804_checkpoints {
            sp804_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::Sp804Checkpoint)?;
        }
        if let Some(sp805_checkpoints) = &self.sp805_checkpoints {
            sp805_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::Sp805Checkpoint)?;
        }
        if let Some(cpu_local_timer_checkpoints) = &self.cpu_local_timer_checkpoints {
            cpu_local_timer_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::CpuLocalTimerCheckpoint)?;
        }
        if let Some(rtc_checkpoints) = &self.rtc_checkpoints {
            rtc_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::RtcCheckpoint)?;
        }
        if let Some(pci_host_checkpoints) = &self.pci_host_checkpoints {
            pci_host_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::PciHostCheckpoint)?;
        }
        if let Some(pci_legacy_interrupt_router_checkpoints) =
            &self.pci_legacy_interrupt_router_checkpoints
        {
            pci_legacy_interrupt_router_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::PciLegacyInterruptRouterCheckpoint)?;
        }
        if let Some(virtio_split_queue_checkpoints) = &self.virtio_split_queue_checkpoints {
            virtio_split_queue_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::VirtioCheckpoint)?;
        }
        if let Some(virtio_pci_isr_checkpoints) = &self.virtio_pci_isr_checkpoints {
            virtio_pci_isr_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::VirtioPciIsrCheckpoint)?;
        }
        if let Some(virtio_pci_common_checkpoints) = &self.virtio_pci_common_checkpoints {
            virtio_pci_common_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::VirtioPciCommonCheckpoint)?;
        }
        if let Some(virtio_pci_notify_checkpoints) = &self.virtio_pci_notify_checkpoints {
            virtio_pci_notify_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::VirtioPciNotifyCheckpoint)?;
        }
        if let Some(virtio_pci_device_config_checkpoints) =
            &self.virtio_pci_device_config_checkpoints
        {
            virtio_pci_device_config_checkpoints
                .validate_restore_from(checkpoints)
                .map_err(SystemError::VirtioPciDeviceConfigCheckpoint)?;
        }
        Ok(())
    }

    fn restore_checkpoint_banks(&self) -> Result<(), SystemError> {
        if let Some(accelerator_checkpoints) = &self.accelerator_checkpoints {
            accelerator_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::AcceleratorCheckpoint)?;
        }
        if let Some(msi_bank_checkpoints) = &self.msi_bank_checkpoints {
            msi_bank_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::MsiBankCheckpoint)?;
        }
        if let Some(fabric_checkpoints) = &self.fabric_checkpoints {
            fabric_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::FabricCheckpoint)?;
        }
        if let Some(gpu_checkpoints) = &self.gpu_checkpoints {
            gpu_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::GpuCheckpoint)?;
        }
        if let Some(riscv_checkpoints) = &self.riscv_checkpoints {
            riscv_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::RiscvCheckpoint)?;
        }
        if let Some(scheduler_checkpoints) = &self.scheduler_checkpoints {
            scheduler_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::SchedulerCheckpoint)?;
        }
        if let Some(memory_checkpoints) = &self.memory_checkpoints {
            memory_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::MemoryCheckpoint)?;
        }
        if let Some(storage_image_checkpoints) = &self.storage_image_checkpoints {
            storage_image_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::StorageCheckpoint)?;
        }
        if let Some(guest_fd_checkpoints) = &self.guest_fd_checkpoints {
            guest_fd_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::GuestFdCheckpoint)?;
        }
        if let Some(guest_futex_checkpoints) = &self.guest_futex_checkpoints {
            guest_futex_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::GuestFutexCheckpoint)?;
        }
        if let Some(guest_wait_checkpoints) = &self.guest_wait_checkpoints {
            guest_wait_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::GuestWaitCheckpoint)?;
        }
        if let Some(ide_controller_checkpoints) = &self.ide_controller_checkpoints {
            ide_controller_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::StorageCheckpoint)?;
        }
        if let Some(sinic_register_checkpoints) = &self.sinic_register_checkpoints {
            sinic_register_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::SinicRegisterCheckpoint)?;
        }
        if let Some(sinic_fifo_checkpoints) = &self.sinic_fifo_checkpoints {
            sinic_fifo_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::SinicFifoCheckpoint)?;
        }
        if let Some(dram_memory_checkpoints) = &self.dram_memory_checkpoints {
            dram_memory_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::DramMemoryCheckpoint)?;
        }
        if let Some(interrupt_controller_checkpoints) = &self.interrupt_controller_checkpoints {
            interrupt_controller_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::InterruptControllerCheckpoint)?;
        }
        if let Some(clint_checkpoints) = &self.clint_checkpoints {
            clint_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::ClintCheckpoint)?;
        }
        if let Some(timer_checkpoints) = &self.timer_checkpoints {
            timer_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::TimerCheckpoint)?;
        }
        if let Some(uart_checkpoints) = &self.uart_checkpoints {
            uart_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::UartCheckpoint)?;
        }
        if let Some(pl011_uart_checkpoints) = &self.pl011_uart_checkpoints {
            pl011_uart_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::Pl011UartCheckpoint)?;
        }
        if let Some(plic_checkpoints) = &self.plic_checkpoints {
            plic_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::PlicCheckpoint)?;
        }
        if let Some(pl031_checkpoints) = &self.pl031_checkpoints {
            pl031_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::Pl031Checkpoint)?;
        }
        if let Some(sp804_checkpoints) = &self.sp804_checkpoints {
            sp804_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::Sp804Checkpoint)?;
        }
        if let Some(sp805_checkpoints) = &self.sp805_checkpoints {
            sp805_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::Sp805Checkpoint)?;
        }
        if let Some(cpu_local_timer_checkpoints) = &self.cpu_local_timer_checkpoints {
            cpu_local_timer_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::CpuLocalTimerCheckpoint)?;
        }
        if let Some(rtc_checkpoints) = &self.rtc_checkpoints {
            rtc_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::RtcCheckpoint)?;
        }
        if let Some(pci_host_checkpoints) = &self.pci_host_checkpoints {
            pci_host_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::PciHostCheckpoint)?;
        }
        if let Some(pci_legacy_interrupt_router_checkpoints) =
            &self.pci_legacy_interrupt_router_checkpoints
        {
            pci_legacy_interrupt_router_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::PciLegacyInterruptRouterCheckpoint)?;
        }
        if let Some(virtio_split_queue_checkpoints) = &self.virtio_split_queue_checkpoints {
            virtio_split_queue_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::VirtioCheckpoint)?;
        }
        if let Some(virtio_pci_isr_checkpoints) = &self.virtio_pci_isr_checkpoints {
            virtio_pci_isr_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::VirtioPciIsrCheckpoint)?;
        }
        if let Some(virtio_pci_common_checkpoints) = &self.virtio_pci_common_checkpoints {
            virtio_pci_common_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::VirtioPciCommonCheckpoint)?;
        }
        if let Some(virtio_pci_notify_checkpoints) = &self.virtio_pci_notify_checkpoints {
            virtio_pci_notify_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::VirtioPciNotifyCheckpoint)?;
        }
        if let Some(virtio_pci_device_config_checkpoints) =
            &self.virtio_pci_device_config_checkpoints
        {
            virtio_pci_device_config_checkpoints
                .restore_all_from(&self.checkpoints)
                .map_err(SystemError::VirtioPciDeviceConfigCheckpoint)?;
        }
        Ok(())
    }

    pub fn apply(&mut self, record: &HostActionRecord) -> Result<SystemActionOutcome, SystemError> {
        match record.action() {
            HostAction::InjectCommand { command } => Ok(SystemActionOutcome::InjectedCommand {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                command: command.clone(),
            }),
            HostAction::RecordGuestHostCall {
                selector,
                arguments,
                payload,
            } => Ok(SystemActionOutcome::GuestHostCall {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                selector: *selector,
                arguments: arguments.clone(),
                payload: payload.clone(),
                response: self.resolve_guest_host_call_response(*selector),
            }),
            HostAction::ResetStats => self
                .stats
                .try_reset(record.tick())
                .map(SystemActionOutcome::StatsReset)
                .map_err(SystemError::Stats),
            HostAction::DumpStats => self
                .stats
                .try_dump(record.tick())
                .map(SystemActionOutcome::StatsDump)
                .map_err(SystemError::Stats),
            HostAction::SwitchExecutionMode { target, mode } => {
                let previous_mode = self.execution_modes.insert(target.clone(), *mode);
                Ok(SystemActionOutcome::ExecutionModeSwitched {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    target: target.clone(),
                    previous_mode,
                    mode: *mode,
                    stats_epoch: self.stats.epoch(),
                    stats_reset_tick: self.stats.reset_tick(),
                })
            }
            HostAction::Checkpoint { label } => {
                if let Some(scheduler_checkpoints) = &self.scheduler_checkpoints {
                    scheduler_checkpoints
                        .validate_quiescent_capture()
                        .map_err(SystemError::SchedulerCheckpoint)?;
                }
                let mut staged_checkpoints = self.checkpoints.clone();
                if let Some(accelerator_checkpoints) = &self.accelerator_checkpoints {
                    accelerator_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(msi_bank_checkpoints) = &self.msi_bank_checkpoints {
                    msi_bank_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::MsiBankCheckpoint)?;
                }
                if let Some(fabric_checkpoints) = &self.fabric_checkpoints {
                    fabric_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::FabricCheckpoint)?;
                }
                if let Some(gpu_checkpoints) = &self.gpu_checkpoints {
                    gpu_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(riscv_checkpoints) = &self.riscv_checkpoints {
                    riscv_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(scheduler_checkpoints) = &self.scheduler_checkpoints {
                    scheduler_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::SchedulerCheckpoint)?;
                }
                if let Some(memory_checkpoints) = &self.memory_checkpoints {
                    memory_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(storage_image_checkpoints) = &self.storage_image_checkpoints {
                    storage_image_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::StorageCheckpoint)?;
                }
                if let Some(guest_fd_checkpoints) = &self.guest_fd_checkpoints {
                    guest_fd_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(guest_futex_checkpoints) = &self.guest_futex_checkpoints {
                    guest_futex_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(guest_wait_checkpoints) = &self.guest_wait_checkpoints {
                    guest_wait_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(ide_controller_checkpoints) = &self.ide_controller_checkpoints {
                    ide_controller_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::StorageCheckpoint)?;
                }
                if let Some(sinic_register_checkpoints) = &self.sinic_register_checkpoints {
                    sinic_register_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(sinic_fifo_checkpoints) = &self.sinic_fifo_checkpoints {
                    sinic_fifo_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(dram_memory_checkpoints) = &self.dram_memory_checkpoints {
                    dram_memory_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(interrupt_controller_checkpoints) =
                    &self.interrupt_controller_checkpoints
                {
                    interrupt_controller_checkpoints
                        .capture_all_into(&mut staged_checkpoints, record.tick())
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(clint_checkpoints) = &self.clint_checkpoints {
                    clint_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(timer_checkpoints) = &self.timer_checkpoints {
                    timer_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(uart_checkpoints) = &self.uart_checkpoints {
                    uart_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(pl011_uart_checkpoints) = &self.pl011_uart_checkpoints {
                    pl011_uart_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(plic_checkpoints) = &self.plic_checkpoints {
                    plic_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(pl031_checkpoints) = &self.pl031_checkpoints {
                    pl031_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(sp804_checkpoints) = &self.sp804_checkpoints {
                    sp804_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(sp805_checkpoints) = &self.sp805_checkpoints {
                    sp805_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(cpu_local_timer_checkpoints) = &self.cpu_local_timer_checkpoints {
                    cpu_local_timer_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(rtc_checkpoints) = &self.rtc_checkpoints {
                    rtc_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(pci_host_checkpoints) = &self.pci_host_checkpoints {
                    pci_host_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::PciHostCheckpoint)?;
                }
                if let Some(pci_legacy_interrupt_router_checkpoints) =
                    &self.pci_legacy_interrupt_router_checkpoints
                {
                    pci_legacy_interrupt_router_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(virtio_split_queue_checkpoints) = &self.virtio_split_queue_checkpoints {
                    virtio_split_queue_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::VirtioCheckpoint)?;
                }
                if let Some(virtio_pci_isr_checkpoints) = &self.virtio_pci_isr_checkpoints {
                    virtio_pci_isr_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::VirtioPciIsrCheckpoint)?;
                }
                if let Some(virtio_pci_common_checkpoints) = &self.virtio_pci_common_checkpoints {
                    virtio_pci_common_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::VirtioPciCommonCheckpoint)?;
                }
                if let Some(virtio_pci_notify_checkpoints) = &self.virtio_pci_notify_checkpoints {
                    virtio_pci_notify_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::VirtioPciNotifyCheckpoint)?;
                }
                if let Some(virtio_pci_device_config_checkpoints) =
                    &self.virtio_pci_device_config_checkpoints
                {
                    virtio_pci_device_config_checkpoints
                        .capture_all_into(&mut staged_checkpoints)
                        .map_err(SystemError::VirtioPciDeviceConfigCheckpoint)?;
                }
                let execution_mode_registered =
                    self.capture_execution_modes_into(&mut staged_checkpoints)?;
                let manifest = staged_checkpoints
                    .capture(label.clone(), record.tick())
                    .map_err(SystemError::Checkpoint)?;
                self.checkpoints = staged_checkpoints;
                if execution_mode_registered {
                    self.execution_mode_checkpoint_registered = true;
                }
                self.captured_manifests
                    .insert(manifest.label().to_string(), manifest.clone());
                Ok(SystemActionOutcome::Checkpoint {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest,
                })
            }
            HostAction::RestoreCheckpointByLabel { label } => {
                let manifest = self.captured_manifests.get(label).cloned().ok_or_else(|| {
                    SystemError::MissingCheckpointManifest {
                        label: label.clone(),
                    }
                })?;
                self.restore_checkpoint_manifest(&manifest)?;
                Ok(SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest,
                })
            }
            HostAction::RestoreCheckpoint { manifest } => {
                self.restore_checkpoint_manifest(manifest)?;
                Ok(SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest: manifest.clone(),
                })
            }
            HostAction::Stop { code } => Ok(SystemActionOutcome::Stop(StopRequest::new(
                record.tick(),
                record.event(),
                record.source(),
                *code,
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRunController {
    policy: HostEventPolicy,
    deliveries: Vec<GuestEventDelivery>,
    actions: Vec<HostActionRecord>,
    outcomes: Vec<SystemActionOutcome>,
    stop_request: Option<StopRequest>,
}

impl SystemRunController {
    pub const fn new(policy: HostEventPolicy) -> Self {
        Self {
            policy,
            deliveries: Vec::new(),
            actions: Vec::new(),
            outcomes: Vec::new(),
            stop_request: None,
        }
    }

    pub fn handle_delivery(&mut self, delivery: GuestEventDelivery) -> Vec<HostActionRecord> {
        let produced: Vec<_> = self
            .policy
            .actions_for(delivery.event())
            .into_iter()
            .map(|action| {
                HostActionRecord::new(
                    delivery.tick(),
                    delivery.source_partition(),
                    delivery.host_partition(),
                    delivery.event().id(),
                    delivery.event().source(),
                    action,
                )
            })
            .collect();

        for record in &produced {
            self.record_stop_request(record);
        }

        self.deliveries.push(delivery);
        self.actions.extend(produced.iter().cloned());
        produced
    }

    pub fn execute_record(
        &mut self,
        record: HostActionRecord,
        executor: &mut SystemActionExecutor,
    ) -> Result<SystemActionOutcome, SystemError> {
        self.record_stop_request(&record);
        self.actions.push(record.clone());
        let outcome = executor.apply(&record)?;
        self.outcomes.push(outcome.clone());
        Ok(outcome)
    }

    fn record_stop_request(&mut self, record: &HostActionRecord) {
        if self.stop_request.is_none() && matches!(record.action(), HostAction::Stop { .. }) {
            let HostAction::Stop { code } = record.action() else {
                unreachable!("stop record was matched above");
            };
            self.stop_request = Some(StopRequest::new(
                record.tick(),
                record.event(),
                record.source(),
                *code,
            ));
        }
    }

    pub fn execute_delivery(
        &mut self,
        delivery: GuestEventDelivery,
        executor: &mut SystemActionExecutor,
    ) -> Result<Vec<SystemActionOutcome>, SystemError> {
        let records = self.handle_delivery(delivery);
        let outcomes = records
            .iter()
            .map(|record| executor.apply(record))
            .collect::<Result<Vec<_>, _>>()?;
        self.outcomes.extend(outcomes.iter().cloned());
        Ok(outcomes)
    }

    pub fn deliveries(&self) -> &[GuestEventDelivery] {
        &self.deliveries
    }

    pub fn action_records(&self) -> &[HostActionRecord] {
        &self.actions
    }

    pub fn action_outcomes(&self) -> &[SystemActionOutcome] {
        &self.outcomes
    }

    pub const fn stop_request(&self) -> Option<&StopRequest> {
        self.stop_request.as_ref()
    }

    pub const fn is_stopped(&self) -> bool {
        self.stop_request.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct SystemHostController {
    run: SystemRunController,
    executor: SystemActionExecutor,
    action_errors: Vec<SystemError>,
}

impl SystemHostController {
    pub fn new(policy: HostEventPolicy, stats: StatsRegistry) -> Self {
        Self {
            run: SystemRunController::new(policy),
            executor: SystemActionExecutor::new(stats),
            action_errors: Vec::new(),
        }
    }

    pub const fn run(&self) -> &SystemRunController {
        &self.run
    }

    pub const fn run_mut(&mut self) -> &mut SystemRunController {
        &mut self.run
    }

    pub const fn executor(&self) -> &SystemActionExecutor {
        &self.executor
    }

    pub const fn executor_mut(&mut self) -> &mut SystemActionExecutor {
        &mut self.executor
    }

    pub fn handle_delivery(&mut self, delivery: GuestEventDelivery) -> Vec<SystemActionOutcome> {
        match self.run.execute_delivery(delivery, &mut self.executor) {
            Ok(outcomes) => outcomes,
            Err(error) => {
                self.action_errors.push(error);
                Vec::new()
            }
        }
    }

    pub fn action_errors(&self) -> &[SystemError] {
        &self.action_errors
    }
}
