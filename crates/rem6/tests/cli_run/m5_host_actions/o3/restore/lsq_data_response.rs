use super::*;

#[test]
fn rem6_run_m5_dump_stats_restores_multicore_o3_lsq_data_response_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_float_vector_lsq_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-float-vector-lsq-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1200",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
            "--dump-memory",
            "0x80000400:16",
            "--dump-memory",
            "0x80000410:16",
            "--dump-memory",
            "0x80000420:16",
            "--dump-memory",
            "0x80000430:16",
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
        Some("000000000000f03f000000000000f03f")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("44332211887766554433221188776655")
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some("00000000000000400000000000000040")
    );
    assert_eq!(
        json.pointer("/memory/3/hex").and_then(Value::as_str),
        Some("ccbbaa9900ffeeddccbbaa9900ffeedd")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let restored_modes = host_actions
        .pointer("/checkpoint_restores/0/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {host_actions}"));
    assert!(
        restored_modes.iter().any(|mode| {
            mode.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && mode.pointer("/mode").and_then(Value::as_str) == Some("detailed")
        }),
        "restored checkpoint should preserve cpu1 detailed-mode authority: {restored_modes:?}"
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );

    assert_float_vector_lsq_dump(first_dump);
    assert_float_vector_lsq_dump(restored_dump);
    assert_matching_lsq_data_response_dumps(first_dump, restored_dump);
    assert_inactive_cpu_lsq_data_response_suppressed(&json, first_dump);
    assert_inactive_cpu_lsq_data_response_suppressed(&json, restored_dump);

    for (operation, alias) in [
        ("float_load", "floatLoad"),
        ("float_store", "floatStore"),
        ("vector_load", "vectorLoad"),
        ("vector_store", "vectorStore"),
    ] {
        assert_json_stat(
            &json,
            &format!("sim.cpu1.o3.lsq_operation.{operation}"),
            "Count",
            2,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!("system.cpu1.lsq0.operation.{alias}"),
            "Count",
            2,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!("system.cpu1.lsq0.dataResponse.{alias}.samples"),
            "Count",
            2,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!("system.cpu1.lsq0.operation.{alias}.dataResponse.samples"),
            "Count",
            2,
            "monotonic",
        );
        assert_json_stat_at_least(
            &json,
            &format!("system.cpu1.lsq0.dataResponse.{alias}.totalLatency"),
            "Tick",
            2,
            "monotonic",
        );
    }
    assert_json_stat(
        &json,
        "system.cpu1.lsq0.operation.total",
        "Count",
        8,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.lsq0.dataResponse.samples",
        "Count",
        8,
        "monotonic",
    );
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.floatLoad.samples");
    assert_json_stat_absent(&json, "system.cpu0.lsq0.dataResponse.floatLoad.samples");
}

fn multicore_hart1_detailed_o3_restore_float_vector_lsq_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),                              // auipc x18, 0
        i_type(data_start - auipc_pc, 18, 0x0, 18, 0x13), // addi x18, x18, data
    ]);
    push_float_vector_lsq_round(&mut words, 0, 8, 16, 24);
    words.extend([
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_CHECKPOINT),
        m5op(M5_DUMP_STATS),
    ]);
    push_float_vector_lsq_round(&mut words, 32, 40, 48, 56);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }

    let mut program = riscv64_program(&words);
    append_float_vector_lsq_data(&mut program, 1.0, 0x1122_3344, 0x5566_7788);
    append_float_vector_lsq_data(&mut program, 2.0, 0x99aa_bbcc, 0xddee_ff00);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn push_float_vector_lsq_round(
    words: &mut Vec<u32>,
    float_load_offset: i32,
    float_store_offset: i32,
    vector_load_offset: i32,
    vector_store_offset: i32,
) {
    words.extend([
        i_type(float_load_offset, 18, 0x3, 1, 0x07), // fld f1, offset(x18)
        float_store_type(float_store_offset, 1, 18, 0x3), // fsd f1, offset(x18)
        i_type(vector_load_offset, 18, 0x0, 12, 0x13), // addi x12, x18, vector src
        i_type(vector_store_offset, 18, 0x0, 16, 0x13), // addi x16, x18, vector dst
        i_type(2, 0, 0x0, 11, 0x13),                 // addi x11, x0, 2
        vsetvli_type(0xd0, 11, 5),                   // e32, m1, vl=2
        vector_unit_stride_load_type(true, 0b110, 12, 1), // vle v1, (x12)
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse v1, (x16)
    ]);
}

