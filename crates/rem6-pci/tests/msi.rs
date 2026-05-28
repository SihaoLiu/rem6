use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptRoute,
    InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciFunctionAddress, PciMsiCapabilitySpec, PciMsiMessage, PciMsiPort, PciMsiRoute,
};

fn storage_endpoint() -> PciEndpointConfig {
    PciEndpointConfig::new(
        PciFunctionAddress::new(0, 8, 0).unwrap(),
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
    )
}

fn install_msi(endpoint: &mut PciEndpointConfig) {
    endpoint
        .install_msi_capability(
            PciMsiCapabilitySpec::new(PciConfigOffset::new(0x50).unwrap(), 4, true, true).unwrap(),
        )
        .unwrap();
}

fn program_enabled_msi(endpoint: &mut PciEndpointConfig) {
    install_msi(endpoint);
    endpoint
        .write_config(
            PciConfigOffset::new(0x52).unwrap(),
            &0x0021_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x54).unwrap(), 0xfee0_0123)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x58).unwrap(), 0x0000_0001)
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x5c).unwrap(),
            &0x0040_u16.to_le_bytes(),
        )
        .unwrap();
}

#[test]
fn pci_msi_capability_exposes_header_control_and_programmed_message() {
    let mut endpoint = storage_endpoint();
    install_msi(&mut endpoint);

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
        Ok(vec![0x50])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x50).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x05, 0x00, 0x84, 0x01])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x52).unwrap(),
            &0x0071_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x54).unwrap(), 0xfee0_0123)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x58).unwrap(), 0x0000_0001)
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x5c).unwrap(),
            &0x0040_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x60).unwrap(), 0b0100)
        .unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x52).unwrap(),
            AccessSize::new(2).unwrap(),
        ),
        Ok(0x01a5_u16.to_le_bytes().to_vec())
    );
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x54).unwrap()),
        Ok(0xfee0_0120)
    );
    assert_eq!(
        endpoint.msi_message(0),
        Ok(Some(PciMsiMessage::new(
            endpoint.function(),
            0,
            Address::new(0x1_fee0_0120),
            0x0040,
        )))
    );
    assert_eq!(endpoint.msi_message(2), Ok(None));
    assert_eq!(
        endpoint.msi_message(3),
        Ok(Some(PciMsiMessage::new(
            endpoint.function(),
            3,
            Address::new(0x1_fee0_0120),
            0x0043,
        )))
    );
}

#[test]
fn pci_msi_capability_rejects_bad_layouts_and_duplicate_install() {
    assert_eq!(
        PciMsiCapabilitySpec::new(PciConfigOffset::new(0x3c).unwrap(), 1, true, false),
        Err(PciError::InvalidMsiCapabilityOffset {
            offset: PciConfigOffset::new(0x3c).unwrap(),
            size: AccessSize::new(0x18).unwrap(),
        })
    );
    assert_eq!(
        PciMsiCapabilitySpec::new(PciConfigOffset::new(0x52).unwrap(), 1, true, false),
        Err(PciError::InvalidMsiCapabilityOffset {
            offset: PciConfigOffset::new(0x52).unwrap(),
            size: AccessSize::new(0x18).unwrap(),
        })
    );
    assert_eq!(
        PciMsiCapabilitySpec::new(PciConfigOffset::new(0xf0).unwrap(), 1, true, true),
        Err(PciError::InvalidMsiCapabilityOffset {
            offset: PciConfigOffset::new(0xf0).unwrap(),
            size: AccessSize::new(0x18).unwrap(),
        })
    );
    assert_eq!(
        PciMsiCapabilitySpec::new(PciConfigOffset::new(0x50).unwrap(), 3, true, false),
        Err(PciError::InvalidMsiVectorCount { count: 3 })
    );

    let mut endpoint = storage_endpoint();
    install_msi(&mut endpoint);
    assert_eq!(
        endpoint.install_msi_capability(
            PciMsiCapabilitySpec::new(PciConfigOffset::new(0x70).unwrap(), 1, false, false)
                .unwrap(),
        ),
        Err(PciError::DuplicateMsiCapability)
    );
}

