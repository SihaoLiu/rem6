use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};
use rem6_system::{
    DramMemoryCheckpointPort, DramMemoryCheckpointRecord, MemoryStoreCheckpointPort,
    MemoryStoreCheckpointRecord,
};

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
    DramTiming::new(3, 5, 7, 2, 4).unwrap()
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
    assert_eq!(row_hit.ready_cycle(), 13);
}
