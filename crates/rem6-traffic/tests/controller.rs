use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    DramTrafficGenerator, LinearTrafficGenerator, RandomTrafficGenerator, StridedTrafficGenerator,
    TrafficController, TrafficControllerConfig, TrafficControllerSnapshot, TrafficControllerState,
    TrafficDramAddressMapping, TrafficDramConfig, TrafficDramMode, TrafficGeneratorError,
    TrafficIdleGenerator, TrafficLinearConfig, TrafficRandomConfig, TrafficRequestKind,
    TrafficStateGenerator, TrafficStateGeneratorSnapshot, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficStridedConfig, TrafficTrace, TrafficTraceConfig,
    TrafficTraceExitStatus, TrafficTraceGenerator, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: u64,
    size: u32,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
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

fn linear_config(period: u64, read_percent: u8) -> TrafficLinearConfig {
    TrafficLinearConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x1040),
        AccessSize::new(16).unwrap(),
    )
    .unwrap()
    .with_period(period, period)
    .unwrap()
    .with_read_percent(read_percent)
    .unwrap()
}

fn random_config(period: u64, read_percent: u8) -> TrafficRandomConfig {
    TrafficRandomConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x1040),
        AccessSize::new(16).unwrap(),
    )
    .unwrap()
    .with_period(period, period)
    .unwrap()
    .with_read_percent(read_percent)
    .unwrap()
}

fn strided_config(period: u64, read_percent: u8) -> TrafficStridedConfig {
    TrafficStridedConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x10a0),
        0,
        AccessSize::new(16).unwrap(),
        32,
        64,
    )
    .unwrap()
    .with_period(period, period)
    .unwrap()
    .with_read_percent(read_percent)
    .unwrap()
}

fn dram_config(period: u64, read_percent: u8) -> TrafficDramConfig {
    TrafficDramConfig::new(
        AgentId::new(7),
        line_layout(),
        TrafficDramMode::Dram,
        Address::new(0x1000),
        Address::new(0x1400),
        AccessSize::new(16).unwrap(),
        64,
        2,
        2,
        TrafficDramAddressMapping::RoRaBaCoCh,
        1,
        1,
    )
    .unwrap()
    .with_period(period, period)
    .unwrap()
    .with_read_percent(read_percent)
    .unwrap()
}

fn linear_state(id: u32, period: u64, read_percent: u8) -> TrafficControllerState {
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Linear(LinearTrafficGenerator::new(linear_config(
            period,
            read_percent,
        ))),
    )
}

fn random_state(id: u32, period: u64, read_percent: u8) -> TrafficControllerState {
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Random(RandomTrafficGenerator::new(random_config(
            period,
            read_percent,
        ))),
    )
}

fn strided_state(id: u32, period: u64, read_percent: u8) -> TrafficControllerState {
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Strided(StridedTrafficGenerator::new(strided_config(
            period,
            read_percent,
        ))),
    )
}

fn dram_state(id: u32, period: u64, read_percent: u8) -> TrafficControllerState {
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Dram(DramTrafficGenerator::new(dram_config(period, read_percent))),
    )
}

fn data_limited_state(id: u32, generator: TrafficStateGenerator) -> TrafficControllerState {
    TrafficControllerState::new(TrafficStateId::new(id), generator)
}

fn idle_state(id: u32, duration: u64) -> TrafficControllerState {
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Idle(TrafficIdleGenerator::new(
            rem6_traffic::TrafficIdleConfig::new(duration),
        )),
    )
}

fn exit_state(id: u32, duration: u64) -> TrafficControllerState {
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Exit(rem6_traffic::TrafficExitGenerator::new(
            rem6_traffic::TrafficExitConfig::new(duration),
        )),
    )
}

fn trace_state(id: u32, duration: u64, packet_tick: u64) -> TrafficControllerState {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: packet_tick,
                command: 1,
                address: 0x20,
                size: 8,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), duration, trace).unwrap();
    TrafficControllerState::new(
        TrafficStateId::new(id),
        TrafficStateGenerator::Trace(TrafficTraceGenerator::new(config)),
    )
}

