use super::*;

#[derive(Clone, Copy)]
struct ExpectedO3FuLatencyEvent {
    pc: &'static str,
    sequence: u64,
    class: &'static str,
    rename_writes: u64,
}

fn detailed_o3_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(42, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        0x0220_81b3,
        0x0220_c1b3,
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_mvv_type(0b100101, 2, 1, 3),
        vector_mvv_type(0b100000, 2, 1, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_mul_family_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_mvv_type(0b101101, 2, 1, 3),
        vector_mvv_type(0b111000, 2, 1, 8),
        vector_mvv_type(0b111100, 2, 1, 10),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_saturating_mul_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(2, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b100111, 0b000, 2, 1, 3),
        vector_arith_type(0b100111, 0b100, 2, 11, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 8, 1),
        vector_arith_type(0b010111, 0b100, 0, 8, 2),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x08, 2, 1, 0x0, 3),
        fp_r_type(0x0c, 2, 1, 0x0, 4),
        vector_arith_type(0b100100, 0b001, 2, 1, 3),
        vector_arith_type(0b100000, 0b001, 2, 1, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_extended_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        fp_r_type(0x78, 0, 8, 0x0, 3),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 8, 1),
        vector_arith_type(0b010111, 0b100, 0, 8, 2),
        vector_arith_type(0b010111, 0b100, 0, 8, 4),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x00, 2, 1, 0x0, 4),
        fp_r4_type(3, 0x0, 2, 1, 0x0, 5, 0x43),
        fp_r_type(0x2c, 0, 1, 0x0, 6),
        vector_arith_type(0b000000, 0b001, 2, 1, 3),
        vector_arith_type(0b101100, 0b001, 2, 1, 4),
        vector_arith_type(0b010011, 0b001, 1, 0, 5),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_compare_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 8, 1),
        vector_arith_type(0b010111, 0b100, 0, 8, 2),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x50, 2, 1, 0x2, 5),
        vector_arith_type(0b011000, 0b001, 2, 1, 3),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_misc_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(3, 0, 0x0, 9, 0x13),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 1, 1),
        vector_arith_type(0b010111, 0b100, 0, 2, 2),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x68, 0, 9, 0x0, 3),
        fp_r_type(0x10, 2, 1, 0x0, 4),
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3),
        vector_arith_type(0b001000, 0b001, 2, 1, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn run_o3_fu_latency_debug(path: std::path::PathBuf, max_tick: &str) -> (String, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            max_tick,
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    (stdout, json)
}

fn o3_fu_latency_record(json: &Value) -> &Value {
    json.pointer("/debug/o3_trace/0")
        .expect("first O3 trace record")
}

fn o3_fu_latency_events(record: &Value) -> &[Value] {
    record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array")
}

fn find_o3_fu_latency_event<'a>(events: &'a [Value], pc: &str, missing_label: &str) -> &'a Value {
    events
        .iter()
        .find(|event| json_record_str(event, "pc") == pc)
        .unwrap_or_else(|| panic!("missing {missing_label} O3 event at {pc}: {events:?}"))
}

fn assert_positive_o3_fu_latency_events(
    events: &[Value],
    expected_events: &[ExpectedO3FuLatencyEvent],
    missing_label: &str,
) -> BTreeMap<&'static str, Vec<u64>> {
    let mut latencies = BTreeMap::new();
    for expected in expected_events {
        let event = find_o3_fu_latency_event(events, expected.pc, missing_label);
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        latencies
            .entry(expected.class)
            .or_insert_with(Vec::new)
            .push(latency);
        assert_o3_event_with_fu(
            event,
            expected.sequence,
            expected.pc,
            expected.rename_writes,
            0,
            0,
            latency,
            Some(expected.class),
            false,
        );
    }
    latencies
}

