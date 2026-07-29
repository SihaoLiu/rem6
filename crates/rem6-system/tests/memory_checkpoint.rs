use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramLowPowerTiming, DramMemoryController,
    DramMemoryTechnology, DramRefreshGranularity, DramRefreshPolicy, DramRefreshTiming, DramTiming,
    ExternalMemoryProfile, NvmMediaTiming,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};
use rem6_system::{
    CheckpointRuntimeState, DramMemoryCheckpointBank, DramMemoryCheckpointError,
    DramMemoryCheckpointPort, DramMemoryCheckpointRecord, MemoryStoreCheckpointBank,
    MemoryStoreCheckpointError, MemoryStoreCheckpointPort, MemoryStoreCheckpointRecord,
};

const TEST_U64_BYTES: usize = 8;
const TEST_DRAM_TARGET_MIN_RECORD_BYTES: usize = 208;
const TEST_DRAM_BANK_STATE_MIN_RECORD_BYTES: usize = TEST_U64_BYTES * 2;
const DRAM_RUNTIME_STATE_CHUNK: &str = "dram-runtime-state";

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

fn single_target_dram_controller(
    target: MemoryTargetId,
    timing: DramTiming,
) -> DramMemoryController {
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
        .insert_line(target, Address::new(0x1000), line_data(0x10))
        .unwrap();
    controller
}

fn dram_runtime_state(controller: &Arc<Mutex<DramMemoryController>>) -> CheckpointRuntimeState {
    let capture = Arc::clone(controller);
    let validate = Arc::clone(controller);
    let restore = Arc::clone(controller);
    CheckpointRuntimeState::new(
        move || {
            Ok(encode_dram_runtime_lengths(
                &capture.lock().unwrap().runtime_log_lengths(),
            ))
        },
        move |payload| {
            let lengths = decode_dram_runtime_lengths(payload)?;
            if !validate.lock().unwrap().can_truncate_runtime_logs(&lengths) {
                return Err("DRAM runtime log prefix exceeds live history".to_string());
            }
            Ok(())
        },
        move |payload| {
            let lengths =
                decode_dram_runtime_lengths(payload).expect("validated DRAM runtime state");
            assert!(restore.lock().unwrap().truncate_runtime_logs(&lengths));
        },
    )
}

fn stored_dram_runtime_state(
    controller: &Arc<Mutex<DramMemoryController>>,
) -> CheckpointRuntimeState {
    let capture = Arc::clone(controller);
    let validate = Arc::clone(controller);
    let restore = Arc::clone(controller);
    CheckpointRuntimeState::stored(
        move || Ok(capture.lock().unwrap().runtime_logs()),
        move |snapshot| {
            validate
                .lock()
                .unwrap()
                .validate_runtime_logs(snapshot)
                .map_err(|error| error.to_string())
        },
        move |snapshot| {
            restore
                .lock()
                .unwrap()
                .restore_runtime_logs(snapshot)
                .expect("validated DRAM runtime logs");
        },
    )
}

fn encode_dram_runtime_lengths(lengths: &[(MemoryTargetId, usize, usize)]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + lengths.len() * 20);
    payload.extend_from_slice(&u64::try_from(lengths.len()).unwrap().to_le_bytes());
    for (target, activity_len, wait_len) in lengths {
        payload.extend_from_slice(&target.get().to_le_bytes());
        payload.extend_from_slice(&u64::try_from(*activity_len).unwrap().to_le_bytes());
        payload.extend_from_slice(&u64::try_from(*wait_len).unwrap().to_le_bytes());
    }
    payload
}

