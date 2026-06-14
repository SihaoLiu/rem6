use rem6_memory::{AgentId, CacheLineLayout};
use rem6_traffic::{
    TrafficController, TrafficGeneratorClass, TrafficMemoryProfile, TrafficProfileSummary,
    TrafficRequestKind, TrafficStateId, TrafficStateProfileSummary, TrafficTextBindingOptions,
    TrafficTextConfig,
};

fn parse(input: &str) -> TrafficTextConfig {
    TrafficTextConfig::parse(input).unwrap()
}

fn binding_options() -> TrafficTextBindingOptions {
    TrafficTextBindingOptions::new(AgentId::new(9), CacheLineLayout::new(64).unwrap())
}

fn profile_for(summaries: &[TrafficStateProfileSummary], state: u32) -> &TrafficProfileSummary {
    summaries
        .iter()
        .find(|summary| summary.state() == TrafficStateId::new(state))
        .expect("state profile summary exists")
        .profile()
}

#[test]
fn traffic_text_config_reports_memory_profile_matrix_as_typed_summaries() {
    let config = parse(
        r#"
        STATE 0 20 DRAM 100 0 4096 16 4 4 0 32 64 2 2 1 1
        STATE 1 20 DRAM_ROTATE 50 4096 8192 16 4 4 0 32 64 2 2 1 1
        STATE 2 20 NVM 0 8192 12288 16 4 4 0 32 64 2 1 0 1
        STATE 3 20 HYBRID 75 16384 20480 64 24576 28672 32 4 4 0 4 1024 8 4 3 256 4 2 1 2 1 60
        STATE 4 20 GUPS 32768 64 1
        INIT 0
        TRANSITION 0 1 1
        TRANSITION 1 2 1
        TRANSITION 2 3 1
        TRANSITION 3 4 1
        TRANSITION 4 4 1
        "#,
    );

    let controller_config = config.to_controller_config(binding_options()).unwrap();
    let profiles = controller_config.profile_summaries();

    assert_eq!(profiles.len(), 5);

    let dram = profile_for(&profiles, 0);
    assert_eq!(dram.generator_class(), TrafficGeneratorClass::Dram);
    assert_eq!(dram.memory_profile(), TrafficMemoryProfile::Dram);
    assert_eq!(dram.summary().packet_count(), 0);

    let rotating = profile_for(&profiles, 1);
    assert_eq!(
        rotating.generator_class(),
        TrafficGeneratorClass::DramRotate
    );
    assert_eq!(rotating.memory_profile(), TrafficMemoryProfile::Dram);

    let nvm = profile_for(&profiles, 2);
    assert_eq!(nvm.generator_class(), TrafficGeneratorClass::Nvm);
    assert_eq!(nvm.memory_profile(), TrafficMemoryProfile::Nvm);

    let hybrid = profile_for(&profiles, 3);
    assert_eq!(hybrid.generator_class(), TrafficGeneratorClass::Hybrid);
    assert_eq!(hybrid.memory_profile(), TrafficMemoryProfile::Hybrid);

    let gups = profile_for(&profiles, 4);
    assert_eq!(gups.generator_class(), TrafficGeneratorClass::Gups);
    assert_eq!(gups.memory_profile(), TrafficMemoryProfile::GupsTable);
}

#[test]
fn traffic_controller_reports_active_profiled_stats_after_deterministic_nvm_run() {
    let config = parse(
        r#"
        STATE 0 100 NVM 0 8192 12288 16 4 4 32 32 64 2 1 0 1
        INIT 0
        TRANSITION 0 0 1
        "#,
    );
    let controller_config = config.to_controller_config(binding_options()).unwrap();
    let mut controller = TrafficController::new(controller_config);

    assert!(controller.start(0).unwrap().is_empty());
    let first = controller.next_event(0, 0).unwrap().unwrap();
    let second = controller
        .next_event(first.request().unwrap().tick(), 0)
        .unwrap()
        .unwrap();

    assert_eq!(first.request().unwrap().kind(), TrafficRequestKind::Write);
    assert_eq!(second.request().unwrap().kind(), TrafficRequestKind::Write);

    let profiled = controller.current_profile_summary().unwrap();

    assert_eq!(profiled.state(), TrafficStateId::new(0));
    assert_eq!(
        profiled.profile().generator_class(),
        TrafficGeneratorClass::Nvm
    );
    assert_eq!(
        profiled.profile().memory_profile(),
        TrafficMemoryProfile::Nvm
    );
    assert_eq!(profiled.profile().summary().packet_count(), 2);
    assert_eq!(profiled.profile().summary().write_count(), 2);
    assert_eq!(profiled.profile().summary().bytes_written(), 32);
    assert_eq!(profiled.profile().summary().first_tick(), Some(4));
    assert_eq!(profiled.profile().summary().last_tick(), Some(8));
}
