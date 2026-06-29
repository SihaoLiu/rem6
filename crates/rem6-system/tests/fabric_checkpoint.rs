use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_fabric::{
    FabricError, FabricLinkId, FabricModel, FabricPacket, FabricPacketId, FabricPath,
    FabricPathHop, FabricRouterId, FabricRouterStage, VirtualNetworkId,
};
use rem6_kernel::PartitionId;
use rem6_stats::StatsRegistry;
use rem6_system::{
    FabricCheckpointBank, FabricCheckpointError, FabricCheckpointPort, GuestEventId, GuestSourceId,
    HostAction, HostActionRecord, SystemActionExecutor, SystemActionOutcome, SystemError,
};

const FABRIC_CHUNK: &str = "fabric";

fn packet(id: u64, bytes: u64, virtual_network: u16) -> FabricPacket {
    FabricPacket::new(
        FabricPacketId::new(id),
        bytes,
        VirtualNetworkId::new(virtual_network),
    )
    .unwrap()
}

fn link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn router(name: &str) -> FabricRouterId {
    FabricRouterId::new(name).unwrap()
}

fn route() -> FabricPath {
    FabricPath::new([FabricPathHop::new(link("fabric_checkpoint"), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()])
    .unwrap()
}

fn router_route() -> FabricPath {
    FabricPath::new([
        FabricPathHop::new(link("fabric_checkpoint_router_out"), 2, 8)
            .unwrap()
            .with_router_stage(
                FabricRouterStage::new(router("fabric_checkpoint_router"), 0, 1, 0, 3).unwrap(),
            ),
    ])
    .unwrap()
}

fn checkpoint_record(source: GuestSourceId) -> HostActionRecord {
    HostActionRecord::new(
        24,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(400),
        source,
        HostAction::Checkpoint {
            label: "fabric-ready".to_string(),
        },
    )
}

fn restore_record(source: GuestSourceId, outcome: &SystemActionOutcome) -> HostActionRecord {
    let SystemActionOutcome::Checkpoint { manifest, .. } = outcome else {
        panic!("checkpoint outcome expected");
    };
    HostActionRecord::new(
        40,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(401),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    )
}

fn duplicate_lane_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, 1);
    write_u64(&mut payload, 2);
    write_lane(&mut payload, "fabric_duplicate", 1, 12, &[30, 20]);
    write_lane(&mut payload, "fabric_duplicate", 1, 18, &[40]);
    payload
}

