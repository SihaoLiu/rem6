use super::*;

#[test]
fn rem6_run_m5_dump_reset_stats_snapshots_nested_o3_fu_latency_classes() {
    let path = detailed_o3_dump_reset_fu_stats_binary(
        "m5-switch-cpu-o3-dump-reset-nested-fu-latency-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
            "--stats-format",
            "json",
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
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the old epoch before resetting: {dump}"
    );

    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.instructions",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.cycles",
            "Cycle",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.max_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.min_cycles",
            "Cycle",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.avg_cycles",
            "Cycle",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.vector_float_misc.instructions",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.vector_float_misc.cycles",
            "Cycle",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.instructions",
            "Count",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.max_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", unit, value, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_class.integer_mul.instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_class.integer_div.cycles",
        "Cycle",
        19,
        "monotonic",
    );
}

#[test]
fn rem6_run_restore_exposes_nested_o3_fu_latency_class_runtime_summary() {
    let path =
        detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-nested-fu-summary");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    for (pointer, value) in [
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/instructions",
            1,
        ),
        ("/cores/0/o3_runtime/fu_latency_class/integer_mul/cycles", 2),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/instructions",
            1,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/cycles",
            19,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/float_misc/instructions",
            2,
        ),
        ("/cores/0/o3_runtime/fu_latency_class/float_misc/cycles", 3),
        (
            "/cores/0/o3_runtime/fu_latency_class/vector_float_misc/instructions",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/vector_float_misc/cycles",
            3,
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "missing restored nested FU latency summary {pointer}: {json}"
        );
    }

    assert_eq!(
        json.pointer("/cores/0/o3_runtime/fu_integer_mul_instructions")
            .and_then(Value::as_u64),
        Some(1),
        "legacy flat runtime summary should remain available: {json}"
    );
}

#[test]
fn rem6_run_restore_exposes_o3_fu_latency_class_min_max_avg_runtime_summary() {
    let path =
        detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-fu-latency-extrema");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    for (path, unit, value) in [
        (
            "sim.cpu0.o3.fu_latency_class.integer_mul.max_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_mul.min_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_mul.avg_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_div.max_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_div.min_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_div.avg_cycles",
            "Cycle",
            19,
        ),
    ] {
        assert_json_stat(&json, path, unit, value, "monotonic");
    }

    for (pointer, value) in [
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/max_cycles",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/min_cycles",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/avg_cycles",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/max_cycles",
            19,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/min_cycles",
            19,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/avg_cycles",
            19,
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "missing restored FU latency class runtime summary {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_records_o3_fu_latency_stats_after_detailed_switch() {
    let path = detailed_o3_fu_latency_binary("m5-switch-cpu-detailed-o3-fu-latency-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
            "--stats-format",
            "json",
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
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        21,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "Cycle",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.int_div",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.int_div",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "Cycle",
        19,
        "monotonic",
    );
}

