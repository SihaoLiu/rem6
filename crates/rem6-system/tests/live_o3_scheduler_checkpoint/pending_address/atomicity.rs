use super::*;

#[test]
fn pending_address_restore_requires_full_scheduler_snapshot() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Serial,
    );
    let (mut executor, _, scheduler_component, memory, target) =
        attached_executor_with_memory(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&checkpoint_record("pending-scheduler"))
            .unwrap(),
    );
    let states = manifest
        .states()
        .iter()
        .filter(|state| state.component() != &scheduler_component)
        .cloned()
        .collect();
    let without_scheduler = CheckpointManifest::new(manifest.label(), manifest.tick(), states);
    seeded.core.write_register(reg(7), 0xfeed);
    let scheduler_before = scheduler.lock().unwrap().snapshot();
    let wakes_before = seeded.core.owned_o3_writeback_wakes();
    let fetches_before = seeded.core.inner().fetch_events();
    memory
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x9000), vec![0xaa; 16])
        .unwrap();
    let memory_before = memory.lock().unwrap().snapshot();
    let registry_before = executor.checkpoints().clone();
    let mode_target = ExecutionModeTarget::new("cpu0");
    executor.set_execution_mode(mode_target.clone(), ExecutionMode::Timing);

    assert!(executor.apply(&restore_record(without_scheduler)).is_err());

    assert_eq!(seeded.core.read_register(reg(7)), 0xfeed);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    assert_eq!(seeded.core.inner().fetch_events(), fetches_before);
    assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
    assert_eq!(memory.lock().unwrap().snapshot(), memory_before);
    assert_eq!(executor.checkpoints(), &registry_before);
    assert_eq!(
        executor.execution_mode(&mode_target),
        Some(ExecutionMode::Timing)
    );
}

#[test]
fn pending_address_corrupt_live_chunk_is_full_executor_atomic() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Serial,
    );
    let (mut executor, cpus, _, memory, target) =
        attached_executor_with_memory(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&checkpoint_record("pending-corrupt-executor"))
            .unwrap(),
    );
    let corrupt = rewrite_o3lc(&manifest, &cpus[0], |live| {
        let sequence = pending_address_support::STORE_SEQUENCE + 1;
        live.pending_addresses[0].sequence = sequence;
        live.issue_rows[0].sequence = sequence;
        live.resident_sequences[0] = sequence;
    });
    seeded.core.write_register(reg(7), 0xfeed);
    memory
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x9000), vec![0xbb; 16])
        .unwrap();
    let core_before = seeded.core.checkpoint_hart_state();
    let wakes_before = seeded.core.owned_o3_writeback_wakes();
    let fetches_before = seeded.core.inner().fetch_events();
    let scheduler_before = scheduler.lock().unwrap().snapshot();
    let memory_before = memory.lock().unwrap().snapshot();
    let registry_before = executor.checkpoints().clone();
    let mode_target = ExecutionModeTarget::new("cpu0");
    executor.set_execution_mode(mode_target.clone(), ExecutionMode::Timing);

    assert!(executor.apply(&restore_record(corrupt)).is_err());

    assert_eq!(seeded.core.checkpoint_hart_state(), core_before);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    assert_eq!(seeded.core.inner().fetch_events(), fetches_before);
    assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
    assert_eq!(memory.lock().unwrap().snapshot(), memory_before);
    assert_eq!(executor.checkpoints(), &registry_before);
    assert_eq!(
        executor.execution_mode(&mode_target),
        Some(ExecutionMode::Timing)
    );
}

#[test]
fn pending_address_live_restore_rejects_low_level_port_and_bank_bypasses() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Serial,
    );
    let (mut executor, cpus, _) = attached_executor(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&checkpoint_record("pending-low-level"))
            .unwrap(),
    );
    let mut registry = CheckpointRegistry::new();
    for state in manifest.states() {
        registry.register(state.component().clone()).unwrap();
    }
    registry.restore(&manifest).unwrap();
    seeded.core.write_register(reg(7), 0xfeed);
    let core_before = seeded.core.checkpoint_hart_state();
    let wakes_before = seeded.core.owned_o3_writeback_wakes();
    let port = RiscvCoreCheckpointPort::new(cpus[0].clone(), seeded.core.clone());

    assert!(port.restore_from(&registry).is_err());
    assert_eq!(seeded.core.checkpoint_hart_state(), core_before);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);

    let bank = RiscvCoreCheckpointBank::new([port]).unwrap();
    assert!(bank.restore_all_from(&registry).is_err());
    assert_eq!(seeded.core.checkpoint_hart_state(), core_before);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
}
