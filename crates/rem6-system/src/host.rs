use rem6_checkpoint::{CheckpointError, CheckpointManifest, CheckpointRegistry};
use rem6_kernel::Tick;
use rem6_stats::{StatSnapshot, StatsRegistry, StatsResetRecord};

use crate::{
    DramMemoryCheckpointBank, GuestEventDelivery, GuestEventId, GuestSourceId, HostAction,
    HostActionRecord, HostEventPolicy, InterruptControllerCheckpointBank,
    MemoryStoreCheckpointBank, RiscvCoreCheckpointBank, StopRequest, SystemError,
    TimerCheckpointBank, UartCheckpointBank,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemActionOutcome {
    InjectedCommand {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        command: String,
    },
    StatsReset(StatsResetRecord),
    StatsSnapshot(StatSnapshot),
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
    Stop(StopRequest),
}

#[derive(Clone, Debug)]
pub struct SystemActionExecutor {
    stats: StatsRegistry,
    checkpoints: CheckpointRegistry,
    riscv_checkpoints: Option<RiscvCoreCheckpointBank>,
    memory_checkpoints: Option<MemoryStoreCheckpointBank>,
    dram_memory_checkpoints: Option<DramMemoryCheckpointBank>,
    interrupt_controller_checkpoints: Option<InterruptControllerCheckpointBank>,
    timer_checkpoints: Option<TimerCheckpointBank>,
    uart_checkpoints: Option<UartCheckpointBank>,
}

impl SystemActionExecutor {
    pub fn new(stats: StatsRegistry) -> Self {
        Self::with_checkpoint(stats, CheckpointRegistry::new())
    }

    pub fn with_checkpoint(stats: StatsRegistry, checkpoints: CheckpointRegistry) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: None,
            memory_checkpoints: None,
            dram_memory_checkpoints: None,
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
        }
    }

    pub fn with_riscv_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        riscv_checkpoints: RiscvCoreCheckpointBank,
    ) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: Some(riscv_checkpoints),
            memory_checkpoints: None,
            dram_memory_checkpoints: None,
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
        }
    }

    pub fn with_memory_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        memory_checkpoints: MemoryStoreCheckpointBank,
    ) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: None,
            memory_checkpoints: Some(memory_checkpoints),
            dram_memory_checkpoints: None,
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
        }
    }

    pub fn with_dram_memory_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        dram_memory_checkpoints: DramMemoryCheckpointBank,
    ) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: None,
            memory_checkpoints: None,
            dram_memory_checkpoints: Some(dram_memory_checkpoints),
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
        }
    }

    pub fn with_uart_checkpoint_bank(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        uart_checkpoints: UartCheckpointBank,
    ) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: None,
            memory_checkpoints: None,
            dram_memory_checkpoints: None,
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: Some(uart_checkpoints),
        }
    }

    pub fn with_checkpoint_banks(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        riscv_checkpoints: RiscvCoreCheckpointBank,
        memory_checkpoints: MemoryStoreCheckpointBank,
    ) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: Some(riscv_checkpoints),
            memory_checkpoints: Some(memory_checkpoints),
            dram_memory_checkpoints: None,
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
        }
    }

    pub fn with_riscv_and_dram_checkpoint_banks(
        stats: StatsRegistry,
        checkpoints: CheckpointRegistry,
        riscv_checkpoints: RiscvCoreCheckpointBank,
        dram_memory_checkpoints: DramMemoryCheckpointBank,
    ) -> Self {
        Self {
            stats,
            checkpoints,
            riscv_checkpoints: Some(riscv_checkpoints),
            memory_checkpoints: None,
            dram_memory_checkpoints: Some(dram_memory_checkpoints),
            interrupt_controller_checkpoints: None,
            timer_checkpoints: None,
            uart_checkpoints: None,
        }
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

    pub fn attach_memory_checkpoint_bank(
        &mut self,
        memory_checkpoints: MemoryStoreCheckpointBank,
    ) -> Result<(), CheckpointError> {
        memory_checkpoints.register_all(&mut self.checkpoints)?;
        self.memory_checkpoints = Some(memory_checkpoints);
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

    pub fn attach_uart_checkpoint_bank(
        &mut self,
        uart_checkpoints: UartCheckpointBank,
    ) -> Result<(), CheckpointError> {
        uart_checkpoints.register_all(&mut self.checkpoints)?;
        self.uart_checkpoints = Some(uart_checkpoints);
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

    pub const fn riscv_checkpoint_bank(&self) -> Option<&RiscvCoreCheckpointBank> {
        self.riscv_checkpoints.as_ref()
    }

    pub const fn memory_checkpoint_bank(&self) -> Option<&MemoryStoreCheckpointBank> {
        self.memory_checkpoints.as_ref()
    }

    pub const fn dram_memory_checkpoint_bank(&self) -> Option<&DramMemoryCheckpointBank> {
        self.dram_memory_checkpoints.as_ref()
    }

    pub const fn interrupt_controller_checkpoint_bank(
        &self,
    ) -> Option<&InterruptControllerCheckpointBank> {
        self.interrupt_controller_checkpoints.as_ref()
    }

    pub const fn timer_checkpoint_bank(&self) -> Option<&TimerCheckpointBank> {
        self.timer_checkpoints.as_ref()
    }

    pub const fn uart_checkpoint_bank(&self) -> Option<&UartCheckpointBank> {
        self.uart_checkpoints.as_ref()
    }

    pub fn apply(&mut self, record: &HostActionRecord) -> Result<SystemActionOutcome, SystemError> {
        match record.action() {
            HostAction::InjectCommand { command } => Ok(SystemActionOutcome::InjectedCommand {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                command: command.clone(),
            }),
            HostAction::ResetStats => Ok(SystemActionOutcome::StatsReset(
                self.stats.reset(record.tick()),
            )),
            HostAction::DumpStats => self
                .stats
                .try_snapshot(record.tick())
                .map(SystemActionOutcome::StatsSnapshot)
                .map_err(SystemError::Stats),
            HostAction::Checkpoint { label } => {
                if let Some(riscv_checkpoints) = &self.riscv_checkpoints {
                    riscv_checkpoints
                        .capture_all_into(&mut self.checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(memory_checkpoints) = &self.memory_checkpoints {
                    memory_checkpoints
                        .capture_all_into(&mut self.checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(dram_memory_checkpoints) = &self.dram_memory_checkpoints {
                    dram_memory_checkpoints
                        .capture_all_into(&mut self.checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(interrupt_controller_checkpoints) =
                    &self.interrupt_controller_checkpoints
                {
                    interrupt_controller_checkpoints
                        .capture_all_into(&mut self.checkpoints, record.tick())
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(timer_checkpoints) = &self.timer_checkpoints {
                    timer_checkpoints
                        .capture_all_into(&mut self.checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                if let Some(uart_checkpoints) = &self.uart_checkpoints {
                    uart_checkpoints
                        .capture_all_into(&mut self.checkpoints)
                        .map_err(SystemError::Checkpoint)?;
                }
                self.checkpoints
                    .capture(label.clone(), record.tick())
                    .map(|manifest| SystemActionOutcome::Checkpoint {
                        tick: record.tick(),
                        event: record.event(),
                        source: record.source(),
                        manifest,
                    })
                    .map_err(SystemError::Checkpoint)
            }
            HostAction::RestoreCheckpoint { manifest } => {
                self.checkpoints
                    .restore(manifest)
                    .map_err(SystemError::Checkpoint)?;
                if let Some(riscv_checkpoints) = &self.riscv_checkpoints {
                    riscv_checkpoints
                        .restore_all_from(&self.checkpoints)
                        .map_err(SystemError::RiscvCheckpoint)?;
                }
                if let Some(memory_checkpoints) = &self.memory_checkpoints {
                    memory_checkpoints
                        .restore_all_from(&self.checkpoints)
                        .map_err(SystemError::MemoryCheckpoint)?;
                }
                if let Some(dram_memory_checkpoints) = &self.dram_memory_checkpoints {
                    dram_memory_checkpoints
                        .restore_all_from(&self.checkpoints)
                        .map_err(SystemError::DramMemoryCheckpoint)?;
                }
                if let Some(interrupt_controller_checkpoints) =
                    &self.interrupt_controller_checkpoints
                {
                    interrupt_controller_checkpoints
                        .restore_all_from(&self.checkpoints)
                        .map_err(SystemError::InterruptControllerCheckpoint)?;
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