#[test]
fn rem6_run_records_o3_float_misc_fu_latency_stats_after_detailed_switch() {
    let path =
        detailed_o3_float_misc_fu_latency_binary("m5-switch-cpu-detailed-o3-float-misc-stats");

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
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_latency_cycles",
        "Cycle",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_latency_cycles",
        "Cycle",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_extended_float_fu_latency_classes() {
    let path = detailed_o3_float_extended_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-float-extended-runtime-json",
    );

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
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert!(
        o3_runtime
            .pointer("/event_window")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should expose an explicit null event window: {o3_runtime}"
    );
    assert_eq!(
        o3_runtime
            .pointer("/fu_latency_instructions")
            .and_then(Value::as_u64),
        Some(6)
    );

    for class in [
        "float_add",
        "float_fma",
        "float_sqrt",
        "vector_float_add",
        "vector_float_fma",
        "vector_float_sqrt",
    ] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        let stat_instruction_path = format!("sim.cpu0.o3.fu_{class}_instructions");
        let stat_latency_path = format!("sim.cpu0.o3.fu_{class}_latency_cycles");
        let runtime_instructions = o3_runtime
            .pointer(&instruction_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {instruction_path}: {o3_runtime}")
            });
        let runtime_latency_cycles = o3_runtime
            .pointer(&latency_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {latency_path}: {o3_runtime}")
            });
        assert_eq!(
            runtime_instructions, 1,
            "structured O3 runtime JSON should count {instruction_path}: {o3_runtime}"
        );
        assert!(
            runtime_latency_cycles > 0,
            "structured O3 runtime JSON should count positive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_instruction_path),
            runtime_instructions,
            "stat registry should match structured runtime {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_latency_path),
            runtime_latency_cycles,
            "stat registry should match structured runtime {latency_path}"
        );
    }
    for (source_class, alias_class) in [
        ("float_add", "FloatAdd"),
        ("float_fma", "FloatMultAcc"),
        ("float_sqrt", "FloatSqrt"),
        ("vector_float_add", "SimdFloatAdd"),
        ("vector_float_fma", "SimdFloatMultAcc"),
        ("vector_float_sqrt", "SimdFloatSqrt"),
    ] {
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.iq.issued_inst_type.{source_class}"),
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.iq.issuedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.commit.committed_inst_type.{source_class}"),
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.commit.committedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
    }
    for class in [
        "float_mul",
        "float_div",
        "vector_float_mul",
        "vector_float_div",
    ] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        assert_eq!(
            o3_runtime
                .pointer(&instruction_path)
                .and_then(Value::as_u64),
            Some(0),
            "structured O3 runtime JSON should expose inactive {instruction_path}: {o3_runtime}"
        );
        assert_eq!(
            o3_runtime.pointer(&latency_path).and_then(Value::as_u64),
            Some(0),
            "structured O3 runtime JSON should expose inactive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &format!("sim.cpu0.o3.fu_{class}_instructions")),
            0,
            "stat registry should expose inactive {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &format!("sim.cpu0.o3.fu_{class}_latency_cycles")),
            0,
            "stat registry should expose inactive {latency_path}"
        );
    }
    for (source_class, alias_class) in [
        ("float_compare", "FloatCmp"),
        ("float_mul", "FloatMult"),
        ("float_div", "FloatDiv"),
        ("vector_float_compare", "SimdFloatCmp"),
        ("vector_float_mul", "SimdFloatMult"),
        ("vector_float_div", "SimdFloatDiv"),
    ] {
        assert_json_stat(
            &json,
            &format!("system.cpu.iq.issuedInstType.{alias_class}"),
            "Count",
            json_stat_u64(
                &json,
                &format!("sim.cpu0.o3.iq.issued_inst_type.{source_class}"),
            ),
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.commit.committedInstType.{alias_class}"),
            "Count",
            json_stat_u64(
                &json,
                &format!("sim.cpu0.o3.commit.committed_inst_type.{source_class}"),
            ),
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_vector_integer_fu_latency_classes() {
    let path = detailed_o3_vector_integer_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-vector-integer-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
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
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/fu_latency_instructions")
            .and_then(Value::as_u64),
        Some(2)
    );

    for class in ["vector_integer_mul", "vector_integer_div"] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        let stat_instruction_path = format!("sim.cpu0.o3.fu_{class}_instructions");
        let stat_latency_path = format!("sim.cpu0.o3.fu_{class}_latency_cycles");
        let runtime_instructions = o3_runtime
            .pointer(&instruction_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {instruction_path}: {o3_runtime}")
            });
        let runtime_latency_cycles = o3_runtime
            .pointer(&latency_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {latency_path}: {o3_runtime}")
            });
        assert_eq!(
            runtime_instructions, 1,
            "structured O3 runtime JSON should count {instruction_path}: {o3_runtime}"
        );
        assert!(
            runtime_latency_cycles > 0,
            "structured O3 runtime JSON should count positive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_instruction_path),
            runtime_instructions,
            "stat registry should match structured runtime {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_latency_path),
            runtime_latency_cycles,
            "stat registry should match structured runtime {latency_path}"
        );
    }

    for (source_class, alias_class) in [
        ("vector_integer_mul", "SimdMult"),
        ("vector_integer_div", "SimdDiv"),
    ] {
        let expected_instructions = json_stat_value(
            &json,
            &format!("sim.cpu0.o3.fu_{source_class}_instructions"),
        );
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.iq.issued_inst_type.{source_class}"),
        );
        assert_eq!(
            value, expected_instructions,
            "IQ issued op-class count should match FU class runtime count for {source_class}"
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.iq.issuedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.commit.committed_inst_type.{source_class}"),
        );
        assert_eq!(
            value, expected_instructions,
            "commit op-class count should match FU class runtime count for {source_class}"
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.commit.committedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_text_stats_alias_o3_fu_latency_after_detailed_switch() {
    let path = detailed_o3_fu_latency_binary("m5-switch-cpu-detailed-o3-fu-latency-text-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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

    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_latency_instructions", 2);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_latency_cycles", 21);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_integer_mul_instructions", 1);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_integer_mul_latency_cycles", 2);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_integer_div_instructions", 1);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_integer_div_latency_cycles", 19);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType.IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType.IntDiv", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::IntDiv", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.commit.committed_inst_type.int_mul", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.commit.committed_inst_type.int_div", 1);
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType.IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType_0::IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType.IntDiv", 1);
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType_0::IntDiv", 1);
}

#[test]
fn rem6_run_text_stats_alias_o3_float_misc_fu_latency_after_detailed_switch() {
    let path =
        detailed_o3_float_misc_fu_latency_binary("m5-switch-cpu-detailed-o3-float-misc-text-stats");

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

    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_latency_instructions", 4);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_latency_cycles", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_float_misc_instructions", 2);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_float_misc_latency_cycles", 3);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_vector_float_misc_instructions", 2);
    assert_text_cycle_stat(
        &stdout,
        "sim.cpu0.o3.fu_vector_float_misc_latency_cycles",
        3,
    );
    assert_text_count_stat(&stdout, "sim.cpu0.o3.iq.issued_inst_type.float_misc", 2);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        2,
    );
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::FloatMisc", 2);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::SimdFloatMisc", 2);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        2,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        2,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.committedInstType_0::FloatMisc",
        2,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.committedInstType_0::SimdFloatMisc",
        2,
    );
}
