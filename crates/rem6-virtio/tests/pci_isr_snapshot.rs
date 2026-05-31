use rem6_memory::{AccessSize, Address};
use rem6_virtio::{
    VirtioError, VirtioPciIsrDevice, VirtioPciIsrEvent, VirtioPciIsrEventKind,
    VirtioPciIsrSnapshot, VirtioPciIsrStatus,
};

#[test]
fn virtio_pci_isr_snapshot_bytes_round_trip_and_restore() {
    let isr = VirtioPciIsrDevice::new();
    isr.raise_queue_interrupt(5);
    isr.raise_configuration_change_interrupt(8);
    assert_eq!(
        isr.read_local(Address::new(0), AccessSize::new(1).unwrap())
            .unwrap(),
        vec![VirtioPciIsrStatus::queue_and_config().bits()]
    );
    isr.raise_queue_interrupt(13);
    let snapshot = isr.snapshot();

    let decoded = VirtioPciIsrSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();

    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        decoded.events(),
        &[
            VirtioPciIsrEvent::new(
                5,
                VirtioPciIsrEventKind::QueueInterrupt,
                VirtioPciIsrStatus::empty(),
                VirtioPciIsrStatus::queue_interrupt(),
            ),
            VirtioPciIsrEvent::new(
                8,
                VirtioPciIsrEventKind::ConfigurationChangeInterrupt,
                VirtioPciIsrStatus::queue_interrupt(),
                VirtioPciIsrStatus::queue_and_config(),
            ),
            VirtioPciIsrEvent::new(
                0,
                VirtioPciIsrEventKind::DriverReadClear,
                VirtioPciIsrStatus::queue_and_config(),
                VirtioPciIsrStatus::empty(),
            ),
            VirtioPciIsrEvent::new(
                13,
                VirtioPciIsrEventKind::QueueInterrupt,
                VirtioPciIsrStatus::empty(),
                VirtioPciIsrStatus::queue_interrupt(),
            ),
        ]
    );

    let restored = VirtioPciIsrDevice::new();
    restored.restore(&decoded);
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn virtio_pci_isr_snapshot_bytes_reject_malformed_payloads() {
    let snapshot = VirtioPciIsrSnapshot::new(
        VirtioPciIsrStatus::queue_interrupt(),
        vec![VirtioPciIsrEvent::new(
            5,
            VirtioPciIsrEventKind::QueueInterrupt,
            VirtioPciIsrStatus::empty(),
            VirtioPciIsrStatus::queue_interrupt(),
        )],
    );
    let payload = snapshot.to_bytes();

    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&payload[..payload.len() - 1]),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut invalid_magic = payload.clone();
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&invalid_magic),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut invalid_version = payload.clone();
    invalid_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&invalid_version),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut invalid_status = payload.clone();
    invalid_status[10] = 0x80;
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&invalid_status),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut invalid_event_count = payload.clone();
    invalid_event_count[11..19].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&invalid_event_count),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut invalid_event_kind = payload.clone();
    invalid_event_kind[19] = 9;
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&invalid_event_kind),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut invalid_event_status = payload.clone();
    invalid_event_status[20] = 0x40;
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&invalid_event_status),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );

    let mut trailing = payload.clone();
    trailing.push(0);
    assert_eq!(
        VirtioPciIsrSnapshot::from_bytes(&trailing),
        Err(VirtioError::InvalidPciIsrSnapshot)
    );
}
