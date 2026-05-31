use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_interrupt::{
    InterruptController, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptPriority, InterruptRoute, InterruptSourceId, InterruptTargetId, PendingInterrupt,
    PlicContextRoute, PlicContextSnapshot, PlicMmioDevice, PlicSnapshot,
};
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_stats::StatsRegistry;
use rem6_system::{
    ClintCheckpointBank, ClintCheckpointError, ClintCheckpointPort, GuestEventId, GuestSourceId,
    HostAction, HostActionRecord, InterruptControllerCheckpointBank,
    InterruptControllerCheckpointError, InterruptControllerCheckpointPort, Pl011UartCheckpointBank,
    Pl011UartCheckpointError, Pl011UartCheckpointPort, Pl031CheckpointBank, Pl031CheckpointError,
    Pl031CheckpointPort, PlicCheckpointBank, PlicCheckpointError, PlicCheckpointPort,
    RtcCheckpointBank, RtcCheckpointError, RtcCheckpointPort, Sp804CheckpointBank,
    Sp804CheckpointError, Sp804CheckpointPort, Sp805CheckpointBank, Sp805CheckpointPort,
    SystemActionExecutor, SystemActionOutcome, TimerCheckpointBank, TimerCheckpointError,
    TimerCheckpointPort, UartCheckpointBank, UartCheckpointError, UartCheckpointPort,
};
use rem6_timer::{
    ClintHartConfig, ClintHartSnapshot, ClintMmioDevice, ClintSnapshot, Mc146818Rtc,
    Mc146818RtcMmioDevice, Mc146818RtcMmioSnapshot, Pl031Rtc, Pl031RtcMmioDevice,
    ProgrammableTimer, RtcDateTime, RtcEncoding, RtcSnapshot, Sp804DualTimer,
    Sp804DualTimerMmioDevice, Sp804TimerControl, Sp805Watchdog, Sp805WatchdogMmioDevice, TimerArm,
    TimerId, TimerSnapshot, PL031_INT_MASK_OFFSET, PL031_LOAD_OFFSET, PL031_MATCH_OFFSET,
    RTC_CMOS_REGISTER_COUNT, RTC_STATUS_C_AF, RTC_STATUS_C_IRQF, RTC_STATUS_C_UF,
    SP804_BGLOAD_OFFSET, SP804_CONTROL_OFFSET, SP804_LOAD_OFFSET, SP805_CONTROL_OFFSET,
    SP805_LOAD_OFFSET,
};
use rem6_uart::{
    Pl011UartMmioDevice, Pl011UartSnapshot, Pl011UartSnapshotFields, UartId, UartMmioDevice,
    UartRxByte, UartSnapshot, UartTxByte,
};

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

fn plic_device(base: u64, contexts: &[PlicContextRoute]) -> PlicMmioDevice {
    PlicMmioDevice::with_contexts(
        Arc::new(Mutex::new(InterruptController::new())),
        Address::new(base),
        contexts.iter().copied(),
    )
}

fn rtc_device(base: u64) -> Mc146818RtcMmioDevice {
    Mc146818RtcMmioDevice::new(
        Address::new(base),
        Mc146818Rtc::new(
            RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap(),
            RtcEncoding::Bcd,
        )
        .unwrap(),
    )
}

fn pl031_device(base: u64, time: u32, ticks_per_second: u64) -> Pl031RtcMmioDevice {
    Pl031RtcMmioDevice::new(
        Address::new(base),
        Pl031Rtc::new(time, ticks_per_second).unwrap(),
    )
}

fn configured_pl031_device(base: u64) -> Pl031RtcMmioDevice {
    let mut rtc = Pl031Rtc::new(10, 5).unwrap();
    rtc.write_register(PL031_LOAD_OFFSET, 40, 15).unwrap();
    rtc.write_register(PL031_MATCH_OFFSET, 45, 15).unwrap();
    rtc.write_register(PL031_INT_MASK_OFFSET, 1, 15).unwrap();
    rtc.record_match(40).unwrap();
    Pl031RtcMmioDevice::new(Address::new(base), rtc)
}

fn sp804_device(base: u64) -> Sp804DualTimerMmioDevice {
    Sp804DualTimerMmioDevice::new(Address::new(base), Sp804DualTimer::new(1, 1).unwrap())
}

