use rem6_stats::{
    MemCheckerMonitor, MemCheckerMonitorCompletion, MemCheckerMonitorPendingTransaction,
    MemCheckerMonitorSnapshot, MemCheckerReadResult, MemCheckerWriteResult, MemProbePacket,
    MemProbePacketAccess, MemProbePacketKind, StatsError,
};

fn read_request(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::read(address)
        .with_packet_id(id)
        .with_size(size)
}

fn write_request(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::write(address)
        .with_packet_id(id)
        .with_size(size)
}

fn read_response(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::response(address)
        .with_access(MemProbePacketAccess::Read)
        .with_packet_id(id)
        .with_size(size)
}

fn write_response(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::response(address)
        .with_access(MemProbePacketAccess::Write)
        .with_packet_id(id)
        .with_size(size)
}

#[test]
fn monitor_tracks_only_forwarded_memory_side_requests() {
    let mut monitor = MemCheckerMonitor::new();

    assert_eq!(
        monitor.observe_timing_request(
            10,
            &write_request(1, 0x1000, 2),
            true,
            false,
            Some(&[0xaa, 0xbb])
        ),
        Ok(None)
    );
    assert_eq!(monitor.snapshot().pending(), &[]);
    assert_eq!(monitor.snapshot().checker().next_serial(), 1);

    assert_eq!(
        monitor.observe_timing_request(11, &read_request(2, 0x1000, 2), false, true, None),
        Ok(None)
    );
    assert_eq!(monitor.snapshot().pending(), &[]);
    assert_eq!(monitor.snapshot().checker().next_serial(), 1);

    assert_eq!(
        monitor.observe_timing_request(12, &read_request(3, 0x1000, 2), true, true, None),
        Ok(Some(MemCheckerMonitorPendingTransaction::new(
            3,
            MemProbePacketAccess::Read,
            0x1000,
            2,
            1,
            12
        )))
    );
    assert_eq!(monitor.snapshot().checker().next_serial(), 2);

    assert_eq!(
        monitor.observe_timing_request(
            13,
            &MemProbePacket::request(0x1000)
                .with_packet_id(4)
                .with_size(2),
            true,
            true,
            None
        ),
        Ok(None)
    );
    assert_eq!(monitor.snapshot().pending().len(), 1);
}

#[test]
fn monitor_completes_read_after_successful_cpu_side_response() {
    let mut monitor = MemCheckerMonitor::new();

    monitor
        .observe_timing_request(
            10,
            &write_request(1, 0x2000, 2),
            true,
            true,
            Some(&[0xaa, 0xbb]),
        )
        .unwrap();
    assert_eq!(
        monitor.observe_timing_response(20, &write_response(1, 0x2000, 2), true, None, false),
        Ok(Some(MemCheckerMonitorCompletion::Write(
            MemCheckerWriteResult::new(1, 2, 0)
        )))
    );

    let pending = monitor
        .observe_timing_request(30, &read_request(2, 0x2000, 2), true, true, None)
        .unwrap()
        .unwrap();
    assert_eq!(
        monitor.observe_timing_response(
            40,
            &read_response(2, 0x2000, 2),
            false,
            Some(&[0xaa, 0xbb]),
            false
        ),
        Ok(None)
    );
    assert_eq!(monitor.snapshot().pending(), &[pending]);
    assert_eq!(
        monitor.observe_timing_response(
            40,
            &read_response(2, 0x2000, 2),
            true,
            Some(&[0xaa, 0xbb]),
            false
        ),
        Ok(Some(MemCheckerMonitorCompletion::Read(
            MemCheckerReadResult::valid(2, 2, 0)
        )))
    );
    assert_eq!(monitor.snapshot().pending(), &[]);
}

