use rem6_traffic::{
    TrafficGeneratorError, TrafficStateGraphConfig, TrafficStateId, TrafficStateMachine,
    TrafficStateSnapshot, TrafficStateSpec, TrafficTransition, TrafficTransitionEvent,
    TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

fn state(id: u32, duration: u64) -> TrafficStateSpec {
    TrafficStateSpec::new(TrafficStateId::new(id), duration)
}

fn transition(from: u32, to: u32, probability: u32) -> TrafficTransition {
    TrafficTransition::new(
        TrafficStateId::new(from),
        TrafficStateId::new(to),
        TrafficTransitionProbability::from_micros(probability).unwrap(),
    )
}

fn deterministic_graph() -> TrafficStateGraphConfig {
    TrafficStateGraphConfig::new(
        vec![state(10, 5), state(20, 7)],
        TrafficStateId::new(10),
        vec![
            transition(10, 20, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
            transition(20, 10, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
        ],
    )
    .unwrap()
}

#[test]
fn traffic_state_machine_enters_initial_state_and_transitions_when_due() {
    let config = deterministic_graph();
    let mut machine = TrafficStateMachine::new(config);

    machine.start(100).unwrap();

    assert_eq!(machine.current_state(), Some(TrafficStateId::new(10)));
    assert_eq!(machine.next_transition_tick(), 105);
    assert_eq!(machine.transition_if_due(104).unwrap(), None);

    let event = machine.transition_if_due(105).unwrap().unwrap();

    assert_eq!(
        event,
        TrafficTransitionEvent::new(
            105,
            0,
            TrafficStateId::new(10),
            TrafficStateId::new(20),
            112,
        )
    );
    assert_eq!(machine.current_state(), Some(TrafficStateId::new(20)));
    assert_eq!(machine.next_transition_tick(), 112);
}

#[test]
fn traffic_state_machine_uses_weighted_markov_selection() {
    let config = TrafficStateGraphConfig::new(
        vec![state(0, 1), state(1, 1), state(2, 1)],
        TrafficStateId::new(0),
        vec![
            transition(0, 1, 250_000),
            transition(0, 2, 750_000),
            transition(1, 1, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
            transition(2, 2, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
        ],
    )
    .unwrap();

    let mut first_bucket = TrafficStateMachine::new(config.clone().with_rng_state(249_999));
    first_bucket.start(0).unwrap();
    assert_eq!(
        first_bucket.transition_if_due(1).unwrap().unwrap().to(),
        TrafficStateId::new(1)
    );

    let mut second_bucket = TrafficStateMachine::new(config.with_rng_state(250_000));
    second_bucket.start(0).unwrap();
    assert_eq!(
        second_bucket.transition_if_due(1).unwrap().unwrap().to(),
        TrafficStateId::new(2)
    );
}

#[test]
fn traffic_state_machine_treats_zero_or_max_duration_as_no_timed_transition() {
    for duration in [0, u64::MAX] {
        let config = TrafficStateGraphConfig::new(
            vec![state(0, duration)],
            TrafficStateId::new(0),
            vec![transition(0, 0, TRAFFIC_TRANSITION_PROBABILITY_SCALE)],
        )
        .unwrap();
        let mut machine = TrafficStateMachine::new(config);

        machine.start(10).unwrap();

        assert_eq!(machine.next_transition_tick(), u64::MAX);
        assert_eq!(machine.transition_if_due(u64::MAX).unwrap(), None);
    }
}

#[test]
fn traffic_state_machine_snapshot_restores_state_sequence_tick_and_rng() {
    let config = TrafficStateGraphConfig::new(
        vec![state(0, 3), state(1, 5), state(2, 7)],
        TrafficStateId::new(0),
        vec![
            transition(0, 1, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
            transition(1, 1, 500_000),
            transition(1, 2, 500_000),
            transition(2, 2, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
        ],
    )
    .unwrap()
    .with_rng_state(499_999);
    let mut machine = TrafficStateMachine::new(config);
    machine.start(10).unwrap();
    machine.transition_if_due(13).unwrap().unwrap();

    let snapshot = machine.snapshot();
    let mut restored = TrafficStateMachine::restore(snapshot).unwrap();
    let event = restored.transition_if_due(18).unwrap().unwrap();

    assert_eq!(event.sequence(), 1);
    assert_eq!(event.from(), TrafficStateId::new(1));
    assert_eq!(event.to(), TrafficStateId::new(1));
    assert_eq!(event.next_transition_tick(), 23);
}

#[test]
fn traffic_state_graph_rejects_invalid_shape_before_runtime() {
    assert_eq!(
        TrafficStateGraphConfig::new(Vec::new(), TrafficStateId::new(0), Vec::new()).unwrap_err(),
        TrafficGeneratorError::TrafficStateGraphEmpty
    );

    assert_eq!(
        TrafficStateGraphConfig::new(
            vec![state(0, 1), state(0, 2)],
            TrafficStateId::new(0),
            vec![transition(0, 0, TRAFFIC_TRANSITION_PROBABILITY_SCALE)],
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficStateDuplicate {
            state: TrafficStateId::new(0),
        }
    );

    assert_eq!(
        TrafficStateGraphConfig::new(
            vec![state(0, 1)],
            TrafficStateId::new(7),
            vec![transition(0, 0, TRAFFIC_TRANSITION_PROBABILITY_SCALE)],
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficStateUnknownInitial {
            state: TrafficStateId::new(7),
        }
    );

    assert_eq!(
        TrafficStateGraphConfig::new(
            vec![state(0, 1), state(1, 1)],
            TrafficStateId::new(0),
            vec![
                transition(0, 1, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
                transition(0, 1, 0),
                transition(1, 1, TRAFFIC_TRANSITION_PROBABILITY_SCALE),
            ],
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficStateDuplicateTransition {
            from: TrafficStateId::new(0),
            to: TrafficStateId::new(1),
        }
    );

    assert_eq!(
        TrafficStateGraphConfig::new(
            vec![state(0, 1)],
            TrafficStateId::new(0),
            vec![transition(0, 0, 999_999)],
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficStateTransitionRowSumMismatch {
            state: TrafficStateId::new(0),
            sum: 999_999,
            expected: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
        }
    );
}

#[test]
fn traffic_state_probability_rejects_out_of_range_ratios() {
    assert_eq!(
        TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE + 1)
            .unwrap_err(),
        TrafficGeneratorError::TrafficTransitionProbabilityOutOfRange {
            probability: TRAFFIC_TRANSITION_PROBABILITY_SCALE + 1,
            scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
        }
    );

    assert_eq!(
        TrafficTransitionProbability::from_ratio(1, 0).unwrap_err(),
        TrafficGeneratorError::TrafficTransitionRatioZeroDenominator
    );
}

#[test]
fn traffic_state_snapshot_rejects_unknown_current_state() {
    let config = deterministic_graph();
    let snapshot = TrafficStateSnapshot::new(config, Some(TrafficStateId::new(99)), 0, 0, 0, true);

    assert_eq!(
        TrafficStateMachine::restore(snapshot).unwrap_err(),
        TrafficGeneratorError::TrafficStateSnapshotUnknownState {
            state: TrafficStateId::new(99),
        }
    );
}

#[test]
fn traffic_state_snapshot_rejects_active_snapshot_without_current_state() {
    let config = deterministic_graph();
    let snapshot = TrafficStateSnapshot::new(config, None, 0, 0, 0, true);

    assert_eq!(
        TrafficStateMachine::restore(snapshot).unwrap_err(),
        TrafficGeneratorError::TrafficStateSnapshotMissingCurrentState
    );
}
