use super::*;

#[test]
fn pending_load_graph_restore_requires_full_scheduler_snapshot() {
    let source = seed_pending_load_graph_core();
    let (mut executor, cpus, scheduler_component, memory, target) =
        attached_executor_with_memory(&[&source.core], &source.scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&graph_checkpoint_record(
                "pending-load-graph-scheduler",
                source.capture_tick,
            ))
            .unwrap(),
    );
    let states = manifest
        .states()
        .iter()
        .filter(|state| state.component() != &scheduler_component)
        .cloned()
        .collect();
    let without_scheduler = CheckpointManifest::new(manifest.label(), manifest.tick(), states);
    memory
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x9000), vec![0xaa; 16])
        .unwrap();
    let mode_target = ExecutionModeTarget::new("cpu0");
    executor.set_execution_mode(mode_target.clone(), ExecutionMode::Timing);
    let before = AtomicSnapshot::capture(
        &source.core,
        &source.scheduler,
        &memory,
        &executor,
        &cpus[0],
        source.capture_tick,
    );

    assert!(executor
        .apply(&graph_restore_record(without_scheduler))
        .is_err());

    before.assert_unchanged(&source.core, &source.scheduler, &memory, &executor);
    assert_eq!(
        executor.execution_mode(&mode_target),
        Some(ExecutionMode::Timing)
    );
}

#[test]
fn pending_load_graph_corrupt_second_row_is_full_executor_atomic() {
    let source = seed_pending_load_graph_core();
    let (mut executor, cpus, _, memory, target) =
        attached_executor_with_memory(&[&source.core], &source.scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&graph_checkpoint_record(
                "pending-load-graph-corrupt",
                source.capture_tick,
            ))
            .unwrap(),
    );
    let corrupt = rewrite_o3lc(&manifest, &cpus[0], corrupt_second_row_destination);
    memory
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x9000), vec![0xbb; 16])
        .unwrap();
    let mode_target = ExecutionModeTarget::new("cpu0");
    executor.set_execution_mode(mode_target.clone(), ExecutionMode::Timing);
    let before = AtomicSnapshot::capture(
        &source.core,
        &source.scheduler,
        &memory,
        &executor,
        &cpus[0],
        source.capture_tick,
    );

    let error = executor.apply(&graph_restore_record(corrupt)).unwrap_err();
    assert!(
        matches!(
            error,
            rem6_system::SystemError::RiscvCheckpoint(
                rem6_system::RiscvCoreCheckpointError::InvalidPreparedRestore { .. }
            )
        ),
        "unexpected restore error: {error:?}"
    );

    before.assert_unchanged(&source.core, &source.scheduler, &memory, &executor);
    assert_eq!(
        executor.execution_mode(&mode_target),
        Some(ExecutionMode::Timing)
    );
}

#[test]
fn pending_load_graph_low_level_port_and_bank_restore_remain_rejected() {
    let source = seed_pending_load_graph_core();
    let (mut executor, cpus, _) = attached_executor(&[&source.core], &source.scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&graph_checkpoint_record(
                "pending-load-graph-low-level",
                source.capture_tick,
            ))
            .unwrap(),
    );
    let mut registry = CheckpointRegistry::new();
    for state in manifest.states() {
        registry.register(state.component().clone()).unwrap();
    }
    registry.restore(&manifest).unwrap();
    let before = CoreSnapshot::capture(&source.core, &cpus[0], source.capture_tick);
    let port = RiscvCoreCheckpointPort::new(cpus[0].clone(), source.core.clone());
    let expected =
        rem6_system::RiscvCoreCheckpointError::PendingDataAddressRestoreRequiresSchedulerAuthority {
            component: cpus[0].clone(),
        };

    assert_eq!(port.restore_from(&registry).unwrap_err(), expected);
    before.assert_unchanged(&source.core);

    let bank = RiscvCoreCheckpointBank::new([port]).unwrap();
    assert_eq!(bank.restore_all_from(&registry).unwrap_err(), expected);
    before.assert_unchanged(&source.core);
}

struct AtomicSnapshot {
    core: CoreSnapshot,
    scheduler: rem6_kernel::SchedulerSnapshot,
    memory: rem6_memory::PartitionedMemorySnapshot,
    registry: CheckpointRegistry,
}