#[allow(clippy::too_many_arguments)]
fn assert_o3_event_with_fu(
    event: &Value,
    sequence: u64,
    pc: &str,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    fu_latency_cycles: u64,
    fu_latency_class: Option<&str>,
    system_event: bool,
) {
    assert_eq!(json_record_u64(event, "sequence"), sequence);
    json_record_u64(event, "tick");
    assert_eq!(json_record_str(event, "pc"), pc);
    assert_eq!(json_record_bool(event, "rob_allocated"), true);
    assert_eq!(json_record_bool(event, "rob_committed"), true);
    assert_eq!(json_record_u64(event, "rename_writes"), rename_writes);
    assert_eq!(json_record_u64(event, "lsq_loads"), lsq_loads);
    assert_eq!(json_record_u64(event, "lsq_stores"), lsq_stores);
    assert_o3_event_lsq_address_shape(event, lsq_loads, lsq_stores);
    assert_eq!(
        json_record_u64(event, "fu_latency_cycles"),
        fu_latency_cycles
    );
    match fu_latency_class {
        Some(expected) => assert_eq!(json_record_str(event, "fu_latency_class"), expected),
        None => assert_eq!(event.get("fu_latency_class"), Some(&Value::Null)),
    }
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_candidate"),
        false
    );
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_match"),
        false
    );
    assert_eq!(json_record_bool(event, "system_event"), system_event);
}

fn o3_event_fu_latency_class_count(events: &[Value], class: &str) -> u64 {
    events
        .iter()
        .filter(|event| event.get("fu_latency_class").and_then(Value::as_str) == Some(class))
        .count() as u64
}

fn o3_fu_latency_class_cycles(latencies: &BTreeMap<&'static str, Vec<u64>>, class: &str) -> u64 {
    latencies
        .get(class)
        .unwrap_or_else(|| panic!("missing latency class {class}: {latencies:?}"))
        .iter()
        .sum()
}

fn o3_fu_latency_class_count(latencies: &BTreeMap<&'static str, Vec<u64>>, class: &str) -> u64 {
    latencies
        .get(class)
        .unwrap_or_else(|| panic!("missing latency class {class}: {latencies:?}"))
        .len() as u64
}

fn o3_fu_latency_total_cycles(latencies: &BTreeMap<&'static str, Vec<u64>>) -> u64 {
    latencies.values().flatten().copied().sum()
}

fn o3_fu_latency_stat_class(class: &str) -> &str {
    class.strip_prefix("scalar_").unwrap_or(class)
}

fn assert_o3_fu_latency_event_timing(events: &[Value]) -> (u64, u64, u64) {
    let event_ticks = events
        .iter()
        .map(|event| json_record_u64(event, "tick"))
        .collect::<Vec<_>>();
    assert!(
        event_ticks.windows(2).all(|window| window[0] < window[1]),
        "O3 event ticks should be strictly increasing: {event_ticks:?}"
    );
    for event in events {
        let issue_tick = json_record_u64(event, "issue_tick");
        let writeback_tick = json_record_u64(event, "writeback_tick");
        let commit_tick = json_record_u64(event, "commit_tick");
        assert!(
            issue_tick <= writeback_tick && writeback_tick <= commit_tick,
            "O3 issue/writeback/commit ticks must remain monotonic: {event}"
        );
    }
    let first_tick = event_ticks[0];
    let last_tick = *event_ticks.last().expect("O3 event tick");
    (first_tick, last_tick, last_tick.saturating_sub(first_tick))
}

fn assert_o3_fu_latency_stat_rows(stdout: &str, rows: &[(&str, &str, u64)]) {
    for (path, unit, value) in rows {
        assert_stat(stdout, path, unit, *value, "monotonic");
    }
}

fn assert_o3_fu_latency_totals(stdout: &str, instructions: u64, cycles: u64) {
    assert_o3_fu_latency_stat_rows(
        stdout,
        &[
            ("sim.debug.o3_trace.records", "Count", 1),
            (
                "sim.debug.o3_trace.fu_latency_instructions",
                "Count",
                instructions,
            ),
            ("sim.debug.o3_trace.fu_latency_cycles", "Cycle", cycles),
            (
                "sim.debug.o3_trace.event.fu_latency_instructions",
                "Count",
                instructions,
            ),
            (
                "sim.debug.o3_trace.event.fu_latency_cycles",
                "Cycle",
                cycles,
            ),
        ],
    );
}

fn assert_o3_fu_latency_class_stats(stdout: &str, class: &str, instructions: u64, cycles: u64) {
    let stat_class = o3_fu_latency_stat_class(class);
    assert_o3_fu_latency_stat_rows(
        stdout,
        &[
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_instructions"),
                "Count",
                instructions,
            ),
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_latency_cycles"),
                "Cycle",
                cycles,
            ),
        ],
    );
}

