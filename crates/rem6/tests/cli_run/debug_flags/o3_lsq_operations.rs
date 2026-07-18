use super::*;

fn detailed_o3_vector_memory_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 10, 0x17),
        i_type(60, 10, 0b000, 10, 0x13),
        i_type(8, 10, 0b000, 16, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        vsetvli_type(0xd0, 11, 5),
        vector_unit_stride_load_type(true, 0b110, 10, 1),
        vector_unit_stride_store_type(true, 0b110, 16, 1),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1122_3344, 0x5566_7788, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_memory_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0x3, 1, 0x07),
        float_store_type(8, 1, 10, 0x3),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&1.0f64.to_bits().to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_atomic_lsq_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(5, 0, 0x0, 6, 0x13),
        atomic_type(0x00, false, false, 6, 5, 0x3, 7),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([9, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_ordered_atomic_lsq_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(0, 5, 0b011, 6, 0x03),
        s_type(8, 6, 5, 0b011),
        atomic_type(0x02, true, false, 0, 5, 0x3, 7),
        i_type(3, 0, 0x0, 8, 0x13),
        atomic_type(0x03, false, true, 8, 5, 0x3, 9),
        i_type(4, 0, 0x0, 12, 0x13), // Preserve a0=0 for the immediate exit sentinel.
        atomic_type(0x01, true, true, 12, 5, 0x3, 11),
        s_type(16, 9, 5, 0b011),
        s_type(24, 11, 5, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_conditional_failure_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(0x2a, 0, 0x0, 6, 0x13),
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),
        s_type(8, 7, 5, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

#[test]
fn rem6_run_o3_debug_flag_emits_vector_lsq_byte_events() {
    let path = detailed_o3_vector_memory_debug_binary("debug-flags-o3-vector-lsq-bytes");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
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
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("lsq_load_bytes", 8),
        ("lsq_store_bytes", 8),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 vector trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let vector_load = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000018"))
        .unwrap_or_else(|| panic!("missing vector load O3 event: {events:?}"));
    assert_eq!(json_record_u64(vector_load, "rename_writes"), 1);
    assert_eq!(json_record_u64(vector_load, "lsq_loads"), 1);
    assert_eq!(json_record_str(vector_load, "lsq_operation"), "vector_load");
    assert_eq!(json_record_u64(vector_load, "lsq_load_bytes"), 8);
    assert_eq!(json_record_u64(vector_load, "lsq_stores"), 0);
    assert_eq!(json_record_u64(vector_load, "lsq_store_bytes"), 0);
    let vector_load_latency = json_record_u64(vector_load, "lsq_data_latency_ticks");
    assert!(vector_load_latency > 0, "{vector_load:?}");

    let vector_store = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000001c"))
        .unwrap_or_else(|| panic!("missing vector store O3 event: {events:?}"));
    assert_eq!(json_record_u64(vector_store, "rename_writes"), 0);
    assert_eq!(json_record_u64(vector_store, "lsq_loads"), 0);
    assert_eq!(json_record_u64(vector_store, "lsq_load_bytes"), 0);
    assert_eq!(json_record_u64(vector_store, "lsq_stores"), 1);
    assert_eq!(
        json_record_str(vector_store, "lsq_operation"),
        "vector_store"
    );
    assert_eq!(json_record_u64(vector_store, "lsq_store_bytes"), 8);
    let vector_store_latency = json_record_u64(vector_store, "lsq_data_latency_ticks");
    assert!(vector_store_latency > 0, "{vector_store:?}");

    for (path, unit, value) in [
        ("sim.cpu0.o3.lsq_load_bytes", "Byte", 8),
        ("sim.cpu0.o3.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 8),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_avg_ticks",
            "Tick",
            vector_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_min_ticks",
            "Tick",
            vector_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_avg_ticks",
            "Tick",
            vector_store_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_min_ticks",
            "Tick",
            vector_store_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_lsq_operation_shape() {
    let path = detailed_o3_float_memory_debug_binary("debug-flags-o3-float-lsq-operation");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("000000000000f03f000000000000f03f")
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("lsq_load_bytes", 8),
        ("lsq_store_bytes", 8),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 float trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let float_load = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing float load O3 event: {events:?}"));
    assert_eq!(json_record_u64(float_load, "rename_writes"), 1);
    assert_eq!(json_record_u64(float_load, "lsq_loads"), 1);
    assert_eq!(json_record_str(float_load, "lsq_operation"), "float_load");
    assert_eq!(
        json_record_str(float_load, "lsq_load_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(float_load, "lsq_load_bytes"), 8);
    assert_eq!(json_record_u64(float_load, "lsq_stores"), 0);
    assert_eq!(json_record_u64(float_load, "lsq_store_bytes"), 0);
    let float_load_latency = json_record_u64(float_load, "lsq_data_latency_ticks");
    assert!(float_load_latency > 0, "{float_load:?}");

    let float_store = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing float store O3 event: {events:?}"));
    assert_eq!(json_record_u64(float_store, "rename_writes"), 0);
    assert_eq!(json_record_u64(float_store, "lsq_loads"), 0);
    assert_eq!(json_record_u64(float_store, "lsq_load_bytes"), 0);
    assert_eq!(json_record_u64(float_store, "lsq_stores"), 1);
    assert_eq!(json_record_str(float_store, "lsq_operation"), "float_store");
    assert_eq!(
        json_record_str(float_store, "lsq_store_address"),
        "0x80000048"
    );
    assert_eq!(json_record_u64(float_store, "lsq_store_bytes"), 8);
    let float_store_latency = json_record_u64(float_store, "lsq_data_latency_ticks");
    assert!(float_store_latency > 0, "{float_store:?}");

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.float_loads", "Count", 1),
        ("sim.debug.o3_trace.float_stores", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 8),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_avg_ticks",
            "Tick",
            float_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_min_ticks",
            "Tick",
            float_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_avg_ticks",
            "Tick",
            float_store_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_min_ticks",
            "Tick",
            float_store_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_atomic_lsq_operation_shape() {
    let path = detailed_o3_atomic_lsq_debug_binary("debug-flags-o3-atomic-lsq-operation");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
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
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 5),
        ("rename_writes", 4),
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("lsq_load_bytes", 8),
        ("lsq_store_bytes", 8),
        ("store_load_forwarding_candidates", 0),
        ("store_load_forwarding_matches", 0),
        ("max_lsq_occupancy", 2),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 atomic trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 5);
    let atomic = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing atomic O3 event: {events:?}"));
    assert_eq!(json_record_str(atomic, "lsq_operation"), "atomic");
    assert_eq!(json_record_u64(atomic, "rename_writes"), 1);
    assert_eq!(json_record_u64(atomic, "lsq_loads"), 1);
    assert_eq!(json_record_u64(atomic, "lsq_stores"), 1);
    assert_eq!(json_record_u64(atomic, "lsq_occupancy"), 2);
    assert_eq!(json_record_u64(atomic, "lsq_load_bytes"), 8);
    assert_eq!(json_record_u64(atomic, "lsq_store_bytes"), 8);
    assert_eq!(json_record_str(atomic, "lsq_load_address"), "0x80000040");
    assert_eq!(json_record_str(atomic, "lsq_store_address"), "0x80000040");
    let atomic_latency = json_record_u64(atomic, "lsq_data_latency_ticks");
    assert!(atomic_latency > 0, "{atomic:?}");
    assert_eq!(
        json_record_bool(atomic, "store_load_forwarding_candidate"),
        false
    );
    assert_eq!(
        json_record_bool(atomic, "store_load_forwarding_match"),
        false
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_operation.atomic", "Count", 1),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_avg_ticks",
            "Tick",
            atomic_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_min_ticks",
            "Tick",
            atomic_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_lsq_memory_ordering() {
    let path = detailed_o3_ordered_atomic_lsq_debug_binary("debug-flags-o3-lsq-ordering");

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
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
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
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/committed_instructions")
            .and_then(Value::as_u64),
        Some(13)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("04000000000000000900000000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000300000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 12),
        ("rename_writes", 8),
        ("lsq_loads", 3),
        ("lsq_stores", 5),
        ("lsq_load_bytes", 24),
        ("lsq_store_bytes", 40),
        ("max_lsq_occupancy", 2),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 ordered LSQ trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 12);
    assert!(events
        .iter()
        .all(|event| event.pointer("/pc").and_then(Value::as_str) != Some("0x80000034")));
    let by_pc = |pc: &str| {
        events
            .iter()
            .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("missing O3 event at {pc}: {events:?}"))
    };
    for (pc, operation, ordering, acquire, release) in [
        ("0x8000000c", "load", "none", false, false),
        ("0x80000010", "store", "none", false, false),
        ("0x80000014", "load_reserved", "acquire", true, false),
        ("0x8000001c", "store_conditional", "release", false, true),
        ("0x80000024", "atomic", "acquire_release", true, true),
        ("0x80000028", "store", "none", false, false),
        ("0x8000002c", "store", "none", false, false),
    ] {
        let event = by_pc(pc);
        assert_eq!(json_record_str(event, "lsq_operation"), operation);
        assert_eq!(json_record_str(event, "lsq_ordering"), ordering);
        assert_eq!(json_record_bool(event, "lsq_acquire"), acquire);
        assert_eq!(json_record_bool(event, "lsq_release"), release);
    }
    assert_eq!(
        json_record_u64(by_pc("0x80000028"), "sequence"),
        json_record_u64(by_pc("0x80000024"), "sequence") + 2,
        "post-atomic memory event should not reuse the atomic store sub-entry sequence"
    );
    let load_reserved_latency = json_record_u64(by_pc("0x80000014"), "lsq_data_latency_ticks");
    let store_conditional_latency = json_record_u64(by_pc("0x8000001c"), "lsq_data_latency_ticks");
    let atomic_latency = json_record_u64(by_pc("0x80000024"), "lsq_data_latency_ticks");
    assert!(load_reserved_latency > 0, "{events:?}");
    assert!(store_conditional_latency > 0, "{events:?}");
    assert!(atomic_latency > 0, "{events:?}");
    let lsq_latency_values = events
        .iter()
        .map(|event| json_record_u64(event, "lsq_data_latency_ticks"))
        .filter(|latency| *latency > 0)
        .collect::<Vec<_>>();
    let lsq_latency_ticks = lsq_latency_values.iter().sum::<u64>();
    let lsq_latency_samples = lsq_latency_values.len() as u64;
    let lsq_latency_min = lsq_latency_values.iter().copied().min().unwrap_or(0);
    let lsq_latency_max = lsq_latency_values.iter().copied().max().unwrap_or(0);
    let lsq_latency_avg = if lsq_latency_samples == 0 {
        0
    } else {
        lsq_latency_ticks / lsq_latency_samples
    };
    let operation_latency = |operation: &str| {
        let values = events
            .iter()
            .filter(|event| json_record_str(event, "lsq_operation") == operation)
            .map(|event| json_record_u64(event, "lsq_data_latency_ticks"))
            .filter(|latency| *latency > 0)
            .collect::<Vec<_>>();
        let samples = values.len() as u64;
        let ticks = values.iter().sum::<u64>();
        let min_ticks = values.iter().copied().min().unwrap_or(0);
        let max_ticks = values.iter().copied().max().unwrap_or(0);
        let avg_ticks = if samples == 0 { 0 } else { ticks / samples };
        (samples, ticks, max_ticks, min_ticks, avg_ticks)
    };
    let lsq = record
        .pointer("/lsq")
        .unwrap_or_else(|| panic!("missing O3 trace LSQ summary: {record}"));
    for (pointer, value) in [
        ("/loads", 3),
        ("/stores", 5),
        ("/load_bytes", 24),
        ("/store_bytes", 40),
        ("/store_conditional_failures", 0),
        ("/max_occupancy", 2),
        ("/operation/load/count", 1),
        ("/operation/store/count", 3),
        ("/operation/load_reserved/count", 1),
        ("/operation/store_conditional/count", 1),
        ("/operation/atomic/count", 1),
        ("/operation/float_load/count", 0),
        ("/operation/vector_load/count", 0),
        ("/ordering/acquire", 1),
        ("/ordering/release", 1),
        ("/ordering/acquire_release", 1),
        ("/data_latency/samples", lsq_latency_samples),
        ("/data_latency/ticks", lsq_latency_ticks),
        ("/data_latency/max_ticks", lsq_latency_max),
        ("/data_latency/min_ticks", lsq_latency_min),
        ("/data_latency/avg_ticks", lsq_latency_avg),
    ] {
        assert_eq!(
            lsq.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "O3 ordered LSQ summary path {pointer}: {lsq}"
        );
    }
    for operation in ["load_reserved", "store_conditional", "atomic"] {
        let (samples, ticks, max_ticks, min_ticks, avg_ticks) = operation_latency(operation);
        for (metric, value) in [
            ("samples", samples),
            ("ticks", ticks),
            ("max_ticks", max_ticks),
            ("min_ticks", min_ticks),
            ("avg_ticks", avg_ticks),
        ] {
            let pointer = format!("/operation/{operation}/latency/{metric}");
            assert_eq!(
                lsq.pointer(&pointer).and_then(Value::as_u64),
                Some(value),
                "O3 ordered LSQ operation-latency summary path {pointer}: {lsq}"
            );
        }
    }
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    let event_summary_ordering = event_summary
        .pointer("/lsq_ordering")
        .expect("O3 event summary LSQ ordering matrix");
    for (ordering, value) in [("acquire", 1), ("release", 1), ("acquire_release", 1)] {
        assert_eq!(
            event_summary_ordering
                .pointer(&format!("/{ordering}"))
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary LSQ ordering {ordering}: {event_summary_ordering}"
        );
    }
    assert!(
        event_summary_ordering.pointer("/none").is_none(),
        "O3 event summary LSQ ordering matrix should track ordered lanes only: {event_summary_ordering}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 24),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 40),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 24),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 40),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 3),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.atomic", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_ordering.acquire", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_ordering.release", "Count", 1),
        (
            "sim.debug.o3_trace.event.lsq_ordering.acquire_release",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_avg_ticks",
            "Tick",
            load_reserved_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_min_ticks",
            "Tick",
            load_reserved_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_avg_ticks",
            "Tick",
            store_conditional_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_min_ticks",
            "Tick",
            store_conditional_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_avg_ticks",
            "Tick",
            atomic_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_min_ticks",
            "Tick",
            atomic_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_marks_store_conditional_failures() {
    let path = detailed_o3_store_conditional_failure_debug_binary(
        "debug-flags-o3-store-conditional-failure",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_load_bytes", 0),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 failed SC trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let store_conditional = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing store-conditional O3 event: {events:?}"));
    assert_eq!(
        json_record_str(store_conditional, "lsq_operation"),
        "store_conditional"
    );
    assert_eq!(json_record_u64(store_conditional, "rename_writes"), 1);
    assert_eq!(json_record_u64(store_conditional, "lsq_loads"), 0);
    assert_eq!(json_record_u64(store_conditional, "lsq_stores"), 1);
    assert_eq!(
        json_record_str(store_conditional, "lsq_store_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(store_conditional, "lsq_store_bytes"), 8);
    assert_eq!(
        json_record_bool(store_conditional, "lsq_store_conditional_failed"),
        true
    );

    let result_store = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .unwrap_or_else(|| panic!("missing SC result store O3 event: {events:?}"));
    assert_eq!(json_record_str(result_store, "lsq_operation"), "store");
    assert_eq!(
        json_record_bool(result_store, "lsq_store_conditional_failed"),
        false
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 16),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_store_conditional_failures",
            "Count",
            1,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}
