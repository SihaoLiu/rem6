use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_interrupt::{InterruptController, InterruptLineId, InterruptTargetId};
use rem6_kernel::PartitionId;
use rem6_pci::{
    PciError, PciFunctionAddress, PciInterruptPin, PciLegacyInterruptMapper,
    PciLegacyInterruptPolicy, PciLegacyInterruptRouter, PciLegacyInterruptRoutingEntry,
    PciLegacyInterruptRoutingTable,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord,
    PciLegacyInterruptRouterCheckpointBank, PciLegacyInterruptRouterCheckpointError,
    PciLegacyInterruptRouterCheckpointPort, PciLegacyInterruptRouterCheckpointRecord,
    SystemActionExecutor, SystemActionOutcome, SystemError,
};

fn root_function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 1, 0).unwrap()
}

fn mapper() -> PciLegacyInterruptMapper {
    PciLegacyInterruptMapper::new(
        InterruptLineId::new(32),
        4,
        PciLegacyInterruptPolicy::DeviceModulo,
    )
    .unwrap()
}

fn routing_entry(pin: PciInterruptPin, line: u64) -> PciLegacyInterruptRoutingEntry {
    PciLegacyInterruptRoutingEntry::new(root_function(), pin, InterruptLineId::new(line)).unwrap()
}

fn router() -> Arc<Mutex<PciLegacyInterruptRouter>> {
    let table = PciLegacyInterruptRoutingTable::new(mapper())
        .with_entry(routing_entry(PciInterruptPin::IntC, 48))
        .unwrap();
    Arc::new(Mutex::new(
        PciLegacyInterruptRouter::new(
            table,
            InterruptTargetId::new(7),
            PartitionId::new(0),
            2,
            Arc::new(Mutex::new(InterruptController::new())),
        )
        .unwrap(),
    ))
}

fn insert_intd_override(router: &Arc<Mutex<PciLegacyInterruptRouter>>, line: u64) {
    router
        .lock()
        .unwrap()
        .insert_entry(routing_entry(PciInterruptPin::IntD, line))
        .unwrap();
}

fn router_line(router: &Arc<Mutex<PciLegacyInterruptRouter>>, pin: PciInterruptPin) -> u64 {
    router
        .lock()
        .unwrap()
        .line(root_function(), pin)
        .unwrap()
        .get()
}

fn corrupt_router_snapshot_latency(
    manifest: &CheckpointManifest,
    component: &CheckpointComponentId,
) -> CheckpointManifest {
    CheckpointManifest::new(
        manifest.label().to_string(),
        manifest.tick(),
        manifest
            .states()
            .iter()
            .map(|state| {
                let chunks = state
                    .chunks()
                    .iter()
                    .map(|chunk| {
                        let mut payload = chunk.payload().to_vec();
                        if state.component() == component && chunk.name() == "legacy-intx-router" {
                            payload[18..26].copy_from_slice(&0_u64.to_le_bytes());
                        }
                        CheckpointChunk::new(chunk.name().to_string(), payload)
                    })
                    .collect();
                CheckpointState::new(state.component().clone(), chunks)
            })
            .collect(),
    )
}

#[test]
fn pci_legacy_interrupt_router_checkpoint_captures_and_restores_snapshot() {
    let router = router();
    let component = CheckpointComponentId::new("pci.intx0").unwrap();
    let port = PciLegacyInterruptRouterCheckpointPort::new(component.clone(), Arc::clone(&router));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        PciLegacyInterruptRouterCheckpointRecord::new(
            component.clone(),
            router.lock().unwrap().snapshot(),
        )
    );
    assert!(
        registry
            .chunk(&component, "legacy-intx-router")
            .unwrap()
            .len()
            > 32
    );

    insert_intd_override(&router, 60);
    assert_eq!(router_line(&router, PciInterruptPin::IntD), 60);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(router_line(&router, PciInterruptPin::IntC), 48);
    assert_eq!(router_line(&router, PciInterruptPin::IntD), 33);
}

#[test]
fn system_action_executor_rejects_malformed_pci_legacy_interrupt_router_snapshot() {
    let router = router();
    let component = CheckpointComponentId::new("pci.intx0").unwrap();
    let bank =
        PciLegacyInterruptRouterCheckpointBank::new([PciLegacyInterruptRouterCheckpointPort::new(
            component.clone(),
            Arc::clone(&router),
        )])
        .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_pci_legacy_interrupt_router_checkpoint_bank(bank)
        .unwrap();
    let checkpoint = HostActionRecord::new(
        21,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(7),
        GuestSourceId::new(3),
        HostAction::Checkpoint {
            label: "pci-intx".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "legacy-intx-router")
        .unwrap()
        .to_vec();
    insert_intd_override(&router, 60);
    let corrupted = corrupt_router_snapshot_latency(&manifest, &component);
    let restore = HostActionRecord::new(
        27,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(8),
        GuestSourceId::new(3),
        HostAction::RestoreCheckpoint {
            manifest: corrupted,
        },
    );

    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::PciLegacyInterruptRouterCheckpoint(
            PciLegacyInterruptRouterCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: PciError::InvalidLegacyInterruptRouterSnapshot.to_string(),
            }
        )
    );
    assert_eq!(router_line(&router, PciInterruptPin::IntD), 60);
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "legacy-intx-router")
            .unwrap(),
        original_payload
    );
}
