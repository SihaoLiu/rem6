use rem6_traffic::{
    TrafficGeneratorError, TrafficStateId, TrafficTextConfig, TrafficTextMemoryParams,
    TrafficTextStateMode, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

fn parse(input: &str) -> TrafficTextConfig {
    TrafficTextConfig::parse(input).unwrap()
}

#[test]
fn traffic_text_config_parses_gem5_memory_example_into_typed_graph() {
    let config = parse(
        r#"
        # The file format contains STATE, INIT, and TRANSITION records.
        STATE 0 1000000 TRACE tgen-simple-mem.trc 100
        STATE 1 100000000 RANDOM 0 0 134217728 64 30000 30000 0
        STATE 2 1000000000 IDLE
        STATE 3 100000000 LINEAR 0 0 134217728 64 30000 30000 0
        STATE 4 1000000 IDLE
        INIT 0
        TRANSITION 0 1 1
        TRANSITION 1 2 1.0
        TRANSITION 2 3 0.5
        TRANSITION 2 4 0.5
        TRANSITION 3 2 1
        TRANSITION 4 4 1
        "#,
    );

    assert_eq!(config.graph().initial_state(), TrafficStateId::new(0));
    assert_eq!(config.graph().states().len(), 5);
    assert_eq!(config.graph().transitions().len(), 6);
    assert_eq!(config.states().len(), 5);

    let trace = config.state(TrafficStateId::new(0)).unwrap();
    assert_eq!(trace.duration(), 1_000_000);
    assert_eq!(
        trace.mode(),
        &TrafficTextStateMode::Trace {
            trace_file: "tgen-simple-mem.trc".to_string(),
            addr_offset: 100,
        }
    );

    let random = config.state(TrafficStateId::new(1)).unwrap();
    assert_eq!(
        random.mode(),
        &TrafficTextStateMode::Random(TrafficTextMemoryParams::new(
            0,
            0,
            134_217_728,
            64,
            30_000,
            30_000,
            0,
        ))
    );

    let idle = config.state(TrafficStateId::new(2)).unwrap();
    assert_eq!(idle.mode(), &TrafficTextStateMode::Idle);

    let linear = config.state(TrafficStateId::new(3)).unwrap();
    assert_eq!(
        linear.mode(),
        &TrafficTextStateMode::Linear(TrafficTextMemoryParams::new(
            0,
            0,
            134_217_728,
            64,
            30_000,
            30_000,
            0,
        ))
    );

    let split_transition = config
        .graph()
        .transitions()
        .iter()
        .find(|transition| transition.from() == TrafficStateId::new(2))
        .unwrap();
    assert_eq!(
        split_transition.probability().micros(),
        TRAFFIC_TRANSITION_PROBABILITY_SCALE / 2
    );
}

#[test]
fn traffic_text_config_parses_strided_and_dram_family_modes() {
    let config = parse(
        r#"
        STATE 0 10 STRIDED 75 4096 8192 128 64 256 512 7 9 1024
        STATE 1 20 DRAM 50 0 4096 64 11 13 2048 256 1024 8 4 1 2
        STATE 2 30 DRAM_ROTATE 100 0 4096 64 11 13 0 256 1024 8 4 1 2
        STATE 3 40 NVM 0 0 4096 64 11 13 4096 256 1024 8 4 1 2
        INIT 0
        TRANSITION 0 1 1
        TRANSITION 1 2 1
        TRANSITION 2 3 1
        TRANSITION 3 3 1
        "#,
    );

    assert!(matches!(
        config.state(TrafficStateId::new(0)).unwrap().mode(),
        TrafficTextStateMode::Strided(_)
    ));
    let TrafficTextStateMode::Dram(dram) = config.state(TrafficStateId::new(1)).unwrap().mode()
    else {
        panic!("state 1 should be DRAM");
    };
    assert_eq!(dram.memory().read_percent(), 50);
    assert_eq!(dram.stride_size(), 256);
    assert_eq!(dram.page_or_buffer_size(), 1024);
    assert!(matches!(
        config.state(TrafficStateId::new(2)).unwrap().mode(),
        TrafficTextStateMode::DramRotate(_)
    ));
    let TrafficTextStateMode::Nvm(nvm) = config.state(TrafficStateId::new(3)).unwrap().mode()
    else {
        panic!("state 3 should be NVM");
    };
    assert_eq!(nvm.memory().data_limit(), 4096);
    assert_eq!(nvm.page_or_buffer_size(), 1024);
}

#[test]
fn traffic_text_config_rejects_missing_initial_state_before_graph_build() {
    let error = TrafficTextConfig::parse(
        r#"
        STATE 0 10 IDLE
        TRANSITION 0 0 1
        "#,
    )
    .unwrap_err();

    assert_eq!(error, TrafficGeneratorError::TrafficConfigMissingInitial);
}

#[test]
fn traffic_text_config_rejects_duplicate_state_ids_before_sparse_id_check() {
    let error = TrafficTextConfig::parse(
        r#"
        STATE 0 10 IDLE
        STATE 0 20 EXIT
        INIT 0
        TRANSITION 0 0 1
        "#,
    )
    .unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TrafficStateDuplicate {
            state: TrafficStateId::new(0),
        }
    );
}

#[test]
fn traffic_text_config_rejects_sparse_state_ids() {
    let error = TrafficTextConfig::parse(
        r#"
        STATE 0 10 IDLE
        STATE 2 10 IDLE
        INIT 0
        TRANSITION 0 2 1
        TRANSITION 2 2 1
        "#,
    )
    .unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TrafficConfigSparseStateIds {
            expected: 1,
            actual: TrafficStateId::new(2),
        }
    );
}

#[test]
fn traffic_text_config_rejects_unknown_mode_and_keyword() {
    assert_eq!(
        TrafficTextConfig::parse("STATE 0 10 HYBRID\nINIT 0\nTRANSITION 0 0 1").unwrap_err(),
        TrafficGeneratorError::TrafficConfigUnknownStateMode {
            line: 1,
            mode: "HYBRID".to_string(),
        }
    );

    assert_eq!(
        TrafficTextConfig::parse("EDGE 0 0 1").unwrap_err(),
        TrafficGeneratorError::TrafficConfigUnknownKeyword {
            line: 1,
            keyword: "EDGE".to_string(),
        }
    );
}

#[test]
fn traffic_text_config_rejects_malformed_numbers_and_extra_tokens() {
    assert_eq!(
        TrafficTextConfig::parse("STATE 0 nope IDLE").unwrap_err(),
        TrafficGeneratorError::TrafficConfigInvalidNumber {
            line: 1,
            field: "duration",
            token: "nope".to_string(),
        }
    );

    assert_eq!(
        TrafficTextConfig::parse("INIT 0 1").unwrap_err(),
        TrafficGeneratorError::TrafficConfigUnexpectedToken {
            line: 1,
            record: "INIT",
            token: "1".to_string(),
        }
    );
}

#[test]
fn traffic_text_config_rejects_invalid_probabilities() {
    assert_eq!(
        TrafficTextConfig::parse("STATE 0 1 IDLE\nINIT 0\nTRANSITION 0 0 0.1234567").unwrap_err(),
        TrafficGeneratorError::TrafficConfigProbabilityTooPrecise {
            line: 3,
            token: "0.1234567".to_string(),
            scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
        }
    );

    assert_eq!(
        TrafficTextConfig::parse("STATE 0 1 IDLE\nINIT 0\nTRANSITION 0 0 1.1").unwrap_err(),
        TrafficGeneratorError::TrafficTransitionProbabilityOutOfRange {
            probability: 1_100_000,
            scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
        }
    );
}