fn assert_o3_fu_latency_positive_class_stats(
    stdout: &str,
    latencies: &BTreeMap<&'static str, Vec<u64>>,
    class: &str,
) {
    assert_o3_fu_latency_class_stats(
        stdout,
        class,
        o3_fu_latency_class_count(latencies, class),
        o3_fu_latency_class_cycles(latencies, class),
    );
}

fn assert_o3_fu_latency_zero_class_stats(stdout: &str, classes: &[&str]) {
    for class in classes {
        assert_o3_fu_latency_class_stats(stdout, class, 0, 0);
    }
}

fn o3_fu_latency_class_latency_stats(
    latencies: &BTreeMap<&'static str, Vec<u64>>,
    class: &str,
) -> (u64, u64, u64, u64, u64) {
    let class_latencies = latencies
        .get(class)
        .unwrap_or_else(|| panic!("missing latency class {class}: {latencies:?}"));
    let count = class_latencies.len() as u64;
    let cycles = class_latencies.iter().sum::<u64>();
    let max = class_latencies.iter().copied().max().unwrap();
    let min = class_latencies.iter().copied().min().unwrap();
    (count, cycles, max, min, cycles / count)
}

fn assert_o3_fu_latency_class_aggregate_stats(
    stdout: &str,
    class: &str,
    count: u64,
    cycles: u64,
    max: u64,
    min: u64,
    avg: u64,
) {
    let stat_class = o3_fu_latency_stat_class(class);
    assert_o3_fu_latency_stat_rows(
        stdout,
        &[
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_instructions"),
                "Count",
                count,
            ),
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_latency_cycles"),
                "Cycle",
                cycles,
            ),
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_latency_max_cycles"),
                "Cycle",
                max,
            ),
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_latency_min_cycles"),
                "Cycle",
                min,
            ),
            (
                &format!("sim.debug.o3_trace.event.fu_{stat_class}_latency_avg_cycles"),
                "Cycle",
                avg,
            ),
        ],
    );
}

fn assert_o3_fu_latency_class_summary(
    fu_latency_class: &Value,
    class: &str,
    instructions: u64,
    cycles: u64,
) {
    let class_summary = fu_latency_class
        .pointer(&format!("/{class}"))
        .unwrap_or_else(|| panic!("missing FU latency class {class}: {fu_latency_class:?}"));
    assert_eq!(
        json_record_u64(class_summary, "instructions"),
        instructions,
        "O3 trace FU latency class {class} instructions"
    );
    assert_eq!(
        json_record_u64(class_summary, "cycles"),
        cycles,
        "O3 trace FU latency class {class} cycles"
    );
}

fn assert_o3_fu_latency_fixed_axis(fu_latency_class: &Value) {
    let expected_fu_classes = [
        "integer_mul",
        "integer_div",
        "float_add",
        "float_compare",
        "float_misc",
        "float_mul",
        "float_fma",
        "float_div",
        "float_sqrt",
        "vector_integer_mul",
        "vector_integer_div",
        "vector_float_add",
        "vector_float_compare",
        "vector_float_misc",
        "vector_float_mul",
        "vector_float_fma",
        "vector_float_div",
        "vector_float_sqrt",
    ];
    let fu_latency_class_object = fu_latency_class
        .as_object()
        .expect("debug O3 trace FU latency class object");
    assert_eq!(
        fu_latency_class_object.len(),
        expected_fu_classes.len(),
        "debug O3 trace FU latency class fixed axis: {fu_latency_class:?}"
    );
    for class in expected_fu_classes {
        assert!(
            fu_latency_class_object.contains_key(class),
            "debug O3 trace FU latency class should include {class}: {fu_latency_class:?}"
        );
    }
}

fn assert_o3_fu_latency_runtime_mirror(
    fu_latency_class: &Value,
    runtime: &Value,
    class: &str,
    instructions: u64,
    cycles: u64,
) {
    assert_o3_fu_latency_class_summary(fu_latency_class, class, instructions, cycles);
    let class_summary = fu_latency_class
        .pointer(&format!("/{class}"))
        .expect("checked FU latency class summary");
    assert_eq!(
        json_record_u64(class_summary, "instructions"),
        json_record_u64(runtime, &format!("fu_{class}_instructions")),
        "O3 trace should mirror runtime FU class {class} instructions"
    );
    assert_eq!(
        json_record_u64(class_summary, "cycles"),
        json_record_u64(runtime, &format!("fu_{class}_latency_cycles")),
        "O3 trace should mirror runtime FU class {class} cycles"
    );
}