#[test]
fn pci_msi_snapshot_restore_preserves_configured_message_state() {
    let mut endpoint = storage_endpoint();
    program_enabled_msi(&mut endpoint);
    let snapshot = endpoint.snapshot();

    endpoint
        .write_config(
            PciConfigOffset::new(0x52).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(endpoint.msi_message(0), Ok(None));

    endpoint.restore(&snapshot).unwrap();
    assert_eq!(
        endpoint.msi_message(0),
        Ok(Some(PciMsiMessage::new(
            endpoint.function(),
            0,
            Address::new(0x1_fee0_0120),
            0x0040,
        )))
    );
}

#[test]
fn pci_endpoint_snapshot_exposes_msi_payload_for_checkpoint_audit() {
    let mut endpoint = storage_endpoint();
    program_enabled_msi(&mut endpoint);
    endpoint
        .write_u32(PciConfigOffset::new(0x60).unwrap(), 0b0100)
        .unwrap();
    let snapshot = endpoint.snapshot();

    let payload = snapshot.msi_payload().unwrap();

    assert_eq!(snapshot.validate_msi_payload(&payload), Ok(()));
    let no_msi = storage_endpoint().snapshot();
    assert_eq!(no_msi.msi_payload(), None);
    assert_eq!(
        no_msi.validate_msi_payload(&payload),
        Err(PciError::SnapshotMsiCapabilityMismatch)
    );

    let different_message = {
        let mut endpoint = storage_endpoint();
        program_enabled_msi(&mut endpoint);
        endpoint
            .write_config(
                PciConfigOffset::new(0x5c).unwrap(),
                &0x0050_u16.to_le_bytes(),
            )
            .unwrap();
        endpoint.snapshot()
    };
    assert_eq!(
        different_message.validate_msi_payload(&payload),
        Err(PciError::SnapshotMsiCapabilityMismatch)
    );
    let different_spec = {
        let mut endpoint = storage_endpoint();
        endpoint
            .install_msi_capability(
                PciMsiCapabilitySpec::new(PciConfigOffset::new(0x70).unwrap(), 2, true, true)
                    .unwrap(),
            )
            .unwrap();
        endpoint.snapshot()
    };
    assert_eq!(
        different_spec.validate_msi_payload(&payload),
        Err(PciError::SnapshotMsiCapabilityMismatch)
    );

    let mut corrupted = payload;
    corrupted.push(0);
    assert_eq!(
        snapshot.validate_msi_payload(&corrupted),
        Err(PciError::InvalidMsiCapabilitySnapshot)
    );
}

#[test]
fn pci_msi_port_sends_configured_message_on_serial_scheduler() {
    let mut endpoint = storage_endpoint();
    program_enabled_msi(&mut endpoint);
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(90);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = InterruptRoute::new(InterruptLineId::new(64), InterruptTargetId::new(0), cpu);
    controller.lock().unwrap().register_route(route).unwrap();
    let msi_route = PciMsiRoute::new(
        endpoint.function(),
        0,
        endpoint.msi_message(0).unwrap().unwrap(),
        route,
        2,
    )
    .unwrap();
    let port = PciMsiPort::new(msi_route, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(pci, 5, move |context| {
            assert!(port.send(&endpoint, context, source).unwrap().is_some());
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            7,
            InterruptLineId::new(64),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn pci_msi_port_sends_configured_message_on_parallel_scheduler() {
    let mut endpoint = storage_endpoint();
    program_enabled_msi(&mut endpoint);
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(91);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = InterruptRoute::new(InterruptLineId::new(65), InterruptTargetId::new(0), cpu);
    controller.lock().unwrap().register_route(route).unwrap();
    let msi_route = PciMsiRoute::new(
        endpoint.function(),
        1,
        endpoint.msi_message(1).unwrap().unwrap(),
        route,
        3,
    )
    .unwrap();
    let port = PciMsiPort::new(msi_route, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    scheduler
        .schedule_parallel_at(pci, 4, move |context| {
            assert!(port
                .send_parallel(&endpoint, context, source)
                .unwrap()
                .is_some());
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            7,
            InterruptLineId::new(65),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}