fn configured_sp804_device(base: u64) -> Sp804DualTimerMmioDevice {
    let mut timers = Sp804DualTimer::new(2, 4).unwrap();
    let timer0_control = Sp804TimerControl::default()
        .with_interrupt_enabled(true)
        .with_enabled(true)
        .with_one_shot(true);
    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_LOAD_OFFSET, 3, 10)
        .unwrap();
    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_CONTROL_OFFSET, timer0_control.bits(), 10)
        .unwrap();
    timers.timer_mut(0).unwrap().record_zero(16).unwrap();

    let timer1_control = Sp804TimerControl::default()
        .with_interrupt_enabled(true)
        .with_periodic(true)
        .with_enabled(true);
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_LOAD_OFFSET, 5, 20)
        .unwrap();
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_BGLOAD_OFFSET, 2, 20)
        .unwrap();
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_CONTROL_OFFSET, timer1_control.bits(), 20)
        .unwrap();

    Sp804DualTimerMmioDevice::new(Address::new(base), timers)
}

fn sp805_device(base: u64) -> Sp805WatchdogMmioDevice {
    Sp805WatchdogMmioDevice::new(Address::new(base), Sp805Watchdog::new(1).unwrap())
}

fn configured_sp805_device(base: u64) -> Sp805WatchdogMmioDevice {
    let mut watchdog = Sp805Watchdog::new(1).unwrap();
    watchdog.write_register(SP805_LOAD_OFFSET, 3, 10).unwrap();
    watchdog
        .write_register(SP805_CONTROL_OFFSET, 0x3, 10)
        .unwrap();
    let first_generation = watchdog.snapshot().generation();
    watchdog
        .record_timeout(13, first_generation)
        .unwrap()
        .unwrap();
    let second_generation = watchdog.snapshot().generation();
    watchdog
        .record_timeout(16, second_generation)
        .unwrap()
        .unwrap();
    Sp805WatchdogMmioDevice::new(Address::new(base), watchdog)
}

fn rtc_snapshot(
    selected_address: u8,
    cmos_index: usize,
    cmos_value: u8,
) -> Mc146818RtcMmioSnapshot {
    rtc_snapshot_with_status_c(selected_address, cmos_index, cmos_value, 0)
}

fn rtc_snapshot_with_status_c(
    selected_address: u8,
    cmos_index: usize,
    cmos_value: u8,
    status_c: u8,
) -> Mc146818RtcMmioSnapshot {
    let mut cmos = [0; RTC_CMOS_REGISTER_COUNT];
    cmos[cmos_index] = cmos_value;
    Mc146818RtcMmioSnapshot::new(
        selected_address,
        cmos,
        RtcSnapshot::with_status_c(
            [0x03, 0, 0x02, 0, 0x01, 0, 0x06, 0x29, 0x05, 0x26],
            0x26,
            0x42,
            status_c,
        ),
    )
}

fn pl011_snapshot() -> Pl011UartSnapshot {
    Pl011UartSnapshot::from_fields(Pl011UartSnapshotFields {
        tx_bytes: vec![UartTxByte::new(9, b'P')],
        rx_injected: vec![UartRxByte::new(10, b'L')],
        rx_pending: b"11".to_vec(),
        rx_consumed: vec![UartRxByte::new(11, b'Q')],
        interrupt_errors: Vec::new(),
        control: 0x301,
        integer_baud_divisor: 7,
        fractional_baud_divisor: 4,
        line_control: 0x70,
        interrupt_fifo_level: 0x24,
        interrupt_mask: 0x50,
        raw_interrupt: 0x50,
    })
}

#[test]
fn pl031_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("pl031_bank_a");
    let invalid_component = checkpoint_component("pl031_bank_b");
    let source = configured_pl031_device(0x1c17_0000);
    let expected = source.snapshot();
    let target_valid = pl031_device(0x1c17_0000, 0, 1);
    let target_invalid = pl031_device(0x1c18_0000, 0, 1);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    Pl031CheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "pl031", vec![0xbb])
        .unwrap();

    let bank = Pl031CheckpointBank::new([
        Pl031CheckpointPort::new(valid_component.clone(), target_valid.clone()),
        Pl031CheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        Pl031CheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);

    let valid_only_bank = Pl031CheckpointBank::new([Pl031CheckpointPort::new(
        valid_component,
        target_valid.clone(),
    )])
    .unwrap();
    valid_only_bank.restore_all_from(&registry).unwrap();
    assert_eq!(target_valid.snapshot(), expected);
}

