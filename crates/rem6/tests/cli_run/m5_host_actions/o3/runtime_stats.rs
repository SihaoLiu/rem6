use super::*;

#[test]
fn rem6_run_o3_runtime_json_exposes_iq_iew_commit_matrices() {
    let path =
        detailed_o3_iq_iew_commit_matrix_binary("m5-switch-cpu-detailed-o3-iq-iew-commit-json");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x800000a0:16",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("78563412785634120000000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    let event_window = o3_runtime.pointer("/event_window").unwrap_or_else(|| {
        panic!("O3 runtime JSON should expose event-window state: {o3_runtime}")
    });
    let runtime_instructions = o3_runtime
        .pointer("/instructions")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose instructions: {o3_runtime}"));
    assert_eq!(
        event_window.pointer("/records").and_then(Value::as_u64),
        Some(runtime_instructions),
        "O3 runtime event window should count the same executed trace rows as instructions: {event_window}"
    );
    assert!(
        event_window
            .pointer("/span_ticks")
            .and_then(Value::as_u64)
            .is_some_and(|span| span > 0),
        "O3 runtime event window should expose a positive tick span: {event_window}"
    );
    let first_event = event_window.pointer("/first").unwrap_or_else(|| {
        panic!("O3 runtime event window should expose first event: {event_window}")
    });
    let last_event = event_window.pointer("/last").unwrap_or_else(|| {
        panic!("O3 runtime event window should expose last event: {event_window}")
    });
    assert!(
        first_event
            .pointer("/pc")
            .and_then(Value::as_str)
            .is_some_and(|pc| pc.starts_with("0x")),
        "first O3 runtime event should expose a hex PC: {first_event}"
    );
    assert!(
        last_event.pointer("/sequence").and_then(Value::as_u64)
            >= first_event.pointer("/sequence").and_then(Value::as_u64),
        "O3 runtime event window should keep sequence order: {event_window}"
    );
    assert!(
        last_event.pointer("/tick").and_then(Value::as_u64)
            >= first_event.pointer("/tick").and_then(Value::as_u64),
        "O3 runtime event window should keep tick order: {event_window}"
    );
    let max_rob_event = event_window
        .pointer("/max_rob_occupancy")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max ROB row: {event_window}")
        });
    let max_lsq_event = event_window
        .pointer("/max_lsq_occupancy")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max LSQ row: {event_window}")
        });
    let max_rename_event = event_window
        .pointer("/max_rename_map_entries")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max rename row: {event_window}")
        });
    assert_eq!(
        max_rob_event.pointer("/rob_occupancy"),
        o3_runtime.pointer("/rob/max_occupancy"),
        "O3 runtime event window should identify the max ROB occupancy row: {event_window}"
    );
    assert_eq!(
        max_lsq_event.pointer("/lsq_occupancy"),
        o3_runtime.pointer("/lsq/max_occupancy"),
        "O3 runtime event window should identify the max LSQ occupancy row: {event_window}"
    );
    assert!(
        max_rename_event
            .pointer("/rename_map_entries")
            .and_then(Value::as_u64)
            >= o3_runtime
                .pointer("/rename/map_entries")
                .and_then(Value::as_u64),
        "O3 runtime event window should identify a rename-map high-water row: {event_window}"
    );

    for (pointer, stat_path) in [
        (
            "/iq/issued_inst_type/mem_read",
            "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        ),
        (
            "/iq/issued_inst_type/mem_write",
            "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        ),
        (
            "/iq/issued_inst_type/int_mul",
            "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        ),
        (
            "/iq/issued_inst_type/int_div",
            "sim.cpu0.o3.iq.issued_inst_type.int_div",
        ),
        (
            "/iq/issued_inst_type/float_misc",
            "sim.cpu0.o3.iq.issued_inst_type.float_misc",
        ),
        (
            "/iq/issued_inst_type/vector_float_misc",
            "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        ),
        (
            "/commit/committed_inst_type/mem_read",
            "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        ),
        (
            "/commit/committed_inst_type/mem_write",
            "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        ),
        (
            "/commit/committed_inst_type/int_mul",
            "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        ),
        (
            "/commit/committed_inst_type/int_div",
            "sim.cpu0.o3.commit.committed_inst_type.int_div",
        ),
        (
            "/commit/committed_inst_type/float_misc",
            "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        ),
        (
            "/commit/committed_inst_type/vector_float_misc",
            "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        ),
    ] {
        let structured = o3_runtime
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
            });
        let stat_value = json_stat_value(&json, stat_path);
        assert_eq!(
            structured, stat_value,
            "structured O3 runtime {pointer} should match stat path {stat_path}"
        );
        assert!(
            structured > 0,
            "representative O3 runtime matrix lane {pointer} should be positive: {o3_runtime}"
        );
    }
    assert_o3_runtime_json_preserves_legacy_inst_type_alias_order(&json);

    for (pointer, stat_path) in [
        ("/iq/insts_issued", "sim.cpu0.o3.iq.insts_issued"),
        ("/iq/mem_insts_issued", "sim.cpu0.o3.iq.mem_insts_issued"),
        (
            "/iq/branch_insts_issued",
            "sim.cpu0.o3.iq.branch_insts_issued",
        ),
        ("/iew/dispatched_insts", "sim.cpu0.o3.iew.dispatched_insts"),
        ("/iew/insts_to_commit", "sim.cpu0.o3.iew.insts_to_commit"),
        ("/iew/writeback_count", "sim.cpu0.o3.iew.writeback_count"),
        ("/iew/producer_inst", "sim.cpu0.o3.iew.producer_inst"),
        ("/iew/consumer_inst", "sim.cpu0.o3.iew.consumer_inst"),
        (
            "/iew/predicted_taken_incorrect",
            "sim.cpu0.o3.iew.predicted_taken_incorrect",
        ),
        (
            "/iew/predicted_not_taken_incorrect",
            "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        ),
        (
            "/iew/branch_mispredicts",
            "sim.cpu0.o3.iew.branch_mispredicts",
        ),
        (
            "/commit/branch_mispredicts",
            "sim.cpu0.o3.commit.branch_mispredicts",
        ),
    ] {
        let structured = o3_runtime
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
            });
        assert_eq!(
            structured,
            json_stat_value(&json, stat_path),
            "structured O3 runtime {pointer} should match stat path {stat_path}"
        );
    }
}

fn assert_o3_runtime_json_preserves_legacy_inst_type_alias_order(json: &Value) {
    let mut expected_paths = Vec::new();
    for alias in ["MemRead", "MemWrite", "IntMult", "IntDiv"] {
        push_o3_runtime_inst_type_alias_pair(&mut expected_paths, "iq.issuedInstType", alias);
    }
    for alias in ["MemRead", "MemWrite", "IntMult", "IntDiv"] {
        push_o3_runtime_inst_type_alias_pair(
            &mut expected_paths,
            "commit.committedInstType",
            alias,
        );
    }
    for alias in [
        "FloatAdd",
        "FloatCmp",
        "FloatMisc",
        "FloatMult",
        "FloatMultAcc",
        "FloatDiv",
        "FloatSqrt",
        "SimdMult",
        "SimdDiv",
        "SimdFloatAdd",
        "SimdFloatCmp",
        "SimdFloatMisc",
        "SimdFloatMult",
        "SimdFloatMultAcc",
        "SimdFloatDiv",
        "SimdFloatSqrt",
    ] {
        push_o3_runtime_inst_type_alias_pair(&mut expected_paths, "iq.issuedInstType", alias);
    }
    for alias in [
        "FloatAdd",
        "FloatCmp",
        "FloatMisc",
        "FloatMult",
        "FloatMultAcc",
        "FloatDiv",
        "FloatSqrt",
        "SimdMult",
        "SimdDiv",
        "SimdFloatAdd",
        "SimdFloatCmp",
        "SimdFloatMisc",
        "SimdFloatMult",
        "SimdFloatMultAcc",
        "SimdFloatDiv",
        "SimdFloatSqrt",
    ] {
        push_o3_runtime_inst_type_alias_pair(
            &mut expected_paths,
            "commit.committedInstType",
            alias,
        );
    }

    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array in run JSON: {json}"));
    let filtered = stats
        .iter()
        .filter_map(|sample| {
            let path = sample.pointer("/path").and_then(Value::as_str)?;
            let relevant = path.starts_with("system.cpu.iq.issuedInstType.")
                || path.starts_with("system.cpu.iq.issuedInstType_0::")
                || path.starts_with("system.cpu.commit.committedInstType.")
                || path.starts_with("system.cpu.commit.committedInstType_0::");
            relevant.then(|| {
                let id = sample
                    .pointer("/id")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| panic!("missing derived sample id for {path}: {sample}"));
                (path.to_string(), id)
            })
        })
        .collect::<Vec<_>>();
    let actual_paths = filtered
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>();
    let expected_paths = expected_paths
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        actual_paths, expected_paths,
        "system.cpu O3 inst-type aliases should preserve legacy derived record order"
    );
    for window in filtered.windows(2) {
        let previous = &window[0];
        let current = &window[1];
        assert_eq!(
            current.1,
            previous.1.saturating_add(1),
            "filtered O3 inst-type alias IDs should be contiguous: previous={previous:?}, current={current:?}"
        );
    }
}

fn push_o3_runtime_inst_type_alias_pair(paths: &mut Vec<String>, family: &str, alias: &str) {
    paths.push(format!("system.cpu.{family}.{alias}"));
    paths.push(format!("system.cpu.{family}_0::{alias}"));
}

#[test]
fn rem6_run_text_stats_alias_o3_runtime_stats_after_detailed_switch() {
    let path = detailed_o3_runtime_stats_binary("m5-switch-cpu-detailed-o3-runtime-text-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.instructions", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rob_allocations", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rob_commits", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rename_writes", 4);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_loads", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_stores", 1);
    assert_text_byte_stat(&stdout, "sim.cpu0.o3.lsq_load_bytes", 4);
    assert_text_byte_stat(&stdout, "sim.cpu0.o3.lsq_store_bytes", 4);
    assert_text_count_stat(&stdout, "system.cpu.rename.renamedInsts", 6);
    assert_text_count_stat(&stdout, "system.cpu.rename.renamedOperands", 4);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispatchedInsts", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispLoadInsts", 1);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispStoreInsts", 1);
    assert_text_count_stat(&stdout, "system.cpu.iew.instsToCommit.total", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.instsToCommit::total", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.writebackCount.total", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.writebackCount::total", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.producerInst.total", 3);
    assert_text_count_stat(&stdout, "system.cpu.iew.producerInst::total", 3);
    assert_text_count_stat(&stdout, "system.cpu.iew.consumerInst.total", 4);
    assert_text_count_stat(&stdout, "system.cpu.iew.consumerInst::total", 4);
    let writeback_rate_ppm = text_stat_u64(&stdout, "sim.cpu0.o3.iew.writeback_rate_ppm");
    assert_text_ppm_stat(&stdout, "system.cpu.iew.wbRate", writeback_rate_ppm);
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.wbRate");
    let producer_consumer_fanout_ppm =
        text_stat_u64(&stdout, "sim.cpu0.o3.iew.producer_consumer_fanout_ppm");
    assert_text_ppm_stat(
        &stdout,
        "system.cpu.iew.wbFanout",
        producer_consumer_fanout_ppm,
    );
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.wbFanout");
    assert_text_count_stat(&stdout, "system.cpu.lsq0.addedLoadsAndStores", 2);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.maxOccupancy", 1);
    assert_text_byte_stat(&stdout, "system.cpu.lsq0.loadBytes", 4);
    assert_text_byte_stat(&stdout, "system.cpu.lsq0.storeBytes", 4);
    let forwarding_matches =
        text_stat_u64(&stdout, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
    assert_text_count_stat(&stdout, "system.cpu.lsq0.forwLoads", forwarding_matches);
    assert_text_count_stat(&stdout, "system.cpu.iq.instsIssued", 6);
    assert_text_count_stat(&stdout, "system.cpu.iq.memInstsIssued", 2);
    assert_text_stat_occurs_once(&stdout, "system.cpu.lsq0.forwLoads");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iq.instsIssued");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iq.memInstsIssued");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iq.branchInstsIssued");
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::MemRead", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::MemWrite", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType_0::MemRead", 1);
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.committedInstType_0::MemWrite",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.rob.writes", 6);
    assert_text_count_stat(&stdout, "system.cpu.rob.reads", 6);
    assert_text_count_stat(&stdout, "system.cpu.rob.maxOccupancy", 1);
}

#[test]
fn rem6_run_does_not_record_o3_runtime_stats_after_timing_switch() {
    let path = timing_switch_o3_stats_binary("m5-switch-cpu-timing-no-o3-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--m5-switch-cpu-mode",
            "timing",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.rob_allocations");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_load_bytes");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_bytes");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_data_latency_samples");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_data_latency_ticks");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_operation.load_latency_ticks");
    assert_json_stat_absent(
        &json,
        "sim.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_mul_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_mul_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_div_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_div_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_mul_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_mul_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_div_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_div_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.max_rob_occupancy");
    assert_json_stat_absent(&json, "sim.cpu0.o3.max_lsq_occupancy");
    assert_json_stat_absent(&json, "sim.cpu0.o3.rename_map_entries");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.mem_insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.branch_insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.mem_read");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.mem_write");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.int_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.int_div");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.vector_integer_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.vector_integer_div");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.mem_read");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.mem_write");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.int_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.int_div");
    assert_json_stat_absent(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.vector_integer_mul",
    );
    assert_json_stat_absent(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.vector_integer_div",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.dispatched_insts");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.insts_to_commit");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.writeback_count");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.producer_inst");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.consumer_inst");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.writeback_rate_ppm");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.producer_consumer_fanout_ppm");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.predicted_taken_incorrect");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.predicted_not_taken_incorrect");
    assert_json_stat_absent(&json, "system.cpu.iew.dispatchedInsts");
    assert_json_stat_absent(&json, "system.cpu.iew.instsToCommit.total");
    assert_json_stat_absent(&json, "system.cpu.iew.instsToCommit::total");
    assert_json_stat_absent(&json, "system.cpu.iew.writebackCount.total");
    assert_json_stat_absent(&json, "system.cpu.iew.writebackCount::total");
    assert_json_stat_absent(&json, "system.cpu.iew.producerInst.total");
    assert_json_stat_absent(&json, "system.cpu.iew.producerInst::total");
    assert_json_stat_absent(&json, "system.cpu.iew.consumerInst.total");
    assert_json_stat_absent(&json, "system.cpu.iew.consumerInst::total");
    assert_json_stat_absent(&json, "system.cpu.iew.wbRate");
    assert_json_stat_absent(&json, "system.cpu.iew.wbFanout");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.MemRead");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.MemWrite");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.IntMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.IntDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatAdd");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatCmp");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatMisc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatMultAcc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatAdd");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatCmp");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatMisc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatMultAcc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.MemRead");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.MemWrite");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.IntMult");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.IntDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatAdd");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatCmp");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatMisc");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatMult");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatMultAcc");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdMult");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatAdd");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatCmp");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatMisc");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatMult");
    assert_json_stat_absent(
        &json,
        "system.cpu.commit.committedInstType.SimdFloatMultAcc",
    );
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.iew.predictedTakenIncorrect");
    assert_json_stat_absent(&json, "system.cpu.iew.predictedNotTakenIncorrect");
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.samples");
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.totalLatency");
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.load.totalLatency");
    assert_json_stat_absent(
        &json,
        "system.cpu.lsq0.dataResponse.storeConditional.totalLatency",
    );
    assert_json_stat_absent(&json, "system.cpu.lsq0.operation.load");
    assert_json_stat_absent(&json, "system.cpu.lsq0.operation.total");
    assert_json_stat_absent(&json, "system.cpu.lsq0.ordering.acquire");
    assert_json_stat_absent(&json, "system.cpu.lsq0.ordering.total");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.targetlessMismatch");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.directionOnly");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.wrongTarget");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.total");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::TargetlessMismatch");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::DirectionOnly");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::WrongTarget");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::total");
    assert_json_stat_absent(&json, "system.cpu.iew.branchMispredicts");
    assert_json_stat_absent(&json, "system.cpu.commit.branchMispredicts");
    assert_json_stat_absent(&json, "system.cpu.rob.maxOccupancy");
    assert_json_stat_absent(&json, "system.cpu.lsq0.maxOccupancy");
    for path in [
        "enqueued_rows",
        "service_turns",
        "wake_requests",
        "current_occupancy",
        "peak_occupancy",
        "issued_by_class.scalar_integer",
        "issued_by_class.integer_mul_div",
        "issued_by_class.memory_agu",
        "issued_by_class.control",
    ] {
        assert_json_stat_absent(&json, &format!("sim.cpu0.o3.issue_queue.{path}"));
    }
    assert!(
        json.pointer("/cores/0/o3_runtime").is_none(),
        "timing-mode run should not emit inactive O3 runtime state: {json}"
    );
}

