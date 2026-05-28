use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptRoute,
    InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciFunctionAddress, PciMsiMessage, PciMsixCapabilitySpec, PciMsixPort, PciMsixRoute,
};

fn storage_endpoint() -> PciEndpointConfig {
    PciEndpointConfig::new(
        PciFunctionAddress::new(0, 9, 0).unwrap(),
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
    )
}

fn install_msix(endpoint: &mut PciEndpointConfig) {
    endpoint
        .install_msix_capability(
            PciMsixCapabilitySpec::new(
                PciConfigOffset::new(0x70).unwrap(),
                4,
                PciBarIndex::new(2).unwrap(),
                Address::new(0x100),
                PciBarIndex::new(2).unwrap(),
                Address::new(0x180),
            )
            .unwrap(),
        )
        .unwrap();
}

fn program_msix_vector(endpoint: &mut PciEndpointConfig, vector: u16, data: u16) {
    let offset = 0x100 + u64::from(vector) * 16;
    endpoint
        .write_msix_region(Address::new(offset), &0xfee0_0123_u32.to_le_bytes())
        .unwrap();
    endpoint
        .write_msix_region(Address::new(offset + 4), &0x0000_0001_u32.to_le_bytes())
        .unwrap();
    endpoint
        .write_msix_region(Address::new(offset + 8), &u32::from(data).to_le_bytes())
        .unwrap();
    endpoint
        .write_msix_region(Address::new(offset + 12), &0_u32.to_le_bytes())
        .unwrap();
}

#[test]
fn pci_msix_capability_exposes_header_table_and_pba_registers() {
    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0, 0, 0x10, 0])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x34).unwrap(),
            AccessSize::new(1).unwrap(),
        ),
        Ok(vec![0x70])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x70).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x11, 0x00, 0x03, 0x00])
    );
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x74).unwrap()),
        Ok(0x0000_0102)
    );
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x78).unwrap()),
        Ok(0x0000_0182)
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0xffff_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x72).unwrap(),
            AccessSize::new(2).unwrap(),
        ),
        Ok(0xc003_u16.to_le_bytes().to_vec())
    );
}

#[test]
fn pci_msix_table_programs_messages_and_masks_vectors() {
    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);
    program_msix_vector(&mut endpoint, 0, 0x0040);
    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0xc000_u16.to_le_bytes(),
        )
        .unwrap();

    assert_eq!(endpoint.msix_message(0), Ok(None));

    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0x8000_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_msix_region(Address::new(0x100), AccessSize::new(16).unwrap()),
        Ok(vec![
            0x20, 0x01, 0xe0, 0xfe, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ])
    );
    assert_eq!(
        endpoint.msix_message(0),
        Ok(Some(PciMsiMessage::new(
            endpoint.function(),
            0,
            Address::new(0x1_fee0_0120),
            0x0040,
        )))
    );

    endpoint
        .write_msix_region(Address::new(0x10c), &1_u32.to_le_bytes())
        .unwrap();
    assert_eq!(endpoint.msix_message(0), Ok(None));
}

#[test]
fn pci_msix_capability_rejects_bad_layouts_and_duplicate_install() {
    assert_eq!(
        PciMsixCapabilitySpec::new(
            PciConfigOffset::new(0x3c).unwrap(),
            1,
            PciBarIndex::new(0).unwrap(),
            Address::new(0),
            PciBarIndex::new(0).unwrap(),
            Address::new(0x80),
        ),
        Err(PciError::InvalidMsixCapabilityOffset {
            offset: PciConfigOffset::new(0x3c).unwrap(),
            size: AccessSize::new(0x0c).unwrap(),
        })
    );
    assert_eq!(
        PciMsixCapabilitySpec::new(
            PciConfigOffset::new(0x70).unwrap(),
            0,
            PciBarIndex::new(0).unwrap(),
            Address::new(0),
            PciBarIndex::new(0).unwrap(),
            Address::new(0x80),
        ),
        Err(PciError::InvalidMsixVectorCount { count: 0 })
    );
    assert_eq!(
        PciMsixCapabilitySpec::new(
            PciConfigOffset::new(0x70).unwrap(),
            1,
            PciBarIndex::new(0).unwrap(),
            Address::new(0x20),
            PciBarIndex::new(0).unwrap(),
            Address::new(0x20),
        ),
        Err(PciError::OverlappingMsixRegions {
            table_bar: PciBarIndex::new(0).unwrap(),
            pba_bar: PciBarIndex::new(0).unwrap(),
        })
    );

    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);
    assert_eq!(
        endpoint.install_msix_capability(
            PciMsixCapabilitySpec::new(
                PciConfigOffset::new(0x80).unwrap(),
                1,
                PciBarIndex::new(1).unwrap(),
                Address::new(0x200),
                PciBarIndex::new(1).unwrap(),
                Address::new(0x280),
            )
            .unwrap(),
        ),
        Err(PciError::DuplicateMsixCapability)
    );
}