#[test]
fn traffic_controller_prioritizes_transition_over_packet_at_same_tick() {
    let config = TrafficControllerConfig::new(
        graph(
            vec![state(0, 4), state(1, u64::MAX)],
            vec![transition(0, 1), transition(1, 1)],
        ),
        vec![linear_state(0, 4, 100), idle_state(1, u64::MAX)],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);

    assert!(controller.start(0).unwrap().is_empty());
    let step = controller.next_event(0, 0).unwrap().unwrap();

    assert!(step.request().is_none());
    assert_eq!(step.transition().unwrap().tick(), 4);
    assert_eq!(step.transition().unwrap().from(), TrafficStateId::new(0));
    assert_eq!(step.transition().unwrap().to(), TrafficStateId::new(1));
    assert_eq!(controller.current_state(), Some(TrafficStateId::new(1)));
}

#[test]
fn traffic_controller_emits_request_without_double_scheduling_rng() {
    let config = TrafficControllerConfig::new(
        graph(vec![state(0, 100)], vec![transition(0, 0)]),
        vec![linear_state(0, 4, 10)],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);

    controller.start(0).unwrap();
    let step = controller.next_event(0, 0).unwrap().unwrap();
    let request = step.request().unwrap();

    assert_eq!(request.tick(), 4);
    assert_eq!(request.kind(), TrafficRequestKind::Read);
    assert_eq!(request.address(), Address::new(0x1000));
    assert_eq!(request.request().operation(), MemoryOperation::ReadShared);
}

#[test]
fn traffic_controller_transitions_to_exit_state_on_entry() {
    let config = TrafficControllerConfig::new(
        graph(
            vec![state(0, 3), state(1, u64::MAX)],
            vec![transition(0, 1), transition(1, 1)],
        ),
        vec![idle_state(0, 3), exit_state(1, u64::MAX)],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);

    controller.start(0).unwrap();
    let step = controller.next_event(0, 0).unwrap().unwrap();

    assert_eq!(step.transition().unwrap().tick(), 3);
    assert_eq!(step.exit().unwrap().tick(), 3);
    assert_eq!(step.exit().unwrap().duration(), u64::MAX);
}

#[test]
fn traffic_controller_forces_transition_when_state_has_no_packet_or_timer() {
    let config = TrafficControllerConfig::new(
        graph(
            vec![state(0, u64::MAX), state(1, u64::MAX)],
            vec![transition(0, 1), transition(1, 1)],
        ),
        vec![idle_state(0, u64::MAX), idle_state(1, u64::MAX)],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);

    controller.start(10).unwrap();
    let step = controller.next_event(10, 0).unwrap().unwrap();

    assert_eq!(step.transition().unwrap().tick(), 10);
    assert_eq!(step.transition().unwrap().from(), TrafficStateId::new(0));
    assert_eq!(step.transition().unwrap().to(), TrafficStateId::new(1));
}

#[test]
fn traffic_controller_snapshot_restores_machine_generator_and_summary_state() {
    let config = TrafficControllerConfig::new(
        graph(vec![state(0, 100)], vec![transition(0, 0)]),
        vec![linear_state(0, 4, 100)],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);
    controller.start(0).unwrap();
    let first = controller.next_event(0, 0).unwrap().unwrap();
    assert_eq!(first.request().unwrap().address(), Address::new(0x1000));

    let snapshot = controller.snapshot();
    let mut restored = TrafficController::restore(snapshot).unwrap();
    let second = restored.next_event(4, 0).unwrap().unwrap();

    assert_eq!(second.request().unwrap().sequence(), 1);
    assert_eq!(second.request().unwrap().address(), Address::new(0x1010));
    assert_eq!(
        second.request().unwrap().request().id(),
        MemoryRequestId::new(AgentId::new(7), 1)
    );
    assert_eq!(restored.summary().packet_count(), 2);
}