fn append_float_vector_lsq_data(program: &mut Vec<u8>, float: f64, low: u32, high: u32) {
    program.extend_from_slice(&float.to_bits().to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    program.extend([low, high, 0, 0].into_iter().flat_map(u32::to_le_bytes));
}

fn assert_float_vector_lsq_dump(dump: &Value) {
    for (operation, alias) in [
        ("float_load", "floatLoad"),
        ("float_store", "floatStore"),
        ("vector_load", "vectorLoad"),
        ("vector_store", "vectorStore"),
    ] {
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu1.o3.lsq_operation.{operation}"),
            "counter",
            "Count",
            1,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            &format!("system.cpu1.lsq0.operation.{alias}"),
            "counter",
            "Count",
            1,
            "resettable",
        );
        assert_lsq_operation_data_response(dump, operation, alias);
    }

    assert_stats_dump_sample(
        dump,
        "system.cpu1.lsq0.operation.total",
        "counter",
        "Count",
        4,
        "resettable",
    );
    let samples = ["floatLoad", "floatStore", "vectorLoad", "vectorStore"]
        .into_iter()
        .map(|alias| {
            stats_dump_sample_value(
                dump,
                &format!("system.cpu1.lsq0.dataResponse.{alias}.samples"),
            )
        })
        .sum::<u64>();
    let total = ["floatLoad", "floatStore", "vectorLoad", "vectorStore"]
        .into_iter()
        .map(|alias| {
            stats_dump_sample_value(
                dump,
                &format!("system.cpu1.lsq0.dataResponse.{alias}.totalLatency"),
            )
        })
        .sum::<u64>();
    let max = ["floatLoad", "floatStore", "vectorLoad", "vectorStore"]
        .into_iter()
        .map(|alias| {
            stats_dump_sample_value(
                dump,
                &format!("system.cpu1.lsq0.dataResponse.{alias}.maxLatency"),
            )
        })
        .max()
        .unwrap();
    let min = ["floatLoad", "floatStore", "vectorLoad", "vectorStore"]
        .into_iter()
        .map(|alias| {
            stats_dump_sample_value(
                dump,
                &format!("system.cpu1.lsq0.dataResponse.{alias}.minLatency"),
            )
        })
        .min()
        .unwrap();
    assert_lsq_data_response_stat(
        dump,
        "system.cpu1.lsq0.dataResponse",
        samples,
        total,
        max,
        min,
    );
    assert_lsq_data_response_stat(
        dump,
        "system.cpu1.lsq0.operation.total.dataResponse",
        samples,
        total,
        max,
        min,
    );
}

fn assert_lsq_operation_data_response(dump: &Value, operation: &str, alias: &str) {
    let sim_prefix = format!("sim.host_actions.stats_dump.cpu1.o3.lsq_operation.{operation}");
    let alias_prefix = format!("system.cpu1.lsq0.dataResponse.{alias}");
    let nested_prefix = format!("system.cpu1.lsq0.operation.{alias}.dataResponse");
    assert_stats_dump_sample(
        dump,
        &format!("{sim_prefix}_latency_samples"),
        "counter",
        "Count",
        1,
        "resettable",
    );
    let total =
        assert_stats_dump_sample_positive(dump, &format!("{sim_prefix}_latency_ticks"), "Tick");
    let max =
        assert_stats_dump_sample_positive(dump, &format!("{sim_prefix}_latency_max_ticks"), "Tick");
    let min =
        assert_stats_dump_sample_positive(dump, &format!("{sim_prefix}_latency_min_ticks"), "Tick");
    let avg = stats_dump_sample_value(dump, &format!("{sim_prefix}_latency_avg_ticks"));
    assert!(max >= min && avg >= min && avg <= max);
    assert_eq!(avg, total);
    assert_lsq_data_response_stat(dump, &alias_prefix, 1, total, max, min);
    assert_lsq_data_response_stat(dump, &nested_prefix, 1, total, max, min);
}

fn assert_lsq_data_response_stat(
    dump: &Value,
    prefix: &str,
    samples: u64,
    total: u64,
    max: u64,
    min: u64,
) {
    let avg = total / samples;
    for (suffix, unit, value) in [
        ("samples", "Count", samples),
        ("totalLatency", "Tick", total),
        ("maxLatency", "Tick", max),
        ("minLatency", "Tick", min),
        ("avgLatency", "Tick", avg),
    ] {
        assert_stats_dump_sample(
            dump,
            &format!("{prefix}.{suffix}"),
            "counter",
            unit,
            value,
            "resettable",
        );
    }
}

fn assert_stats_dump_sample_positive(dump: &Value, path: &str, unit: &str) -> u64 {
    assert_stats_dump_sample_at_least(dump, path, "counter", unit, 1, "resettable");
    stats_dump_sample_value(dump, path)
}

fn assert_matching_lsq_data_response_dumps(first_dump: &Value, restored_dump: &Value) {
    for path in [
        "system.cpu1.lsq0.dataResponse.samples",
        "system.cpu1.lsq0.dataResponse.totalLatency",
        "system.cpu1.lsq0.dataResponse.maxLatency",
        "system.cpu1.lsq0.dataResponse.minLatency",
        "system.cpu1.lsq0.dataResponse.avgLatency",
        "system.cpu1.lsq0.operation.total.dataResponse.samples",
        "system.cpu1.lsq0.operation.total.dataResponse.totalLatency",
        "system.cpu1.lsq0.operation.total.dataResponse.maxLatency",
        "system.cpu1.lsq0.operation.total.dataResponse.minLatency",
        "system.cpu1.lsq0.operation.total.dataResponse.avgLatency",
        "system.cpu1.lsq0.dataResponse.floatLoad.samples",
        "system.cpu1.lsq0.dataResponse.floatLoad.totalLatency",
        "system.cpu1.lsq0.dataResponse.floatStore.samples",
        "system.cpu1.lsq0.dataResponse.floatStore.totalLatency",
        "system.cpu1.lsq0.dataResponse.vectorLoad.samples",
        "system.cpu1.lsq0.dataResponse.vectorLoad.totalLatency",
        "system.cpu1.lsq0.dataResponse.vectorStore.samples",
        "system.cpu1.lsq0.dataResponse.vectorStore.totalLatency",
    ] {
        assert_eq!(
            stats_dump_sample_value(first_dump, path),
            stats_dump_sample_value(restored_dump, path),
            "restored dump should match checkpointed LSQ data response sample {path}"
        );
    }
}

fn assert_inactive_cpu_lsq_data_response_suppressed(json: &Value, dump: &Value) {
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.float_load",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.vector_store",
        "system.cpu.lsq0.dataResponse.floatLoad.samples",
        "system.cpu0.lsq0.dataResponse.floatLoad.samples",
        "system.cpu.lsq0.operation.floatLoad.dataResponse.samples",
        "system.cpu0.lsq0.operation.floatLoad.dataResponse.samples",
    ] {
        assert_stats_dump_sample_absent(dump, path);
        assert_json_stat_absent(json, path);
    }
}
