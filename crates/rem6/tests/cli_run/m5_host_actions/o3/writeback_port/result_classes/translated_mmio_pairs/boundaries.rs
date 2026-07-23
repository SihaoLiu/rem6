use super::*;
#[path = "boundaries/handoff.rs"]
mod handoff;
use handoff::{assert_mixed_live_handoff, sole_data_request_at_tick};
const HOST_EVENT_DELAY: u64 = 1;
const DIRECT_ROUTE_DELAY: u64 = 9;
#[test]
fn rem6_run_o3_translated_result_pair_dependency_and_fault_boundaries() {
    let dependent = boundary_fixture(
        "dependent",
        [
            i_type(0, 5, 0b011, 11, 0x03),
            i_type(0, 11, 0b011, 12, 0x03),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            i_type(1, 12, 0, 13, 0x13),
        ],
        SECOND_VIRTUAL_PAGE,
        None,
        true,
    );
    let json = boundary_json(&dependent, "detailed", fixture::SecondMapping::Allowed, &[]);
    let first = memory_result_event_at_pc(&json, FIRST_PC);
    let second = memory_result_event_at_pc(&json, SECOND_PC);
    assert!(event_u64(second, "issue_tick") >= event_u64(first, "writeback_tick"));
    assert_eq!(requests_from(&json, event_u64(first, "issue_tick")), 2);
    for mapping in [
        fixture::SecondMapping::Missing,
        fixture::SecondMapping::Denied,
    ] {
        let fixture = boundary_fixture(
            "fault",
            [
                i_type(0, 5, 0b011, 11, 0x03),
                i_type(0, 6, 0b011, 12, 0x03),
                r_type(0x01, 2, 1, 0b100, 3, 0x33),
                i_type(1, 12, 0, 13, 0x13),
            ],
            0x11,
            None,
            false,
        );
        let json = boundary_json(&fixture, "detailed", mapping, &[]);
        let first = memory_result_event_at_pc(&json, FIRST_PC);
        let completion = assert_data_completion_at_pc(&json, first, FIRST_PC, FIRST_PHYSICAL_PAGE);
        assert_eq!(event_str(completion, "target"), "memory");
        assert!(event_u64(first, "writeback_tick") <= event_u64(first, "commit_tick"));
        assert_register(&json, "x11", "0x11");
        assert_register_absent(&json, "x12");
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("executed_until_trap")
        );
        assert_eq!(
            json.pointer("/simulation/stop_reason")
                .and_then(Value::as_str),
            Some("host_trap")
        );
        assert_eq!(
            json.pointer("/simulation/trap").and_then(Value::as_str),
            Some("load_page_fault")
        );
        assert_eq!(
            json.pointer("/simulation/trap_pc").and_then(Value::as_str),
            Some(SECOND_PC)
        );
        fetch_record_at_pc(&json, SECOND_PC);
        assert_eq!(requests_from(&json, event_u64(first, "issue_tick")), 1);
    }
}
#[test]
fn rem6_run_o3_translated_result_pair_target_ordering_and_capacity_boundaries() {
    for (label, acquire, release) in [("acquire", true, false), ("release", false, true)] {
        let fixture = boundary_fixture(
            label,
            [
                atomic_type(0x00, acquire, release, 1, 5, 0x3, 11),
                i_type(0, 6, 0b011, 12, 0x03),
                r_type(0x01, 2, 1, 0b100, 3, 0x33),
                i_type(1, 12, 0, 13, 0x13),
            ],
            0x11,
            None,
            true,
        );
        let json = boundary_json(&fixture, "detailed", fixture::SecondMapping::Allowed, &[]);
        let first = memory_result_event_at_pc(&json, FIRST_PC);
        let second = memory_result_event_at_pc(&json, SECOND_PC);
        assert!(event_u64(second, "issue_tick") >= event_u64(first, "writeback_tick"));
    }
    let third = boundary_fixture(
        "third",
        [
            i_type(0, 5, 0b011, 11, 0x03),
            i_type(0, 6, 0b011, 12, 0x03),
            i_type(8, 5, 0b011, 14, 0x03),
            i_type(1, 12, 0, 13, 0x13),
        ],
        0x11,
        Some(0x44),
        true,
    );
    let json = boundary_json(&third, "detailed", fixture::SecondMapping::Allowed, &[]);
    let pair = [FIRST_PC, SECOND_PC].map(|pc| memory_result_event_at_pc(&json, pc));
    let third = memory_result_event_at_pc(&json, DIV_PC);
    let earliest = pair
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .into_iter()
        .min()
        .unwrap();
    assert!(pair
        .iter()
        .all(|event| event_u64(event, "issue_tick") < earliest));
    assert!(event_u64(third, "issue_tick") >= event_u64(pair[1], "writeback_tick"));

    let alias = boundary_fixture(
        "physical-alias",
        [
            i_type(0, 5, 0b011, 11, 0x03),
            i_type(0, 6, 0b011, 12, 0x03),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            i_type(1, 12, 0, 13, 0x13),
        ],
        0x11,
        None,
        true,
    );
    let json = boundary_json(&alias, "detailed", fixture::SecondMapping::Aliased, &[]);
    let first = memory_result_event_at_pc(&json, FIRST_PC);
    let second = memory_result_event_at_pc(&json, SECOND_PC);
    assert!(event_u64(second, "issue_tick") >= event_u64(first, "lsq_data_response_tick"));
    assert_eq!(requests_from(&json, event_u64(first, "issue_tick")), 2);
    for (event, pc) in [(first, FIRST_PC), (second, SECOND_PC)] {
        let completion = assert_data_completion_at_pc(&json, event, pc, FIRST_PHYSICAL_PAGE);
        assert_eq!(event_str(completion, "target"), "memory");
    }
}
#[test]
fn rem6_run_o3_translated_result_pair_live_checkpoint_and_prebind_switch_reject() {
    let fixture = TranslatedMemoryPairFixture::new_mmio();
    let baseline = fixture.run_mixed("direct", DIRECT_ROUTE_DELAY, PAIR_MAX_TICK);
    let first = memory_result_event_at_pc(&baseline, FIRST_PC);
    let second = memory_result_event_at_pc(&baseline, SECOND_PC);
    assert_live_action_rejects(
        &fixture,
        event_u64(second, "issue_tick") + 1,
        "--host-checkpoint",
        "translated-pair-live",
    );
    assert_live_action_rejects(
        &fixture,
        event_u64(first, "issue_tick").saturating_sub(1),
        "--host-switch-cpu-mode",
        "cpu0:timing",
    );
}
#[test]
fn rem6_run_host_switch_transfers_o3_translated_memory_mmio_result_pair() {
    let fixture = TranslatedMemoryPairFixture::new_mmio();
    let baseline = fixture.run_mixed("direct", DIRECT_ROUTE_DELAY, PAIR_MAX_TICK);
    let baseline_identities = assert_mixed_completion_identities(&baseline);
    let first = memory_result_event_at_pc(&baseline, FIRST_PC);
    let second = memory_result_event_at_pc(&baseline, SECOND_PC);
    let switched = assert_mixed_live_handoff(
        &fixture,
        "direct",
        DIRECT_ROUTE_DELAY,
        event_u64(first, "issue_tick"),
        event_u64(second, "issue_tick"),
        [first, second]
            .map(|event| event_u64(event, "lsq_data_response_tick"))
            .into_iter()
            .min()
            .unwrap(),
    );
    for pc in [FIRST_PC, SECOND_PC, DIV_PC, DEPENDENT_PC] {
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(event_at_pc(&switched, pc), field),
                event_u64(event_at_pc(&baseline, pc), field)
            );
        }
    }
    for pc in [FIRST_PC, SECOND_PC] {
        assert_eq!(
            event_u64(event_at_pc(&switched, pc), "lsq_data_response_tick"),
            event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick")
        );
    }
    assert_eq!(
        assert_mixed_completion_identities(&switched),
        baseline_identities
    );
    assert_mixed_final_witness(&switched);
    assert_oldest_first_commit(&switched);
}
#[test]
fn rem6_run_o3_translated_result_pair_drained_restore() {
    let fixture = TranslatedMemoryPairFixture::new_mmio();
    let baseline = fixture.run_mixed("direct", DIRECT_ROUTE_DELAY, PAIR_MAX_TICK);
    let checkpoint_tick = event_u64(event_at_pc(&baseline, "0x8000004c"), "commit_tick") + 1;
    let checkpoint = format!("{checkpoint_tick}:translated-pair-drained");
    let restore = format!("{}:translated-pair-drained", checkpoint_tick + 1);
    let restored = boundary_json(
        &fixture,
        "detailed",
        fixture::SecondMapping::Allowed,
        &[
            "--host-checkpoint",
            &checkpoint,
            "--host-restore-checkpoint",
            &restore,
        ],
    );
    assert_eq!(json_u64(&restored, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(&restored, "/host_actions/checkpoint_restored_count"),
        1
    );
    for pointer in ["/cores/0/registers", "/memory", "/readfiles"] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "{pointer}"
        );
    }
    assert_eq!(
        json_u64(&restored, "/cores/0/o3_runtime/snapshot/rob/count"),
        0
    );
    assert_eq!(
        json_u64(&restored, "/cores/0/o3_runtime/snapshot/lsq/count"),
        0
    );
    assert!(json_u64(&restored, "/cores/0/o3_runtime/rob/max_occupancy") >= 4);
    assert!(json_u64(&restored, "/cores/0/o3_runtime/lsq/max_occupancy") >= 2);
}
#[test]
fn rem6_run_timing_suppresses_o3_translated_result_pairs() {
    let fixture = TranslatedMemoryPairFixture::new_mmio();
    let detailed = fixture.run_mixed("direct", DIRECT_ROUTE_DELAY, PAIR_MAX_TICK);
    let timing = boundary_json(&fixture, "timing", fixture::SecondMapping::Allowed, &[]);
    for pointer in ["/cores/0/registers", "/memory", "/readfiles"] {
        assert_eq!(
            timing.pointer(pointer),
            detailed.pointer(pointer),
            "{pointer}"
        );
    }
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert!(timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .all(|sample| !event_str(sample, "path").starts_with("sim.cpu0.o3.")));
}
fn boundary_json(
    fixture: &TranslatedMemoryPairFixture,
    mode: &str,
    mapping: fixture::SecondMapping,
    extra_args: &[&str],
) -> Value {
    let output =
        fixture.boundary_output(DIRECT_ROUTE_DELAY, PAIR_MAX_TICK, mode, mapping, extra_args);
    assert!(
        output.status.success(),
        "translated boundary stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn requests_from(json: &Value, tick: u64) -> usize {
    data_request_sent_records(json)
        .into_iter()
        .filter(|record| event_u64(record, "tick") >= tick)
        .count()
}
fn assert_live_action_rejects(
    fixture: &TranslatedMemoryPairFixture,
    tick: u64,
    flag: &str,
    value: &str,
) {
    let id = super::super::RESULT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let artifact = std::env::temp_dir().join(format!("rem6-translated-pair-reject-{id}.json"));
    let _ = std::fs::remove_file(&artifact);
    let spec = format!("{tick}:{value}");
    let output = fixture.boundary_output(
        DIRECT_ROUTE_DELAY,
        PAIR_MAX_TICK,
        "detailed",
        fixture::SecondMapping::Allowed,
        &[flag, &spec, "--output", artifact.to_str().unwrap()],
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "{flag} at {tick}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("checkpoint component is not quiescent: cpu0"));
    assert!(!artifact.exists());
}
fn boundary_fixture(
    label: &str,
    window: [u32; 4],
    first_value: u64,
    third_value: Option<u64>,
    probe_second: bool,
) -> TranslatedMemoryPairFixture {
    let mut words = vec![
        u_type(FIRST_VIRTUAL_PAGE as i32, 5, 0x37),
        u_type(SECOND_VIRTUAL_PAGE as i32, 6, 0x37),
        i_type(64, 5, 0b011, 0, 0x03),
        if probe_second {
            i_type(64, 6, 0b011, 0, 0x03)
        } else {
            i_type(0, 0, 0, 0, 0x13)
        },
        i_type(84, 0, 0, 1, 0x13),
        i_type(2, 0, 0, 2, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend(window);
    append_host_stop(&mut words);
    let mut payload = riscv64_program(&words);
    payload.resize(0x2048, 0);
    payload[0x1000..0x1008].copy_from_slice(&first_value.to_le_bytes());
    payload[0x2000..0x2008].copy_from_slice(&0x33_u64.to_le_bytes());
    if let Some(value) = third_value {
        payload[0x1008..0x1010].copy_from_slice(&value.to_le_bytes());
    }
    let mut fixture = TranslatedMemoryPairFixture::new();
    fixture.binary = super::super::unique_result_temp_binary(
        &format!("o3-translated-pair-boundary-{label}"),
        &fixture::translated_pair_elf(&payload),
    );
    fixture
}
pub(super) fn assert_mixed_pair(
    fixture: &TranslatedMemoryPairFixture,
    memory_system: &str,
    route_delay: u64,
) {
    let completed = fixture.run_mixed(memory_system, route_delay, PAIR_MAX_TICK);
    assert_mixed_completion_identities(&completed);
    let first = memory_result_event_at_pc(&completed, FIRST_PC);
    let second = memory_result_event_at_pc(&completed, SECOND_PC);
    assert_data_completion_at_pc(&completed, first, FIRST_PC, FIRST_PHYSICAL_PAGE);
    assert_data_completion_at_pc(&completed, second, SECOND_PC, fixture::MMIO_PAGE);
    let first_issue = event_u64(first, "issue_tick");
    let second_issue = event_u64(second, "issue_tick");
    assert!(first_issue < second_issue);
    assert_ne!(event_u64(first, "sequence"), event_u64(second, "sequence"));
    let earliest_response = [first, second]
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .into_iter()
        .min()
        .unwrap();
    assert!(second_issue < earliest_response);
    let resident = fixture.run_mixed(
        memory_system,
        route_delay,
        earliest_response.saturating_sub(1),
    );
    assert_pre_response_residency(&resident, first_issue);
    let first_request = sole_data_request_at_tick(&completed, first_issue, FIRST_PC);
    assert_no_data_request_at_tick(&completed, second_issue, SECOND_PC);
    let first_identity = request_identity(first_request);
    assert!(data_record_for_identity(&completed, "response_arrived", first_identity).is_some());
    let pair_fetches = [FIRST_PC, SECOND_PC]
        .map(|pc| fetch_request_identity(&completed, fetch_record_at_pc(&completed, pc)));
    assert_ne!(pair_fetches[0], pair_fetches[1]);
    assert_mixed_live_handoff(
        fixture,
        memory_system,
        route_delay,
        first_issue,
        second_issue,
        earliest_response,
    );
    assert_mixed_final_witness(&completed);
    assert_oldest_first_commit(&completed);
    let before = fixture.run_mixed(memory_system, route_delay, first_issue.saturating_sub(1));
    let through = fixture.run_mixed(
        memory_system,
        route_delay,
        [first, second]
            .map(|event| event_u64(event, "lsq_data_response_tick"))
            .into_iter()
            .max()
            .unwrap()
            + 1,
    );
    assert_mixed_route_resources(&before, &through, memory_system);
}
fn assert_no_data_request_at_tick(json: &Value, tick: u64, pc: &str) {
    let records = data_request_sent_records(json)
        .into_iter()
        .filter(|record| event_u64(record, "tick") == tick)
        .collect::<Vec<_>>();
    assert!(
        records.is_empty(),
        "{pc} must use the MMIO path: {records:?}"
    );
}
fn assert_mixed_final_witness(completed: &Value) {
    assert_eq!(json_u64(completed, "/readfiles/0/bytes"), 8);
    assert_register(completed, "x11", "0x11");
    assert_register(completed, "x12", "0x33");
    assert_register(completed, "x13", "0x34");
    assert_eq!(
        page_witness_hex(completed, FIRST_PHYSICAL_PAGE).as_deref(),
        Some("110000000000000011000000000000002a00000000000000")
    );
    assert_eq!(
        memory_dump_hex(completed, FIRST_PHYSICAL_PAGE + 24),
        Some("3300000000000000")
    );
    assert_eq!(
        memory_dump_hex(completed, FIRST_PHYSICAL_PAGE + 32),
        Some("3400000000000000")
    );
}
fn assert_mixed_route_resources(before: &Value, through: &Value, memory_system: &str) {
    assert_eq!(
        fixture::resource_delta(before, through, "/memory_resources/transport/data/activity"),
        1
    );
    let expected = match memory_system {
        "direct" => [0; 9],
        "cache-fabric-dram" => [3, 1, 1, 1, 1, 2, 0, 1, 1],
        _ => unreachable!(),
    };
    for (pointer, expected) in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/cache/data/dram_accesses",
        "/memory_resources/cache/data/l1/activity",
        "/memory_resources/cache/data/l2/activity",
        "/memory_resources/cache/data/l3/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
        "/memory_resources/dram/accesses",
        "/memory_resources/dram/reads",
    ]
    .into_iter()
    .zip(expected)
    {
        assert_eq!(fixture::resource_delta(before, through, pointer), expected);
    }
}