#[test]
fn traffic_controller_snapshot_preserves_every_leaf_generator() {
    let config = TrafficControllerConfig::new(
        graph(
            (0..7).map(|id| state(id, u64::MAX)).collect(),
            (0..7).map(|id| transition(id, id)).collect(),
        ),
        vec![
            linear_state(0, 4, 100),
            idle_state(1, u64::MAX),
            exit_state(2, u64::MAX),
            random_state(3, 4, 100),
            strided_state(4, 4, 100),
            dram_state(5, 4, 100),
            trace_state(6, u64::MAX, 0),
        ],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);

    controller.start(0).unwrap();
    let snapshot = controller.snapshot();
    let restored = TrafficController::restore(snapshot.clone()).unwrap();

    assert!(matches!(
        snapshot_generator(&snapshot, 0),
        TrafficStateGeneratorSnapshot::Linear(_)
    ));
    assert!(matches!(
        snapshot_generator(&snapshot, 1),
        TrafficStateGeneratorSnapshot::Idle(_)
    ));
    assert!(matches!(
        snapshot_generator(&snapshot, 2),
        TrafficStateGeneratorSnapshot::Exit(_)
    ));
    assert!(matches!(
        snapshot_generator(&snapshot, 3),
        TrafficStateGeneratorSnapshot::Random(_)
    ));
    assert!(matches!(
        snapshot_generator(&snapshot, 4),
        TrafficStateGeneratorSnapshot::Strided(_)
    ));
    assert!(matches!(
        snapshot_generator(&snapshot, 5),
        TrafficStateGeneratorSnapshot::Dram(_)
    ));
    assert!(matches!(
        snapshot_generator(&snapshot, 6),
        TrafficStateGeneratorSnapshot::Trace(_)
    ));
    assert_eq!(restored.snapshot().generators(), snapshot.generators());
}

#[test]
fn traffic_controller_reports_trace_exit_status_on_state_leave() {
    let incomplete_config = TrafficControllerConfig::new(
        graph(
            vec![state(0, 1), state(1, u64::MAX)],
            vec![transition(0, 1), transition(1, 1)],
        ),
        vec![trace_state(0, 1, 5), idle_state(1, u64::MAX)],
    )
    .unwrap();
    let mut incomplete = TrafficController::new(incomplete_config);
    incomplete.start(0).unwrap();
    let step = incomplete.next_event(0, 0).unwrap().unwrap();
    assert_eq!(
        step.trace_exit().unwrap(),
        TrafficTraceExitStatus::incomplete()
    );

    let complete_config = TrafficControllerConfig::new(
        graph(
            vec![state(0, 100), state(1, u64::MAX)],
            vec![transition(0, 1), transition(1, 1)],
        ),
        vec![trace_state(0, 100, 0), idle_state(1, u64::MAX)],
    )
    .unwrap();
    let mut complete = TrafficController::new(complete_config);
    complete.start(0).unwrap();
    assert!(complete
        .next_event(0, 0)
        .unwrap()
        .unwrap()
        .request()
        .is_some());
    let step = complete.next_event(0, 0).unwrap().unwrap();
    assert_eq!(
        step.trace_exit().unwrap(),
        TrafficTraceExitStatus::completed()
    );
}

#[test]
fn traffic_controller_forces_transition_after_last_request_without_timer() {
    let config = TrafficControllerConfig::new(
        graph(
            vec![state(0, u64::MAX), state(1, u64::MAX)],
            vec![transition(0, 1), transition(1, 1)],
        ),
        vec![trace_state(0, u64::MAX, 0), exit_state(1, u64::MAX)],
    )
    .unwrap();
    let mut controller = TrafficController::new(config);

    controller.start(0).unwrap();
    let step = controller.next_event(0, 0).unwrap().unwrap();

    assert!(step.request().is_some());
    assert_eq!(
        step.trace_exit().unwrap(),
        TrafficTraceExitStatus::completed()
    );
    assert_eq!(step.transition().unwrap().tick(), 0);
    assert_eq!(step.exit().unwrap().tick(), 0);
    assert_eq!(controller.current_state(), Some(TrafficStateId::new(1)));
}

