use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_interrupt::{
    InterruptController, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptPriority, InterruptRoute, InterruptSourceId, InterruptTargetId, PendingInterrupt,
};
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_system::{
    ClintCheckpointBank, ClintCheckpointError, ClintCheckpointPort,
    InterruptControllerCheckpointBank, InterruptControllerCheckpointError,
    InterruptControllerCheckpointPort, TimerCheckpointBank, TimerCheckpointError,
    TimerCheckpointPort, UartCheckpointBank, UartCheckpointError, UartCheckpointPort,
};
use rem6_timer::{
    ClintHartConfig, ClintHartSnapshot, ClintMmioDevice, ClintSnapshot, ProgrammableTimer,
    TimerArm, TimerId, TimerSnapshot,
};
use rem6_uart::{UartId, UartMmioDevice, UartSnapshot, UartTxByte};

fn checkpoint_component(name: &str) -> CheckpointComponentId {
    CheckpointComponentId::new(name).unwrap()
}

fn interrupt_port(line: u64, target: u32, target_partition: u32) -> InterruptLinePort {
    let route = InterruptRoute::new(
        InterruptLineId::new(line),
        InterruptTargetId::new(target),
        PartitionId::new(target_partition),
    );
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    InterruptLinePort::new(InterruptLineChannel::new(route, 1).unwrap(), controller)
}

fn clint_device(base: u64, hart: u32) -> ClintMmioDevice {
    ClintMmioDevice::new(
        Address::new(base),
        [ClintHartConfig::new(
            hart,
            interrupt_port(10 + u64::from(hart), 0, 0),
            InterruptSourceId::new(20 + hart),
            interrupt_port(30 + u64::from(hart), 0, 0),
            InterruptSourceId::new(40 + hart),
        )],
    )
    .unwrap()
}

fn timer(id: u64, partition: u32, source: u32) -> ProgrammableTimer {
    ProgrammableTimer::new(
        TimerId::new(id),
        PartitionId::new(partition),
        InterruptSourceId::new(source),
        interrupt_port(100 + id, 0, 0),
    )
}

#[test]
fn clint_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("clint_bank_a");
    let invalid_component = checkpoint_component("clint_bank_b");
    let expected = ClintSnapshot::with_mtime(
        Address::new(0x200_0000),
        41,
        vec![ClintHartSnapshot::new(0, 1, 64, 3, true)],
    );
    let source = clint_device(0x200_0000, 0);
    source.restore(&expected).unwrap();
    let target_valid = clint_device(0x200_0000, 0);
    let target_invalid = clint_device(0x200_1000, 1);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    ClintCheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "clint", vec![0xcc])
        .unwrap();

    let bank = ClintCheckpointBank::new([
        ClintCheckpointPort::new(valid_component, target_valid.clone()),
        ClintCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        ClintCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);
}

#[test]
fn uart_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("uart_bank_a");
    let invalid_component = checkpoint_component("uart_bank_b");
    let expected = UartSnapshot::new(
        vec![UartTxByte::new(9, b'O')],
        Vec::new(),
        b"AB".to_vec(),
        Vec::new(),
        Vec::new(),
    );
    let source = UartMmioDevice::new(UartId::new(0), Address::new(0x1000));
    source.restore(&expected);
    let target_valid = UartMmioDevice::new(UartId::new(0), Address::new(0x1000));
    let target_invalid = UartMmioDevice::new(UartId::new(1), Address::new(0x2000));
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    UartCheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "uart", vec![0xdd])
        .unwrap();

    let bank = UartCheckpointBank::new([
        UartCheckpointPort::new(valid_component, target_valid.clone()),
        UartCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        UartCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);
}

#[test]
fn timer_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("timer_bank_a");
    let invalid_component = checkpoint_component("timer_bank_b");
    let expected = TimerSnapshot::new(
        TimerId::new(0),
        PartitionId::new(2),
        InterruptSourceId::new(50),
        Some(64),
        vec![TimerArm::new(1, 12, 64)],
        Vec::new(),
        Vec::new(),
    );
    let source = timer(0, 2, 50);
    source.restore(&expected).unwrap();
    let target_valid = timer(0, 2, 50);
    let target_invalid = timer(1, 3, 51);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    TimerCheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "timer", vec![0xee])
        .unwrap();

    let bank = TimerCheckpointBank::new([
        TimerCheckpointPort::new(valid_component, target_valid.clone()),
        TimerCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        TimerCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);
}

#[test]
fn interrupt_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("interrupt_bank_a");
    let invalid_component = checkpoint_component("interrupt_bank_b");
    let target = InterruptTargetId::new(0);
    let partition = PartitionId::new(0);
    let line = InterruptLineId::new(7);
    let source = InterruptSourceId::new(70);
    let route = InterruptRoute::new(line, target, partition);
    let expected = rem6_interrupt::InterruptSnapshot::new(
        19,
        vec![route],
        vec![(line, InterruptPriority::new(6))],
        vec![PendingInterrupt::routed(
            line, target, partition, source, 12,
        )],
        Vec::new(),
        Vec::new(),
    );
    let source_controller = Arc::new(Mutex::new(InterruptController::new()));
    source_controller.lock().unwrap().restore(&expected);
    let target_valid = Arc::new(Mutex::new(InterruptController::new()));
    let target_invalid = Arc::new(Mutex::new(InterruptController::new()));
    let original_valid = target_valid.lock().unwrap().snapshot(0);
    let original_invalid = target_invalid.lock().unwrap().snapshot(0);

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    InterruptControllerCheckpointPort::new(valid_component.clone(), source_controller)
        .capture_into(&mut registry, 19)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "interrupt", vec![0xff])
        .unwrap();

    let bank = InterruptControllerCheckpointBank::new([
        InterruptControllerCheckpointPort::new(valid_component, Arc::clone(&target_valid)),
        InterruptControllerCheckpointPort::new(
            invalid_component.clone(),
            Arc::clone(&target_invalid),
        ),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        InterruptControllerCheckpointError::InvalidChunk { component, .. }
            if component == invalid_component
    ));
    assert_eq!(target_valid.lock().unwrap().snapshot(0), original_valid);
    assert_eq!(target_invalid.lock().unwrap().snapshot(0), original_invalid);
}
