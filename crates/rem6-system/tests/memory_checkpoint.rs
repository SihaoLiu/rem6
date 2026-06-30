use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramLowPowerTiming, DramMemoryController,
    DramMemoryTechnology, DramTiming, ExternalMemoryProfile, NvmMediaTiming,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};
use rem6_system::{
    DramMemoryCheckpointBank, DramMemoryCheckpointError, DramMemoryCheckpointPort,
    DramMemoryCheckpointRecord, MemoryStoreCheckpointBank, MemoryStoreCheckpointError,
    MemoryStoreCheckpointPort, MemoryStoreCheckpointRecord,
};

const TEST_U64_BYTES: usize = 8;
const TEST_DRAM_TARGET_MIN_RECORD_BYTES: usize = 208;
const TEST_DRAM_BANK_STATE_MIN_RECORD_BYTES: usize = TEST_U64_BYTES * 2;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line_data(base: u8) -> Vec<u8> {
    (0..64).map(|offset| base.wrapping_add(offset)).collect()
}

fn dram_geometry() -> DramGeometry {
    DramGeometry::new(4, 256, 64).unwrap()
}

fn dram_timing() -> DramTiming {
    DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_burst_spacing(2)
        .unwrap()
        .with_command_window(10, 2)
        .unwrap()
}

fn fast_dram_timing() -> DramTiming {
    DramTiming::new(2, 4, 6, 2, 3).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(9), sequence)
}