#[test]
fn rem6_run_o3_debug_flag_emits_fu_latency_event_classes() {
    let path = detailed_o3_fu_latency_debug_binary("debug-flags-o3-fu-latency-runtime");
    let (stdout, json) = run_o3_fu_latency_debug(path, "140");
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];

    for (field, value) in [
        ("instructions", 5),
        ("rob_allocations", 5),
        ("rob_commits", 5),
        ("rename_writes", 4),
        ("lsq_loads", 0),
        ("lsq_stores", 0),
        ("fu_latency_instructions", 2),
        ("fu_latency_cycles", 21),
        ("fu_integer_mul_instructions", 1),
        ("fu_integer_mul_latency_cycles", 2),
        ("fu_integer_div_instructions", 1),
        ("fu_integer_div_latency_cycles", 19),
        ("max_rob_occupancy", 2),
        ("max_lsq_occupancy", 0),
        ("rename_map_entries", 3),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 trace field {field}"
        );
    }

    let events = o3_fu_latency_events(record);
    assert_eq!(events.len(), 5);
    let (first_tick, last_tick, tick_span) = assert_o3_fu_latency_event_timing(events);
    assert_o3_event_with_fu(&events[0], 0, "0x80000004", 1, 0, 0, 0, None, false);
    assert_o3_event_with_fu(&events[1], 1, "0x80000008", 1, 0, 0, 0, None, false);
    assert_o3_event_with_fu(
        &events[2],
        2,
        "0x8000000c",
        1,
        0,
        0,
        2,
        Some("scalar_integer_mul"),
        false,
    );
    assert_o3_event_with_fu(
        &events[3],
        3,
        "0x80000010",
        1,
        0,
        0,
        19,
        Some("scalar_integer_div"),
        false,
    );
    assert_o3_event_with_fu(&events[4], 4, "0x80000014", 0, 0, 0, 0, None, true);

    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    let event_fu_latency_instructions = events
        .iter()
        .filter(|event| json_record_u64(event, "fu_latency_cycles") > 0)
        .count() as u64;
    let event_fu_latency_cycles = events
        .iter()
        .map(|event| json_record_u64(event, "fu_latency_cycles"))
        .sum::<u64>();
    assert_eq!(
        json_record_u64(event_summary, "fu_latency_instructions"),
        event_fu_latency_instructions
    );
    assert_eq!(
        json_record_u64(event_summary, "fu_latency_cycles"),
        event_fu_latency_cycles
    );
    assert_eq!(json_record_u64(event_summary, "fu_latency_max_cycles"), 19);
    assert_eq!(json_record_u64(event_summary, "fu_latency_min_cycles"), 2);
    assert_eq!(json_record_u64(event_summary, "fu_latency_avg_cycles"), 10);
    let event_summary_fu_latency_class = event_summary
        .pointer("/fu_latency_class")
        .expect("O3 trace event summary FU latency class matrix");
    let event_summary_iq = event_summary
        .pointer("/iq")
        .expect("O3 trace event summary IQ matrix");
    let event_summary_issued_inst_type = event_summary_iq
        .pointer("/issued_inst_type")
        .expect("O3 trace event summary IQ issued-inst-type matrix");
    let event_summary_commit = event_summary
        .pointer("/commit")
        .expect("O3 trace event summary commit matrix");
    let event_summary_committed_inst_type = event_summary_commit
        .pointer("/committed_inst_type")
        .expect("O3 trace event summary commit committed-inst-type matrix");
    let event_iq_insts_issued = json_record_u64(event_summary_iq, "insts_issued");
    let event_iq_mem_insts_issued = json_record_u64(event_summary_iq, "mem_insts_issued");
    let event_iq_branch_insts_issued = json_record_u64(event_summary_iq, "branch_insts_issued");
    let event_commit_branch_mispredicts =
        json_record_u64(event_summary_commit, "branch_mispredicts");
    assert_eq!(event_iq_insts_issued, events.len() as u64);
    assert_eq!(event_iq_mem_insts_issued, 0);
    assert_eq!(event_iq_branch_insts_issued, 0);
    for (class, instructions, cycles) in [("integer_mul", 1, 2), ("integer_div", 1, 19)] {
        let class_summary = event_summary_fu_latency_class
            .pointer(&format!("/{class}"))
            .unwrap_or_else(|| {
                panic!(
                    "missing event summary FU latency class {class}: {event_summary_fu_latency_class:?}"
                )
            });
        assert_eq!(
            json_record_u64(class_summary, "instructions"),
            instructions,
            "event summary FU latency class {class} instructions"
        );
        assert_eq!(
            json_record_u64(class_summary, "cycles"),
            cycles,
            "event summary FU latency class {class} cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "max_cycles"),
            cycles,
            "event summary FU latency class {class} max cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "min_cycles"),
            cycles,
            "event summary FU latency class {class} min cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "avg_cycles"),
            cycles,
            "event summary FU latency class {class} avg cycles"
        );
    }
    for (trace_class, matrix_class) in [
        ("scalar_integer_mul", "int_mul"),
        ("scalar_integer_div", "int_div"),
    ] {
        let class_events = o3_event_fu_latency_class_count(events, trace_class);
        assert!(
            class_events > 0,
            "expected positive O3 event count for {trace_class}: {events:?}"
        );
        assert_eq!(
            event_summary_issued_inst_type
                .pointer(&format!("/{matrix_class}"))
                .and_then(Value::as_u64),
            Some(class_events),
            "event summary IQ issued-inst-type {matrix_class} should derive from emitted {trace_class} events: {event_summary_issued_inst_type}"
        );
        assert_eq!(
            event_summary_committed_inst_type
                .pointer(&format!("/{matrix_class}"))
                .and_then(Value::as_u64),
            Some(class_events),
            "event summary commit committed-inst-type {matrix_class} should derive from emitted {trace_class} events: {event_summary_committed_inst_type}"
        );
    }
    assert_o3_fu_latency_stat_rows(
        &stdout,
        &[
            ("sim.debug.o3_trace.records", "Count", 1),
            ("sim.debug.o3_trace.instructions", "Count", 5),
            ("sim.debug.o3_trace.fu_latency_instructions", "Count", 2),
            ("sim.debug.o3_trace.fu_latency_cycles", "Cycle", 21),
            ("sim.debug.o3_trace.fu_integer_mul_instructions", "Count", 1),
            (
                "sim.debug.o3_trace.fu_integer_mul_latency_cycles",
                "Cycle",
                2,
            ),
            ("sim.debug.o3_trace.fu_integer_div_instructions", "Count", 1),
            (
                "sim.debug.o3_trace.fu_integer_div_latency_cycles",
                "Cycle",
                19,
            ),
            ("sim.debug.o3_trace.event.records", "Count", 5),
            ("sim.debug.o3_trace.event.first_tick", "Tick", first_tick),
            ("sim.debug.o3_trace.event.last_tick", "Tick", last_tick),
            ("sim.debug.o3_trace.event.tick_span", "Tick", tick_span),
            (
                "sim.debug.o3_trace.event.iq_insts_issued",
                "Count",
                event_iq_insts_issued,
            ),
            (
                "sim.debug.o3_trace.event.iq_mem_insts_issued",
                "Count",
                event_iq_mem_insts_issued,
            ),
            (
                "sim.debug.o3_trace.event.iq_branch_insts_issued",
                "Count",
                event_iq_branch_insts_issued,
            ),
            (
                "sim.debug.o3_trace.event.iq_issued_inst_type.mem_read",
                "Count",
                json_record_u64(event_summary_issued_inst_type, "mem_read"),
            ),
            (
                "sim.debug.o3_trace.event.iq_issued_inst_type.mem_write",
                "Count",
                json_record_u64(event_summary_issued_inst_type, "mem_write"),
            ),
            (
                "sim.debug.o3_trace.event.iq_issued_inst_type.int_mul",
                "Count",
                json_record_u64(event_summary_issued_inst_type, "int_mul"),
            ),
            (
                "sim.debug.o3_trace.event.iq_issued_inst_type.int_div",
                "Count",
                json_record_u64(event_summary_issued_inst_type, "int_div"),
            ),
            (
                "sim.debug.o3_trace.event.commit_branch_mispredicts",
                "Count",
                event_commit_branch_mispredicts,
            ),
            (
                "sim.debug.o3_trace.event.commit_committed_inst_type.mem_read",
                "Count",
                json_record_u64(event_summary_committed_inst_type, "mem_read"),
            ),
            (
                "sim.debug.o3_trace.event.commit_committed_inst_type.mem_write",
                "Count",
                json_record_u64(event_summary_committed_inst_type, "mem_write"),
            ),
            (
                "sim.debug.o3_trace.event.commit_committed_inst_type.int_mul",
                "Count",
                json_record_u64(event_summary_committed_inst_type, "int_mul"),
            ),
            (
                "sim.debug.o3_trace.event.commit_committed_inst_type.int_div",
                "Count",
                json_record_u64(event_summary_committed_inst_type, "int_div"),
            ),
            (
                "sim.debug.o3_trace.event.fu_latency_instructions",
                "Count",
                2,
            ),
            ("sim.debug.o3_trace.event.fu_latency_cycles", "Cycle", 21),
            (
                "sim.debug.o3_trace.event.fu_latency_max_cycles",
                "Cycle",
                19,
            ),
            ("sim.debug.o3_trace.event.fu_latency_min_cycles", "Cycle", 2),
            (
                "sim.debug.o3_trace.event.fu_latency_avg_cycles",
                "Cycle",
                10,
            ),
        ],
    );
    assert_o3_fu_latency_class_aggregate_stats(&stdout, "integer_mul", 1, 2, 2, 2, 2);
    assert_o3_fu_latency_class_aggregate_stats(&stdout, "integer_div", 1, 19, 19, 19, 19);
}

