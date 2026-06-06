use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    GupsTrafficGenerator, TrafficGeneratorError, TrafficGupsConfig, TrafficRequestKind,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn gups_config() -> TrafficGupsConfig {
    TrafficGupsConfig::new(AgentId::new(5), line_layout(), Address::new(0x1000), 64)
        .unwrap()
        .with_update_limit(2)
        .unwrap()
        .with_rng_state(0)
}

#[test]
fn gups_generator_emits_random_reads_and_response_driven_writes() {
    let mut generator = GupsTrafficGenerator::new(gups_config());

    let read = generator.next_request(10).unwrap().unwrap();
    assert_eq!(read.tick(), 11);
    assert_eq!(read.sequence(), 0);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.address(), Address::new(0x1000));
    assert_eq!(
        read.request().id(),
        MemoryRequestId::new(AgentId::new(5), 0)
    );
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(read.request().size(), AccessSize::new(8).unwrap());

    generator
        .complete_read(read.sequence(), 0x0102_0304_0506_0708)
        .unwrap();

    let write = generator.next_request(read.tick()).unwrap().unwrap();
    assert_eq!(write.tick(), 12);
    assert_eq!(write.sequence(), 1);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.address(), Address::new(0x1000));
    assert_eq!(write.request().operation(), MemoryOperation::Write);
    assert_eq!(
        write.request().data().unwrap(),
        &0x0102_0304_0506_0708_u64.to_le_bytes()
    );
    assert_eq!(write.request().byte_mask().unwrap().len(), 8);

    let next_read = generator.next_request(write.tick()).unwrap().unwrap();
    assert_eq!(next_read.tick(), 13);
    assert_eq!(next_read.sequence(), 2);
    assert_eq!(next_read.kind(), TrafficRequestKind::Read);
    assert_eq!(next_read.address(), Address::new(0x1038));

    generator.complete_read(next_read.sequence(), 0x10).unwrap();
    let next_write = generator.next_request(next_read.tick()).unwrap().unwrap();
    assert_eq!(next_write.address(), Address::new(0x1038));
    assert_eq!(
        next_write.request().data().unwrap(),
        &0x11_u64.to_le_bytes()
    );

    assert_eq!(generator.next_request(next_write.tick()).unwrap(), None);
    assert!(generator.is_complete());
    assert_eq!(generator.summary().packet_count(), 4);
    assert_eq!(generator.summary().read_count(), 2);
    assert_eq!(generator.summary().write_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 16);
    assert_eq!(generator.summary().bytes_written(), 16);
}

#[test]
fn gups_generator_default_update_count_is_four_times_table_entries() {
    let config = TrafficGupsConfig::new(AgentId::new(1), line_layout(), Address::new(0x2000), 16)
        .unwrap()
        .with_rng_state(0);
    let mut generator = GupsTrafficGenerator::new(config);

    for expected_sequence in 0..8 {
        let event = generator.next_request(expected_sequence).unwrap().unwrap();
        assert_eq!(event.sequence(), expected_sequence);
        assert_eq!(event.kind(), TrafficRequestKind::Read);
    }

    assert_eq!(generator.next_request(8).unwrap(), None);
    assert!(!generator.is_complete());
    assert_eq!(generator.pending_read_count(), 8);
    assert_eq!(generator.summary().read_count(), 8);
}

#[test]
fn gups_generator_update_limit_cannot_extend_default_update_count() {
    let config = TrafficGupsConfig::new(AgentId::new(1), line_layout(), Address::new(0x2000), 16)
        .unwrap()
        .with_update_limit(99)
        .unwrap()
        .with_rng_state(0);
    let mut generator = GupsTrafficGenerator::new(config);

    for expected_sequence in 0..8 {
        let event = generator.next_request(expected_sequence).unwrap().unwrap();
        assert_eq!(event.sequence(), expected_sequence);
        assert_eq!(event.kind(), TrafficRequestKind::Read);
    }

    assert_eq!(generator.next_request(8).unwrap(), None);
    assert_eq!(generator.summary().read_count(), 8);
}

#[test]
fn gups_generator_snapshot_restores_pending_updates_and_rng() {
    let mut generator = GupsTrafficGenerator::new(gups_config());
    let read = generator.next_request(0).unwrap().unwrap();
    generator.complete_read(read.sequence(), 0x22).unwrap();

    let snapshot = generator.snapshot();
    let mut restored = GupsTrafficGenerator::restore(snapshot).unwrap();

    let write = restored.next_request(read.tick()).unwrap().unwrap();
    assert_eq!(write.sequence(), 1);
    assert_eq!(write.address(), Address::new(0x1000));
    assert_eq!(write.request().data().unwrap(), &0x22_u64.to_le_bytes());

    let next_read = restored.next_request(write.tick()).unwrap().unwrap();
    assert_eq!(next_read.address(), Address::new(0x1038));
    assert_eq!(restored.summary().packet_count(), 3);
}

#[test]
fn gups_generator_rejects_invalid_configs_and_unknown_completions() {
    assert_eq!(
        TrafficGupsConfig::new(AgentId::new(1), line_layout(), Address::new(0), 0).unwrap_err(),
        TrafficGeneratorError::TrafficGupsZeroMemorySize
    );
    assert_eq!(
        TrafficGupsConfig::new(AgentId::new(1), line_layout(), Address::new(0), 15).unwrap_err(),
        TrafficGeneratorError::TrafficGupsMemorySizeNotMultiple {
            mem_size: 15,
            element_size: 8,
        }
    );
    assert_eq!(
        TrafficGupsConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(u64::MAX - 3),
            8
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficGupsAddressRangeOverflow {
            start: Address::new(u64::MAX - 3),
            mem_size: 8,
        }
    );

    let mut generator = GupsTrafficGenerator::new(gups_config());
    assert_eq!(
        generator.complete_read(99, 1).unwrap_err(),
        TrafficGeneratorError::TrafficGupsUnknownReadCompletion { sequence: 99 }
    );
}