#[test]
fn sp804_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("sp804_bank_a");
    let invalid_component = checkpoint_component("sp804_bank_b");
    let source = configured_sp804_device(0x1c11_0000);
    let expected = source.snapshot();
    let target_valid = sp804_device(0x1c11_0000);
    let target_invalid = sp804_device(0x1c12_0000);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    Sp804CheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "sp804", vec![0xbb])
        .unwrap();

    let bank = Sp804CheckpointBank::new([
        Sp804CheckpointPort::new(valid_component.clone(), target_valid.clone()),
        Sp804CheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        Sp804CheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);

    let valid_only_bank = Sp804CheckpointBank::new([Sp804CheckpointPort::new(
        valid_component,
        target_valid.clone(),
    )])
    .unwrap();
    valid_only_bank.restore_all_from(&registry).unwrap();
    assert_eq!(target_valid.snapshot(), expected);
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
fn pl011_uart_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("pl011_bank_a");
    let invalid_component = checkpoint_component("pl011_bank_b");
    let expected = pl011_snapshot();
    let source = Pl011UartMmioDevice::new(UartId::new(0), Address::new(0x1c09_0000));
    source.restore(&expected);
    let target_valid = Pl011UartMmioDevice::new(UartId::new(0), Address::new(0x1c09_0000));
    let target_invalid = Pl011UartMmioDevice::new(UartId::new(1), Address::new(0x1c0a_0000));
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    Pl011UartCheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "pl011", vec![0xee])
        .unwrap();

    let bank = Pl011UartCheckpointBank::new([
        Pl011UartCheckpointPort::new(valid_component.clone(), target_valid.clone()),
        Pl011UartCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        Pl011UartCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);

    let valid_only_bank = Pl011UartCheckpointBank::new([Pl011UartCheckpointPort::new(
        valid_component,
        target_valid.clone(),
    )])
    .unwrap();
    valid_only_bank.restore_all_from(&registry).unwrap();
    assert_eq!(target_valid.snapshot(), expected);
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
fn plic_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("plic_bank_a");
    let invalid_component = checkpoint_component("plic_bank_b");
    let contexts = [
        PlicContextRoute::new(0, InterruptTargetId::new(0), PartitionId::new(0)),
        PlicContextRoute::new(1, InterruptTargetId::new(1), PartitionId::new(1)),
    ];
    let expected = PlicSnapshot::new(
        Address::new(0x0c00_0000),
        vec![
            PlicContextSnapshot::new(
                0,
                InterruptTargetId::new(0),
                PartitionId::new(0),
                vec![InterruptLineId::new(2)],
                InterruptPriority::new(4),
            ),
            PlicContextSnapshot::new(
                1,
                InterruptTargetId::new(1),
                PartitionId::new(1),
                vec![InterruptLineId::new(35)],
                InterruptPriority::new(6),
            ),
        ],
    );
    let source = plic_device(0x0c00_0000, &contexts);
    source.restore(&expected).unwrap();
    let target_valid = plic_device(0x0c00_0000, &contexts);
    let target_invalid = plic_device(0x0c10_0000, &contexts);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    PlicCheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "plic", vec![0xaa])
        .unwrap();

    let bank = PlicCheckpointBank::new([
        PlicCheckpointPort::new(valid_component, target_valid.clone()),
        PlicCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        PlicCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);
}

#[test]
fn rtc_checkpoint_bank_rejects_invalid_bank_without_partial_restore() {
    let valid_component = checkpoint_component("rtc_bank_a");
    let invalid_component = checkpoint_component("rtc_bank_b");
    let expected = rtc_snapshot(0xa0, 0x20, 0x5a);
    let source = rtc_device(0x70);
    source.restore(&expected).unwrap();
    let target_valid = rtc_device(0x70);
    let target_invalid = rtc_device(0x80);
    let original_valid = target_valid.snapshot();
    let original_invalid = target_invalid.snapshot();

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    RtcCheckpointPort::new(valid_component.clone(), source)
        .capture_into(&mut registry)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(&invalid_component, "rtc", vec![0xbb])
        .unwrap();

    let bank = RtcCheckpointBank::new([
        RtcCheckpointPort::new(valid_component, target_valid.clone()),
        RtcCheckpointPort::new(invalid_component.clone(), target_invalid.clone()),
    ])
    .unwrap();
    let error = bank.restore_all_from(&registry).unwrap_err();
    assert!(matches!(
        error,
        RtcCheckpointError::InvalidChunk { component, .. } if component == invalid_component
    ));
    assert_eq!(target_valid.snapshot(), original_valid);
    assert_eq!(target_invalid.snapshot(), original_invalid);
}

