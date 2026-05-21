use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_coherence::{MsiBankDirectoryHarness, SubmitKind};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};
use rem6_protocol_msi::MsiState;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, MsiBankCheckpointBank,
    MsiBankCheckpointPort, MsiBankCheckpointRecord, SystemActionExecutor, SystemActionOutcome,
};

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn request_id(agent_id: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(agent_id), sequence)
}

fn read(agent_id: u32, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn write(agent_id: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = size(data.len() as u64);
    MemoryRequest::write(
        request_id(agent_id, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn line_data(byte: u8) -> Vec<u8> {
    vec![byte; layout().bytes() as usize]
}

fn harness() -> MsiBankDirectoryHarness {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
}

fn warm_harness(harness: &mut MsiBankDirectoryHarness) {
    harness
        .submit_cpu_request(agent(1), write(1, 10, 0x1004, vec![0xaa; 8]))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 20, 0x1004))
        .unwrap();
    harness
        .submit_cpu_request(agent(1), read(1, 30, 0x1018))
        .unwrap();
}

#[test]
fn msi_bank_checkpoint_captures_and_restores_harness_state() {
    let mut live = harness();
    warm_harness(&mut live);
    let live = Arc::new(Mutex::new(live));
    let component = CheckpointComponentId::new("l1d-msi").unwrap();
    let port = MsiBankCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        MsiBankCheckpointRecord::new(component.clone(), live.lock().unwrap().snapshot())
    );
    assert!(registry.chunk(&component, "msi-bank").unwrap().len() > 128);

    {
        let mut live = live.lock().unwrap();
        live.submit_cpu_request(agent(2), write(2, 40, 0x1004, vec![0xcc; 8]))
            .unwrap();
        live.submit_cpu_request(agent(2), read(2, 41, 0x1018))
            .unwrap();
        assert_ne!(live.snapshot(), captured.snapshot().clone());
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    let mut live = live.lock().unwrap();
    assert_eq!(live.snapshot(), captured.snapshot().clone());
    assert_eq!(
        live.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        live.cache_state(agent(2), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );

    let hit = live
        .submit_cpu_request(agent(2), read(2, 50, 0x1004))
        .unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        live.cpu_responses().last().unwrap().data().unwrap(),
        &[0xaa; 8]
    );
}

#[test]
fn system_action_executor_checkpoints_and_restores_msi_bank() {
    let mut live = harness();
    warm_harness(&mut live);
    let expected = live.snapshot();
    let live = Arc::new(Mutex::new(live));
    let component = CheckpointComponentId::new("l1d-msi").unwrap();
    let bank = MsiBankCheckpointBank::new([MsiBankCheckpointPort::new(
        component.clone(),
        Arc::clone(&live),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    let mut executor = SystemActionExecutor::with_msi_bank_checkpoint_bank(
        StatsRegistry::new(),
        checkpoints,
        bank,
    );

    let checkpoint = HostActionRecord::new(
        32,
        rem6_kernel::PartitionId::new(0),
        rem6_kernel::PartitionId::new(1),
        GuestEventId::new(1),
        GuestSourceId::new(7),
        HostAction::Checkpoint {
            label: "coherence-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(executor
        .checkpoints()
        .chunk(&component, "msi-bank")
        .is_some());

    {
        let mut live = live.lock().unwrap();
        live.submit_cpu_request(agent(2), write(2, 60, 0x1004, vec![0xdd; 8]))
            .unwrap();
        assert_ne!(live.snapshot(), expected);
    }

    let restore = HostActionRecord::new(
        48,
        rem6_kernel::PartitionId::new(0),
        rem6_kernel::PartitionId::new(1),
        GuestEventId::new(2),
        GuestSourceId::new(7),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 48,
            event: GuestEventId::new(2),
            source: GuestSourceId::new(7),
            manifest,
        }
    );
    assert_eq!(live.lock().unwrap().snapshot(), expected);
}

#[test]
fn msi_bank_checkpoint_preserves_parallel_cycle_history() {
    let mut live = harness();
    let recorded = live
        .submit_parallel_cycle(
            72,
            [
                (agent(2), read(2, 70, 0x1018)),
                (agent(1), read(1, 71, 0x1004)),
            ],
        )
        .unwrap();
    let expected = live.snapshot();
    assert_eq!(
        expected.parallel_cycle_runs(),
        std::slice::from_ref(&recorded)
    );

    let live = Arc::new(Mutex::new(live));
    let component = CheckpointComponentId::new("l1d-msi").unwrap();
    let port = MsiBankCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();
    port.capture_into(&mut registry).unwrap();

    {
        let mut live = live.lock().unwrap();
        live.submit_parallel_cycle(88, [(agent(1), write(1, 80, 0x1004, vec![0xdd; 8]))])
            .unwrap();
        assert_ne!(live.parallel_cycle_runs(), expected.parallel_cycle_runs());
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored.snapshot(), &expected);
    assert_eq!(restored.parallel_cycle_history().cycle_count(), 1);
    assert_eq!(restored.parallel_cycle_history().total_accepted(), 2);
    assert_eq!(restored.parallel_cycle_history().total_responses(), 2);
    assert_eq!(
        restored.parallel_cycle_history().max_accepted_per_cycle(),
        2
    );
    assert!(restored.parallel_cycle_history().has_parallel_work());
    assert_eq!(
        restored.snapshot().parallel_cycle_runs(),
        std::slice::from_ref(&recorded)
    );
    assert_eq!(live.lock().unwrap().parallel_cycle_runs(), &[recorded]);
}