#[test]
fn monitor_aborts_failed_llsc_writes_and_resets_functional_ranges() {
    let mut monitor = MemCheckerMonitor::new();

    monitor
        .observe_timing_request(10, &write_request(1, 0x3000, 1), true, true, Some(&[0x11]))
        .unwrap();
    assert_eq!(
        monitor.observe_timing_response(20, &write_response(1, 0x3000, 1), true, None, true),
        Ok(Some(MemCheckerMonitorCompletion::AbortedWrite(
            MemCheckerWriteResult::new(1, 1, 0)
        )))
    );

    monitor
        .observe_timing_request(30, &read_request(2, 0x3000, 1), true, true, None)
        .unwrap();
    assert_eq!(
        monitor.observe_timing_response(
            40,
            &read_response(2, 0x3000, 1),
            true,
            Some(&[0x00]),
            false
        ),
        Ok(Some(MemCheckerMonitorCompletion::Read(
            MemCheckerReadResult::valid(2, 1, 0)
        )))
    );

    monitor
        .observe_timing_request(50, &write_request(3, 0x4000, 1), true, true, Some(&[0x44]))
        .unwrap();
    monitor.observe_functional(0x4000, 1).unwrap();
    assert_eq!(
        monitor.observe_timing_response(60, &write_response(3, 0x4000, 1), true, None, false),
        Ok(Some(MemCheckerMonitorCompletion::Write(
            MemCheckerWriteResult::new(3, 0, 1)
        )))
    );

    monitor
        .observe_timing_request(70, &read_request(4, 0x4000, 1), true, true, None)
        .unwrap();
    assert_eq!(
        monitor.observe_timing_response(
            80,
            &read_response(4, 0x4000, 1),
            true,
            Some(&[0x99]),
            false
        ),
        Ok(Some(MemCheckerMonitorCompletion::Read(
            MemCheckerReadResult::valid(4, 1, 0)
        )))
    );
}

#[test]
fn monitor_rejects_ambiguous_runtime_state_transactionally() {
    let mut monitor = MemCheckerMonitor::new();

    assert_eq!(
        monitor.observe_timing_request(10, &write_request(1, 0x5000, 2), true, true, None),
        Err(StatsError::MemCheckerMonitorRequestDataMissing { packet_id: 1 })
    );
    assert_eq!(monitor.snapshot().checker().next_serial(), 1);
    assert_eq!(
        monitor.observe_timing_request(10, &write_request(1, 0x5000, 2), true, true, Some(&[0xaa])),
        Err(StatsError::MemCheckerMonitorRequestDataSizeMismatch {
            packet_id: 1,
            packet_size: 2,
            data_size: 1,
        })
    );
    assert_eq!(monitor.snapshot().checker().next_serial(), 1);

    monitor
        .observe_timing_request(20, &read_request(2, 0x5000, 4), true, true, None)
        .unwrap();
    let before_duplicate = monitor.snapshot();
    assert_eq!(
        monitor.observe_timing_request(21, &read_request(2, 0x5040, 4), true, true, None),
        Err(StatsError::DuplicateMemCheckerMonitorPendingPacket { packet_id: 2 })
    );
    assert_eq!(monitor.snapshot(), before_duplicate);

    let before_response_errors = monitor.snapshot();
    assert_eq!(
        monitor.observe_timing_response(
            30,
            &read_response(9, 0x5000, 4),
            true,
            Some(&[0; 4]),
            false
        ),
        Err(StatsError::UnknownMemCheckerMonitorPendingPacket { packet_id: 9 })
    );
    assert_eq!(
        monitor.observe_timing_response(30, &write_response(2, 0x5000, 4), true, None, false),
        Err(StatsError::MemCheckerMonitorResponseAccessMismatch {
            packet_id: 2,
            request_access: MemProbePacketAccess::Read,
            response_access: MemProbePacketAccess::Write,
        })
    );
    assert_eq!(
        monitor.observe_timing_response(
            30,
            &read_response(2, 0x5004, 4),
            true,
            Some(&[0; 4]),
            false
        ),
        Err(StatsError::MemCheckerMonitorResponseAddressMismatch {
            packet_id: 2,
            request_address: 0x5000,
            response_address: 0x5004,
        })
    );
    assert_eq!(
        monitor.observe_timing_response(
            30,
            &read_response(2, 0x5000, 8),
            true,
            Some(&[0; 8]),
            false
        ),
        Err(StatsError::MemCheckerMonitorResponseSizeMismatch {
            packet_id: 2,
            request_size: 4,
            response_size: 8,
        })
    );
    assert_eq!(
        monitor.observe_timing_response(30, &read_response(2, 0x5000, 4), true, None, false),
        Err(StatsError::MemCheckerMonitorResponseDataMissing { packet_id: 2 })
    );
    assert_eq!(
        monitor.observe_timing_response(
            30,
            &read_response(2, 0x5000, 4),
            true,
            Some(&[0; 3]),
            false
        ),
        Err(StatsError::MemCheckerMonitorResponseDataSizeMismatch {
            packet_id: 2,
            packet_size: 4,
            data_size: 3,
        })
    );
    assert_eq!(monitor.snapshot(), before_response_errors);
}