#[test]
fn system_action_executor_checkpoints_and_restores_plic_state() {
    let component = checkpoint_component("plic0");
    let contexts = [
        PlicContextRoute::new(0, InterruptTargetId::new(0), PartitionId::new(0)),
        PlicContextRoute::new(1, InterruptTargetId::new(1), PartitionId::new(1)),
    ];
    let expected = PlicSnapshot::new(
        Address::new(0x0c00_0000),
        vec![
            PlicContextSnapshot::new(
                0,
                InterruptTargetId::new(0),
                PartitionId::new(0),
                vec![InterruptLineId::new(2)],
                InterruptPriority::new(4),
            ),
            PlicContextSnapshot::new(
                1,
                InterruptTargetId::new(1),
                PartitionId::new(1),
                vec![InterruptLineId::new(35)],
                InterruptPriority::new(6),
            ),
        ],
    );
    let empty = PlicSnapshot::new(
        Address::new(0x0c00_0000),
        vec![
            PlicContextSnapshot::new(
                0,
                InterruptTargetId::new(0),
                PartitionId::new(0),
                Vec::new(),
                InterruptPriority::ZERO,
            ),
            PlicContextSnapshot::new(
                1,
                InterruptTargetId::new(1),
                PartitionId::new(1),
                Vec::new(),
                InterruptPriority::ZERO,
            ),
        ],
    );
    let live = plic_device(0x0c00_0000, &contexts);
    live.restore(&expected).unwrap();
    let bank = PlicCheckpointBank::new([PlicCheckpointPort::new(component.clone(), live.clone())])
        .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_plic_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        18,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(1),
        GuestSourceId::new(9),
        HostAction::Checkpoint {
            label: "plic-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component && state.chunks().iter().any(|chunk| chunk.name() == "plic")
    }));

    live.restore(&empty).unwrap();

    let restore = HostActionRecord::new(
        24,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(2),
        GuestSourceId::new(9),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 24,
            event: GuestEventId::new(2),
            source: GuestSourceId::new(9),
            manifest,
        }
    );
    assert_eq!(live.snapshot(), expected);
}

#[test]
fn system_action_executor_checkpoints_and_restores_pl031_state() {
    let component = checkpoint_component("pl031.1c170000");
    let live = configured_pl031_device(0x1c17_0000);
    let captured = live.snapshot();
    let empty = pl031_device(0x1c17_0000, 0, 1).snapshot();
    let bank =
        Pl031CheckpointBank::new([Pl031CheckpointPort::new(component.clone(), live.clone())])
            .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_pl031_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(5),
        GuestSourceId::new(11),
        HostAction::Checkpoint {
            label: "pl031-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && state.chunks().iter().any(|chunk| chunk.name() == "pl031")
    }));

    live.restore(&empty).unwrap();

    let restore = HostActionRecord::new(
        26,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(6),
        GuestSourceId::new(11),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 26,
            event: GuestEventId::new(6),
            source: GuestSourceId::new(11),
            manifest,
        }
    );
    assert_eq!(live.snapshot(), captured);
}

#[test]
fn system_action_executor_checkpoints_and_restores_sp804_state() {
    let component = checkpoint_component("sp804.1c110000");
    let live = configured_sp804_device(0x1c11_0000);
    let captured = live.snapshot();
    let empty = sp804_device(0x1c11_0000).snapshot();
    let bank =
        Sp804CheckpointBank::new([Sp804CheckpointPort::new(component.clone(), live.clone())])
            .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_sp804_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        21,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(7),
        GuestSourceId::new(12),
        HostAction::Checkpoint {
            label: "sp804-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && state.chunks().iter().any(|chunk| chunk.name() == "sp804")
    }));

    live.restore(&empty).unwrap();

    let restore = HostActionRecord::new(
        27,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(8),
        GuestSourceId::new(12),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 27,
            event: GuestEventId::new(8),
            source: GuestSourceId::new(12),
            manifest,
        }
    );
    assert_eq!(live.snapshot(), captured);
}

