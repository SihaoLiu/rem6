use rem6_memory::{AgentId, CacheLineLayout, MemoryOperation, ResponseStatus};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace, TrafficTraceConfig,
    TrafficTraceErrorKind, TrafficTraceReplayCompletion, TrafficTraceReplayFailure,
    TrafficTraceReplaySource, TrafficTraceResponseKind, TrafficTransition,
    TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_READ_RESP: u32 = 2;
const GEM5_READ_RESP_WITH_INVALIDATE: u32 = 3;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_WRITE_RESP: u32 = 5;
const GEM5_SC_UPGRADE_FAIL_REQ: u32 = 20;
const GEM5_UPGRADE_FAIL_RESP: u32 = 21;
const GEM5_STORE_COND_FAIL_REQ: u32 = 28;
const GEM5_STORE_COND_RESP: u32 = 29;
const GEM5_MEM_FENCE_REQ: u32 = 38;
const GEM5_MEM_FENCE_RESP: u32 = 41;
const GEM5_INVALID_DEST_ERROR: u32 = 46;
const GEM5_WRITE_ERROR: u32 = 49;
const GEM5_HTM_REQ: u32 = 56;
const GEM5_HTM_REQ_RESP: u32 = 57;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    packet_id: Option<u64>,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn controller_for_packets(packets: &[PacketFields]) -> TrafficController {
    controller_for_packets_with_offset(packets, 0)
}

fn controller_for_packets_with_offset(
    packets: &[PacketFields],
    addr_offset: u64,
) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace)
        .unwrap()
        .with_addr_offset(addr_offset)
        .unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, u64::MAX)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(rem6_traffic::TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    TrafficController::new(controller_config)
}

#[test]
fn traffic_controller_matches_trace_response_to_pending_memory_request() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(3),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(3),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::ReadShared);
    assert!(request.request().requires_response());
    assert!(request_batch.trace_response_match().is_none());

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert_eq!(
        response.kind(),
        TrafficTraceResponseKind::ReadWithInvalidate
    );
    assert!(response.invalidates_line());

    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.response(), response);
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
            assert_eq!(source.trace_packet_id(), Some(3));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(memory_response.status(), ResponseStatus::Completed);
            assert_eq!(memory_response.data().unwrap().len(), 8);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_response_to_pending_sync_event() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(4),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(4),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let sync_batch = controller.next_event(20, 0).unwrap().unwrap();
    let sync = sync_batch.trace_sync().unwrap();
    assert!(sync.requires_response());
    assert!(sync_batch.trace_response_match().is_none());

    let response_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Sync(source) => {
            assert_eq!(source.sequence(), sync.sequence());
            assert_eq!(source.trace_packet_id(), Some(4));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
}