fn read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(address: u64, bytes: &[u8], sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes.len() as u64).unwrap(),
        bytes.to_vec(),
        ByteMask::full(AccessSize::new(bytes.len() as u64).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write_test_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_test_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn memory_store() -> (PartitionedMemoryStore, MemoryTargetId, MemoryTargetId) {
    let low = MemoryTargetId::new(10);
    let high = MemoryTargetId::new(20);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(high, layout()).unwrap();
    store.add_partition(low, layout()).unwrap();
    store
        .map_region(low, Address::new(0x0000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    store
        .map_region(high, Address::new(0x8000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    store
        .insert_line(low, Address::new(0x1000), line_data(0x10))
        .unwrap();
    store
        .insert_line(high, Address::new(0x8000), line_data(0x80))
        .unwrap();
    (store, low, high)
}

fn empty_store_checkpoint_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    payload
}

fn write_dram_store_header(payload: &mut Vec<u8>) {
    let store = empty_store_checkpoint_payload();
    write_test_u64(payload, store.len() as u64);
    payload.extend_from_slice(&store);
}

fn write_minimal_dram_target_prefix_with_bank_count(payload: &mut Vec<u8>, bank_count: u32) {
    write_test_u32(payload, 30);
    write_test_u32(payload, bank_count);
    write_test_u64(payload, 256);
    write_test_u64(payload, 64);
    write_test_u64(payload, 0);
    write_test_u64(payload, 3);
    write_test_u64(payload, 5);
    write_test_u64(payload, 7);
    write_test_u64(payload, 2);
    write_test_u64(payload, 4);
    write_test_u64(payload, 2);
    write_test_u64(payload, 0);
    write_test_u64(payload, 0);
    write_test_u64(payload, 0);
    write_test_u64(payload, 0);
}

fn write_minimal_dram_target_prefix(payload: &mut Vec<u8>) {
    write_minimal_dram_target_prefix_with_bank_count(payload, 1);
}

fn write_dram_payload_until_nvm_pending_counts_with_target_start(payload: &mut Vec<u8>) -> usize {
    write_dram_store_header(payload);
    write_test_u64(payload, 1);
    let target_start = payload.len();
    write_minimal_dram_target_prefix(payload);
    write_test_u64(payload, 0);
    target_start
}

fn pad_minimal_dram_target_payload(payload: &mut Vec<u8>, target_start: usize) -> usize {
    let target_bytes = payload.len() - target_start;
    let padding = TEST_DRAM_TARGET_MIN_RECORD_BYTES.saturating_sub(target_bytes);
    payload.resize(payload.len() + padding, 0);
    padding
}

fn dram_controller_for_malformed_restore() -> Arc<Mutex<DramMemoryController>> {
    Arc::new(Mutex::new(dram_memory_controller().0))
}

fn dram_memory_controller() -> (DramMemoryController, MemoryTargetId, MemoryTargetId) {
    let low = MemoryTargetId::new(30);
    let high = MemoryTargetId::new(40);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            low,
            layout(),
            dram_geometry(),
            dram_timing(),
        ))
        .unwrap();
    controller
        .add_target(DramControllerConfig::new(
            high,
            layout(),
            dram_geometry(),
            fast_dram_timing(),
        ))
        .unwrap();
    controller
        .map_region(low, Address::new(0x0000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    controller
        .map_region(high, Address::new(0x8000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    controller
        .insert_line(low, Address::new(0x1000), line_data(0x10))
        .unwrap();
    controller
        .insert_line(high, Address::new(0x8000), line_data(0x80))
        .unwrap();
    (controller, low, high)
}

#[test]
fn memory_store_checkpoint_rejects_impossible_partition_count_without_mutating_store() {
    let (store, _low, _high) = memory_store();
    let store = Arc::new(Mutex::new(store));
    let before = store.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("memory_partition_count").unwrap();
    let port = MemoryStoreCheckpointPort::new(component.clone(), Arc::clone(&store));
    let mut payload = Vec::new();
    write_test_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "store", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        MemoryStoreCheckpointError::InvalidChunk {
            component,
            reason:
                "partition count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(store.lock().unwrap().snapshot(), before);
}

#[test]
fn memory_store_checkpoint_rejects_impossible_line_count_without_mutating_store() {
    let (store, _low, _high) = memory_store();
    let store = Arc::new(Mutex::new(store));
    let before = store.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("memory_line_count").unwrap();
    let port = MemoryStoreCheckpointPort::new(component.clone(), Arc::clone(&store));
    let mut payload = Vec::new();
    write_test_u64(&mut payload, 1);
    write_test_u32(&mut payload, 10);
    write_test_u64(&mut payload, 64);
    write_test_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "store", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        MemoryStoreCheckpointError::InvalidChunk {
            component,
            reason: "line count 18446744073709551615 exceeds remaining payload capacity 0 records"
                .to_string(),
        }
    );
    assert_eq!(store.lock().unwrap().snapshot(), before);
}

#[test]
fn memory_store_checkpoint_rejects_impossible_region_count_without_mutating_store() {
    let (store, _low, _high) = memory_store();
    let store = Arc::new(Mutex::new(store));
    let before = store.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("memory_region_count").unwrap();
    let port = MemoryStoreCheckpointPort::new(component.clone(), Arc::clone(&store));
    let mut payload = Vec::new();
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "store", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        MemoryStoreCheckpointError::InvalidChunk {
            component,
            reason:
                "region count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(store.lock().unwrap().snapshot(), before);
}

#[test]
fn memory_store_checkpoint_rejects_impossible_sparse_hole_count_without_mutating_store() {
    let (store, _low, _high) = memory_store();
    let store = Arc::new(Mutex::new(store));
    let before = store.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("memory_sparse_hole_count").unwrap();
    let port = MemoryStoreCheckpointPort::new(component.clone(), Arc::clone(&store));
    let mut payload = Vec::new();
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 1);
    write_test_u32(&mut payload, 10);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0x1000);
    write_test_u64(&mut payload, u64::MAX);
    write_test_u64(&mut payload, 0);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "store", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        MemoryStoreCheckpointError::InvalidChunk {
            component,
            reason:
                "region sparse hole count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(store.lock().unwrap().snapshot(), before);
}

#[test]
fn memory_store_checkpoint_captures_and_restores_partitioned_store() {
    let (store, low, high) = memory_store();
    let store = Arc::new(Mutex::new(store));
    let component = CheckpointComponentId::new("memory0").unwrap();
    let port = MemoryStoreCheckpointPort::new(component.clone(), Arc::clone(&store));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        MemoryStoreCheckpointRecord::new(component.clone(), store.lock().unwrap().snapshot())
    );
    assert!(registry.chunk(&component, "store").unwrap().len() > 128);

    {
        let mut store = store.lock().unwrap();
        store
            .insert_line(low, Address::new(0x1000), line_data(0xaa))
            .unwrap();
        store
            .insert_line(high, Address::new(0x8000), line_data(0x40))
            .unwrap();
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    let store = store.lock().unwrap();
    assert_eq!(
        store.line_data(low, Address::new(0x1000)).unwrap(),
        line_data(0x10)
    );
    assert_eq!(
        store.line_data(high, Address::new(0x8000)).unwrap(),
        line_data(0x80)
    );
    assert_eq!(store.snapshot(), captured.snapshot().clone());
}

#[test]
fn memory_store_checkpoint_rejects_truncated_payload_without_mutating_store() {
    let (store, low, _high) = memory_store();
    let store = Arc::new(Mutex::new(store));
    let original: PartitionedMemorySnapshot = store.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("memory0").unwrap();
    let port = MemoryStoreCheckpointPort::new(component.clone(), Arc::clone(&store));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    port.capture_into(&mut registry).unwrap();
    registry
        .write_chunk(&component, "store", vec![1, 0, 0])
        .unwrap();
    store
        .lock()
        .unwrap()
        .insert_line(low, Address::new(0x1000), line_data(0xaa))
        .unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(error.component(), &component);
    assert_eq!(
        store
            .lock()
            .unwrap()
            .line_data(low, Address::new(0x1000))
            .unwrap(),
        line_data(0xaa)
    );
    assert_ne!(store.lock().unwrap().snapshot(), original);
}

#[test]
fn memory_store_checkpoint_bank_rejects_truncated_payload_without_partial_restore() {
    let (store0, low0, _high0) = memory_store();
    let (store1, low1, _high1) = memory_store();
    let store0 = Arc::new(Mutex::new(store0));
    let store1 = Arc::new(Mutex::new(store1));
    let component0 = CheckpointComponentId::new("memory0").unwrap();
    let component1 = CheckpointComponentId::new("memory1").unwrap();
    let bank = MemoryStoreCheckpointBank::new([
        MemoryStoreCheckpointPort::new(component0.clone(), Arc::clone(&store0)),
        MemoryStoreCheckpointPort::new(component1.clone(), Arc::clone(&store1)),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    registry
        .write_chunk(&component1, "store", vec![1, 0, 0])
        .unwrap();
    store0
        .lock()
        .unwrap()
        .insert_line(low0, Address::new(0x1000), line_data(0xaa))
        .unwrap();
    store1
        .lock()
        .unwrap()
        .insert_line(low1, Address::new(0x1000), line_data(0xbb))
        .unwrap();
    let before0 = store0.lock().unwrap().snapshot();
    let before1 = store1.lock().unwrap().snapshot();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(error.component(), &component1);
    assert_eq!(store0.lock().unwrap().snapshot(), before0);
    assert_eq!(store1.lock().unwrap().snapshot(), before1);
}

#[test]
fn dram_memory_checkpoint_rejects_impossible_target_count_without_mutating_controller() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram_target_count").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = Vec::new();
    write_dram_store_header(&mut payload);
    write_test_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason:
                "DRAM target count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_rejects_impossible_pending_read_count_without_mutating_controller() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram_pending_read_count").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = Vec::new();
    let target_start = write_dram_payload_until_nvm_pending_counts_with_target_start(&mut payload);
    write_test_u64(&mut payload, u64::MAX);
    let capacity = pad_minimal_dram_target_payload(&mut payload, target_start) / TEST_U64_BYTES;
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason: format!(
                "DRAM NVM pending read completion count 18446744073709551615 exceeds remaining payload capacity {capacity} records"
            ),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_rejects_impossible_pending_write_count_without_mutating_controller() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram_pending_write_count").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = Vec::new();
    let target_start = write_dram_payload_until_nvm_pending_counts_with_target_start(&mut payload);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, u64::MAX);
    let capacity = pad_minimal_dram_target_payload(&mut payload, target_start) / TEST_U64_BYTES;
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason: format!(
                "DRAM NVM pending write completion count 18446744073709551615 exceeds remaining payload capacity {capacity} records"
            ),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_rejects_impossible_bank_state_count_without_mutating_controller() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram_bank_state_count").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = Vec::new();
    let target_start = write_dram_payload_until_nvm_pending_counts_with_target_start(&mut payload);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, u64::MAX);
    write_test_u64(&mut payload, u64::MAX);
    let capacity = pad_minimal_dram_target_payload(&mut payload, target_start)
        / TEST_DRAM_BANK_STATE_MIN_RECORD_BYTES;
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason: format!(
                "DRAM bank state count 18446744073709551615 exceeds remaining payload capacity {capacity} records"
            ),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_rejects_bank_state_count_overflow_without_mutating_controller() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram_bank_state_overflow").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = Vec::new();
    write_dram_store_header(&mut payload);
    write_test_u64(&mut payload, 1);
    let target_start = payload.len();
    write_minimal_dram_target_prefix_with_bank_count(&mut payload, 4);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, usize::MAX as u64);
    write_test_u64(&mut payload, 0);
    pad_minimal_dram_target_payload(&mut payload, target_start);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason: "DRAM target 30 bank state count overflows host usize".to_string(),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_rejects_impossible_command_window_count_without_mutating_controller() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram_command_window_count").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = Vec::new();
    let target_start = write_dram_payload_until_nvm_pending_counts_with_target_start(&mut payload);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 1);
    write_test_u64(&mut payload, 1);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, 0);
    write_test_u64(&mut payload, u64::MAX);
    let capacity = pad_minimal_dram_target_payload(&mut payload, target_start) / TEST_U64_BYTES;
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason: format!(
                "DRAM port command window start count 18446744073709551615 exceeds remaining payload capacity {capacity} records"
            ),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_captures_and_restores_controller() {
    let (mut controller, low, high) = dram_memory_controller();
    let first = controller.accept(0, &read(0x1000, 8, 20)).unwrap();
    assert_eq!(first.ready_cycle(), 8);
    assert!(!first.dram_access().row_hit());
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram0").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        DramMemoryCheckpointRecord::new(component.clone(), controller.lock().unwrap().snapshot())
    );
    assert_eq!(
        captured.snapshot().targets()[0]
            .controller()
            .timing()
            .burst_spacing(),
        2
    );
    assert_eq!(
        captured.snapshot().targets()[0].controller().ports()[0].command_window_starts(),
        &[0, 0]
    );
    assert!(registry.chunk(&component, "dram").unwrap().len() > 192);

    {
        let mut controller = controller.lock().unwrap();
        controller
            .accept(8, &write(0x1000, &[0xaa, 0xbb, 0xcc, 0xdd], 21))
            .unwrap();
        controller.accept(0, &read(0x8000, 8, 22)).unwrap();
        assert_eq!(
            &controller.line_data(low, Address::new(0x1000)).unwrap()[..4],
            &[0xaa, 0xbb, 0xcc, 0xdd]
        );
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(
        restored.snapshot().targets()[0]
            .controller()
            .timing()
            .burst_spacing(),
        2
    );
    assert_eq!(
        restored.snapshot().targets()[0].controller().ports()[0].command_window_starts(),
        &[0, 0]
    );
    let mut controller = controller.lock().unwrap();
    assert_eq!(controller.snapshot(), captured.snapshot().clone());
    assert_eq!(
        &controller.line_data(low, Address::new(0x1000)).unwrap()[..4],
        &[0x10, 0x11, 0x12, 0x13]
    );
    assert_eq!(
        &controller.line_data(high, Address::new(0x8000)).unwrap()[..4],
        &[0x80, 0x81, 0x82, 0x83]
    );
    let low_bank = controller
        .dram_controller(low)
        .unwrap()
        .bank_state(0)
        .unwrap();
    assert_eq!(low_bank.open_row(), Some(4));
    assert_eq!(low_bank.available_cycle(), 8);

    let row_hit = controller.accept(8, &read(0x1008, 4, 23)).unwrap();
    assert!(row_hit.dram_access().row_hit());
    assert_eq!(row_hit.dram_access().command_cycle(), 10);
    assert_eq!(row_hit.ready_cycle(), 15);
}

#[test]
fn dram_memory_checkpoint_restores_unaccessed_minimal_target() {
    let target = MemoryTargetId::new(0);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            CacheLineLayout::new(16).unwrap(),
            DramGeometry::new(1, 128, 16).unwrap(),
            DramTiming::new(5, 7, 11, 3, 2).unwrap(),
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(target, Address::new(0x8000), vec![0x5a; 16])
        .unwrap();
    let expected = controller.snapshot();
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram_minimal").unwrap();
    let port = DramMemoryCheckpointPort::new(component, Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();
    {
        let mut controller = controller.lock().unwrap();
        controller
            .insert_line(target, Address::new(0x8010), vec![0xa5; 16])
            .unwrap();
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(captured.snapshot(), &expected);
    assert_eq!(restored.snapshot(), &expected);
    assert_eq!(controller.lock().unwrap().snapshot(), expected);
}

#[test]
fn dram_memory_checkpoint_bank_rejects_truncated_payload_without_partial_restore() {
    let (controller0, low0, _high0) = dram_memory_controller();
    let (controller1, low1, _high1) = dram_memory_controller();
    let controller0 = Arc::new(Mutex::new(controller0));
    let controller1 = Arc::new(Mutex::new(controller1));
    let component0 = CheckpointComponentId::new("dram0").unwrap();
    let component1 = CheckpointComponentId::new("dram1").unwrap();
    let bank = DramMemoryCheckpointBank::new([
        DramMemoryCheckpointPort::new(component0.clone(), Arc::clone(&controller0)),
        DramMemoryCheckpointPort::new(component1.clone(), Arc::clone(&controller1)),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    registry
        .write_chunk(&component1, "dram", vec![1, 0, 0])
        .unwrap();
    {
        let mut controller = controller0.lock().unwrap();
        controller
            .accept(8, &write(0x1000, &[0xaa, 0xbb, 0xcc, 0xdd], 40))
            .unwrap();
    }
    {
        let mut controller = controller1.lock().unwrap();
        controller
            .accept(8, &write(0x1000, &[0x55, 0x66, 0x77, 0x88], 41))
            .unwrap();
    }
    let before0 = controller0.lock().unwrap().snapshot();
    let before1 = controller1.lock().unwrap().snapshot();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(error.component(), &component1);
    assert_eq!(controller0.lock().unwrap().snapshot(), before0);
    assert_eq!(controller1.lock().unwrap().snapshot(), before1);
    assert_eq!(
        &controller0
            .lock()
            .unwrap()
            .line_data(low0, Address::new(0x1000))
            .unwrap()[..4],
        &[0xaa, 0xbb, 0xcc, 0xdd]
    );
    assert_eq!(
        &controller1
            .lock()
            .unwrap()
            .line_data(low1, Address::new(0x1000))
            .unwrap()[..4],
        &[0x55, 0x66, 0x77, 0x88]
    );
}

#[test]
fn dram_memory_checkpoint_preserves_bank_group_burst_history() {
    let target = MemoryTargetId::new(70);
    let geometry = DramGeometry::new(4, 256, 64)
        .unwrap()
        .with_bank_groups(2)
        .unwrap();
    let timing = DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_burst_spacing(2)
        .unwrap()
        .with_same_bank_group_burst_spacing(6)
        .unwrap();
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            geometry,
            timing,
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(target, Address::new(0x0000), line_data(0x10))
        .unwrap();
    controller
        .insert_line(target, Address::new(0x0080), line_data(0x20))
        .unwrap();
    let first = controller.accept(0, &read(0x0000, 8, 34)).unwrap();
    assert_eq!(first.dram_access().command_cycle(), 3);
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram-bank-groups").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();
    let captured_port = &captured.snapshot().targets()[0].controller().ports()[0];
    assert_eq!(captured_port.last_data_command_cycle(), Some(3));
    assert_eq!(captured_port.last_bank_group(), Some(0));

    controller
        .lock()
        .unwrap()
        .accept(0, &read(0x0080, 8, 35))
        .unwrap();
    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    let mut controller = controller.lock().unwrap();
    let same_group = controller.accept(0, &read(0x0080, 8, 36)).unwrap();
    assert_eq!(same_group.dram_access().bank(), 2);
    assert_eq!(same_group.dram_access().command_cycle(), 9);
    assert_eq!(same_group.ready_cycle(), 14);
}

#[test]
fn dram_memory_checkpoint_preserves_low_power_timing() {
    let target = MemoryTargetId::new(72);
    let timing = DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_low_power_timing(
            DramLowPowerTiming::new(20, 80, 7)
                .unwrap()
                .with_self_refresh_exit_latency(17)
                .unwrap(),
        );
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            dram_geometry(),
            timing,
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(target, Address::new(0x0000), line_data(0x10))
        .unwrap();
    controller.accept(0, &read(0x0000, 8, 41)).unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram-low-power").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();
    assert_eq!(
        captured.snapshot().targets()[0]
            .controller()
            .timing()
            .low_power_timing(),
        Some(
            DramLowPowerTiming::new(20, 80, 7)
                .unwrap()
                .with_self_refresh_exit_latency(17)
                .unwrap()
        )
    );

    controller
        .lock()
        .unwrap()
        .accept(120, &read(0x0000, 8, 42))
        .unwrap();
    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(
        controller
            .lock()
            .unwrap()
            .dram_controller(target)
            .unwrap()
            .timing()
            .low_power_timing(),
        Some(
            DramLowPowerTiming::new(20, 80, 7)
                .unwrap()
                .with_self_refresh_exit_latency(17)
                .unwrap()
        )
    );
}

#[test]
fn dram_memory_checkpoint_reads_legacy_shared_low_power_exit_timing() {
    let target = MemoryTargetId::new(73);
    let timing = DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_low_power_timing(
            DramLowPowerTiming::new(20, 80, 7)
                .unwrap()
                .with_self_refresh_exit_latency(17)
                .unwrap(),
        );
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            dram_geometry(),
            timing,
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram-low-power-legacy").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    port.capture_into(&mut registry).unwrap();
    let legacy_payload = legacy_shared_low_power_exit_payload(
        registry.chunk(&component, "dram").unwrap(),
        20,
        80,
        7,
        17,
    );
    registry
        .write_chunk(&component, "dram", legacy_payload)
        .unwrap();
    port.restore_from(&registry).unwrap();

    assert_eq!(
        controller
            .lock()
            .unwrap()
            .dram_controller(target)
            .unwrap()
            .timing()
            .low_power_timing(),
        Some(DramLowPowerTiming::new(20, 80, 7).unwrap())
    );
}

fn legacy_shared_low_power_exit_payload(
    payload: &[u8],
    precharge_powerdown_entry_delay: u64,
    self_refresh_entry_delay: u64,
    powerdown_exit_latency: u64,
    self_refresh_exit_latency: u64,
) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&2u64.to_le_bytes());
    encoded.extend_from_slice(&precharge_powerdown_entry_delay.to_le_bytes());
    encoded.extend_from_slice(&self_refresh_entry_delay.to_le_bytes());
    encoded.extend_from_slice(&powerdown_exit_latency.to_le_bytes());
    encoded.extend_from_slice(&self_refresh_exit_latency.to_le_bytes());
    let start = payload
        .windows(encoded.len())
        .position(|window| window == encoded)
        .unwrap();

    let mut legacy = Vec::new();
    legacy.extend_from_slice(&payload[..start]);
    legacy.extend_from_slice(&1u64.to_le_bytes());
    legacy.extend_from_slice(&precharge_powerdown_entry_delay.to_le_bytes());
    legacy.extend_from_slice(&self_refresh_entry_delay.to_le_bytes());
    legacy.extend_from_slice(&powerdown_exit_latency.to_le_bytes());
    legacy.extend_from_slice(&payload[start + encoded.len()..]);
    legacy
}

#[test]
fn dram_memory_checkpoint_preserves_profiled_parallel_ports() {
    let target = MemoryTargetId::new(50);
    let media_timing = NvmMediaTiming::new(30, 50, 6, 4, 1).unwrap();
    let profile =
        ExternalMemoryProfile::nvm(target, layout(), 2, 8, dram_geometry(), dram_timing())
            .unwrap()
            .with_nvm_media_timing(media_timing)
            .unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(target, Address::new(0x0000), line_data(0x10))
        .unwrap();
    controller
        .insert_line(target, Address::new(0x0040), line_data(0x20))
        .unwrap();
    let first = controller.accept(0, &read(0x0000, 8, 30)).unwrap();
    let second = controller
        .accept(0, &write(0x0040, &[0xaa, 0xbb, 0xcc, 0xdd], 31))
        .unwrap();
    assert_eq!(first.dram_access().parallel_port(), 0);
    assert_eq!(second.dram_access().parallel_port(), 1);
    assert_eq!(second.ready_cycle(), 9);
    assert_eq!(second.dram_access().persistent_ready_cycle(), Some(59));
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram-profiled").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();
    let captured_target = captured
        .snapshot()
        .targets()
        .iter()
        .find(|target_snapshot| target_snapshot.target() == target)
        .unwrap();
    assert_eq!(
        captured_target.controller().nvm_pending_read_completions(),
        &[39]
    );
    assert_eq!(
        captured_target.controller().nvm_pending_write_completions(),
        &[59]
    );

    {
        let mut controller = controller.lock().unwrap();
        controller
            .accept(14, &write(0x0000, &[0x55, 0x66, 0x77, 0x88], 32))
            .unwrap();
        assert_eq!(
            &controller.line_data(target, Address::new(0x0000)).unwrap()[..4],
            &[0x55, 0x66, 0x77, 0x88]
        );
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    let restored_target = restored
        .snapshot()
        .targets()
        .iter()
        .find(|target_snapshot| target_snapshot.target() == target)
        .unwrap();
    assert_eq!(
        restored_target.controller().nvm_pending_read_completions(),
        &[39]
    );
    let mut controller = controller.lock().unwrap();
    assert_eq!(controller.memory_profile(target).unwrap(), &profile);
    assert_eq!(
        controller.memory_profile(target).unwrap().technology(),
        DramMemoryTechnology::Nvm,
    );
    assert_eq!(
        controller
            .memory_profile(target)
            .unwrap()
            .nvm_media_timing(),
        Some(media_timing),
    );
    assert_eq!(
        controller
            .dram_controller(target)
            .unwrap()
            .parallel_port_count(),
        2
    );
    assert_eq!(
        &controller.line_data(target, Address::new(0x0000)).unwrap()[..4],
        &[0x10, 0x11, 0x12, 0x13]
    );
    let row_hit = controller.accept(14, &read(0x0040, 4, 33)).unwrap();
    assert_eq!(row_hit.dram_access().parallel_port(), 1);
    assert!(row_hit.dram_access().row_hit());
    assert_eq!(row_hit.ready_cycle(), 95);
}
