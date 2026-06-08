use rem6_memory::{AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace, TrafficTraceConfig,
    TrafficTraceReplayFailure, TrafficTraceReplaySource, TrafficTraceResponseKind,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_READ_RESP: u32 = 2;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_WRITE_ERROR: u32 = 49;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    packet_id: Option<u64>,
}

fn controller_for_packets(packets: &[PacketFields]) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(
        AgentId::new(7),
        CacheLineLayout::new(64).unwrap(),
        99,
        trace,
    )
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
fn traffic_controller_keeps_pending_request_after_metadata_free_response() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(60),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: None,
            size: None,
            packet_id: None,
        },
        PacketFields {
            tick: 9,
            command: GEM5_READ_RESP,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(60),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::ReadShared);

    let metadata_free_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        metadata_free_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::Read
    );
    assert!(metadata_free_batch.trace_response_match().is_none());

    let response_batch = controller
        .next_event(metadata_free_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
            assert_eq!(source.trace_packet_id(), Some(60));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_untagged_memory_response_with_address_size_fallback() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6100),
            size: Some(8),
            packet_id: Some(61),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x6100),
            size: Some(8),
            packet_id: None,
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_packet_id_response_with_omitted_address_size() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6300),
            size: Some(8),
            packet_id: Some(63),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: None,
            size: None,
            packet_id: Some(63),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
            assert_eq!(source.trace_packet_id(), Some(63));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_keeps_pending_request_after_response_address_mismatch() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6400),
            size: Some(8),
            packet_id: Some(64),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x6480),
            size: Some(8),
            packet_id: Some(64),
        },
        PacketFields {
            tick: 9,
            command: GEM5_READ_RESP,
            address: Some(0x6400),
            size: Some(8),
            packet_id: Some(64),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let mismatched_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert!(mismatched_batch.trace_response_match().is_none());

    let response_batch = controller
        .next_event(mismatched_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_keeps_pending_request_after_metadata_free_error() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x6200),
            size: Some(8),
            packet_id: Some(62),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: None,
            size: None,
            packet_id: None,
        },
        PacketFields {
            tick: 9,
            command: GEM5_WRITE_ERROR,
            address: Some(0x6200),
            size: Some(8),
            packet_id: Some(62),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::Write);

    let metadata_free_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert!(metadata_free_batch.trace_error_match().is_none());

    let error_batch = controller
        .next_event(metadata_free_batch.trace_error().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    match matched.failure() {
        TrafficTraceReplayFailure::Memory(failure) => {
            assert_eq!(failure.request_id(), request.request().id());
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
}

#[test]
fn traffic_controller_matches_untagged_memory_error_with_address_size_fallback() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x6500),
            size: Some(8),
            packet_id: Some(65),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x6500),
            size: Some(8),
            packet_id: None,
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    match matched.failure() {
        TrafficTraceReplayFailure::Memory(failure) => {
            assert_eq!(failure.request_id(), request.request().id());
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
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
    append_varint(bytes, message.len() as u64);
    bytes.extend_from_slice(message);
}

fn append_key(bytes: &mut Vec<u8>, field: u32, wire: u8) {
    append_varint(bytes, u64::from((field << 3) | u32::from(wire)));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}
