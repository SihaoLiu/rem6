use std::sync::Arc;

use crate::{
    AcceleratorCheckpointBank, AcceleratorCheckpointPort, ClintCheckpointBank, ClintCheckpointPort,
    DramMemoryCheckpointBank, DramMemoryCheckpointPort, FabricCheckpointBank, FabricCheckpointPort,
    GpuCheckpointBank, GpuCheckpointPort, InterruptControllerCheckpointBank,
    InterruptControllerCheckpointPort, MemoryStoreCheckpointBank, MemoryStoreCheckpointPort,
    PlicCheckpointBank, PlicCheckpointPort, RiscvCoreCheckpointBank, RiscvCoreCheckpointPort,
    RtcCheckpointBank, RtcCheckpointPort, SchedulerCheckpointBank, SchedulerCheckpointPort,
    SystemError, TimerCheckpointBank, TimerCheckpointPort, UartCheckpointBank, UartCheckpointPort,
};

use super::{
    default_accelerator_checkpoint_component, default_clint_checkpoint_component,
    default_gpu_checkpoint_component, default_interrupt_checkpoint_component,
    default_plic_checkpoint_component, default_riscv_checkpoint_component,
    default_rtc_checkpoint_component, default_timer_checkpoint_component,
    default_uart_checkpoint_component, RiscvTopologyMemoryBackend, RiscvTopologySystem,
    RiscvTopologySystemError,
};

impl RiscvTopologySystem {
    pub(super) fn attach_fabric_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let Some(fabric) = self.transport.fabric() else {
            return Ok(());
        };
        let bank = FabricCheckpointBank::new([FabricCheckpointPort::new(
            host.fabric_checkpoint_component.clone(),
            fabric,
        )])
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_fabric_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }

    pub(super) fn attach_scheduler_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            host.scheduler_checkpoint_component.clone(),
            Arc::clone(&self.scheduler),
        )])
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_scheduler_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }

    pub(super) fn attach_memory_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let Some(memory) = self.memory.as_ref() else {
            return Ok(());
        };
        let mut host = host
            .controller
            .lock()
            .expect("topology host controller lock");
        match memory {
            RiscvTopologyMemoryBackend::Store { component, memory } => {
                let bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                    component.clone(),
                    Arc::clone(memory),
                )])
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
                host.executor_mut()
                    .attach_memory_checkpoint_bank(bank)
                    .map_err(SystemError::Checkpoint)
                    .map_err(RiscvTopologySystemError::System)?;
            }
            RiscvTopologyMemoryBackend::Dram { component, memory } => {
                let bank = DramMemoryCheckpointBank::new([DramMemoryCheckpointPort::new(
                    component.clone(),
                    Arc::clone(memory),
                )])
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
                host.executor_mut()
                    .attach_dram_memory_checkpoint_bank(bank)
                    .map_err(SystemError::Checkpoint)
                    .map_err(RiscvTopologySystemError::System)?;
            }
        }
        Ok(())
    }

    pub(super) fn attach_heterogeneous_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let accelerator_bank =
            AcceleratorCheckpointBank::new(self.accelerators.iter().map(|(engine, device)| {
                AcceleratorCheckpointPort::new(
                    default_accelerator_checkpoint_component(*engine),
                    device.engine().clone(),
                )
            }))
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        if accelerator_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_accelerator_checkpoint_bank(accelerator_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        let gpu_bank = GpuCheckpointBank::new(self.gpus.iter().map(|(device_id, device)| {
            GpuCheckpointPort::new(
                default_gpu_checkpoint_component(*device_id),
                device.gpu().clone(),
            )
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if gpu_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_gpu_checkpoint_bank(gpu_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        Ok(())
    }

    pub(super) fn attach_riscv_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let ports = self.cluster.core_ids().into_iter().map(|cpu| {
            RiscvCoreCheckpointPort::new(
                default_riscv_checkpoint_component(cpu),
                self.cluster
                    .core(cpu)
                    .expect("cluster core ids resolve to cores"),
            )
        });
        let bank = RiscvCoreCheckpointBank::new(ports)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_riscv_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }

    pub(super) fn attach_platform_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let Some(platform) = self.platform.as_ref() else {
            return Ok(());
        };
        let interrupt_bank =
            InterruptControllerCheckpointBank::new([InterruptControllerCheckpointPort::new(
                default_interrupt_checkpoint_component(),
                platform.interrupt_controller(),
            )])
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_interrupt_controller_checkpoint_bank(interrupt_bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;

        let timer_bank = TimerCheckpointBank::new(platform.timers().map(|(timer, device)| {
            TimerCheckpointPort::new(default_timer_checkpoint_component(timer), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if timer_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_timer_checkpoint_bank(timer_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        let clint_bank = ClintCheckpointBank::new(platform.clints().map(|(clint, device)| {
            ClintCheckpointPort::new(default_clint_checkpoint_component(clint), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if clint_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_clint_checkpoint_bank(clint_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        let plic_bank = PlicCheckpointBank::new(platform.plics().map(|(base, device)| {
            PlicCheckpointPort::new(default_plic_checkpoint_component(base), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if plic_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_plic_checkpoint_bank(plic_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        let rtc_bank = RtcCheckpointBank::new(platform.rtcs().map(|(base, device)| {
            RtcCheckpointPort::new(default_rtc_checkpoint_component(base), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if rtc_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_rtc_checkpoint_bank(rtc_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        let bank = UartCheckpointBank::new(platform.uarts().map(|(uart, device)| {
            UartCheckpointPort::new(default_uart_checkpoint_component(uart), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if bank.component_count() == 0 {
            return Ok(());
        }
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_uart_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }
}
