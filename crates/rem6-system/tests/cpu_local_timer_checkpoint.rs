use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_system::{
    CpuLocalTimerCheckpointBank, CpuLocalTimerCheckpointError, CpuLocalTimerCheckpointPort,
};
use rem6_timer::{
    CpuLocalTimerBank, CpuLocalTimerBankSnapshot, CpuLocalTimerControl,
    CpuLocalTimerCounterSnapshot, CpuLocalTimerCounterSnapshotFields, CpuLocalTimerCpuSnapshot,
    CpuLocalTimerError, CpuLocalTimerMmioDevice, CpuLocalWatchdogControl, CpuLocalWatchdogSnapshot,
    CpuLocalWatchdogSnapshotFields, CPU_LOCAL_TIMER_CONTROL_OFFSET, CPU_LOCAL_TIMER_LOAD_OFFSET,
    CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, CPU_LOCAL_WATCHDOG_LOAD_OFFSET,
    CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET,
};

fn component(name: &str) -> CheckpointComponentId {
    CheckpointComponentId::new(name).unwrap()
}

fn cpu_local_timer(base: u64, cpu_count: usize) -> CpuLocalTimerMmioDevice {
    CpuLocalTimerMmioDevice::new(
        Address::new(base),
        CpuLocalTimerBank::new(cpu_count, 2).unwrap(),
        (0..cpu_count)
            .map(|index| PartitionId::new(index as u32))
            .collect(),
    )
    .unwrap()
}

fn configured_cpu_local_timer(base: u64) -> CpuLocalTimerMmioDevice {
    let mut bank = CpuLocalTimerBank::new(2, 2).unwrap();
    let timer_control = CpuLocalTimerControl::new(0)
        .with_interrupt_enabled(true)
        .with_auto_reload(true)
        .with_enabled(true);
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_LOAD_OFFSET, 3, 10)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_CONTROL_OFFSET, timer_control.bits(), 10)
        .unwrap();
    let timer_generation = bank.cpu(0).unwrap().snapshot().timer().generation();
    bank.cpu_mut(0)
        .unwrap()
        .record_timer_zero(16, timer_generation)
        .unwrap()
        .unwrap();

    let watchdog_control = CpuLocalWatchdogControl::new(0)
        .with_watchdog_mode(true)
        .with_enabled(true);
    bank.cpu_mut(1)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_LOAD_OFFSET, 2, 20)
        .unwrap();
    bank.cpu_mut(1)
        .unwrap()
        .write_register(
            CPU_LOCAL_WATCHDOG_CONTROL_OFFSET,
            watchdog_control.bits(),
            20,
        )
        .unwrap();
    let watchdog_generation = bank.cpu(1).unwrap().snapshot().watchdog().generation();
    bank.cpu_mut(1)
        .unwrap()
        .record_watchdog_zero(24, watchdog_generation)
        .unwrap()
        .unwrap();

    CpuLocalTimerMmioDevice::new(
        Address::new(base),
        bank,
        [PartitionId::new(0), PartitionId::new(1)].to_vec(),
    )
    .unwrap()
}

#[test]
fn cpu_local_timer_checkpoint_bank_round_trips_snapshot() {
    let checkpoint_component = component("cpu_local_timer0");
    let device = configured_cpu_local_timer(0x2c00);
    let expected = device.snapshot();
    let target = cpu_local_timer(0x2c00, 2);
    let mut registry = CheckpointRegistry::new();
    let bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component.clone(),
        device,
    )])
    .unwrap();

    bank.register_all(&mut registry).unwrap();
    let captured = bank.capture_all_into(&mut registry).unwrap();
    assert_eq!(captured[0].snapshot(), &expected);

    let restore_bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component,
        target.clone(),
    )])
    .unwrap();
    let restored = restore_bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored[0].snapshot(), &expected);
    assert_eq!(target.snapshot(), expected);
}

#[test]
fn cpu_local_timer_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = component("cpu_local_timer_valid");
    let invalid_component = component("cpu_local_timer_invalid");
    let source = configured_cpu_local_timer(0x2c00);
    let target_valid = cpu_local_timer(0x2c00, 2);
    let target_invalid = cpu_local_timer(0x2d00, 2);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    CpuLocalTimerCheckpointPort::new(valid_component, source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "cpu-local-timer", vec![0xee])
        .unwrap();

    let bank = CpuLocalTimerCheckpointBank::new([
        CpuLocalTimerCheckpointPort::new(component("cpu_local_timer_valid"), target_valid.clone()),
        CpuLocalTimerCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        CpuLocalTimerCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);
}