fn decode_dram_runtime_lengths(
    payload: &[u8],
) -> Result<Vec<(MemoryTargetId, usize, usize)>, String> {
    let count = payload
        .get(..8)
        .ok_or_else(|| "truncated DRAM runtime checkpoint".to_string())?;
    let count = usize::try_from(u64::from_le_bytes(count.try_into().unwrap()))
        .map_err(|_| "DRAM runtime target count exceeds host size".to_string())?;
    if payload.len() != 8 + count.saturating_mul(20) {
        return Err("invalid DRAM runtime checkpoint length".to_string());
    }
    payload[8..]
        .chunks_exact(20)
        .map(|entry| {
            let target = MemoryTargetId::new(u32::from_le_bytes(entry[..4].try_into().unwrap()));
            let activity_len =
                usize::try_from(u64::from_le_bytes(entry[4..12].try_into().unwrap()))
                    .map_err(|_| "DRAM runtime activity prefix exceeds host size".to_string())?;
            let wait_len = usize::try_from(u64::from_le_bytes(entry[12..20].try_into().unwrap()))
                .map_err(|_| "DRAM runtime wait prefix exceeds host size".to_string())?;
            Ok((target, activity_len, wait_len))
        })
        .collect()
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
fn dram_runtime_log_validation_rejects_duplicate_targets() {
    let (controller, low, _high) = dram_memory_controller();
    let duplicate = [(low, 0, 0), (low, 0, 0)];

    assert!(!controller.can_truncate_runtime_logs(&duplicate));
}

#[test]
fn dram_memory_checkpoint_bank_rejects_aliased_controllers() {
    let controller = Arc::new(Mutex::new(dram_memory_controller().0));
    let first = CheckpointComponentId::new("dram-alias-0").unwrap();
    let second = CheckpointComponentId::new("dram-alias-1").unwrap();

    let error = DramMemoryCheckpointBank::new([
        DramMemoryCheckpointPort::new(first, Arc::clone(&controller)),
        DramMemoryCheckpointPort::new(second.clone(), controller),
    ])
    .unwrap_err();

    assert_eq!(
        error,
        CheckpointError::DuplicateComponent { component: second }
    );
}

#[test]
fn dram_runtime_sidecar_target_mismatch_fails_bank_preflight() {
    let live_target = MemoryTargetId::new(80);
    let snapshot_target = MemoryTargetId::new(81);
    let mut live = single_target_dram_controller(live_target, dram_timing());
    live.accept(0, &read(0x1000, 8, 90)).unwrap();
    let live = Arc::new(Mutex::new(live));
    let component = CheckpointComponentId::new("dram-runtime-targets").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&live))
        .with_runtime_state(dram_runtime_state(&live));
    let bank = DramMemoryCheckpointBank::new([port]).unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();

    let replacement = Arc::new(Mutex::new(single_target_dram_controller(
        snapshot_target,
        dram_timing(),
    )));
    let replacement_port =
        DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&replacement));
    let mut replacement_registry = CheckpointRegistry::new();
    replacement_port
        .register(&mut replacement_registry)
        .unwrap();
    replacement_port
        .capture_into(&mut replacement_registry)
        .unwrap();
    registry
        .write_chunk(
            &component,
            "dram",
            replacement_registry
                .chunk(&component, "dram")
                .unwrap()
                .to_vec(),
        )
        .unwrap();
    let before = live.lock().unwrap().clone();

    assert!(bank.validate_restore_from(&registry).is_err());
    assert_eq!(*live.lock().unwrap(), before);
}

#[test]
fn dram_runtime_sidecar_restores_logs_without_overriding_wire_snapshot() {
    let target = MemoryTargetId::new(83);
    let mut source = single_target_dram_controller(target, dram_timing());
    source.accept(0, &read(0x1000, 8, 93)).unwrap();
    let source = Arc::new(Mutex::new(source));
    let component = CheckpointComponentId::new("dram-wire-authority").unwrap();
    let source_port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&source))
        .with_runtime_state(stored_dram_runtime_state(&source));
    let mut registry = CheckpointRegistry::new();
    source_port.register(&mut registry).unwrap();
    source_port.capture_into(&mut registry).unwrap();
    let captured_runtime = source.lock().unwrap().runtime_log_lengths();

    let mut replacement = single_target_dram_controller(target, fast_dram_timing());
    replacement
        .accept(0, &write(0x1000, &[0xa5; 8], 94))
        .unwrap();
    let replacement = Arc::new(Mutex::new(replacement));
    let replacement_port =
        DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&replacement));
    let mut replacement_registry = CheckpointRegistry::new();
    replacement_port
        .register(&mut replacement_registry)
        .unwrap();
    let replacement_record = replacement_port
        .capture_into(&mut replacement_registry)
        .unwrap();
    registry
        .write_chunk(
            &component,
            "dram",
            replacement_registry
                .chunk(&component, "dram")
                .unwrap()
                .to_vec(),
        )
        .unwrap();
    source
        .lock()
        .unwrap()
        .accept(20, &read(0x1000, 8, 95))
        .unwrap();

    source_port.restore_from(&registry).unwrap();

    assert_eq!(
        source.lock().unwrap().snapshot(),
        *replacement_record.snapshot()
    );
    assert_eq!(
        source.lock().unwrap().runtime_log_lengths(),
        captured_runtime
    );
}