fn write_lane(
    payload: &mut Vec<u8>,
    link: &str,
    virtual_network: u32,
    next_available_tick: u64,
    credits: &[u64],
) {
    write_string(payload, link);
    write_u32(payload, virtual_network);
    write_u64(payload, next_available_tick);
    write_u64(payload, credits.len() as u64);
    for credit in credits {
        write_u64(payload, *credit);
    }
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_string(payload: &mut Vec<u8>, value: &str) {
    write_u64(payload, value.len() as u64);
    payload.extend_from_slice(value.as_bytes());
}

#[test]
fn host_checkpoint_refreshes_and_restores_fabric_state() {
    let component = CheckpointComponentId::new("fabric0").unwrap();
    let path = route();
    let mut live_fabric = FabricModel::new();
    live_fabric
        .transmit_batch(
            0,
            [
                (packet(1, 8, 1), path.clone()),
                (packet(2, 8, 1), path.clone()),
            ],
        )
        .unwrap();
    let fabric_snapshot = live_fabric.lane_snapshots();
    let mut expected = live_fabric.clone();
    let expected_transfer = expected.transmit(1, packet(3, 8, 1), path.clone()).unwrap();
    let fabric = Arc::new(Mutex::new(live_fabric));
    let bank = FabricCheckpointBank::new([FabricCheckpointPort::new(
        component.clone(),
        Arc::clone(&fabric),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_fabric_checkpoint_bank(bank).unwrap();
    let source = GuestSourceId::new(80);

    let checkpoint = executor.apply(&checkpoint_record(source)).unwrap();
    let SystemActionOutcome::Checkpoint { manifest, .. } = &checkpoint else {
        panic!("checkpoint outcome expected");
    };
    assert!(manifest
        .states()
        .iter()
        .any(|state| state.component() == &component));
    assert!(
        executor
            .checkpoints()
            .chunk(&component, "fabric")
            .unwrap()
            .len()
            > 32
    );

    fabric
        .lock()
        .unwrap()
        .transmit(20, packet(9, 8, 1), path.clone())
        .unwrap();
    assert_ne!(fabric.lock().unwrap().lane_snapshots(), fabric_snapshot);

    let restore = restore_record(source, &checkpoint);
    executor.apply(&restore).unwrap();

    assert_eq!(fabric.lock().unwrap().lane_snapshots(), fabric_snapshot);
    let replayed = fabric
        .lock()
        .unwrap()
        .transmit(1, packet(3, 8, 1), path)
        .unwrap();
    assert_eq!(replayed, expected_transfer);
    assert_eq!(
        fabric.lock().unwrap().lane_snapshots(),
        expected.lane_snapshots()
    );
}

#[test]
fn host_checkpoint_refreshes_and_restores_fabric_router_state() {
    let component = CheckpointComponentId::new("fabric-router").unwrap();
    let path = router_route();
    let mut live_fabric = FabricModel::new();
    live_fabric
        .transmit_batch(
            0,
            [
                (packet(1, 8, 1), path.clone()),
                (packet(2, 8, 1), path.clone()),
            ],
        )
        .unwrap();
    let fabric_snapshot = live_fabric.snapshot();
    let mut expected = live_fabric.clone();
    let expected_transfer = expected.transmit(1, packet(3, 8, 1), path.clone()).unwrap();
    assert_eq!(
        expected_transfer.hops()[0]
            .router()
            .unwrap()
            .queue_delay_ticks(),
        5
    );
    let fabric = Arc::new(Mutex::new(live_fabric));
    let bank = FabricCheckpointBank::new([FabricCheckpointPort::new(
        component.clone(),
        Arc::clone(&fabric),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_fabric_checkpoint_bank(bank).unwrap();
    let source = GuestSourceId::new(180);

    let checkpoint = executor.apply(&checkpoint_record(source)).unwrap();
    fabric
        .lock()
        .unwrap()
        .transmit(20, packet(9, 8, 1), path.clone())
        .unwrap();
    assert_ne!(fabric.lock().unwrap().snapshot(), fabric_snapshot);

    let restore = restore_record(source, &checkpoint);
    executor.apply(&restore).unwrap();

    assert_eq!(fabric.lock().unwrap().snapshot(), fabric_snapshot);
    let replayed = fabric
        .lock()
        .unwrap()
        .transmit(1, packet(3, 8, 1), path)
        .unwrap();
    assert_eq!(replayed, expected_transfer);
    assert_eq!(fabric.lock().unwrap().snapshot(), expected.snapshot());
}

#[test]
fn host_checkpoint_rejects_invalid_fabric_chunk_without_mutating_state() {
    let component = CheckpointComponentId::new("fabric0").unwrap();
    let path = route();
    let mut live_fabric = FabricModel::new();
    live_fabric
        .transmit(0, packet(1, 8, 1), path.clone())
        .unwrap();
    let fabric = Arc::new(Mutex::new(live_fabric));
    let bank = FabricCheckpointBank::new([FabricCheckpointPort::new(
        component.clone(),
        Arc::clone(&fabric),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_fabric_checkpoint_bank(bank).unwrap();
    executor
        .checkpoints_mut()
        .write_chunk(&component, "fabric", vec![1, 2, 3])
        .unwrap();
    let manifest = executor.checkpoints().capture("bad-fabric", 30).unwrap();
    let before = fabric.lock().unwrap().lane_snapshots();
    let source = GuestSourceId::new(81);
    let restore = HostActionRecord::new(
        42,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(402),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );

    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::FabricCheckpoint(FabricCheckpointError::InvalidChunk {
            component,
            reason: "fabric checkpoint version is truncated".to_string(),
        })
    );
    assert_eq!(fabric.lock().unwrap().lane_snapshots(), before);
}

#[test]
fn host_checkpoint_rejects_duplicate_fabric_lanes_without_mutating_state() {
    let component = CheckpointComponentId::new("fabric0").unwrap();
    let path = route();
    let mut live_fabric = FabricModel::new();
    live_fabric
        .transmit(0, packet(1, 8, 1), path.clone())
        .unwrap();
    let fabric = Arc::new(Mutex::new(live_fabric));
    let bank = FabricCheckpointBank::new([FabricCheckpointPort::new(
        component.clone(),
        Arc::clone(&fabric),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_fabric_checkpoint_bank(bank).unwrap();
    executor
        .checkpoints_mut()
        .write_chunk(&component, "fabric", duplicate_lane_payload())
        .unwrap();
    let manifest = executor
        .checkpoints()
        .capture("duplicate-fabric-lane", 32)
        .unwrap();
    let before = fabric.lock().unwrap().lane_snapshots();
    let source = GuestSourceId::new(82);
    let restore = HostActionRecord::new(
        44,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(403),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );

    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::FabricCheckpoint(FabricCheckpointError::Fabric {
            component,
            error: FabricError::DuplicateLaneSnapshot {
                link: link("fabric_duplicate"),
                virtual_network: VirtualNetworkId::new(1),
            },
        })
    );
    assert_eq!(fabric.lock().unwrap().lane_snapshots(), before);
}

#[test]
fn fabric_checkpoint_port_rejects_impossible_lane_count_without_mutating_state() {
    let component = CheckpointComponentId::new("fabric_lane_count").unwrap();
    let fabric = Arc::new(Mutex::new(FabricModel::new()));
    let before = fabric.lock().unwrap().lane_snapshots();
    let mut payload = Vec::new();
    write_u64(&mut payload, 1);
    write_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry
        .write_chunk(&component, FABRIC_CHUNK, payload)
        .unwrap();

    let error = FabricCheckpointPort::new(component.clone(), Arc::clone(&fabric))
        .restore_from(&registry)
        .unwrap_err();

    assert_eq!(
        error,
        FabricCheckpointError::InvalidChunk {
            component,
            reason:
                "fabric lane count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(fabric.lock().unwrap().lane_snapshots(), before);
}

#[test]
fn fabric_checkpoint_port_rejects_impossible_credit_count_without_mutating_state() {
    let component = CheckpointComponentId::new("fabric_credit_count").unwrap();
    let fabric = Arc::new(Mutex::new(FabricModel::new()));
    let before = fabric.lock().unwrap().lane_snapshots();
    let mut payload = Vec::new();
    write_u64(&mut payload, 1);
    write_u64(&mut payload, 1);
    write_string(&mut payload, "fabric_credit_count");
    write_u32(&mut payload, 1);
    write_u64(&mut payload, 10);
    write_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry
        .write_chunk(&component, FABRIC_CHUNK, payload)
        .unwrap();

    let error = FabricCheckpointPort::new(component.clone(), Arc::clone(&fabric))
        .restore_from(&registry)
        .unwrap_err();

    assert_eq!(
        error,
        FabricCheckpointError::InvalidChunk {
            component,
            reason:
                "fabric credit return count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(fabric.lock().unwrap().lane_snapshots(), before);
}

#[test]
fn fabric_checkpoint_bank_rejects_truncated_payload_without_partial_restore() {
    let component0 = CheckpointComponentId::new("fabric0").unwrap();
    let component1 = CheckpointComponentId::new("fabric1").unwrap();
    let path = route();
    let mut live_fabric0 = FabricModel::new();
    let mut live_fabric1 = FabricModel::new();
    live_fabric0
        .transmit(0, packet(1, 8, 1), path.clone())
        .unwrap();
    live_fabric1
        .transmit(0, packet(2, 8, 1), path.clone())
        .unwrap();
    let fabric0 = Arc::new(Mutex::new(live_fabric0));
    let fabric1 = Arc::new(Mutex::new(live_fabric1));
    let bank = FabricCheckpointBank::new([
        FabricCheckpointPort::new(component0.clone(), Arc::clone(&fabric0)),
        FabricCheckpointPort::new(component1.clone(), Arc::clone(&fabric1)),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    registry
        .write_chunk(&component1, "fabric", vec![1, 2, 3])
        .unwrap();
    fabric0
        .lock()
        .unwrap()
        .transmit(20, packet(10, 8, 1), path.clone())
        .unwrap();
    fabric1
        .lock()
        .unwrap()
        .transmit(20, packet(11, 8, 1), path)
        .unwrap();
    let before0 = fabric0.lock().unwrap().lane_snapshots();
    let before1 = fabric1.lock().unwrap().lane_snapshots();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(error.component(), Some(&component1));
    assert_eq!(fabric0.lock().unwrap().lane_snapshots(), before0);
    assert_eq!(fabric1.lock().unwrap().lane_snapshots(), before1);
}

#[test]
fn fabric_checkpoint_bank_rejects_duplicate_components() {
    let component = CheckpointComponentId::new("fabric0").unwrap();
    let fabric = Arc::new(Mutex::new(FabricModel::new()));

    assert!(FabricCheckpointBank::new([
        FabricCheckpointPort::new(component.clone(), Arc::clone(&fabric)),
        FabricCheckpointPort::new(component, fabric),
    ])
    .is_err());
}
