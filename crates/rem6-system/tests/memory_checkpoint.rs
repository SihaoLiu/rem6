use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_memory::{
    AccessSize, Address, CacheLineLayout, MemoryTargetId, PartitionedMemorySnapshot,
    PartitionedMemoryStore,
};
use rem6_system::{MemoryStoreCheckpointPort, MemoryStoreCheckpointRecord};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line_data(base: u8) -> Vec<u8> {
    (0..64).map(|offset| base.wrapping_add(offset)).collect()
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