#[test]
fn rem6_run_o3_debug_flag_classifies_vector_integer_fu_latency_events() {
    let path =
        detailed_o3_vector_fu_latency_debug_binary("debug-flags-o3-vector-fu-latency-runtime");
    let (stdout, json) = run_o3_fu_latency_debug(path, "180");
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    let events = o3_fu_latency_events(record);
    assert!(
        events.len() >= 5,
        "expected vector FU latency events: {events:?}"
    );

    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x8000000c",
                sequence: 2,
                class: "vector_integer_mul",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000010",
                sequence: 3,
                class: "vector_integer_div",
                rename_writes: 0,
            },
        ],
        "vector FU latency",
    );
    let vector_mul_latency = o3_fu_latency_class_cycles(&latencies, "vector_integer_mul");
    let vector_div_latency = o3_fu_latency_class_cycles(&latencies, "vector_integer_div");
    assert!(
        vector_div_latency > vector_mul_latency,
        "vector div should model a longer FU latency than vector mul: mul={vector_mul_latency}, div={vector_div_latency}"
    );
    assert_o3_fu_latency_totals(&stdout, 2, vector_mul_latency + vector_div_latency);
    assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, "vector_integer_mul");
    assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, "vector_integer_div");
}

#[test]
fn rem6_run_o3_debug_flag_classifies_vector_mul_family_fu_latency_events() {
    let path = detailed_o3_vector_mul_family_fu_latency_debug_binary(
        "debug-flags-o3-vector-mul-family-fu-latency-runtime",
    );
    let (stdout, json) = run_o3_fu_latency_debug(path, "220");
    let events = o3_fu_latency_events(o3_fu_latency_record(&json));
    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x8000000c",
                sequence: 2,
                class: "vector_integer_mul",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000010",
                sequence: 3,
                class: "vector_integer_mul",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000014",
                sequence: 4,
                class: "vector_integer_mul",
                rename_writes: 0,
            },
        ],
        "vector multiply-family",
    );
    let vector_mul_cycles = o3_fu_latency_total_cycles(&latencies);
    assert_o3_fu_latency_totals(&stdout, 3, vector_mul_cycles);
    assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, "vector_integer_mul");
    assert_o3_fu_latency_zero_class_stats(&stdout, &["vector_integer_div"]);
}