#[test]
fn cpu_local_timer_checkpoint_bank_rejects_cpu_count_mismatch() {
    let checkpoint_component = component("cpu_local_timer_count");
    let source = configured_cpu_local_timer(0x2c00);
    let target = cpu_local_timer(0x2c00, 1);
    let original = target.snapshot();
    let mut registry = CheckpointRegistry::new();
    CpuLocalTimerCheckpointPort::new(checkpoint_component.clone(), source)
        .register(&mut registry)
        .unwrap();
    CpuLocalTimerCheckpointPort::new(
        checkpoint_component.clone(),
        configured_cpu_local_timer(0x2c00),
    )
    .capture_into(&mut registry)
    .unwrap();
    let bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component.clone(),
        target.clone(),
    )])
    .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        CpuLocalTimerCheckpointError::CpuLocalTimer { component, .. } if component == checkpoint_component
    ));
    assert_eq!(target.snapshot(), original);
}

#[test]
fn cpu_local_timer_checkpoint_bank_restores_masked_pending_interrupts() {
    let checkpoint_component = component("cpu_local_timer_masked_pending");
    let mut bank = CpuLocalTimerBank::new(1, 1).unwrap();
    let timer_enabled = CpuLocalTimerControl::new(0)
        .with_interrupt_enabled(true)
        .with_enabled(true);
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_LOAD_OFFSET, 1, 1)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_CONTROL_OFFSET, timer_enabled.bits(), 1)
        .unwrap();
    let timer_generation = bank.cpu(0).unwrap().snapshot().timer().generation();
    bank.cpu_mut(0)
        .unwrap()
        .record_timer_zero(2, timer_generation)
        .unwrap()
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(
            CPU_LOCAL_TIMER_CONTROL_OFFSET,
            CpuLocalTimerControl::new(0).with_enabled(true).bits(),
            2,
        )
        .unwrap();

    let watchdog_enabled = CpuLocalWatchdogControl::new(0)
        .with_interrupt_enabled(true)
        .with_enabled(true);
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_LOAD_OFFSET, 1, 3)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(
            CPU_LOCAL_WATCHDOG_CONTROL_OFFSET,
            watchdog_enabled.bits(),
            3,
        )
        .unwrap();
    let watchdog_generation = bank.cpu(0).unwrap().snapshot().watchdog().generation();
    bank.cpu_mut(0)
        .unwrap()
        .record_watchdog_zero(4, watchdog_generation)
        .unwrap()
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(
            CPU_LOCAL_WATCHDOG_CONTROL_OFFSET,
            CpuLocalWatchdogControl::new(0).with_enabled(true).bits(),
            4,
        )
        .unwrap();
    let source =
        CpuLocalTimerMmioDevice::new(Address::new(0x2c00), bank, [PartitionId::new(0)].to_vec())
            .unwrap();
    let expected = source.snapshot();
    assert!(expected.cpu(0).unwrap().timer().pending_interrupt());
    assert!(!expected
        .cpu(0)
        .unwrap()
        .timer()
        .control()
        .interrupt_enabled());
    assert!(expected.cpu(0).unwrap().watchdog().pending_interrupt());
    assert!(!expected
        .cpu(0)
        .unwrap()
        .watchdog()
        .control()
        .interrupt_enabled());

    let target = cpu_local_timer(0x2c00, 1);
    let mut registry = CheckpointRegistry::new();
    let capture_bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component.clone(),
        source,
    )])
    .unwrap();
    capture_bank.register_all(&mut registry).unwrap();
    capture_bank.capture_all_into(&mut registry).unwrap();
    let restore_bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component,
        target.clone(),
    )])
    .unwrap();

    restore_bank.restore_all_from(&registry).unwrap();

    assert_eq!(target.snapshot(), expected);
}

#[test]
fn cpu_local_timer_checkpoint_bank_restores_cleared_watchdog_reset_history() {
    let checkpoint_component = component("cpu_local_timer_cleared_reset");
    let mut bank = CpuLocalTimerBank::new(1, 1).unwrap();
    let watchdog_control = CpuLocalWatchdogControl::new(0)
        .with_watchdog_mode(true)
        .with_enabled(true);
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_LOAD_OFFSET, 1, 1)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(
            CPU_LOCAL_WATCHDOG_CONTROL_OFFSET,
            watchdog_control.bits(),
            1,
        )
        .unwrap();
    let watchdog_generation = bank.cpu(0).unwrap().snapshot().watchdog().generation();
    bank.cpu_mut(0)
        .unwrap()
        .record_watchdog_zero(2, watchdog_generation)
        .unwrap()
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET, 1, 2)
        .unwrap();
    let source =
        CpuLocalTimerMmioDevice::new(Address::new(0x2c00), bank, [PartitionId::new(0)].to_vec())
            .unwrap();
    let expected = source.snapshot();
    assert!(!expected.cpu(0).unwrap().watchdog().raw_reset());
    assert_eq!(expected.cpu(0).unwrap().watchdog().reset_assertions(), &[2]);

    let target = cpu_local_timer(0x2c00, 1);
    let mut registry = CheckpointRegistry::new();
    let capture_bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component.clone(),
        source,
    )])
    .unwrap();
    capture_bank.register_all(&mut registry).unwrap();
    capture_bank.capture_all_into(&mut registry).unwrap();
    let restore_bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component,
        target.clone(),
    )])
    .unwrap();

    restore_bank.restore_all_from(&registry).unwrap();

    assert_eq!(target.snapshot(), expected);
}