#[test]
fn traffic_controller_matches_trace_error_to_pending_write_request() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(5),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(5),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::Write);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    assert!(matched.error().is_write());
    match matched.failure() {
        TrafficTraceReplayFailure::Memory(failure) => {
            assert_eq!(failure.request_id(), request.request().id());
            assert_eq!(failure.error(), TrafficTraceErrorKind::Write);
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_error_to_pending_htm_request() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0x5400),
            size: Some(16),
            packet_id: Some(14),
        },
        PacketFields {
            tick: 7,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x5400),
            size: Some(16),
            packet_id: Some(14),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let error_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    match matched.failure() {
        TrafficTraceReplayFailure::Control(failure) => {
            assert_eq!(failure.error(), TrafficTraceErrorKind::InvalidDestination);
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
            assert_eq!(source.trace_packet_id(), Some(14));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_upgrade_fail_response_to_failed_sc_upgrade() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_SC_UPGRADE_FAIL_REQ,
            address: Some(0x8000),
            size: Some(64),
            packet_id: Some(8),
        },
        PacketFields {
            tick: 7,
            command: GEM5_UPGRADE_FAIL_RESP,
            address: Some(0x8000),
            size: Some(64),
            packet_id: Some(8),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::StoreConditionalUpgradeFail
    );

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(memory_response.status(), ResponseStatus::Completed);
            assert_eq!(memory_response.data().unwrap().len(), 64);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_preserves_forced_store_conditional_failure_status() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_STORE_COND_FAIL_REQ,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(9),
        },
        PacketFields {
            tick: 7,
            command: GEM5_STORE_COND_RESP,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(9),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::StoreConditionalFail
    );

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(
                memory_response.status(),
                ResponseStatus::StoreConditionalFailed
            );
            assert_eq!(memory_response.data(), None);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_response_after_addr_offset() {
    let mut controller = controller_for_packets_with_offset(
        &[
            PacketFields {
                tick: 5,
                command: GEM5_READ_REQ,
                address: Some(0x9000),
                size: Some(8),
                packet_id: Some(10),
            },
            PacketFields {
                tick: 7,
                command: GEM5_READ_RESP,
                address: Some(0x9000),
                size: Some(8),
                packet_id: Some(10),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9040);

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        response_batch
            .trace_response()
            .unwrap()
            .address()
            .unwrap()
            .get(),
        0x9040
    );
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_error_after_addr_offset() {
    let mut controller = controller_for_packets_with_offset(
        &[
            PacketFields {
                tick: 5,
                command: GEM5_WRITE_REQ,
                address: Some(0x9400),
                size: Some(8),
                packet_id: Some(11),
            },
            PacketFields {
                tick: 7,
                command: GEM5_WRITE_ERROR,
                address: Some(0x9400),
                size: Some(8),
                packet_id: Some(11),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9440);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        error_batch.trace_error().unwrap().address().unwrap().get(),
        0x9440
    );
    let matched = error_batch.trace_error_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_htm_response_after_addr_offset() {
    let mut controller = controller_for_packets_with_offset(
        &[
            PacketFields {
                tick: 5,
                command: GEM5_HTM_REQ,
                address: Some(0x9800),
                size: Some(16),
                packet_id: Some(12),
            },
            PacketFields {
                tick: 7,
                command: GEM5_HTM_REQ_RESP,
                address: Some(0x9800),
                size: Some(16),
                packet_id: Some(12),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert_eq!(htm.address().unwrap().get(), 0x9840);

    let response_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    assert_eq!(
        response_batch
            .trace_response()
            .unwrap()
            .address()
            .unwrap()
            .get(),
        0x9840
    );
    assert_eq!(
        response_batch.trace_response_match().unwrap().completion(),
        &TrafficTraceReplayCompletion::Ack
    );
}

#[test]
fn traffic_controller_keeps_pending_htm_after_metadata_mismatch() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0xa000),
            size: Some(16),
            packet_id: None,
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xb000),
            size: Some(16),
            packet_id: None,
        },
        PacketFields {
            tick: 9,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0xa000),
            size: Some(8),
            packet_id: None,
        },
        PacketFields {
            tick: 11,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xa000),
            size: Some(16),
            packet_id: None,
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let wrong_address_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    let wrong_address = wrong_address_batch.trace_response().unwrap();
    assert!(wrong_address_batch.trace_response_match().is_none());

    let wrong_size_batch = controller
        .next_event(wrong_address.tick(), 0)
        .unwrap()
        .unwrap();
    let wrong_size = wrong_size_batch.trace_error().unwrap();
    assert!(wrong_size_batch.trace_error_match().is_none());

    let response_batch = controller
        .next_event(wrong_size.tick(), 0)
        .unwrap()
        .unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_keeps_pending_request_after_policy_mismatch() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(6),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_RESP,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(6),
        },
        PacketFields {
            tick: 9,
            command: GEM5_READ_RESP,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(6),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let mismatch_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        mismatch_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::Write
    );
    assert!(mismatch_batch.trace_response_match().is_none());

    let response_batch = controller.next_event(27, 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_snapshot_restores_pending_trace_response_match() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(7),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(7),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let snapshot = controller.snapshot();
    let mut restored = TrafficController::restore(snapshot).unwrap();
    let response_batch = restored.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();

    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_snapshot_restores_pending_htm_trace_response_match() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0xa400),
            size: Some(16),
            packet_id: Some(13),
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xa400),
            size: Some(16),
            packet_id: Some(13),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let snapshot = controller.snapshot();
    let mut restored = TrafficController::restore(snapshot).unwrap();
    let response_batch = restored.next_event(htm.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();

    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
            assert_eq!(source.trace_packet_id(), Some(13));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

fn state(id: u32, duration: u64) -> TrafficStateSpec {
    TrafficStateSpec::new(TrafficStateId::new(id), duration)
}

fn transition(from: u32, to: u32) -> TrafficTransition {
    TrafficTransition::new(
        TrafficStateId::new(from),
        TrafficStateId::new(to),
        TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE).unwrap(),
    )
}

fn graph(
    states: Vec<TrafficStateSpec>,
    transitions: Vec<TrafficTransition>,
) -> TrafficStateGraphConfig {
    TrafficStateGraphConfig::new(states, TrafficStateId::new(0), transitions).unwrap()
}

fn gem5_packet_trace(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut bytes = GEM5_MAGIC.to_vec();
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    for packet in packets {
        let mut message = Vec::new();
        append_key(&mut message, 1, 0);
        append_varint(&mut message, packet.tick);
        append_key(&mut message, 2, 0);
        append_varint(&mut message, u64::from(packet.command));
        if let Some(address) = packet.address {
            append_key(&mut message, 3, 0);
            append_varint(&mut message, address);
        }
        if let Some(size) = packet.size {
            append_key(&mut message, 4, 0);
            append_varint(&mut message, u64::from(size));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        append_record(&mut bytes, &message);
    }

    bytes
}

fn append_record(bytes: &mut Vec<u8>, message: &[u8]) {
    append_varint(
        bytes,
        u64::try_from(message.len()).expect("test message length fits u64"),
    );
    bytes.extend_from_slice(message);
}

fn append_key(bytes: &mut Vec<u8>, field: u32, wire_type: u8) {
    append_varint(bytes, (u64::from(field) << 3) | u64::from(wire_type));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}