#[test]
fn rem6_run_o3_debug_flag_classifies_vector_saturating_mul_fu_latency_events() {
    let path = detailed_o3_vector_saturating_mul_fu_latency_debug_binary(
        "debug-flags-o3-vector-saturating-mul-fu-latency-runtime",
    );
    let (stdout, json) = run_o3_fu_latency_debug(path, "220");
    let events = o3_fu_latency_events(o3_fu_latency_record(&json));
    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x80000010",
                sequence: 3,
                class: "vector_integer_mul",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000014",
                sequence: 4,
                class: "vector_integer_mul",
                rename_writes: 0,
            },
        ],
        "vector saturating multiply",
    );
    let vector_mul_cycles = o3_fu_latency_total_cycles(&latencies);
    assert_o3_fu_latency_totals(&stdout, 2, vector_mul_cycles);
    assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, "vector_integer_mul");
    assert_o3_fu_latency_zero_class_stats(&stdout, &["vector_integer_div"]);
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_fu_latency_events() {
    let path = detailed_o3_float_fu_latency_debug_binary("debug-flags-o3-float-fu-latency-runtime");
    let (stdout, json) = run_o3_fu_latency_debug(path, "220");
    let record = o3_fu_latency_record(&json);
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 4);
    let events = o3_fu_latency_events(record);
    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x80000020",
                sequence: 0,
                class: "scalar_float_mul",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000024",
                sequence: 1,
                class: "scalar_float_div",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000028",
                sequence: 2,
                class: "vector_float_mul",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x8000002c",
                sequence: 3,
                class: "vector_float_div",
                rename_writes: 0,
            },
        ],
        "float FU latency",
    );
    let float_cycles = o3_fu_latency_total_cycles(&latencies);
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        float_cycles,
        "{record:?}"
    );

    let runtime = json
        .pointer("/cores/0/o3_runtime")
        .expect("runtime O3 summary");
    let fu_latency_class = record
        .pointer("/fu_latency_class")
        .expect("debug O3 trace FU latency class matrix");
    assert_o3_fu_latency_fixed_axis(fu_latency_class);
    for (class, instructions, cycles) in [
        (
            "float_mul",
            1,
            o3_fu_latency_class_cycles(&latencies, "scalar_float_mul"),
        ),
        (
            "float_div",
            1,
            o3_fu_latency_class_cycles(&latencies, "scalar_float_div"),
        ),
        (
            "vector_float_mul",
            1,
            o3_fu_latency_class_cycles(&latencies, "vector_float_mul"),
        ),
        (
            "vector_float_div",
            1,
            o3_fu_latency_class_cycles(&latencies, "vector_float_div"),
        ),
        ("float_add", 0, 0),
        ("integer_mul", 0, 0),
        ("integer_div", 0, 0),
        ("vector_integer_mul", 0, 0),
    ] {
        assert_o3_fu_latency_runtime_mirror(fu_latency_class, runtime, class, instructions, cycles);
    }

    assert_o3_fu_latency_totals(&stdout, 4, float_cycles);
    for class in [
        "scalar_float_mul",
        "scalar_float_div",
        "vector_float_mul",
        "vector_float_div",
    ] {
        assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, class);
    }
    assert_o3_fu_latency_zero_class_stats(
        &stdout,
        &[
            "integer_mul",
            "integer_div",
            "vector_integer_mul",
            "vector_integer_div",
        ],
    );
}