#[test]
fn cpu_local_timer_checkpoint_bank_rejects_invalid_prescalar_snapshot() {
    let checkpoint_component = component("cpu_local_timer_invalid_prescalar");
    let target = cpu_local_timer(0x2c00, 1);
    let original = target.snapshot();
    let invalid_timer =
        CpuLocalTimerCounterSnapshot::from_fields(CpuLocalTimerCounterSnapshotFields {
            load_value: 1,
            base_value: 1,
            last_updated_tick: 0,
            control: CpuLocalTimerControl::new(16 << 8),
            raw_interrupt: false,
            pending_interrupt: false,
            clock_tick: 1,
            generation: 0,
        });
    let watchdog = CpuLocalWatchdogSnapshot::from_fields(CpuLocalWatchdogSnapshotFields {
        load_value: 0,
        base_value: 0,
        last_updated_tick: 0,
        control: CpuLocalWatchdogControl::new(0),
        raw_interrupt: false,
        pending_interrupt: false,
        raw_reset: false,
        disable_register: 0,
        clock_tick: 1,
        generation: 0,
        reset_assertions: Vec::new(),
    });
    let snapshot = CpuLocalTimerBankSnapshot::new(vec![CpuLocalTimerCpuSnapshot::new(
        invalid_timer,
        watchdog,
    )]);
    let mut registry = CheckpointRegistry::new();
    let source =
        CpuLocalTimerCheckpointPort::new(checkpoint_component.clone(), cpu_local_timer(0x2c00, 1));
    source.register(&mut registry).unwrap();
    source.capture_into(&mut registry).unwrap();
    registry
        .write_chunk(
            &checkpoint_component,
            "cpu-local-timer",
            encode_cpu_local_timer_test_snapshot(&snapshot),
        )
        .unwrap();
    let bank = CpuLocalTimerCheckpointBank::new([CpuLocalTimerCheckpointPort::new(
        checkpoint_component.clone(),
        target.clone(),
    )])
    .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        CpuLocalTimerCheckpointError::CpuLocalTimer {
            component,
            error: CpuLocalTimerError::InvalidPrescalar { prescalar }
        } if component == checkpoint_component && prescalar == 16
    ));
    assert_eq!(target.snapshot(), original);
}

fn encode_cpu_local_timer_test_snapshot(snapshot: &CpuLocalTimerBankSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_test_u64(&mut payload, snapshot.cpus().len() as u64);
    for cpu in snapshot.cpus() {
        let timer = cpu.timer();
        write_test_u32(&mut payload, timer.load_value());
        write_test_u32(&mut payload, timer.base_value());
        write_test_u64(&mut payload, timer.last_updated_tick());
        write_test_u32(&mut payload, timer.control().bits());
        write_test_bool(&mut payload, timer.raw_interrupt());
        write_test_bool(&mut payload, timer.pending_interrupt());
        write_test_u64(&mut payload, timer.clock_tick());
        write_test_u64(&mut payload, timer.generation());

        let watchdog = cpu.watchdog();
        write_test_u32(&mut payload, watchdog.load_value());
        write_test_u32(&mut payload, watchdog.base_value());
        write_test_u64(&mut payload, watchdog.last_updated_tick());
        write_test_u32(&mut payload, watchdog.control().bits());
        write_test_bool(&mut payload, watchdog.raw_interrupt());
        write_test_bool(&mut payload, watchdog.pending_interrupt());
        write_test_bool(&mut payload, watchdog.raw_reset());
        write_test_u32(&mut payload, watchdog.disable_register());
        write_test_u64(&mut payload, watchdog.clock_tick());
        write_test_u64(&mut payload, watchdog.generation());
        write_test_u64(&mut payload, watchdog.reset_assertions().len() as u64);
        for tick in watchdog.reset_assertions() {
            write_test_u64(&mut payload, *tick);
        }
    }
    payload
}

fn write_test_bool(payload: &mut Vec<u8>, value: bool) {
    payload.push(u8::from(value));
}

fn write_test_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_test_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}
