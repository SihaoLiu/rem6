use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    LinearTrafficGenerator, TrafficController, TrafficControllerConfig, TrafficControllerState,
    TrafficGeneratorError, TrafficLinearConfig, TrafficStateGenerator, TrafficStateGraphConfig,
    TrafficStateId, TrafficStateSpec, TrafficStreamConfig, TrafficStreamIdMode, TrafficTransition,
    TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn linear_config() -> TrafficLinearConfig {
    TrafficLinearConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x1040),
        AccessSize::new(16).unwrap(),
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap()
}

fn controller_with_stream(stream: TrafficStreamConfig) -> TrafficController {
    let graph = TrafficStateGraphConfig::new(
        vec![TrafficStateSpec::new(TrafficStateId::new(0), 100)],
        TrafficStateId::new(0),
        vec![TrafficTransition::new(
            TrafficStateId::new(0),
            TrafficStateId::new(0),
            TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
                .unwrap(),
        )],
    )
    .unwrap();
    let state = TrafficControllerState::new(
        TrafficStateId::new(0),
        TrafficStateGenerator::Linear(LinearTrafficGenerator::new(linear_config())),
    );
    let config = TrafficControllerConfig::new(graph, vec![state])
        .unwrap()
        .with_stream(stream);
    TrafficController::new(config)
}

#[test]
fn fixed_stream_generator_tags_controller_requests_with_stream_ids() {
    let stream = TrafficStreamConfig::fixed(11).with_fixed_substream_id(29);
    let mut controller = controller_with_stream(stream);

    controller.start(10).unwrap();
    let event = controller
        .next_event(10, 0)
        .unwrap()
        .unwrap()
        .request()
        .unwrap()
        .clone();

    assert_eq!(event.tick(), 14);
    assert_eq!(event.address(), Address::new(0x1000));
    assert_eq!(
        event.request().id(),
        MemoryRequestId::new(AgentId::new(7), 0)
    );
    assert_eq!(event.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(event.request().stream_id(), Some(11));
    assert_eq!(event.request().substream_id(), Some(29));
}

#[test]
fn random_stream_generator_chooses_only_declared_stream_ids_and_restores_rng() {
    let stream = TrafficStreamConfig::new(
        TrafficStreamIdMode::Random,
        vec![4, 8, 15],
        vec![16, 23, 42],
    )
    .unwrap()
    .with_rng_state(2);
    let mut controller = controller_with_stream(stream);

    controller.start(0).unwrap();
    let first = controller
        .next_event(0, 0)
        .unwrap()
        .unwrap()
        .request()
        .unwrap()
        .clone();
    let second = controller
        .next_event(4, 0)
        .unwrap()
        .unwrap()
        .request()
        .unwrap()
        .clone();
    let snapshot = controller.snapshot();
    let mut restored = TrafficController::restore(snapshot).unwrap();
    let restored_next = restored
        .next_event(8, 0)
        .unwrap()
        .unwrap()
        .request()
        .unwrap()
        .clone();

    assert_eq!(first.request().stream_id(), Some(15));
    assert_eq!(first.request().substream_id(), Some(23));
    assert!(matches!(second.request().stream_id(), Some(4 | 8 | 15)));
    assert!(matches!(
        second.request().substream_id(),
        Some(16 | 23 | 42)
    ));
    assert_eq!(restored_next.request().stream_id(), Some(8));
    assert_eq!(restored_next.request().substream_id(), Some(42));
}

#[test]
fn stream_generator_rejects_invalid_gem5_stream_configurations() {
    assert_eq!(
        TrafficStreamConfig::new(TrafficStreamIdMode::Fixed, Vec::new(), Vec::new()).unwrap_err(),
        TrafficGeneratorError::TrafficStreamMissingIds
    );

    assert_eq!(
        TrafficStreamConfig::new(TrafficStreamIdMode::Fixed, vec![1, 2], Vec::new()).unwrap_err(),
        TrafficGeneratorError::TrafficStreamInvalidFixedIds {
            stream_ids: 2,
            substream_ids: 0,
        }
    );

    assert_eq!(
        TrafficStreamConfig::new(TrafficStreamIdMode::Fixed, vec![1], vec![2, 3]).unwrap_err(),
        TrafficGeneratorError::TrafficStreamInvalidFixedIds {
            stream_ids: 1,
            substream_ids: 2,
        }
    );
}
