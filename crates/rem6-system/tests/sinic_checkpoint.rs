use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_net::{SinicError, SinicInterrupts, SinicRegisterBlock, SinicRegisterParams};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SinicRegisterCheckpointBank,
    SinicRegisterCheckpointError, SinicRegisterCheckpointPort, SinicRegisterCheckpointRecord,
    SystemActionExecutor, SystemActionOutcome, SystemError,
};

fn register_block() -> Arc<Mutex<SinicRegisterBlock>> {
    let params = SinicRegisterParams::default()
        .with_interrupt_mask(SinicInterrupts::SOFT | SinicInterrupts::RX_PACKET)
        .with_hardware_address(0x00aa_bbcc_ddee);
    let mut registers = SinicRegisterBlock::new(params).unwrap();
    registers
        .change_config(
            SinicRegisterBlock::CONFIG_RX_EN | SinicRegisterBlock::CONFIG_INT_EN,
            10,
        )
        .unwrap();
    registers
        .post_interrupt(SinicInterrupts::RX_PACKET, 12, 5)
        .unwrap();
    Arc::new(Mutex::new(registers))
}

fn clear_register_block(registers: &Arc<Mutex<SinicRegisterBlock>>) {
    let mut registers = registers.lock().unwrap();
    registers
        .clear_interrupts(SinicInterrupts::SOFT | SinicInterrupts::RX_PACKET)
        .unwrap();
    let params = registers.params();
    *registers = SinicRegisterBlock::new(params).unwrap();
}

fn checkpoint_record(label: &str) -> HostActionRecord {
    HostActionRecord::new(
        21,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(7),
        GuestSourceId::new(3),
        HostAction::Checkpoint {
            label: label.to_string(),
        },
    )
}

fn restore_record(manifest: CheckpointManifest) -> HostActionRecord {
    HostActionRecord::new(
        27,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(8),
        GuestSourceId::new(3),
        HostAction::RestoreCheckpoint { manifest },
    )
}

fn corrupt_sinic_snapshot_version(
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
                        if state.component() == component && chunk.name() == "sinic-register" {
                            payload[0..4].copy_from_slice(&2_u32.to_le_bytes());
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
fn sinic_register_checkpoint_captures_and_restores_snapshot() {
    let registers = register_block();
    let component = CheckpointComponentId::new("net.sinic0.registers").unwrap();
    let port = SinicRegisterCheckpointPort::new(component.clone(), Arc::clone(&registers));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        SinicRegisterCheckpointRecord::new(component.clone(), registers.lock().unwrap().snapshot())
    );
    assert!(registry.chunk(&component, "sinic-register").unwrap().len() > 32);

    clear_register_block(&registers);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(*restored.snapshot(), registers.lock().unwrap().snapshot());
}

#[test]
fn system_action_executor_rejects_malformed_sinic_register_snapshot() {
    let registers = register_block();
    let component = CheckpointComponentId::new("net.sinic0.registers").unwrap();
    let bank = SinicRegisterCheckpointBank::new([SinicRegisterCheckpointPort::new(
        component.clone(),
        Arc::clone(&registers),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_sinic_register_checkpoint_bank(bank)
        .unwrap();
    let manifest = match executor
        .apply(&checkpoint_record("sinic-register"))
        .unwrap()
    {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "sinic-register")
        .unwrap()
        .to_vec();
    clear_register_block(&registers);
    let cleared = registers.lock().unwrap().snapshot();
    let corrupted = corrupt_sinic_snapshot_version(&manifest, &component);

    assert_eq!(
        executor.apply(&restore_record(corrupted)).unwrap_err(),
        SystemError::SinicRegisterCheckpoint(SinicRegisterCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: SinicError::InvalidSnapshotPayload {
                reason: "unsupported SINIC register snapshot version 2".to_string(),
            }
            .to_string(),
        })
    );
    assert_eq!(cleared, registers.lock().unwrap().snapshot());
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "sinic-register")
            .unwrap(),
        original_payload
    );
}
