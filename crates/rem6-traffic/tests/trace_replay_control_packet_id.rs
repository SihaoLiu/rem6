use rem6_memory::{Address, AgentId, CacheLineLayout};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace, TrafficTraceConfig,
    TrafficTraceReplayCompletion, TrafficTraceReplaySource, TrafficTraceResponseKind,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_MEM_FENCE_REQ: u32 = 38;
const GEM5_MEM_FENCE_RESP: u32 = 41;
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
fn traffic_controller_keeps_pending_sync_after_missing_response_packet_id() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(44),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: None,
        },
        PacketFields {
            tick: 9,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(44),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let sync_batch = controller.next_event(20, 0).unwrap().unwrap();
    let sync = sync_batch.trace_sync().unwrap();
    assert!(sync.requires_response());

    let missing_packet_id_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    assert_eq!(
        missing_packet_id_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::MemFence
    );
    assert!(missing_packet_id_batch.trace_response_match().is_none());

    let response_batch = controller
        .next_event(missing_packet_id_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
    match matched.source() {
        TrafficTraceReplaySource::Sync(source) => {
            assert_eq!(source.sequence(), sync.sequence());
            assert_eq!(source.trace_packet_id(), Some(44));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_keeps_pending_htm_after_missing_response_packet_id() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0xa400),
            size: Some(16),
            packet_id: Some(45),
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xa400),
            size: Some(16),
            packet_id: None,
        },
        PacketFields {
            tick: 9,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xa400),
            size: Some(16),
            packet_id: Some(45),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let missing_packet_id_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    assert_eq!(
        missing_packet_id_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::HtmRequest
    );
    assert!(missing_packet_id_batch.trace_response_match().is_none());

    let response_batch = controller
        .next_event(missing_packet_id_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
            assert_eq!(source.trace_packet_id(), Some(45));
            assert_eq!(source.address(), Some(Address::new(0xa400)));
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