#[test]
fn system_action_executor_checkpoints_and_restores_sp805_state() {
    let component = checkpoint_component("sp805.1c0f0000");
    let live = configured_sp805_device(0x1c0f_0000);
    let captured = live.snapshot();
    let empty = sp805_device(0x1c0f_0000).snapshot();
    let bank =
        Sp805CheckpointBank::new([Sp805CheckpointPort::new(component.clone(), live.clone())])
            .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_sp805_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        22,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(9),
        GuestSourceId::new(13),
        HostAction::Checkpoint {
            label: "sp805-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && state.chunks().iter().any(|chunk| chunk.name() == "sp805")
    }));

    live.restore(&empty).unwrap();

    let restore = HostActionRecord::new(
        28,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(10),
        GuestSourceId::new(13),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 28,
            event: GuestEventId::new(10),
            source: GuestSourceId::new(13),
            manifest,
        }
    );
    assert_eq!(live.snapshot(), captured);
}

#[test]
fn system_action_executor_checkpoints_and_restores_rtc_state() {
    let component = checkpoint_component("rtc.70");
    let captured = rtc_snapshot_with_status_c(
        0xa0,
        0x20,
        0x5a,
        RTC_STATUS_C_IRQF | RTC_STATUS_C_AF | RTC_STATUS_C_UF,
    );
    let empty = rtc_device(0x70).snapshot();
    let live = rtc_device(0x70);
    live.restore(&captured).unwrap();
    let bank =
        RtcCheckpointBank::new([RtcCheckpointPort::new(component.clone(), live.clone())]).unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_rtc_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        19,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(3),
        GuestSourceId::new(10),
        HostAction::Checkpoint {
            label: "rtc-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component && state.chunks().iter().any(|chunk| chunk.name() == "rtc")
    }));

    live.restore(&empty).unwrap();

    let restore = HostActionRecord::new(
        25,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(4),
        GuestSourceId::new(10),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 25,
            event: GuestEventId::new(4),
            source: GuestSourceId::new(10),
            manifest,
        }
    );
    assert_eq!(live.snapshot(), captured);
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
    source_controller
        .lock()
        .unwrap()
        .restore(&expected)
        .unwrap();
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