#[test]
fn traffic_controller_forces_transition_after_data_limited_leaf_exhausts() {
    let cases = vec![
        (
            "linear",
            TrafficStateGenerator::Linear(LinearTrafficGenerator::new(
                linear_config(4, 100).with_data_limit(16).unwrap(),
            )),
        ),
        (
            "random",
            TrafficStateGenerator::Random(RandomTrafficGenerator::new(
                random_config(4, 100).with_data_limit(16).unwrap(),
            )),
        ),
        (
            "strided",
            TrafficStateGenerator::Strided(StridedTrafficGenerator::new(
                strided_config(4, 100).with_data_limit(16).unwrap(),
            )),
        ),
        (
            "dram",
            TrafficStateGenerator::Dram(DramTrafficGenerator::new(
                dram_config(4, 100).with_data_limit(16).unwrap(),
            )),
        ),
    ];

    for (name, generator) in cases {
        let config = TrafficControllerConfig::new(
            graph(
                vec![state(0, u64::MAX), state(1, u64::MAX)],
                vec![transition(0, 1), transition(1, 1)],
            ),
            vec![data_limited_state(0, generator), exit_state(1, u64::MAX)],
        )
        .unwrap();
        let mut controller = TrafficController::new(config);

        controller.start(0).unwrap();
        let step = controller.next_event(0, 0).unwrap().unwrap();

        assert!(step.request().is_some(), "{name}");
        assert_eq!(step.transition().unwrap().tick(), 4, "{name}");
        assert_eq!(step.exit().unwrap().tick(), 4, "{name}");
        assert_eq!(
            controller.current_state(),
            Some(TrafficStateId::new(1)),
            "{name}"
        );
    }
}

#[test]
fn traffic_controller_rejects_missing_duplicate_or_unknown_state_generators() {
    let graph = graph(
        vec![state(0, 1), state(1, 1)],
        vec![transition(0, 1), transition(1, 1)],
    );

    assert_eq!(
        TrafficControllerConfig::new(graph.clone(), vec![idle_state(0, 1)]).unwrap_err(),
        TrafficGeneratorError::TrafficControllerMissingStateGenerator {
            state: TrafficStateId::new(1),
        }
    );

    assert_eq!(
        TrafficControllerConfig::new(
            graph.clone(),
            vec![idle_state(0, 1), idle_state(0, 1), idle_state(1, 1)],
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficControllerDuplicateStateGenerator {
            state: TrafficStateId::new(0),
        }
    );

    assert_eq!(
        TrafficControllerConfig::new(
            graph,
            vec![idle_state(0, 1), idle_state(1, 1), idle_state(2, 1)],
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficControllerUnknownStateGenerator {
            state: TrafficStateId::new(2),
        }
    );
}

fn gem5_packet_trace(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut trace = GEM5_MAGIC.to_vec();
    append_message(&mut trace, &header_message(tick_frequency));

    for packet in packets {
        append_message(&mut trace, &packet_message(*packet));
    }

    trace
}

fn append_message(trace: &mut Vec<u8>, message: &[u8]) {
    append_varint(trace, message.len() as u64);
    trace.extend_from_slice(message);
}

fn header_message(tick_frequency: u64) -> Vec<u8> {
    let mut message = Vec::new();
    append_field_varint(&mut message, 3, tick_frequency);
    message
}

fn packet_message(packet: PacketFields) -> Vec<u8> {
    let mut message = Vec::new();
    append_field_varint(&mut message, 1, packet.tick);
    append_field_varint(&mut message, 2, u64::from(packet.command));
    append_field_varint(&mut message, 3, packet.address);
    append_field_varint(&mut message, 4, u64::from(packet.size));
    message
}

fn append_field_varint(message: &mut Vec<u8>, field: u32, value: u64) {
    append_varint(message, u64::from(field << 3));
    append_varint(message, value);
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn snapshot_generator(
    snapshot: &TrafficControllerSnapshot,
    id: u32,
) -> &TrafficStateGeneratorSnapshot {
    snapshot
        .generators()
        .iter()
        .find(|entry| entry.id() == TrafficStateId::new(id))
        .expect("snapshot contains controller state")
        .generator()
}
