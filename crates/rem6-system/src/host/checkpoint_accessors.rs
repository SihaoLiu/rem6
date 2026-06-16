use rem6_checkpoint::CheckpointComponentId;

use crate::{
    AcceleratorCheckpointBank, ClintCheckpointBank, CpuLocalTimerCheckpointBank,
    DramMemoryCheckpointBank, FabricCheckpointBank, GpuCheckpointBank, GuestFdCheckpointBank,
    GuestFutexCheckpointBank, GuestWaitCheckpointBank, IdeControllerCheckpointBank,
    InterruptControllerCheckpointBank, MemoryStoreCheckpointBank, MsiBankCheckpointBank,
    PciHostCheckpointBank, PciLegacyInterruptRouterCheckpointBank, Pl011UartCheckpointBank,
    Pl031CheckpointBank, PlicCheckpointBank, ReadfileCheckpointBank, RiscvCoreCheckpointBank,
    RtcCheckpointBank, SchedulerCheckpointBank, SinicFifoCheckpointBank,
    SinicRegisterCheckpointBank, Sp804CheckpointBank, Sp805CheckpointBank,
    StorageImageCheckpointBank, TimerCheckpointBank, UartCheckpointBank,
    VirtioPciCommonCheckpointBank, VirtioPciDeviceConfigCheckpointBank, VirtioPciIsrCheckpointBank,
    VirtioPciNotifyCheckpointBank, VirtioSplitQueueCheckpointBank,
};

use super::SystemActionExecutor;

impl SystemActionExecutor {
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

    pub const fn readfile_checkpoint_bank(&self) -> Option<&ReadfileCheckpointBank> {
        self.readfile_checkpoints.as_ref()
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
}