#[test]
fn rem6_run_o3_debug_flag_classifies_extended_float_fu_latency_events() {
    let path = detailed_o3_float_extended_fu_latency_debug_binary(
        "debug-flags-o3-float-extended-fu-latency-runtime",
    );
    let (stdout, json) = run_o3_fu_latency_debug(path, "260");
    let record = o3_fu_latency_record(&json);
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 6);
    let events = o3_fu_latency_events(record);
    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x80000028",
                sequence: 0,
                class: "scalar_float_add",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x8000002c",
                sequence: 1,
                class: "scalar_float_fma",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000030",
                sequence: 2,
                class: "scalar_float_sqrt",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000034",
                sequence: 3,
                class: "vector_float_add",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000038",
                sequence: 4,
                class: "vector_float_fma",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x8000003c",
                sequence: 5,
                class: "vector_float_sqrt",
                rename_writes: 0,
            },
        ],
        "extended float FU latency",
    );
    let float_cycles = o3_fu_latency_total_cycles(&latencies);
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        float_cycles,
        "{record:?}"
    );

    assert_o3_fu_latency_totals(&stdout, 6, float_cycles);
    for class in [
        "scalar_float_add",
        "scalar_float_fma",
        "scalar_float_sqrt",
        "vector_float_add",
        "vector_float_fma",
        "vector_float_sqrt",
    ] {
        assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, class);
    }
    assert_o3_fu_latency_zero_class_stats(
        &stdout,
        &[
            "float_mul",
            "float_div",
            "vector_float_mul",
            "vector_float_div",
            "integer_mul",
            "integer_div",
            "vector_integer_mul",
            "vector_integer_div",
        ],
    );
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_compare_fu_latency_events() {
    let path =
        detailed_o3_float_compare_fu_latency_debug_binary("debug-flags-o3-float-compare-runtime");
    let (stdout, json) = run_o3_fu_latency_debug(path, "220");
    let record = o3_fu_latency_record(&json);
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 2);
    let events = o3_fu_latency_events(record);
    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x80000020",
                sequence: 0,
                class: "scalar_float_compare",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000024",
                sequence: 1,
                class: "vector_float_compare",
                rename_writes: 0,
            },
        ],
        "float compare",
    );
    let compare_cycles = o3_fu_latency_total_cycles(&latencies);
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        compare_cycles,
        "{record:?}"
    );

    assert_o3_fu_latency_totals(&stdout, 2, compare_cycles);
    assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, "scalar_float_compare");
    assert_o3_fu_latency_positive_class_stats(&stdout, &latencies, "vector_float_compare");
    assert_o3_fu_latency_zero_class_stats(
        &stdout,
        &[
            "float_add",
            "float_mul",
            "float_fma",
            "float_div",
            "float_sqrt",
            "vector_float_add",
            "vector_float_mul",
            "vector_float_fma",
            "vector_float_div",
            "vector_float_sqrt",
            "integer_mul",
            "integer_div",
            "vector_integer_mul",
            "vector_integer_div",
        ],
    );
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_misc_fu_latency_events() {
    let path = detailed_o3_float_misc_fu_latency_debug_binary("debug-flags-o3-float-misc-runtime");
    let (stdout, json) = run_o3_fu_latency_debug(path, "260");
    let record = o3_fu_latency_record(&json);
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 4);
    let events = o3_fu_latency_events(record);
    let latencies = assert_positive_o3_fu_latency_events(
        events,
        &[
            ExpectedO3FuLatencyEvent {
                pc: "0x80000024",
                sequence: 0,
                class: "scalar_float_misc",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000028",
                sequence: 1,
                class: "scalar_float_misc",
                rename_writes: 1,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x8000002c",
                sequence: 2,
                class: "vector_float_misc",
                rename_writes: 0,
            },
            ExpectedO3FuLatencyEvent {
                pc: "0x80000030",
                sequence: 3,
                class: "vector_float_misc",
                rename_writes: 0,
            },
        ],
        "float misc",
    );
    let misc_cycles = o3_fu_latency_total_cycles(&latencies);
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        misc_cycles,
        "{record:?}"
    );
    let (scalar_count, scalar_cycles, scalar_max, scalar_min, scalar_avg) =
        o3_fu_latency_class_latency_stats(&latencies, "scalar_float_misc");
    let (vector_count, vector_cycles, vector_max, vector_min, vector_avg) =
        o3_fu_latency_class_latency_stats(&latencies, "vector_float_misc");

    assert_o3_fu_latency_totals(&stdout, 4, misc_cycles);
    assert_o3_fu_latency_class_aggregate_stats(
        &stdout,
        "scalar_float_misc",
        scalar_count,
        scalar_cycles,
        scalar_max,
        scalar_min,
        scalar_avg,
    );
    assert_o3_fu_latency_class_aggregate_stats(
        &stdout,
        "vector_float_misc",
        vector_count,
        vector_cycles,
        vector_max,
        vector_min,
        vector_avg,
    );
    assert_o3_fu_latency_zero_class_stats(
        &stdout,
        &[
            "float_add",
            "float_compare",
            "float_mul",
            "float_fma",
            "float_div",
            "float_sqrt",
            "vector_float_add",
            "vector_float_compare",
            "vector_float_mul",
            "vector_float_fma",
            "vector_float_div",
            "vector_float_sqrt",
            "integer_mul",
            "integer_div",
            "vector_integer_mul",
            "vector_integer_div",
        ],
    );
}
