use super::super::cpu::update_resettable_counter_delta;
use super::*;

const ISSUE_QUEUE_PATHS: [&str; 9] = [
    "enqueued_rows",
    "service_turns",
    "wake_requests",
    "current_occupancy",
    "peak_occupancy",
    "issued_by_class.scalar_integer",
    "issued_by_class.integer_mul_div",
    "issued_by_class.memory_agu",
    "issued_by_class.control",
];

#[test]
fn o3_issue_queue_stats_register_stable_resettable_paths() {
    let cpu = CpuId::new(0);
    let mut registry = StatsRegistry::new();
    RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();

    for path in ISSUE_QUEUE_PATHS {
        let sample = issue_queue_sample(&registry, 0, path);
        assert_eq!(sample.unit(), "Count");
        assert_eq!(sample.reset_policy(), StatResetPolicy::Resettable);
        assert_eq!(sample.value(), 0);
    }
}

#[test]
fn o3_issue_queue_stats_counter_drop_rebases_resettable_value() {
    let mut registry = StatsRegistry::new();
    let stat = registry
        .register_counter("test.issue_queue", "Count")
        .unwrap();

    update_resettable_counter_delta(&mut registry, stat, 0, 5).unwrap();
    assert_eq!(stat_sample(&registry, 1, "test.issue_queue").value(), 5);
    update_resettable_counter_delta(&mut registry, stat, 5, 5).unwrap();
    assert_eq!(stat_sample(&registry, 2, "test.issue_queue").value(), 5);
    update_resettable_counter_delta(&mut registry, stat, 5, 0).unwrap();
    assert_eq!(stat_sample(&registry, 3, "test.issue_queue").value(), 0);
}

fn issue_queue_sample(registry: &StatsRegistry, tick: u64, path: &str) -> rem6_stats::StatSample {
    stat_sample(
        registry,
        tick,
        &format!("sim.host_actions.stats_dump.cpu0.o3.issue_queue.{path}"),
    )
}