#[test]
fn pci_msix_snapshot_restore_preserves_table_and_pending_state() {
    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);
    program_msix_vector(&mut endpoint, 2, 0x0060);
    endpoint
        .write_msix_region(Address::new(0x12c), &1_u32.to_le_bytes())
        .unwrap();
    endpoint.queue_msix_pending(2).unwrap();
    let snapshot = endpoint.snapshot();

    endpoint
        .write_msix_region(Address::new(0x12c), &0_u32.to_le_bytes())
        .unwrap();
    endpoint.clear_msix_pending(2).unwrap();

    endpoint.restore(&snapshot).unwrap();
    assert_eq!(endpoint.msix_message(2), Ok(None));
    assert_eq!(
        endpoint.read_msix_region(Address::new(0x180), AccessSize::new(8).unwrap()),
        Ok(0b100_u64.to_le_bytes().to_vec())
    );
}

#[test]
fn pci_endpoint_snapshot_exposes_msix_payload_for_checkpoint_audit() {
    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);
    program_msix_vector(&mut endpoint, 2, 0x0060);
    endpoint.queue_msix_pending(2).unwrap();
    let snapshot = endpoint.snapshot();

    let payload = snapshot.msix_payload().unwrap();

    assert_eq!(snapshot.validate_msix_payload(&payload), Ok(()));
    let no_msix = storage_endpoint().snapshot();
    assert_eq!(no_msix.msix_payload(), None);
    assert_eq!(
        no_msix.validate_msix_payload(&payload),
        Err(PciError::SnapshotMsixCapabilityMismatch)
    );

    let different_table = {
        let mut endpoint = storage_endpoint();
        install_msix(&mut endpoint);
        program_msix_vector(&mut endpoint, 2, 0x0070);
        endpoint.queue_msix_pending(2).unwrap();
        endpoint.snapshot()
    };
    assert_eq!(
        different_table.validate_msix_payload(&payload),
        Err(PciError::SnapshotMsixCapabilityMismatch)
    );
    let different_spec = {
        let mut endpoint = storage_endpoint();
        endpoint
            .install_msix_capability(
                PciMsixCapabilitySpec::new(
                    PciConfigOffset::new(0x80).unwrap(),
                    2,
                    PciBarIndex::new(2).unwrap(),
                    Address::new(0x200),
                    PciBarIndex::new(2).unwrap(),
                    Address::new(0x280),
                )
                .unwrap(),
            )
            .unwrap();
        endpoint.snapshot()
    };
    assert_eq!(
        different_spec.validate_msix_payload(&payload),
        Err(PciError::SnapshotMsixCapabilityMismatch)
    );

    let mut corrupted = payload;
    corrupted.push(0);
    assert_eq!(
        snapshot.validate_msix_payload(&corrupted),
        Err(PciError::InvalidMsixCapabilitySnapshot)
    );
}

#[test]
fn pci_msix_port_sends_enabled_vector_on_serial_scheduler() {
    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);
    program_msix_vector(&mut endpoint, 1, 0x0050);
    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0x8000_u16.to_le_bytes(),
        )
        .unwrap();
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(92);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = InterruptRoute::new(InterruptLineId::new(66), InterruptTargetId::new(0), cpu);
    controller.lock().unwrap().register_route(route).unwrap();
    let msix_route = PciMsixRoute::new(
        endpoint.function(),
        1,
        endpoint.msix_message(1).unwrap().unwrap(),
        route,
        2,
    )
    .unwrap();
    let port = PciMsixPort::new(msix_route, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(pci, 5, move |context| {
            assert!(port.send(&mut endpoint, context, source).unwrap().is_some());
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            7,
            InterruptLineId::new(66),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn pci_msix_port_records_masked_parallel_delivery_as_pending() {
    let mut endpoint = storage_endpoint();
    install_msix(&mut endpoint);
    program_msix_vector(&mut endpoint, 3, 0x0070);
    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0x8000_u16.to_le_bytes(),
        )
        .unwrap();
    let message = endpoint.msix_message(3).unwrap().unwrap();
    endpoint
        .write_msix_region(Address::new(0x13c), &1_u32.to_le_bytes())
        .unwrap();
    let endpoint = Arc::new(Mutex::new(endpoint));
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(93);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = InterruptRoute::new(InterruptLineId::new(67), InterruptTargetId::new(0), cpu);
    controller.lock().unwrap().register_route(route).unwrap();
    let msix_route = PciMsixRoute::new(
        PciFunctionAddress::new(0, 9, 0).unwrap(),
        3,
        message,
        route,
        3,
    )
    .unwrap();
    let port = PciMsixPort::new(msix_route, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let scheduled_endpoint = Arc::clone(&endpoint);
    scheduler
        .schedule_parallel_at(pci, 4, move |context| {
            let mut endpoint = scheduled_endpoint.lock().unwrap();
            assert!(port
                .send_parallel(&mut endpoint, context, source)
                .unwrap()
                .is_none());
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert!(controller.lock().unwrap().history().is_empty());
    assert_eq!(
        endpoint
            .lock()
            .unwrap()
            .read_msix_region(Address::new(0x180), AccessSize::new(8).unwrap()),
        Ok(0b1000_u64.to_le_bytes().to_vec())
    );
}