impl AtomicSnapshot {
    fn capture(
        core: &RiscvCore,
        scheduler: &Arc<Mutex<PartitionedScheduler>>,
        memory: &Arc<Mutex<PartitionedMemoryStore>>,
        executor: &SystemActionExecutor,
        component: &CheckpointComponentId,
        checkpoint_tick: u64,
    ) -> Self {
        Self {
            core: CoreSnapshot::capture(core, component, checkpoint_tick),
            scheduler: scheduler.lock().unwrap().snapshot(),
            memory: memory.lock().unwrap().snapshot(),
            registry: executor.checkpoints().clone(),
        }
    }

    fn assert_unchanged(
        &self,
        core: &RiscvCore,
        scheduler: &Arc<Mutex<PartitionedScheduler>>,
        memory: &Arc<Mutex<PartitionedMemoryStore>>,
        executor: &SystemActionExecutor,
    ) {
        self.core.assert_unchanged(core);
        assert_eq!(scheduler.lock().unwrap().snapshot(), self.scheduler);
        assert_eq!(memory.lock().unwrap().snapshot(), self.memory);
        assert_eq!(executor.checkpoints(), &self.registry);
    }
}

struct CoreSnapshot {
    component: CheckpointComponentId,
    checkpoint_tick: u64,
    checkpoint: rem6_system::RiscvCoreCheckpointRecord,
    raw_hart: rem6_isa_riscv::RiscvHartState,
    raw_o3: rem6_cpu::O3RuntimeSnapshot,
    execution_events: Vec<rem6_cpu::RiscvCpuExecutionEvent>,
    data_access_events: Vec<rem6_cpu::RiscvDataAccessEvent>,
    live_issue: rem6_cpu::O3LiveIssueTelemetry,
    live_issue_trace: Vec<rem6_cpu::O3LiveIssueTraceRecord>,
    o3_trace: Vec<rem6_cpu::O3RuntimeTraceRecord>,
    wakes: Vec<(rem6_kernel::SchedulerInstanceId, PendingEventSnapshot)>,
    cpu_pc: Address,
    cpu_next_sequence: u64,
    fetches: Vec<rem6_cpu::CpuFetchEvent>,
    fetch_history: Vec<rem6_cpu::CpuFetchEvent>,
    pending_live_retirements: usize,
}

impl CoreSnapshot {
    fn capture(core: &RiscvCore, component: &CheckpointComponentId, checkpoint_tick: u64) -> Self {
        let raw_hart = core.checkpoint_hart_state();
        let raw_o3 = core.o3_runtime_snapshot();
        let execution_events = core.execution_events();
        let data_access_events = core.data_access_events();
        let live_issue = core.o3_runtime_live_issue_telemetry();
        let live_issue_trace = core.o3_runtime_live_issue_trace_records();
        let o3_trace = core.o3_runtime_trace_records();
        let wakes = core.owned_o3_writeback_wakes();
        let cpu = core.inner();
        let cpu_pc = cpu.pc();
        let cpu_next_sequence = cpu.next_sequence();
        let fetches = cpu.fetch_events();
        let fetch_history = cpu.fetch_history();
        let pending_live_retirements = core.pending_o3_live_data_access_retirement_count();
        let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
        let mut registry = CheckpointRegistry::new();
        port.register(&mut registry).unwrap();
        let checkpoint = port
            .capture_into_at(&mut registry, checkpoint_tick)
            .unwrap();
        Self {
            component: component.clone(),
            checkpoint_tick,
            checkpoint,
            raw_hart,
            raw_o3,
            execution_events,
            data_access_events,
            live_issue,
            live_issue_trace,
            o3_trace,
            wakes,
            cpu_pc,
            cpu_next_sequence,
            fetches,
            fetch_history,
            pending_live_retirements,
        }
    }

    fn assert_unchanged(&self, core: &RiscvCore) {
        let after = Self::capture(core, &self.component, self.checkpoint_tick);
        assert_eq!(after.checkpoint, self.checkpoint);
        assert_eq!(after.raw_hart, self.raw_hart);
        assert_eq!(after.raw_o3, self.raw_o3);
        assert_eq!(after.execution_events, self.execution_events);
        assert_eq!(after.data_access_events, self.data_access_events);
        assert_eq!(after.live_issue, self.live_issue);
        assert_eq!(after.live_issue_trace, self.live_issue_trace);
        assert_eq!(after.o3_trace, self.o3_trace);
        assert_eq!(after.wakes, self.wakes);
        assert_eq!(after.cpu_pc, self.cpu_pc);
        assert_eq!(after.cpu_next_sequence, self.cpu_next_sequence);
        assert_eq!(after.fetches, self.fetches);
        assert_eq!(after.fetch_history, self.fetch_history);
        assert_eq!(
            after.pending_live_retirements,
            self.pending_live_retirements
        );
    }
}