#[test]
fn monitor_snapshot_restore_preserves_pending_sender_state() {
    let mut monitor = MemCheckerMonitor::new();

    let write = monitor
        .observe_timing_request(10, &write_request(7, 0x6000, 1), true, true, Some(&[0x55]))
        .unwrap()
        .unwrap();
    let read = monitor
        .observe_timing_request(12, &read_request(8, 0x6000, 1), true, true, None)
        .unwrap()
        .unwrap();

    let snapshot = monitor.snapshot();
    let mut restored = MemCheckerMonitor::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.observe_timing_response(
            20,
            &read_response(8, 0x6000, 1),
            true,
            Some(&[0x55]),
            false
        ),
        Ok(Some(MemCheckerMonitorCompletion::Read(
            MemCheckerReadResult::valid(read.serial(), 1, 0)
        )))
    );
    assert_eq!(
        restored.observe_timing_response(22, &write_response(7, 0x6000, 1), true, None, false),
        Ok(Some(MemCheckerMonitorCompletion::Write(
            MemCheckerWriteResult::new(write.serial(), 1, 0)
        )))
    );

    let duplicate_pending =
        MemCheckerMonitorSnapshot::new(snapshot.checker().clone(), vec![write, write]);
    assert_eq!(
        MemCheckerMonitor::from_snapshot(&duplicate_pending),
        Err(StatsError::DuplicateMemCheckerMonitorPendingPacket { packet_id: 7 })
    );

    let duplicate_serial = MemCheckerMonitorSnapshot::new(
        snapshot.checker().clone(),
        vec![
            write,
            MemCheckerMonitorPendingTransaction::new(
                9,
                MemProbePacketAccess::Read,
                0x6001,
                1,
                write.serial(),
                14,
            ),
        ],
    );
    assert_eq!(
        MemCheckerMonitor::from_snapshot(&duplicate_serial),
        Err(StatsError::DuplicateMemCheckerMonitorPendingSerial {
            serial: write.serial()
        })
    );

    let unallocated_serial = MemCheckerMonitorSnapshot::new(
        snapshot.checker().clone(),
        vec![MemCheckerMonitorPendingTransaction::new(
            9,
            MemProbePacketAccess::Read,
            0x6001,
            1,
            snapshot.checker().next_serial(),
            14,
        )],
    );
    assert_eq!(
        MemCheckerMonitor::from_snapshot(&unallocated_serial),
        Err(StatsError::MemCheckerMonitorPendingSerialNotAllocated {
            packet_id: 9,
            serial: snapshot.checker().next_serial(),
            next_serial: snapshot.checker().next_serial()
        })
    );

    let invalid_access = MemCheckerMonitorSnapshot::new(
        snapshot.checker().clone(),
        vec![MemCheckerMonitorPendingTransaction::new(
            9,
            MemProbePacketAccess::Other,
            0x6000,
            1,
            9,
            30,
        )],
    );
    assert_eq!(
        MemCheckerMonitor::from_snapshot(&invalid_access),
        Err(StatsError::InvalidMemCheckerMonitorPendingAccess {
            packet_id: 9,
            access: MemProbePacketAccess::Other,
        })
    );

    assert_eq!(
        MemCheckerMonitor::new().observe_timing_response(
            1,
            &MemProbePacket::new(0x6000, MemProbePacketKind::Request),
            true,
            None,
            false
        ),
        Ok(None)
    );
}