#[test]
fn interrupt_checkpoint_bank_rejects_duplicate_route_without_partial_restore() {
    let valid_component = checkpoint_component("interrupt_shape_a");
    let invalid_component = checkpoint_component("interrupt_shape_b");
    let target = InterruptTargetId::new(0);
    let partition = PartitionId::new(0);
    let valid_line = InterruptLineId::new(8);
    let invalid_line = InterruptLineId::new(9);
    let valid_route = InterruptRoute::new(valid_line, target, partition);
    let invalid_route = InterruptRoute::new(invalid_line, target, partition);
    let valid_snapshot = rem6_interrupt::InterruptSnapshot::new(
        20,
        vec![valid_route],
        vec![(valid_line, InterruptPriority::new(6))],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let source_valid = Arc::new(Mutex::new(InterruptController::new()));
    source_valid
        .lock()
        .unwrap()
        .restore(&valid_snapshot)
        .unwrap();
    let target_valid = Arc::new(Mutex::new(InterruptController::new()));
    let target_invalid = Arc::new(Mutex::new(InterruptController::new()));
    let original_valid = target_valid.lock().unwrap().snapshot(0);
    let original_invalid = target_invalid.lock().unwrap().snapshot(0);
    assert_ne!(valid_snapshot, original_valid);

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    InterruptControllerCheckpointPort::new(valid_component.clone(), source_valid)
        .capture_into(&mut registry, 20)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(
            &invalid_component,
            "interrupt",
            duplicate_route_interrupt_payload(21, invalid_route, InterruptPriority::new(5)),
        )
        .unwrap();

    let bank = InterruptControllerCheckpointBank::new([
        InterruptControllerCheckpointPort::new(valid_component, Arc::clone(&target_valid)),
        InterruptControllerCheckpointPort::new(invalid_component, Arc::clone(&target_invalid)),
    ])
    .unwrap();
    assert!(bank.restore_all_from(&registry).is_err());
    assert_eq!(target_valid.lock().unwrap().snapshot(0), original_valid);
    assert_eq!(target_invalid.lock().unwrap().snapshot(0), original_invalid);
}

#[test]
fn interrupt_checkpoint_bank_rejects_pending_route_mismatch_without_partial_restore() {
    let valid_component = checkpoint_component("interrupt_pending_a");
    let invalid_component = checkpoint_component("interrupt_pending_b");
    let target = InterruptTargetId::new(0);
    let partition = PartitionId::new(0);
    let invalid_target = InterruptTargetId::new(1);
    let invalid_partition = PartitionId::new(1);
    let valid_line = InterruptLineId::new(10);
    let invalid_line = InterruptLineId::new(11);
    let valid_route = InterruptRoute::new(valid_line, target, partition);
    let invalid_route = InterruptRoute::new(invalid_line, target, partition);
    let valid_snapshot = rem6_interrupt::InterruptSnapshot::new(
        22,
        vec![valid_route],
        vec![(valid_line, InterruptPriority::new(6))],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let source_valid = Arc::new(Mutex::new(InterruptController::new()));
    source_valid
        .lock()
        .unwrap()
        .restore(&valid_snapshot)
        .unwrap();
    let target_valid = Arc::new(Mutex::new(InterruptController::new()));
    let target_invalid = Arc::new(Mutex::new(InterruptController::new()));
    let original_valid = target_valid.lock().unwrap().snapshot(0);
    let original_invalid = target_invalid.lock().unwrap().snapshot(0);
    assert_ne!(valid_snapshot, original_valid);

    let mut registry = CheckpointRegistry::new();
    registry.register(valid_component.clone()).unwrap();
    InterruptControllerCheckpointPort::new(valid_component.clone(), source_valid)
        .capture_into(&mut registry, 22)
        .unwrap();
    registry.register(invalid_component.clone()).unwrap();
    registry
        .write_chunk(
            &invalid_component,
            "interrupt",
            pending_route_mismatch_interrupt_payload(
                23,
                invalid_route,
                invalid_target,
                invalid_partition,
            ),
        )
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
        InterruptControllerCheckpointError::Restore {
            component,
            source,
        } if component == invalid_component
            && *source == rem6_interrupt::InterruptError::RouteMismatch {
                line: invalid_line,
                expected: invalid_route,
                actual: InterruptRoute::new(invalid_line, invalid_target, invalid_partition),
            }
    ));
    assert_eq!(target_valid.lock().unwrap().snapshot(0), original_valid);
    assert_eq!(target_invalid.lock().unwrap().snapshot(0), original_invalid);
}

fn duplicate_route_interrupt_payload(
    tick: u64,
    route: InterruptRoute,
    priority: InterruptPriority,
) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, tick);
    write_u64(&mut payload, 2);
    write_interrupt_route(&mut payload, route);
    write_interrupt_route(&mut payload, route);
    write_u64(&mut payload, 1);
    write_u64(&mut payload, route.line().get());
    write_u32(&mut payload, priority.get());
    write_u64(&mut payload, 0);
    write_u64(&mut payload, 0);
    write_u64(&mut payload, 0);
    payload
}

fn pending_route_mismatch_interrupt_payload(
    tick: u64,
    route: InterruptRoute,
    pending_target: InterruptTargetId,
    pending_partition: PartitionId,
) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, tick);
    write_u64(&mut payload, 1);
    write_interrupt_route(&mut payload, route);
    write_u64(&mut payload, 1);
    write_u64(&mut payload, route.line().get());
    write_u32(&mut payload, InterruptPriority::new(5).get());
    write_u64(&mut payload, 1);
    write_u64(&mut payload, route.line().get());
    write_u32(&mut payload, pending_target.get());
    write_u32(&mut payload, pending_partition.index());
    write_u32(&mut payload, InterruptSourceId::new(55).get());
    write_u64(&mut payload, 12);
    write_u64(&mut payload, 0);
    write_u64(&mut payload, 0);
    payload
}

fn write_interrupt_route(payload: &mut Vec<u8>, route: InterruptRoute) {
    write_u64(payload, route.line().get());
    write_u32(payload, route.target().get());
    write_u32(payload, route.target_partition().index());
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}
