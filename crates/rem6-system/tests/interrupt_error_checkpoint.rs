use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_interrupt::{
    InterruptController, InterruptError, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_system::{
    TimerCheckpointBank, TimerCheckpointPort, UartCheckpointBank, UartCheckpointPort,
};
use rem6_timer::{ProgrammableTimer, TimerId, TimerSignalError, TimerSnapshot};
use rem6_uart::{UartId, UartInterruptError, UartMmioDevice, UartSnapshot};

#[test]
fn uart_checkpoint_round_trips_snapshot_interrupt_errors() {
    let component = CheckpointComponentId::new("uart_error_variants").unwrap();
    let uart = UartMmioDevice::new(UartId::new(1), Address::new(0xa100));
    let captured = UartSnapshot::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            UartInterruptError::new(
                1,
                InterruptSourceId::new(41),
                InterruptEventKind::Assert,
                InterruptError::DuplicateSnapshotPriority {
                    line: InterruptLineId::new(11),
                },
            ),
            UartInterruptError::new(
                2,
                InterruptSourceId::new(42),
                InterruptEventKind::Deassert,
                InterruptError::DuplicateSnapshotPending {
                    line: InterruptLineId::new(12),
                },
            ),
            UartInterruptError::new(
                3,
                InterruptSourceId::new(43),
                InterruptEventKind::Assert,
                InterruptError::DuplicateSnapshotClaim {
                    target: InterruptTargetId::new(4),
                    target_partition: PartitionId::new(5),
                },
            ),
            UartInterruptError::new(
                4,
                InterruptSourceId::new(44),
                InterruptEventKind::Complete,
                InterruptError::MissingSnapshotPriority {
                    line: InterruptLineId::new(13),
                },
            ),
        ],
    );
    uart.restore(&captured);
    let bank = UartCheckpointBank::new([UartCheckpointPort::new(component.clone(), uart.clone())])
        .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    bank.capture_all_into(&mut checkpoints).unwrap();
    uart.restore(&UartSnapshot::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ));

    let restored = bank.restore_all_from(&checkpoints).unwrap();

    assert_eq!(restored[0].snapshot(), &captured);
    assert_eq!(uart.snapshot(), captured);
}

#[test]
fn timer_checkpoint_round_trips_snapshot_interrupt_errors() {
    let timer_partition = PartitionId::new(2);
    let target_partition = PartitionId::new(0);
    let interrupt_source = InterruptSourceId::new(53);
    let component = CheckpointComponentId::new("timer_error_variants").unwrap();
    let timer = timer_with_interrupt(
        TimerId::new(1),
        timer_partition,
        target_partition,
        interrupt_source,
    );
    let captured = TimerSnapshot::new(
        TimerId::new(1),
        timer_partition,
        interrupt_source,
        None,
        Vec::new(),
        Vec::new(),
        vec![
            TimerSignalError::new(
                1,
                11,
                InterruptError::DuplicateSnapshotPriority {
                    line: InterruptLineId::new(21),
                },
            ),
            TimerSignalError::new(
                2,
                12,
                InterruptError::DuplicateSnapshotPending {
                    line: InterruptLineId::new(22),
                },
            ),
            TimerSignalError::new(
                3,
                13,
                InterruptError::DuplicateSnapshotClaim {
                    target: InterruptTargetId::new(6),
                    target_partition: PartitionId::new(7),
                },
            ),
            TimerSignalError::new(
                4,
                14,
                InterruptError::MissingSnapshotPriority {
                    line: InterruptLineId::new(23),
                },
            ),
        ],
    );
    let empty = TimerSnapshot::new(
        TimerId::new(1),
        timer_partition,
        interrupt_source,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    timer.restore(&captured).unwrap();
    let bank =
        TimerCheckpointBank::new([TimerCheckpointPort::new(component.clone(), timer.clone())])
            .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    bank.capture_all_into(&mut checkpoints).unwrap();
    timer.restore(&empty).unwrap();

    let restored = bank.restore_all_from(&checkpoints).unwrap();

    assert_eq!(restored[0].snapshot(), &captured);
    assert_eq!(timer.snapshot(), captured);
}

fn timer_with_interrupt(
    id: TimerId,
    timer_partition: PartitionId,
    target_partition: PartitionId,
    source: InterruptSourceId,
) -> ProgrammableTimer {
    let route = InterruptRoute::new(
        InterruptLineId::new(70),
        InterruptTargetId::new(0),
        target_partition,
    );
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    ProgrammableTimer::new(id, timer_partition, source, port)
}
