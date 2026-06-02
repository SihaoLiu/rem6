use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_net::{
    EthernetPacket, SinicDataDescriptor, SinicError, SinicFifoDevice, SinicInterrupts,
    SinicRegisterBlock, SinicRegisterParams,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SinicFifoCheckpointBank,
    SinicFifoCheckpointError, SinicFifoCheckpointPort, SinicFifoCheckpointRecord,
    SinicRegisterCheckpointBank, SinicRegisterCheckpointError, SinicRegisterCheckpointPort,
    SinicRegisterCheckpointRecord, SystemActionExecutor, SystemActionOutcome, SystemError,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

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

fn fifo_device() -> Arc<Mutex<SinicFifoDevice>> {
    let params = SinicRegisterParams::default()
        .with_zero_copy(true)
        .with_fifo_limits(48, 48, 4, 4, 16, 16)
        .with_interrupt_mask(
            SinicInterrupts::RX_PACKET
                | SinicInterrupts::RX_DMA
                | SinicInterrupts::TX_DMA
                | SinicInterrupts::TX_FULL,
        );
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN
                | SinicRegisterBlock::CONFIG_RX_EN
                | SinicRegisterBlock::CONFIG_TX_EN
                | SinicRegisterBlock::CONFIG_ZERO_COPY,
            1,
        )
        .unwrap();
    device
        .receive_from_wire(packet(&[1, 2, 3, 4, 5, 6, 7, 8]), 2, 3)
        .unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x1000, 4).unwrap())
        .unwrap()
        .expect("pending receive DMA copy");
    device.complete_rx_dma_copy(3, 4).unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x2000, 4).unwrap())
        .unwrap()
        .expect("pending second receive DMA copy");
    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x3000, 3).unwrap().with_more(true))
        .unwrap();
    device.complete_tx_dma_copy(&[9, 8, 7], 4, 5).unwrap();
    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x4000, 2).unwrap())
        .unwrap();
    Arc::new(Mutex::new(device))
}

fn clear_fifo_device(device: &Arc<Mutex<SinicFifoDevice>>) {
    let params = device.lock().unwrap().registers().params();
    *device.lock().unwrap() = SinicFifoDevice::new(params).unwrap();
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

fn corrupt_chunk_version(
    manifest: &CheckpointManifest,
    component: &CheckpointComponentId,
    chunk_name: &str,
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
                        if state.component() == component && chunk.name() == chunk_name {
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

fn corrupt_sinic_snapshot_version(
    manifest: &CheckpointManifest,
    component: &CheckpointComponentId,
) -> CheckpointManifest {
    corrupt_chunk_version(manifest, component, "sinic-register")
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

#[test]
fn sinic_fifo_checkpoint_captures_and_restores_snapshot() {
    let device = fifo_device();
    let component = CheckpointComponentId::new("net.sinic0.fifo").unwrap();
    let port = SinicFifoCheckpointPort::new(component.clone(), Arc::clone(&device));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        SinicFifoCheckpointRecord::new(component.clone(), device.lock().unwrap().snapshot())
    );
    assert!(registry.chunk(&component, "sinic-fifo").unwrap().len() > 64);

    clear_fifo_device(&device);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(*restored.snapshot(), device.lock().unwrap().snapshot());
}

#[test]
fn system_action_executor_rejects_malformed_sinic_fifo_snapshot() {
    let device = fifo_device();
    let component = CheckpointComponentId::new("net.sinic0.fifo").unwrap();
    let bank = SinicFifoCheckpointBank::new([SinicFifoCheckpointPort::new(
        component.clone(),
        Arc::clone(&device),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_sinic_fifo_checkpoint_bank(bank).unwrap();
    let manifest = match executor.apply(&checkpoint_record("sinic-fifo")).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "sinic-fifo")
        .unwrap()
        .to_vec();
    clear_fifo_device(&device);
    let cleared = device.lock().unwrap().snapshot();
    let corrupted = corrupt_chunk_version(&manifest, &component, "sinic-fifo");

    assert_eq!(
        executor.apply(&restore_record(corrupted)).unwrap_err(),
        SystemError::SinicFifoCheckpoint(SinicFifoCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: SinicError::InvalidSnapshotPayload {
                reason: "unsupported SINIC FIFO snapshot version 2".to_string(),
            }
            .to_string(),
        })
    );
    assert_eq!(cleared, device.lock().unwrap().snapshot());
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "sinic-fifo")
            .unwrap(),
        original_payload
    );
}