#[test]
fn dram_current_format_marker_cannot_collide_with_legacy_store_length() {
    let controller = dram_controller_for_malformed_restore();
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram-legacy-marker-collision").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut payload = u64::from_le_bytes(*b"DRM2\0\0\0\0").to_le_bytes().to_vec();
    payload.extend_from_slice(&[0; 16]);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::InvalidChunk {
            component,
            reason: "store payload is truncated".to_string(),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_rejects_impossible_refresh_frontier_without_mutation() {
    let target = MemoryTargetId::new(84);
    let timing = dram_timing()
        .with_refresh_timing(DramRefreshTiming::new(40, 10).unwrap())
        .unwrap();
    let controller = Arc::new(Mutex::new(single_target_dram_controller(target, timing)));
    let before = controller.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("dram-refresh-frontier-invalid").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();
    port.capture_into(&mut registry).unwrap();
    let mut payload = registry.chunk(&component, "dram").unwrap().to_vec();
    let encoded_interval = 40_u64.to_le_bytes();
    let frontier_offsets = payload
        .windows(encoded_interval.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == encoded_interval).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(
        frontier_offsets.len(),
        dram_geometry().bank_count() as usize + 1
    );
    for offset in frontier_offsets.into_iter().skip(1) {
        payload[offset..offset + 8].copy_from_slice(&80_u64.to_le_bytes());
    }
    registry.write_chunk(&component, "dram", payload).unwrap();

    let error = port.restore_from(&registry).unwrap_err();

    let DramMemoryCheckpointError::InvalidChunk { reason, .. } = error else {
        panic!("unexpected error: {error}");
    };
    assert!(reason.contains("next refresh cycle 80"), "{reason}");
    assert_eq!(controller.lock().unwrap().snapshot(), before);
}

#[test]
fn dram_memory_checkpoint_preserves_refresh_timing_and_frontier() {
    let target = MemoryTargetId::new(82);
    let timing = dram_timing()
        .with_refresh_timing(
            DramRefreshTiming::new(40, 10)
                .unwrap()
                .with_granularity(DramRefreshGranularity::TwoX),
        )
        .unwrap()
        .with_refresh_policy(DramRefreshPolicy::AllBank)
        .unwrap();
    let mut controller = single_target_dram_controller(target, timing);
    controller.accept(45, &read(0x1000, 8, 91)).unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram-refresh").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();

    let captured = port.capture_into(&mut registry).unwrap();
    let captured_controller = captured.snapshot().targets()[0].controller();
    assert_eq!(
        captured_controller.timing().refresh_timing(),
        timing.refresh_timing()
    );
    assert_eq!(
        captured_controller.timing().refresh_policy(),
        DramRefreshPolicy::AllBank
    );
    assert!(captured_controller
        .banks()
        .iter()
        .all(|bank| bank.next_refresh_cycle() > 0));
    controller
        .lock()
        .unwrap()
        .accept(90, &read(0x1000, 8, 92))
        .unwrap();

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(controller.lock().unwrap().snapshot(), *captured.snapshot());
}

#[test]
fn dram_memory_checkpoint_captures_and_restores_controller() {
    let (mut controller, low, high) = dram_memory_controller();
    let first = controller.accept(0, &read(0x1000, 8, 20)).unwrap();
    assert_eq!(first.ready_cycle(), 8);
    assert!(!first.dram_access().row_hit());
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram0").unwrap();
    let runtime_at_checkpoint = controller.lock().unwrap().runtime_log_lengths();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller))
        .with_runtime_state(dram_runtime_state(&controller));
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
    assert_eq!(
        registry
            .chunk(&component, DRAM_RUNTIME_STATE_CHUNK)
            .unwrap(),
        encode_dram_runtime_lengths(&runtime_at_checkpoint)
    );

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
    assert_ne!(
        controller.lock().unwrap().runtime_log_lengths(),
        runtime_at_checkpoint
    );

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(
        controller.lock().unwrap().runtime_log_lengths(),
        runtime_at_checkpoint
    );
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
fn dram_memory_checkpoint_missing_runtime_state_is_atomic() {
    let (mut controller, _low, _high) = dram_memory_controller();
    controller.accept(0, &read(0x1000, 8, 30)).unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("dram-runtime-atomic").unwrap();
    let port = DramMemoryCheckpointPort::new(component.clone(), Arc::clone(&controller))
        .with_runtime_state(dram_runtime_state(&controller));
    let bank = DramMemoryCheckpointBank::new([port]).unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    assert!(registry.remove_chunk(&component, DRAM_RUNTIME_STATE_CHUNK));

    controller
        .lock()
        .unwrap()
        .accept(20, &write(0x1000, &[0xaa; 8], 31))
        .unwrap();
    let before_snapshot = controller.lock().unwrap().snapshot();
    let before_runtime = controller.lock().unwrap().runtime_log_lengths();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(
        error,
        DramMemoryCheckpointError::MissingChunk {
            component,
            name: DRAM_RUNTIME_STATE_CHUNK.to_string(),
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(), before_snapshot);
    assert_eq!(
        controller.lock().unwrap().runtime_log_lengths(),
        before_runtime
    );
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