#[test]
fn rem6_run_text_stats_omit_o3_runtime_aliases_after_timing_switch() {
    let path = timing_switch_o3_stats_binary("m5-switch-cpu-timing-no-o3-runtime-text-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
            "--m5-switch-cpu-mode",
            "timing",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for path in [
        "sim.cpu0.o3.instructions",
        "sim.cpu0.o3.rob_allocations",
        "sim.cpu0.o3.rob_commits",
        "sim.cpu0.o3.rename_writes",
        "sim.cpu0.o3.lsq_loads",
        "sim.cpu0.o3.lsq_stores",
        "sim.cpu0.o3.lsq_load_bytes",
        "sim.cpu0.o3.lsq_store_bytes",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.cpu0.o3.lsq_data_latency_samples",
        "sim.cpu0.o3.lsq_data_latency_ticks",
        "sim.cpu0.o3.lsq_operation.load_latency_ticks",
        "sim.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
        "sim.cpu0.o3.fu_latency_instructions",
        "sim.cpu0.o3.fu_latency_cycles",
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "sim.cpu0.o3.fu_integer_div_instructions",
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "sim.cpu0.o3.max_rob_occupancy",
        "sim.cpu0.o3.max_lsq_occupancy",
        "sim.cpu0.o3.rename_map_entries",
        "sim.cpu0.o3.iq.insts_issued",
        "sim.cpu0.o3.iq.mem_insts_issued",
        "sim.cpu0.o3.iq.branch_insts_issued",
        "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        "sim.cpu0.o3.iq.issued_inst_type.int_div",
        "sim.cpu0.o3.issue_queue.enqueued_rows",
        "sim.cpu0.o3.issue_queue.service_turns",
        "sim.cpu0.o3.issue_queue.wake_requests",
        "sim.cpu0.o3.issue_queue.current_occupancy",
        "sim.cpu0.o3.issue_queue.peak_occupancy",
        "sim.cpu0.o3.issue_queue.issued_by_class.scalar_integer",
        "sim.cpu0.o3.issue_queue.issued_by_class.integer_mul_div",
        "sim.cpu0.o3.issue_queue.issued_by_class.memory_agu",
        "sim.cpu0.o3.issue_queue.issued_by_class.control",
        "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        "sim.cpu0.o3.commit.committed_inst_type.int_div",
        "system.cpu.rename.renamedInsts",
        "system.cpu.rename.renamedOperands",
        "system.cpu.iew.dispatchedInsts",
        "system.cpu.iew.dispLoadInsts",
        "system.cpu.iew.dispStoreInsts",
        "system.cpu.iew.instsToCommit.total",
        "system.cpu.iew.instsToCommit::total",
        "sim.cpu0.o3.iew.writeback_count",
        "sim.cpu0.o3.iew.producer_inst",
        "sim.cpu0.o3.iew.consumer_inst",
        "sim.cpu0.o3.iew.writeback_rate_ppm",
        "sim.cpu0.o3.iew.producer_consumer_fanout_ppm",
        "system.cpu.iew.writebackCount.total",
        "system.cpu.iew.writebackCount::total",
        "system.cpu.iew.producerInst.total",
        "system.cpu.iew.producerInst::total",
        "system.cpu.iew.consumerInst.total",
        "system.cpu.iew.consumerInst::total",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "sim.cpu0.o3.iew.predicted_taken_incorrect",
        "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        "system.cpu.iew.predictedTakenIncorrect",
        "system.cpu.iew.predictedNotTakenIncorrect",
        "system.cpu.lsq0.dataResponse.samples",
        "system.cpu.lsq0.dataResponse.totalLatency",
        "system.cpu.lsq0.dataResponse.load.totalLatency",
        "system.cpu.lsq0.dataResponse.storeConditional.totalLatency",
        "system.cpu.lsq0.operation.load",
        "system.cpu.lsq0.operation.total",
        "system.cpu.lsq0.ordering.acquire",
        "system.cpu.lsq0.ordering.total",
        "system.cpu.iew.branchRepair.targetlessMismatch",
        "system.cpu.iew.branchRepair.directionOnly",
        "system.cpu.iew.branchRepair.wrongTarget",
        "system.cpu.iew.branchRepair.total",
        "system.cpu.iew.branchRepair_0::TargetlessMismatch",
        "system.cpu.iew.branchRepair_0::DirectionOnly",
        "system.cpu.iew.branchRepair_0::WrongTarget",
        "system.cpu.iew.branchRepair_0::total",
        "system.cpu.iew.branchMispredicts",
        "system.cpu.commit.branchMispredicts",
        "system.cpu.commit.committedInstType_0::MemRead",
        "system.cpu.commit.committedInstType_0::MemWrite",
        "system.cpu.commit.committedInstType_0::IntMult",
        "system.cpu.commit.committedInstType_0::IntDiv",
        "system.cpu.lsq0.addedLoadsAndStores",
        "system.cpu.lsq0.loadBytes",
        "system.cpu.lsq0.storeBytes",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.storeLoadForwardingMatches",
        "system.cpu.lsq0.forwLoads",
        "system.cpu.lsq0.maxOccupancy",
        "system.cpu.iq.instsIssued",
        "system.cpu.iq.memInstsIssued",
        "system.cpu.iq.branchInstsIssued",
        "system.cpu.iq.issuedInstType_0::MemRead",
        "system.cpu.iq.issuedInstType_0::MemWrite",
        "system.cpu.iq.issuedInstType_0::IntMult",
        "system.cpu.iq.issuedInstType_0::IntDiv",
        "system.cpu.rob.writes",
        "system.cpu.rob.reads",
        "system.cpu.rob.maxOccupancy",
    ] {
        assert_text_stat_absent(&stdout, path);
    }
}
