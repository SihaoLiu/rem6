use rem6_memory::{AgentId, CacheLineLayout};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerEvent, TrafficControllerState,
    TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace,
    TrafficTraceConfig, TrafficTraceErrorKind, TrafficTraceReplayAction,
    TrafficTraceReplayActionQueue, TrafficTraceReplayFailure, TrafficTraceReplayOutcome,
    TrafficTraceReplaySource, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_INVALID_DEST_ERROR: u32 = 46;
const GEM5_READ_ERROR: u32 = 48;
const GEM5_WRITE_ERROR: u32 = 49;
const GEM5_PRINT_REQ: u32 = 52;
const GEM5_FLUSH_REQ: u32 = 53;
const GEM5_HTM_ABORT: u32 = 58;
const GEM5_TLBI_EXT_SYNC: u32 = 59;

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
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap();
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
fn traffic_controller_turns_no_response_control_errors_into_actions() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_TLBI_EXT_SYNC,
            address: Some(0),
            size: Some(64),
            packet_id: Some(40),
        },
        PacketFields {
            tick: 7,
            command: GEM5_INVALID_DEST_ERROR,
            address: None,
            size: None,
            packet_id: Some(40),
        },
        PacketFields {
            tick: 9,
            command: GEM5_PRINT_REQ,
            address: Some(0x6400),
            size: Some(1),
            packet_id: Some(41),
        },
        PacketFields {
            tick: 11,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x6400),
            size: Some(1),
            packet_id: Some(41),
        },
        PacketFields {
            tick: 13,
            command: GEM5_FLUSH_REQ,
            address: Some(0x6500),
            size: Some(64),
            packet_id: Some(42),
        },
        PacketFields {
            tick: 15,
            command: GEM5_WRITE_ERROR,
            address: Some(0x6500),
            size: Some(64),
            packet_id: Some(42),
        },
        PacketFields {
            tick: 17,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(43),
        },
        PacketFields {
            tick: 19,
            command: GEM5_READ_ERROR,
            address: None,
            size: None,
            packet_id: Some(43),
        },
    ]);
    let mut action_queue = TrafficTraceReplayActionQueue::default();

    assert!(controller.start(20).unwrap().is_empty());
    assert_no_response_control_failure_action(
        &mut controller,
        &mut action_queue,
        20,
        TrafficTraceErrorKind::InvalidDestination,
    );
    assert_no_response_control_failure_action(
        &mut controller,
        &mut action_queue,
        7,
        TrafficTraceErrorKind::InvalidDestination,
    );
    assert_no_response_control_failure_action(
        &mut controller,
        &mut action_queue,
        11,
        TrafficTraceErrorKind::Write,
    );
    assert_no_response_control_failure_action(
        &mut controller,
        &mut action_queue,
        15,
        TrafficTraceErrorKind::Read,
    );

    assert_eq!(controller.trace_replay_summary().memory_completions(), 0);
    assert_eq!(controller.trace_replay_summary().control_completions(), 0);
    assert_eq!(controller.trace_replay_summary().memory_failures(), 0);
    assert_eq!(controller.trace_replay_summary().control_failures(), 4);
    assert_eq!(action_queue.summary().control_failures(), 4);
    assert!(action_queue.is_empty());
}

#[test]
fn traffic_controller_prefers_packet_id_memory_error_over_untagged_control_source() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_FLUSH_REQ,
            address: Some(0x6600),
            size: Some(64),
            packet_id: None,
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_REQ,
            address: Some(0x6600),
            size: Some(64),
            packet_id: Some(44),
        },
        PacketFields {
            tick: 9,
            command: GEM5_WRITE_ERROR,
            address: Some(0x6600),
            size: Some(64),
            packet_id: Some(44),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let flush_batch = controller.next_event(20, 0).unwrap().unwrap();
    assert!(flush_batch.trace_cache().is_some());
    let write_batch = controller
        .next_event(flush_batch.trace_cache().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let write = write_batch.request().unwrap().clone();

    let error_batch = controller.next_event(write.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    match matched.failure() {
        TrafficTraceReplayFailure::Memory(failure) => {
            assert_eq!(failure.request_id(), write.request().id());
            assert_eq!(failure.error(), TrafficTraceErrorKind::Write);
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), write.request().id());
            assert_eq!(source.trace_packet_id(), Some(44));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_failures(), 1);
    assert_eq!(controller.trace_replay_summary().control_failures(), 0);
}

fn assert_no_response_control_failure_action(
    controller: &mut TrafficController,
    action_queue: &mut TrafficTraceReplayActionQueue,
    tick: u64,
    expected: TrafficTraceErrorKind,
) {
    let control_batch = controller.next_event(tick, 0).unwrap().unwrap();
    assert!(control_batch.trace_replay_outcome().is_none());
    assert!(control_batch.trace_replay_action().is_none());
    action_queue.record_batch(&control_batch).unwrap();
    assert!(action_queue.is_empty());

    let error_tick = control_batch
        .events()
        .iter()
        .find_map(|event| match event {
            TrafficControllerEvent::TraceTlb(tlb) => Some(tlb.tick()),
            TrafficControllerEvent::TraceDiagnostic(diagnostic) => Some(diagnostic.tick()),
            TrafficControllerEvent::TraceCache(cache) => Some(cache.tick()),
            TrafficControllerEvent::TraceHtm(htm) => Some(htm.tick()),
            _ => None,
        })
        .expect("control trace event should be in the batch");
    let error_batch = controller.next_event(error_tick, 0).unwrap().unwrap();

    match error_batch.trace_replay_outcome().unwrap() {
        TrafficTraceReplayOutcome::Failure(match_) => match match_.failure() {
            TrafficTraceReplayFailure::Control(failure) => {
                assert_eq!(failure.error(), expected);
            }
            failure => panic!("unexpected trace replay failure: {failure:?}"),
        },
        outcome => panic!("unexpected trace replay outcome: {outcome:?}"),
    }
    match error_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::ControlFailure { tick, failure } => {
            assert_eq!(*tick, error_batch.trace_error().unwrap().tick());
            assert_eq!(failure.error(), expected);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    action_queue.record_batch(&error_batch).unwrap();
    let failure = action_queue.pop_control_failure().unwrap();
    assert_eq!(failure.tick(), error_batch.trace_error().unwrap().tick());
    assert_eq!(failure.failure().error(), expected);
    assert!(action_queue.is_empty());
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
