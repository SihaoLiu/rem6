use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    process::Command,
};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_exec_debug_flag_emits_real_instruction_trace() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-exec", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "60",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Exec",
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
    assert_exec_trace(
        &json,
        &[
            ExpectedExecTraceRecord {
                tick: 2,
                pc: "0x80000000",
                bytes: "93027000",
            },
            ExpectedExecTraceRecord {
                tick: 4,
                pc: "0x80000004",
                bytes: "13831200",
            },
            ExpectedExecTraceRecord {
                tick: 6,
                pc: "0x80000008",
                bytes: "73000000",
            },
        ],
    );
    let trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    let retired_records = trace
        .iter()
        .filter(|record| record.get("retired").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    let exec_bytes = trace
        .iter()
        .map(|record| {
            record
                .get("bytes")
                .and_then(Value::as_str)
                .expect("exec bytes")
                .len() as u64
                / 2
        })
        .sum::<u64>();
    assert_eq!(retired_records, trace.len() as u64);
    assert!(exec_bytes > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.exec_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.exec_trace.retired",
        "Count",
        retired_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.exec_trace.bytes",
        "Byte",
        exec_bytes,
        "monotonic",
    );
    assert_exec_trace_hierarchy_stats(&stdout, trace);
}

#[test]
fn rem6_run_fetch_debug_flag_emits_real_fetch_issue_trace() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fetch", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "60",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Exec,Fetch",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![
            Value::String("Exec".to_string()),
            Value::String("Fetch".to_string())
        ])
    );
    assert_fetch_trace(
        &json,
        &[
            ExpectedFetchTraceRecord {
                tick: 0,
                pc: "0x80000000",
                sequence: 0,
                size: 4,
            },
            ExpectedFetchTraceRecord {
                tick: 2,
                pc: "0x80000004",
                sequence: 1,
                size: 4,
            },
            ExpectedFetchTraceRecord {
                tick: 4,
                pc: "0x80000008",
                sequence: 2,
                size: 4,
            },
        ],
    );
    let fetch_trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    let exec_trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    let fetch_bytes = fetch_trace
        .iter()
        .map(|record| {
            record
                .get("size")
                .and_then(Value::as_u64)
                .expect("fetch size")
        })
        .sum::<u64>();
    let exec_bytes = exec_trace
        .iter()
        .map(|record| {
            record
                .get("bytes")
                .and_then(Value::as_str)
                .expect("exec bytes")
                .len() as u64
                / 2
        })
        .sum::<u64>();
    let trace_records = fetch_trace.len() as u64 + exec_trace.len() as u64;
    let trace_payload_bytes = fetch_bytes + exec_bytes;
    assert!(fetch_bytes > 0, "trace: {fetch_trace:?}");
    assert!(exec_bytes > 0, "trace: {exec_trace:?}");
    assert_eq!(exec_trace.len(), 3);
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.payload_bytes",
        "Byte",
        trace_payload_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fetch_trace.records",
        "Count",
        fetch_trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fetch_trace.bytes",
        "Byte",
        fetch_bytes,
        "monotonic",
    );
    assert_fetch_trace_hierarchy_stats(&stdout, fetch_trace);
}

#[test]
fn rem6_run_fetch_debug_flag_keeps_fetches_across_riscv_se_stream_reset() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        0x0070_0293,                   // addi x5, x0, 7
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fetch-riscv-se-reset", &elf);

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
            "json",
            "--execute",
            "--riscv-se",
            "--debug-flags",
            "Fetch",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Fetch".to_string())])
    );
    assert_fetch_pcs(
        &json,
        &[
            "0x80000000",
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
        ],
    );
}

#[test]
fn rem6_run_data_debug_flag_emits_real_data_access_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297,                                   // auipc x5, 0
        0x0402_8293,                                   // addi x5, x5, 64
        0x0052_b023,                                   // sd x5, 0(x5)
        0x0002_b303,                                   // ld x6, 0(x5)
        atomic_type(0x00, false, false, 6, 5, 0x3, 7), // amoadd.d x7, x6, (x5)
        0x0000_0073,                                   // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-data", &elf);

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
            "json",
            "--execute",
            "--debug-flags",
            "Data",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Data".to_string())])
    );
    let trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("debug data trace array");
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[0].get("kind").and_then(Value::as_str), Some("store"));
    assert_eq!(trace[1].get("kind").and_then(Value::as_str), Some("load"));
    assert_eq!(trace[2].get("kind").and_then(Value::as_str), Some("atomic"));
    let load_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("load"))
        .count() as u64;
    let store_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("store"))
        .count() as u64;
    let atomic_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("atomic"))
        .count() as u64;
    let load_bytes = debug_trace_sum(trace, "load", "size");
    let store_bytes = debug_trace_sum(trace, "store", "size");
    let atomic_bytes = debug_trace_sum(trace, "atomic", "size");
    assert!(load_bytes > 0, "trace: {trace:?}");
    assert!(store_bytes > 0, "trace: {trace:?}");
    assert!(atomic_bytes > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.data_trace.loads",
        "Count",
        load_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.stores",
        "Count",
        store_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.atomics",
        "Count",
        atomic_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.load_bytes",
        "Byte",
        load_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.store_bytes",
        "Byte",
        store_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.atomic_bytes",
        "Byte",
        atomic_bytes,
        "monotonic",
    );
    assert_data_trace_hierarchy_stats(&stdout, trace);
    for record in trace {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("address").and_then(Value::as_str),
            Some("0x80000040")
        );
        assert_eq!(record.get("size").and_then(Value::as_u64), Some(8));
        assert!(record.get("tick").and_then(Value::as_u64).is_some());
    }
}

#[test]
fn rem6_run_memory_debug_flag_emits_real_transport_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-memory", &elf);

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
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Memory",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Memory".to_string())])
    );
    let trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("debug memory trace array");
    assert!(
        trace.len() >= 6,
        "expected fetch and data transport events, got {trace:?}"
    );
    assert!(trace.iter().any(|record| {
        record.get("channel").and_then(Value::as_str) == Some("fetch")
            && record.get("kind").and_then(Value::as_str) == Some("request_sent")
    }));
    assert!(trace.iter().any(|record| {
        record.get("channel").and_then(Value::as_str) == Some("data")
            && record.get("kind").and_then(Value::as_str) == Some("request_sent")
    }));
    assert!(trace.iter().any(|record| {
        record.get("kind").and_then(Value::as_str) == Some("response_arrived")
            && record.get("response_status").and_then(Value::as_str) == Some("completed")
    }));
    let fetch_records = trace
        .iter()
        .filter(|record| record.get("channel").and_then(Value::as_str) == Some("fetch"))
        .count() as u64;
    let data_records = trace
        .iter()
        .filter(|record| record.get("channel").and_then(Value::as_str) == Some("data"))
        .count() as u64;
    let request_sent_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("request_sent"))
        .count() as u64;
    let request_arrived_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("request_arrived"))
        .count() as u64;
    let response_arrived_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("response_arrived"))
        .count() as u64;
    let completed_responses = trace
        .iter()
        .filter(|record| record.get("response_status").and_then(Value::as_str) == Some("completed"))
        .count() as u64;
    let retry_responses = trace
        .iter()
        .filter(|record| record.get("response_status").and_then(Value::as_str) == Some("retry"))
        .count() as u64;
    let store_conditional_failed_responses = trace
        .iter()
        .filter(|record| {
            record.get("response_status").and_then(Value::as_str)
                == Some("store_conditional_failed")
        })
        .count() as u64;
    let requests = memory_trace_unique_requests(trace, None);
    let fetch_requests = memory_trace_unique_requests(trace, Some("fetch"));
    let data_requests = memory_trace_unique_requests(trace, Some("data"));
    let routes = memory_trace_unique_routes(trace, None);
    let fetch_routes = memory_trace_unique_routes(trace, Some("fetch"));
    let data_routes = memory_trace_unique_routes(trace, Some("data"));
    let request_agents = memory_trace_unique_request_agents(trace);
    assert!(request_sent_records > 0, "trace: {trace:?}");
    assert!(request_arrived_records > 0, "trace: {trace:?}");
    assert!(response_arrived_records > 0, "trace: {trace:?}");
    assert!(completed_responses > 0, "trace: {trace:?}");
    assert!(requests > 0, "trace: {trace:?}");
    assert!(fetch_requests > 0, "trace: {trace:?}");
    assert!(data_requests > 0, "trace: {trace:?}");
    assert!(routes > 0, "trace: {trace:?}");
    assert!(fetch_routes > 0, "trace: {trace:?}");
    assert!(data_routes > 0, "trace: {trace:?}");
    assert!(request_agents > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.fetch.records",
        "Count",
        fetch_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.data.records",
        "Count",
        data_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.events.request_sent",
        "Count",
        request_sent_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.events.request_arrived",
        "Count",
        request_arrived_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.events.response_arrived",
        "Count",
        response_arrived_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_status.completed",
        "Count",
        completed_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_status.retry",
        "Count",
        retry_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_status.store_conditional_failed",
        "Count",
        store_conditional_failed_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.requests",
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.fetch.requests",
        "Count",
        fetch_requests,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.data.requests",
        "Count",
        data_requests,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.routes",
        "Count",
        routes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.fetch.routes",
        "Count",
        fetch_routes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.data.routes",
        "Count",
        data_routes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.request_agents",
        "Count",
        request_agents,
        "monotonic",
    );
    assert_memory_trace_hierarchy_stats(&stdout, trace);
    for record in trace {
        assert!(record.get("tick").and_then(Value::as_u64).is_some());
        assert!(record.get("route").and_then(Value::as_u64).is_some());
        assert!(record.get("request").and_then(Value::as_u64).is_some());
        assert!(record.get("endpoint").and_then(Value::as_str).is_some());
    }
}

#[test]
fn rem6_run_cache_debug_flag_emits_real_cache_hierarchy_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03),                 // ld x6, 16(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 48, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 16..DATA_OFFSET + 24]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-cache", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "cache-fabric-dram",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
            "--debug-flags",
            "Cache",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Cache".to_string())])
    );
    let trace = json
        .pointer("/debug/cache_trace")
        .and_then(Value::as_array)
        .expect("debug cache trace array");
    assert_eq!(trace.len(), 6, "trace should cover I/D L1/L2/L3: {trace:?}");

    assert_cache_trace_record(
        trace,
        "instruction",
        "l1",
        &json,
        "/memory_resources/cache/instruction/l1",
    );
    assert_cache_trace_record(
        trace,
        "instruction",
        "l2",
        &json,
        "/memory_resources/cache/instruction/l2",
    );
    assert_cache_trace_record(
        trace,
        "instruction",
        "l3",
        &json,
        "/memory_resources/cache/instruction/l3",
    );
    assert_cache_trace_record(
        trace,
        "data",
        "l1",
        &json,
        "/memory_resources/cache/data/l1",
    );
    assert_cache_trace_record(
        trace,
        "data",
        "l2",
        &json,
        "/memory_resources/cache/data/l2",
    );
    assert_cache_trace_record(
        trace,
        "data",
        "l3",
        &json,
        "/memory_resources/cache/data/l3",
    );

    let active_scopes = cache_trace_active_count(trace);
    let activity = cache_trace_sum(trace, "activity");
    let cpu_responses = cache_trace_sum(trace, "cpu_responses");
    let directory_decisions = cache_trace_sum(trace, "directory_decisions");
    let dram_accesses = cache_trace_sum(trace, "dram_accesses");
    assert!(active_scopes > 0, "trace: {trace:?}");
    assert!(activity > 0, "trace: {trace:?}");
    assert!(cpu_responses > 0, "trace: {trace:?}");
    assert!(directory_decisions > 0, "trace: {trace:?}");
    assert!(dram_accesses > 0, "trace: {trace:?}");
    assert!(
        json_path_u64(
            &json,
            "/memory_resources/cache/data/l1/prefetch_queue_issued"
        ) > 0,
        "trace: {trace:?}"
    );
    assert!(
        json_path_u64(&json, "/memory_resources/cache/data/l1/prefetch_useful") > 0,
        "trace: {trace:?}"
    );
    assert_eq!(
        active_scopes,
        json_path_u64(&json, "/memory_resources/cache/active")
    );
    assert_eq!(
        activity,
        json_path_u64(&json, "/memory_resources/cache/activity")
    );
    assert_eq!(
        cpu_responses,
        json_path_u64(&json, "/memory_resources/cache/cpu_responses")
    );
    assert_eq!(
        directory_decisions,
        json_path_u64(&json, "/memory_resources/cache/directory_decisions")
    );
    assert_eq!(
        dram_accesses,
        json_path_u64(&json, "/memory_resources/cache/dram_accesses")
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.active_scopes",
        "Count",
        active_scopes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.activity",
        "Count",
        activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.cpu_responses",
        "Count",
        cpu_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.directory_decisions",
        "Count",
        directory_decisions,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.dram_accesses",
        "Count",
        dram_accesses,
        "monotonic",
    );
    for (field, stat_suffix) in CACHE_TRACE_COUNT_FIELDS {
        let value = cache_trace_sum(trace, field);
        assert_eq!(
            value,
            json_path_u64(&json, &format!("/memory_resources/cache/{field}")),
            "cache trace aggregate {field}: {trace:?}"
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.cache_trace.{stat_suffix}"),
            "Count",
            value,
            "monotonic",
        );
    }
    for (field, stat_suffix) in [
        ("prefetch_accuracy_ppm", "prefetch.accuracy_ppm"),
        ("prefetch_coverage_ppm", "prefetch.coverage_ppm"),
    ] {
        let value = json_path_u64(&json, &format!("/memory_resources/cache/{field}"));
        assert_stat(
            &stdout,
            &format!("sim.debug.cache_trace.{stat_suffix}"),
            "Ppm",
            value,
            "monotonic",
        );
    }
    for record in trace {
        assert_cache_trace_hierarchy_stats(&stdout, record);
    }
}

#[test]
fn rem6_run_fabric_debug_flag_emits_real_fabric_activity_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fabric", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--fabric-link",
            "cpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "8",
            "--fabric-request-virtual-network",
            "3",
            "--fabric-response-virtual-network",
            "4",
            "--fabric-credit-depth",
            "2",
            "--debug-flags",
            "Fabric",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Fabric".to_string())])
    );
    let trace = json
        .pointer("/debug/fabric_trace")
        .and_then(Value::as_array)
        .expect("debug fabric trace array");
    assert!(
        trace.iter().any(|record| {
            record.get("kind").and_then(Value::as_str) == Some("lane")
                && record.get("link").and_then(Value::as_str) == Some("cpu_mem")
                && record.get("virtual_network").and_then(Value::as_u64) == Some(3)
                && record
                    .get("transfer_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|transfers| transfers > 0)
                && record
                    .get("flit_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|flits| flits > 0)
        }),
        "missing request-lane fabric record: {trace:?}"
    );
    assert!(
        trace.iter().any(|record| {
            record.get("kind").and_then(Value::as_str) == Some("hop")
                && record.get("link").and_then(Value::as_str) == Some("cpu_mem")
                && record.get("virtual_network").and_then(Value::as_u64) == Some(4)
                && record
                    .get("arrival_tick")
                    .and_then(Value::as_u64)
                    .zip(record.get("start_tick").and_then(Value::as_u64))
                    .is_some_and(|(arrival, start)| arrival >= start)
        }),
        "missing response-hop fabric record: {trace:?}"
    );
    let lane_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .count() as u64;
    let hop_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("hop"))
        .count() as u64;
    let lane_transfers = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .map(|record| {
            record
                .get("transfer_count")
                .and_then(Value::as_u64)
                .expect("lane transfer_count")
        })
        .sum::<u64>();
    let lane_bytes = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .map(|record| {
            record
                .get("byte_count")
                .and_then(Value::as_u64)
                .expect("lane byte_count")
        })
        .sum::<u64>();
    let lane_flits = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .map(|record| {
            record
                .get("flit_count")
                .and_then(Value::as_u64)
                .expect("lane flit_count")
        })
        .sum::<u64>();
    let hop_bytes = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("hop"))
        .map(|record| {
            record
                .get("bytes")
                .and_then(Value::as_u64)
                .expect("hop bytes")
        })
        .sum::<u64>();
    let hop_flits = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("hop"))
        .map(|record| {
            record
                .get("flits")
                .and_then(Value::as_u64)
                .expect("hop flits")
        })
        .sum::<u64>();
    assert!(lane_records >= 2, "trace: {trace:?}");
    assert!(hop_records >= 2, "trace: {trace:?}");
    assert!(lane_transfers > 0, "trace: {trace:?}");
    assert!(lane_bytes > 0, "trace: {trace:?}");
    assert!(lane_flits > 0, "trace: {trace:?}");
    assert!(hop_bytes > 0, "trace: {trace:?}");
    assert!(hop_flits > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lanes",
        "Count",
        lane_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hops",
        "Count",
        hop_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.transfers",
        "Count",
        lane_transfers,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.bytes",
        "Byte",
        lane_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.flits",
        "Count",
        lane_flits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.bytes",
        "Byte",
        hop_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.flits",
        "Count",
        hop_flits,
        "monotonic",
    );
    assert_fabric_trace_hierarchy_stats(&stdout, trace);
}

#[test]
fn rem6_run_dram_debug_flag_emits_real_dram_hierarchy_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-dram", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--debug-flags",
            "Dram",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Dram".to_string())])
    );
    let trace = json
        .pointer("/debug/dram_trace")
        .and_then(Value::as_array)
        .expect("debug DRAM trace array");
    let target_record = trace
        .iter()
        .find(|record| {
            record.get("kind").and_then(Value::as_str) == Some("target")
                && record.get("target").and_then(Value::as_u64) == Some(0)
                && record
                    .get("accesses")
                    .and_then(Value::as_u64)
                    .is_some_and(|accesses| accesses > 0)
                && record
                    .get("reads")
                    .and_then(Value::as_u64)
                    .is_some_and(|reads| reads > 0)
        })
        .unwrap_or_else(|| panic!("missing target DRAM record: {trace:?}"));
    assert!(target_record.get("read_bytes").is_none());
    assert!(target_record.get("write_bytes").is_none());

    let port_record = trace
        .iter()
        .find(|record| {
            record.get("kind").and_then(Value::as_str) == Some("port")
                && record.get("target").and_then(Value::as_u64) == Some(0)
                && record.get("port").and_then(Value::as_u64).is_some()
                && record
                    .get("commands")
                    .and_then(Value::as_u64)
                    .is_some_and(|commands| commands > 0)
        })
        .unwrap_or_else(|| panic!("missing port DRAM record: {trace:?}"));
    assert!(
        port_record
            .get("row_hits")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose row hits: {port_record:?}"
    );
    assert!(
        port_record
            .get("row_misses")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose row misses: {port_record:?}"
    );
    assert!(
        port_record
            .get("refreshes")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose refreshes: {port_record:?}"
    );
    assert!(
        port_record
            .get("refresh_ticks")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose refresh ticks: {port_record:?}"
    );
    assert!(
        port_record
            .get("total_ready_latency_ticks")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose total ready latency: {port_record:?}"
    );
    assert!(
        port_record
            .get("max_ready_latency_ticks")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose max ready latency: {port_record:?}"
    );

    let bank_record = trace
        .iter()
        .find(|record| {
            record.get("kind").and_then(Value::as_str) == Some("bank")
                && record.get("target").and_then(Value::as_u64) == Some(0)
                && record.get("port").and_then(Value::as_u64).is_some()
                && record.get("bank").and_then(Value::as_u64).is_some()
                && record
                    .get("read_bytes")
                    .and_then(Value::as_u64)
                    .is_some_and(|bytes| bytes > 0)
                && record
                    .get("max_ready_latency_ticks")
                    .and_then(Value::as_u64)
                    .is_some()
        })
        .unwrap_or_else(|| panic!("missing bank DRAM record: {trace:?}"));
    assert!(bank_record.get("reads").is_none());
    assert!(bank_record.get("writes").is_none());
    assert!(bank_record.get("turnarounds").is_none());
    let target_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("target"))
        .count() as u64;
    let port_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("port"))
        .count() as u64;
    let bank_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("bank"))
        .count() as u64;
    let target_accesses = debug_trace_sum(trace, "target", "accesses");
    let target_reads = debug_trace_sum(trace, "target", "reads");
    let target_writes = debug_trace_sum(trace, "target", "writes");
    let port_commands = debug_trace_sum(trace, "port", "commands");
    let port_row_hits = debug_trace_sum(trace, "port", "row_hits");
    let port_row_misses = debug_trace_sum(trace, "port", "row_misses");
    let port_refreshes = debug_trace_sum(trace, "port", "refreshes");
    let port_refresh_ticks = debug_trace_sum(trace, "port", "refresh_ticks");
    let port_total_ready_latency_ticks =
        debug_trace_sum(trace, "port", "total_ready_latency_ticks");
    let port_max_ready_latency_ticks = debug_trace_max(trace, "port", "max_ready_latency_ticks");
    let bank_read_bytes = debug_trace_sum(trace, "bank", "read_bytes");
    let bank_write_bytes = debug_trace_sum(trace, "bank", "write_bytes");
    let bank_row_hits = debug_trace_sum(trace, "bank", "row_hits");
    let bank_row_misses = debug_trace_sum(trace, "bank", "row_misses");
    let bank_refreshes = debug_trace_sum(trace, "bank", "refreshes");
    let bank_refresh_ticks = debug_trace_sum(trace, "bank", "refresh_ticks");
    let bank_total_ready_latency_ticks =
        debug_trace_sum(trace, "bank", "total_ready_latency_ticks");
    let bank_max_ready_latency_ticks = debug_trace_max(trace, "bank", "max_ready_latency_ticks");
    assert!(target_records >= 1, "trace: {trace:?}");
    assert!(port_records >= 1, "trace: {trace:?}");
    assert!(bank_records >= 1, "trace: {trace:?}");
    assert!(target_accesses > 0, "trace: {trace:?}");
    assert!(target_reads > 0, "trace: {trace:?}");
    assert!(target_writes > 0, "trace: {trace:?}");
    assert!(port_commands > 0, "trace: {trace:?}");
    assert!(bank_read_bytes > 0, "trace: {trace:?}");
    assert!(bank_write_bytes > 0, "trace: {trace:?}");
    assert_eq!(port_row_hits, bank_row_hits);
    assert_eq!(port_row_misses, bank_row_misses);
    assert_eq!(port_refreshes, bank_refreshes);
    assert_eq!(port_refresh_ticks, bank_refresh_ticks);
    assert_eq!(
        port_total_ready_latency_ticks,
        bank_total_ready_latency_ticks
    );
    assert_eq!(port_max_ready_latency_ticks, bank_max_ready_latency_ticks);
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.targets",
        "Count",
        target_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.ports",
        "Count",
        port_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.banks",
        "Count",
        bank_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.accesses",
        "Count",
        target_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.reads",
        "Count",
        target_reads,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.writes",
        "Count",
        target_writes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.commands",
        "Count",
        port_commands,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.row_hits",
        "Count",
        port_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.row_misses",
        "Count",
        port_row_misses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.refreshes",
        "Count",
        port_refreshes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.refresh_ticks",
        "Tick",
        port_refresh_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.total_ready_latency_ticks",
        "Tick",
        port_total_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.max_ready_latency_ticks",
        "Tick",
        port_max_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.read_bytes",
        "Byte",
        bank_read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.write_bytes",
        "Byte",
        bank_write_bytes,
        "monotonic",
    );
    for record in trace {
        assert_dram_trace_hierarchy_stats(&stdout, record);
    }
}

#[test]
fn rem6_run_dram_debug_flag_participates_in_sorted_deduped_flag_lists() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-dram-dedup", &elf);

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
            "--dram-memory",
            "--debug-flags",
            "Fetch,Dram,Data,Dram",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![
            Value::String("Data".to_string()),
            Value::String("Dram".to_string()),
            Value::String("Fetch".to_string())
        ])
    );
    assert!(json
        .pointer("/debug/dram_trace")
        .and_then(Value::as_array)
        .is_some_and(|trace| !trace.is_empty()));
    assert!(json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .is_some_and(|trace| !trace.is_empty()));
}

#[test]
fn rem6_run_syscall_debug_flag_emits_real_riscv_se_syscall_trace() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 10, 0x13),   // addi a0, x0, 7
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-syscall", &elf);

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
            "json",
            "--execute",
            "--riscv-se",
            "--debug-flags",
            "Syscall",
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
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Syscall".to_string())])
    );
    let trace = json
        .pointer("/debug/syscall_trace")
        .and_then(Value::as_array)
        .expect("debug syscall trace array");
    assert_eq!(trace.len(), 2);

    assert_eq!(trace[0].get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        trace[0].get("pc").and_then(Value::as_str),
        Some("0x80000008")
    );
    assert_eq!(trace[0].get("number").and_then(Value::as_u64), Some(172));
    assert_eq!(
        trace[0].pointer("/arguments/0").and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        trace[0].pointer("/outcome/kind").and_then(Value::as_str),
        Some("return")
    );
    assert_eq!(
        trace[0].pointer("/outcome/value").and_then(Value::as_u64),
        Some(100)
    );

    assert_eq!(trace[1].get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        trace[1].get("pc").and_then(Value::as_str),
        Some("0x80000014")
    );
    assert_eq!(trace[1].get("number").and_then(Value::as_u64), Some(93));
    assert_eq!(
        trace[1].pointer("/outcome/kind").and_then(Value::as_str),
        Some("exit")
    );
    assert_eq!(
        trace[1].pointer("/outcome/code").and_then(Value::as_i64),
        Some(0)
    );
    let syscall_numbers = syscall_trace_unique_u64(trace, "number");
    let call_sites = syscall_trace_unique_strings(trace, "pc");
    let cpus = syscall_trace_unique_u64(trace, "cpu");
    let argument_words = syscall_trace_argument_words(trace);
    let nonzero_arguments = syscall_trace_nonzero_arguments(trace);
    assert_eq!(syscall_numbers, 2, "trace: {trace:?}");
    assert_eq!(call_sites, 2, "trace: {trace:?}");
    assert_eq!(cpus, 1, "trace: {trace:?}");
    assert_eq!(argument_words, 12, "trace: {trace:?}");
    assert_eq!(nonzero_arguments, 1, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.returns",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.exits",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.blocked",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.syscall_numbers",
        "Count",
        syscall_numbers,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.call_sites",
        "Count",
        call_sites,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.cpus",
        "Count",
        cpus,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.argument_words",
        "Count",
        argument_words,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.nonzero_arguments",
        "Count",
        nonzero_arguments,
        "monotonic",
    );
    assert_syscall_trace_hierarchy_stats(&stdout, trace);
}

fn debug_trace_sum(trace: &[Value], kind: &str, field: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{kind} {field}"))
        })
        .sum()
}

fn debug_trace_max(trace: &[Value], kind: &str, field: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{kind} {field}"))
        })
        .max()
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExecTraceStats {
    records: u64,
    retired: u64,
    bytes: u64,
}

impl ExecTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        if record
            .get("retired")
            .and_then(Value::as_bool)
            .expect("exec retired")
        {
            self.retired = self.retired.saturating_add(1);
        }
        self.bytes = self.bytes.saturating_add(
            record
                .get("bytes")
                .and_then(Value::as_str)
                .expect("exec bytes")
                .len() as u64
                / 2,
        );
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("retired", "Count", self.retired),
            ("bytes", "Byte", self.bytes),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_exec_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, ExecTraceStats>::new();
    let mut retirement = BTreeMap::<&'static str, ExecTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let retired = match record
            .get("retired")
            .and_then(Value::as_bool)
            .expect("exec retired")
        {
            true => "retired",
            false => "not_retired",
        };
        cpus.entry(cpu).or_default().add_record(record);
        retirement.entry(retired).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.exec_trace.cpu.cpu{cpu}"));
    }
    for (retirement, stats) in retirement {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.exec_trace.retirement.{retirement}"),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FetchTraceStats {
    records: u64,
    bytes: u64,
}

impl FetchTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(json_record_u64(record, "size"));
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("bytes", "Byte", self.bytes),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_fetch_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, FetchTraceStats>::new();
    let mut endpoints = BTreeMap::<String, FetchTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let endpoint = json_record_str(record, "endpoint").to_string();
        cpus.entry(cpu).or_default().add_record(record);
        endpoints.entry(endpoint).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.fetch_trace.cpu.cpu{cpu}"));
    }
    for (endpoint, stats) in endpoints {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.fetch_trace.endpoint.{}",
                stat_path_segment(&endpoint)
            ),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DataTraceStats {
    records: u64,
    loads: u64,
    stores: u64,
    atomics: u64,
    bytes: u64,
    load_bytes: u64,
    store_bytes: u64,
    atomic_bytes: u64,
}

impl DataTraceStats {
    fn add_record(&mut self, record: &Value) {
        let size = json_record_u64(record, "size");
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(size);
        match json_record_str(record, "kind") {
            "load" => {
                self.loads = self.loads.saturating_add(1);
                self.load_bytes = self.load_bytes.saturating_add(size);
            }
            "store" => {
                self.stores = self.stores.saturating_add(1);
                self.store_bytes = self.store_bytes.saturating_add(size);
            }
            "atomic" => {
                self.atomics = self.atomics.saturating_add(1);
                self.atomic_bytes = self.atomic_bytes.saturating_add(size);
            }
            other => panic!("unexpected data trace kind {other}: {record:?}"),
        }
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("loads", "Count", self.loads),
            ("stores", "Count", self.stores),
            ("atomics", "Count", self.atomics),
            ("bytes", "Byte", self.bytes),
            ("load_bytes", "Byte", self.load_bytes),
            ("store_bytes", "Byte", self.store_bytes),
            ("atomic_bytes", "Byte", self.atomic_bytes),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_data_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, DataTraceStats>::new();
    let mut kinds = BTreeMap::<String, DataTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let kind = json_record_str(record, "kind").to_string();
        cpus.entry(cpu).or_default().add_record(record);
        kinds.entry(kind).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.data_trace.cpu.cpu{cpu}"));
    }
    for (kind, stats) in kinds {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.data_trace.kind.{}", stat_path_segment(&kind)),
        );
    }
}

fn assert_dram_trace_hierarchy_stats(stdout: &str, record: &Value) {
    let kind = record
        .get("kind")
        .and_then(Value::as_str)
        .expect("DRAM trace kind");
    let target = record
        .get("target")
        .and_then(Value::as_u64)
        .expect("DRAM trace target");
    match kind {
        "target" => {
            let prefix = format!("sim.debug.dram_trace.target{target}");
            assert_dram_trace_record_stats(
                stdout,
                &prefix,
                record,
                &[
                    ("accesses", "Count"),
                    ("reads", "Count"),
                    ("writes", "Count"),
                    ("row_hits", "Count"),
                    ("row_misses", "Count"),
                    ("refreshes", "Count"),
                    ("refresh_ticks", "Tick"),
                    ("commands", "Count"),
                    ("turnarounds", "Count"),
                    ("total_ready_latency_ticks", "Tick"),
                    ("max_ready_latency_ticks", "Tick"),
                ],
            );
        }
        "port" => {
            let port = record
                .get("port")
                .and_then(Value::as_u64)
                .expect("DRAM trace port");
            let prefix = format!("sim.debug.dram_trace.target{target}.port{port}");
            assert_dram_trace_record_stats(
                stdout,
                &prefix,
                record,
                &[
                    ("accesses", "Count"),
                    ("reads", "Count"),
                    ("writes", "Count"),
                    ("row_hits", "Count"),
                    ("row_misses", "Count"),
                    ("refreshes", "Count"),
                    ("refresh_ticks", "Tick"),
                    ("commands", "Count"),
                    ("turnarounds", "Count"),
                    ("total_ready_latency_ticks", "Tick"),
                    ("max_ready_latency_ticks", "Tick"),
                ],
            );
        }
        "bank" => {
            let port = record
                .get("port")
                .and_then(Value::as_u64)
                .expect("DRAM trace port");
            let bank = record
                .get("bank")
                .and_then(Value::as_u64)
                .expect("DRAM trace bank");
            let prefix = format!("sim.debug.dram_trace.target{target}.port{port}.bank{bank}");
            assert_dram_trace_record_stats(
                stdout,
                &prefix,
                record,
                &[
                    ("accesses", "Count"),
                    ("read_bytes", "Byte"),
                    ("write_bytes", "Byte"),
                    ("row_hits", "Count"),
                    ("row_misses", "Count"),
                    ("refreshes", "Count"),
                    ("refresh_ticks", "Tick"),
                    ("commands", "Count"),
                    ("total_ready_latency_ticks", "Tick"),
                    ("max_ready_latency_ticks", "Tick"),
                ],
            );
        }
        other => panic!("unexpected DRAM trace kind {other}: {record:?}"),
    }
}

fn assert_dram_trace_record_stats(
    stdout: &str,
    prefix: &str,
    record: &Value,
    fields: &[(&str, &str)],
) {
    for (field, unit) in fields {
        assert_stat(
            stdout,
            &format!("{prefix}.{field}"),
            unit,
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("DRAM trace {prefix}.{field}: {record:?}")),
            "monotonic",
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FabricHopStats {
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
}

fn assert_fabric_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut hop_stats = BTreeMap::<(String, u64, u64), FabricHopStats>::new();
    for record in trace {
        let kind = record
            .get("kind")
            .and_then(Value::as_str)
            .expect("fabric trace kind");
        match kind {
            "lane" => assert_fabric_lane_trace_stats(stdout, record),
            "hop" => {
                let link = record
                    .get("link")
                    .and_then(Value::as_str)
                    .expect("fabric hop link")
                    .to_string();
                let virtual_network = record
                    .get("virtual_network")
                    .and_then(Value::as_u64)
                    .expect("fabric hop virtual_network");
                let hop_index = record
                    .get("hop_index")
                    .and_then(Value::as_u64)
                    .expect("fabric hop hop_index");
                let summary = hop_stats
                    .entry((link, virtual_network, hop_index))
                    .or_default();
                summary.transfers = summary.transfers.saturating_add(1);
                summary.bytes = summary
                    .bytes
                    .saturating_add(json_record_u64(record, "bytes"));
                summary.flits = summary
                    .flits
                    .saturating_add(json_record_u64(record, "flits"));
                summary.occupied_ticks = summary
                    .occupied_ticks
                    .saturating_add(json_record_u64(record, "occupied_ticks"));
                let queue_delay_ticks = json_record_u64(record, "queue_delay_ticks");
                summary.queue_delay_ticks =
                    summary.queue_delay_ticks.saturating_add(queue_delay_ticks);
                summary.max_queue_delay_ticks =
                    summary.max_queue_delay_ticks.max(queue_delay_ticks);
                summary.credit_delay_ticks = summary
                    .credit_delay_ticks
                    .saturating_add(json_record_u64(record, "credit_delay_ticks"));
            }
            other => panic!("unexpected fabric trace kind {other}: {record:?}"),
        }
    }
    for ((link, virtual_network, hop_index), summary) in hop_stats {
        let prefix = format!(
            "sim.debug.fabric_trace.hop.link.{}.vn{virtual_network}.hop{hop_index}",
            stat_path_segment(&link)
        );
        for (suffix, unit, value) in [
            ("transfers", "Count", summary.transfers),
            ("bytes", "Byte", summary.bytes),
            ("flits", "Count", summary.flits),
            ("occupied_ticks", "Tick", summary.occupied_ticks),
            ("queue_delay_ticks", "Tick", summary.queue_delay_ticks),
            (
                "max_queue_delay_ticks",
                "Tick",
                summary.max_queue_delay_ticks,
            ),
            ("credit_delay_ticks", "Tick", summary.credit_delay_ticks),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_fabric_lane_trace_stats(stdout: &str, record: &Value) {
    let link = record
        .get("link")
        .and_then(Value::as_str)
        .expect("fabric lane link");
    let virtual_network = record
        .get("virtual_network")
        .and_then(Value::as_u64)
        .expect("fabric lane virtual_network");
    let prefix = format!(
        "sim.debug.fabric_trace.lane.link.{}.vn{virtual_network}",
        stat_path_segment(link)
    );
    for (stat_suffix, field, unit) in [
        ("transfers", "transfer_count", "Count"),
        ("bytes", "byte_count", "Byte"),
        ("flits", "flit_count", "Count"),
        ("occupied_ticks", "occupied_ticks", "Tick"),
        ("queue_delay_ticks", "queue_delay_ticks", "Tick"),
        ("max_queue_delay_ticks", "max_queue_delay_ticks", "Tick"),
        ("credit_delay_ticks", "credit_delay_ticks", "Tick"),
        ("max_credit_delay_ticks", "max_credit_delay_ticks", "Tick"),
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{stat_suffix}"),
            unit,
            json_record_u64(record, field),
            "monotonic",
        );
    }
}

fn json_record_u64(record: &Value, field: &str) -> u64 {
    record
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing JSON u64 field {field}: {record:?}"))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MemoryTraceStats {
    records: u64,
    requests: BTreeSet<(u64, u64)>,
    routes: BTreeSet<u64>,
    request_agents: BTreeSet<u64>,
    events: BTreeMap<String, u64>,
    response_status: BTreeMap<String, u64>,
}

impl MemoryTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        let request_agent = json_record_u64(record, "request_agent");
        let request = json_record_u64(record, "request");
        let route = json_record_u64(record, "route");
        let kind = json_record_str(record, "kind").to_string();
        self.requests.insert((request_agent, request));
        self.routes.insert(route);
        self.request_agents.insert(request_agent);
        self.events
            .entry(kind)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        if let Some(status) = record.get("response_status").and_then(Value::as_str) {
            self.response_status
                .entry(status.to_string())
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
        }
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("requests", self.requests.len() as u64),
            ("routes", self.routes.len() as u64),
            ("request_agents", self.request_agents.len() as u64),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                "Count",
                value,
                "monotonic",
            );
        }
        for (kind, value) in &self.events {
            assert_stat(
                stdout,
                &format!("{prefix}.events.{kind}"),
                "Count",
                *value,
                "monotonic",
            );
        }
        for (status, value) in &self.response_status {
            assert_stat(
                stdout,
                &format!("{prefix}.response_status.{status}"),
                "Count",
                *value,
                "monotonic",
            );
        }
    }
}

fn assert_memory_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut channels = BTreeMap::<String, MemoryTraceStats>::new();
    let mut routes = BTreeMap::<(String, u64, String), MemoryTraceStats>::new();
    let mut request_agents = BTreeMap::<(String, u64), MemoryTraceStats>::new();
    for record in trace {
        let channel = json_record_str(record, "channel").to_string();
        let route = json_record_u64(record, "route");
        let endpoint = json_record_str(record, "endpoint").to_string();
        let request_agent = json_record_u64(record, "request_agent");
        channels
            .entry(channel.clone())
            .or_default()
            .add_record(record);
        routes
            .entry((channel.clone(), route, endpoint))
            .or_default()
            .add_record(record);
        request_agents
            .entry((channel, request_agent))
            .or_default()
            .add_record(record);
    }
    for (channel, stats) in channels {
        let prefix = format!(
            "sim.debug.memory_trace.channel.{}",
            stat_path_segment(&channel)
        );
        stats.assert_stats(stdout, &prefix);
    }
    for ((channel, route, endpoint), stats) in routes {
        let prefix = format!(
            "sim.debug.memory_trace.channel.{}.route{route}.endpoint.{}",
            stat_path_segment(&channel),
            stat_path_segment(&endpoint)
        );
        stats.assert_stats(stdout, &prefix);
    }
    for ((channel, request_agent), stats) in request_agents {
        let prefix = format!(
            "sim.debug.memory_trace.channel.{}.request_agent.agent{request_agent}",
            stat_path_segment(&channel)
        );
        stats.assert_stats(stdout, &prefix);
    }
}

fn json_record_str<'a>(record: &'a Value, field: &str) -> &'a str {
    record
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing JSON string field {field}: {record:?}"))
}

const CACHE_TRACE_COUNT_FIELDS: &[(&str, &str)] = &[
    ("bank_accepted", "bank.accepted"),
    ("bank_immediate_hits", "bank.immediate_hits"),
    ("bank_scheduled_misses", "bank.scheduled_misses"),
    ("bank_coalesced_misses", "bank.coalesced_misses"),
    ("prefetch_identified", "prefetch.identified"),
    ("prefetch_issued", "prefetch.issued"),
    ("prefetch_useful", "prefetch.useful"),
    ("prefetch_useful_but_miss", "prefetch.useful_but_miss"),
    ("prefetch_unused", "prefetch.unused"),
    ("prefetch_demand_mshr_misses", "prefetch.demand_mshr_misses"),
    ("prefetch_hit_in_cache", "prefetch.hit_in_cache"),
    ("prefetch_hit_in_mshr", "prefetch.hit_in_mshr"),
    (
        "prefetch_hit_in_write_buffer",
        "prefetch.hit_in_write_buffer",
    ),
    ("prefetch_late", "prefetch.late"),
    ("prefetch_span_page", "prefetch.span_page"),
    ("prefetch_useful_span_page", "prefetch.useful_span_page"),
    ("prefetch_in_cache", "prefetch.in_cache"),
    ("prefetch_queue_enqueued", "prefetch.queue.enqueued"),
    ("prefetch_queue_issued", "prefetch.queue.issued"),
    ("prefetch_queue_dropped", "prefetch.queue.dropped"),
    (
        "prefetch_translation_queue_enqueued",
        "prefetch.translation_queue.enqueued",
    ),
    (
        "prefetch_translation_queue_issued",
        "prefetch.translation_queue.issued",
    ),
    (
        "prefetch_translation_queue_translated",
        "prefetch.translation_queue.translated",
    ),
    (
        "prefetch_translation_queue_dropped",
        "prefetch.translation_queue.dropped",
    ),
];

fn assert_cache_trace_record(
    trace: &[Value],
    hierarchy: &str,
    level: &str,
    json: &Value,
    resource_path: &str,
) {
    let record = trace
        .iter()
        .find(|record| {
            record.get("hierarchy").and_then(Value::as_str) == Some(hierarchy)
                && record.get("level").and_then(Value::as_str) == Some(level)
        })
        .unwrap_or_else(|| panic!("missing cache trace record {hierarchy}.{level}: {trace:?}"));
    for field in [
        "activity",
        "active",
        "cpu_responses",
        "directory_decisions",
        "dram_accesses",
    ] {
        assert_eq!(
            record.get(field),
            json.pointer(&format!("{resource_path}/{field}")),
            "cache trace {hierarchy}.{level}.{field}: {record:?}"
        );
    }
    for (field, _) in CACHE_TRACE_COUNT_FIELDS {
        assert_eq!(
            record.get(field),
            json.pointer(&format!("{resource_path}/{field}")),
            "cache trace {hierarchy}.{level}.{field}: {record:?}"
        );
    }
    for field in ["prefetch_accuracy_ppm", "prefetch_coverage_ppm"] {
        assert_eq!(
            record.get(field),
            json.pointer(&format!("{resource_path}/{field}")),
            "cache trace {hierarchy}.{level}.{field}: {record:?}"
        );
    }
}

fn cache_trace_active_count(trace: &[Value]) -> u64 {
    trace
        .iter()
        .filter(|record| {
            record
                .get("active")
                .and_then(Value::as_u64)
                .is_some_and(|active| active > 0)
        })
        .count() as u64
}

fn cache_trace_sum(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("cache trace {field}"))
        })
        .sum()
}

fn assert_cache_trace_hierarchy_stats(stdout: &str, record: &Value) {
    let hierarchy = record
        .get("hierarchy")
        .and_then(Value::as_str)
        .expect("cache trace hierarchy");
    let level = record
        .get("level")
        .and_then(Value::as_str)
        .expect("cache trace level");
    let prefix = format!("sim.debug.cache_trace.hierarchy.{hierarchy}.{level}");
    for field in [
        "activity",
        "active",
        "cpu_responses",
        "directory_decisions",
        "dram_accesses",
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{field}"),
            "Count",
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("cache trace {hierarchy}.{level}.{field}")),
            "monotonic",
        );
    }
    for (field, stat_suffix) in CACHE_TRACE_COUNT_FIELDS {
        assert_stat(
            stdout,
            &format!("{prefix}.{stat_suffix}"),
            "Count",
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("cache trace {hierarchy}.{level}.{field}")),
            "monotonic",
        );
    }
    for (field, stat_suffix) in [
        ("prefetch_accuracy_ppm", "prefetch.accuracy_ppm"),
        ("prefetch_coverage_ppm", "prefetch.coverage_ppm"),
    ] {
        if let Some(value) = record.get(field).and_then(Value::as_u64) {
            assert_stat(
                stdout,
                &format!("{prefix}.{stat_suffix}"),
                "Ppm",
                value,
                "monotonic",
            );
        }
    }
}

fn json_path_u64(json: &Value, path: &str) -> u64 {
    json.pointer(path)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing JSON u64 path {path}"))
}

#[test]
fn rem6_run_power_debug_flag_emits_activity_power_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-power", &elf);

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
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--debug-flags",
            "Power",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Power".to_string())])
    );
    let trace = json
        .pointer("/debug/power_trace")
        .and_then(Value::as_array)
        .expect("debug power trace array");
    let targets = power_trace_unique_strings(trace, "target");
    let states = power_trace_unique_strings(trace, "state");
    let on_records = power_trace_state_count(trace, "on");
    let residency_ticks = power_trace_sum_u64(trace, "residency_ticks");
    let dynamic_microwatts = power_trace_microwatts(trace, "dynamic_watts");
    let static_microwatts = power_trace_microwatts(trace, "static_watts");
    let total_microwatts = power_trace_microwatts(trace, "total_watts");
    let dynamic_microwatt_ticks = power_trace_microwatt_ticks(trace, "dynamic_watts");
    let static_microwatt_ticks = power_trace_microwatt_ticks(trace, "static_watts");
    let total_microwatt_ticks = power_trace_microwatt_ticks(trace, "total_watts");
    let max_temperature_millicelsius = power_trace_max_millicelsius(trace, "temperature_c");
    let json_text = json.to_string();
    assert!(targets > 0, "trace: {trace:?}");
    assert!(states > 0, "trace: {trace:?}");
    assert!(on_records > 0, "trace: {trace:?}");
    assert!(residency_ticks > 0, "trace: {trace:?}");
    assert!(dynamic_microwatts > 0, "trace: {trace:?}");
    assert!(static_microwatts > 0, "trace: {trace:?}");
    assert!(total_microwatts >= dynamic_microwatts, "trace: {trace:?}");
    assert!(dynamic_microwatt_ticks > 0, "trace: {trace:?}");
    assert!(static_microwatt_ticks > 0, "trace: {trace:?}");
    assert!(
        total_microwatt_ticks >= dynamic_microwatt_ticks,
        "trace: {trace:?}"
    );
    assert!(max_temperature_millicelsius > 0, "trace: {trace:?}");
    for target in [
        "cpu0.core",
        "cpu.instruction_cache",
        "cpu.data_cache",
        "memory.transport",
        "memory.dram",
    ] {
        let record = trace
            .iter()
            .find(|record| record.get("target").and_then(Value::as_str) == Some(target))
            .unwrap_or_else(|| panic!("missing power trace target {target}: {trace:?}"));
        assert_eq!(record.get("state").and_then(Value::as_str), Some("on"));
        assert!(
            record
                .get("residency_ticks")
                .and_then(Value::as_u64)
                .is_some_and(|ticks| ticks > 0),
            "missing residency ticks for {target}: {record:?}"
        );
        assert!(
            record
                .get("dynamic_watts")
                .and_then(Value::as_f64)
                .is_some_and(|watts| watts > 0.0),
            "missing dynamic watts for {target}: {record:?}"
        );
        let target_prefix = power_trace_target_stat_prefix(target);
        assert_stat(
            &json_text,
            &format!("{target_prefix}.records"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.states.on"),
            "Count",
            power_trace_record_state_count(record, "on"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.residency_ticks"),
            "Tick",
            power_trace_record_u64(record, "residency_ticks"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.dynamic_microwatts"),
            "MicroWatt",
            power_trace_record_microwatts(record, "dynamic_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.static_microwatts"),
            "MicroWatt",
            power_trace_record_microwatts(record, "static_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.total_microwatts"),
            "MicroWatt",
            power_trace_record_microwatts(record, "total_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.dynamic_microwatt_ticks"),
            "MicroWattTick",
            power_trace_record_microwatt_ticks(record, "dynamic_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.static_microwatt_ticks"),
            "MicroWattTick",
            power_trace_record_microwatt_ticks(record, "static_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.total_microwatt_ticks"),
            "MicroWattTick",
            power_trace_record_microwatt_ticks(record, "total_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.max_temperature_millicelsius"),
            "MilliCelsius",
            power_trace_record_millicelsius(record, "temperature_c"),
            "monotonic",
        );
    }
    assert_stat(
        &json_text,
        "sim.debug.power_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.targets",
        "Count",
        targets,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.states",
        "Count",
        states,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.states.on",
        "Count",
        on_records,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.residency_ticks",
        "Tick",
        residency_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.dynamic_microwatts",
        "MicroWatt",
        dynamic_microwatts,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.static_microwatts",
        "MicroWatt",
        static_microwatts,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.total_microwatts",
        "MicroWatt",
        total_microwatts,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.dynamic_microwatt_ticks",
        "MicroWattTick",
        dynamic_microwatt_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.static_microwatt_ticks",
        "MicroWattTick",
        static_microwatt_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.total_microwatt_ticks",
        "MicroWattTick",
        total_microwatt_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.max_temperature_millicelsius",
        "MilliCelsius",
        max_temperature_millicelsius,
        "monotonic",
    );
}

#[test]
fn rem6_run_loads_debug_flags_from_toml_config() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("debug-flags-config");
    let binary = workspace.join("kernel.elf");
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"kernel.elf\"\nmax_tick = 60\nexecute = true\nmemory_system = \"direct\"\nstats_format = \"json\"\ndebug_flags = [\"Exec\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_exec_trace(
        &json,
        &[
            ExpectedExecTraceRecord {
                tick: 2,
                pc: "0x80000000",
                bytes: "93027000",
            },
            ExpectedExecTraceRecord {
                tick: 4,
                pc: "0x80000004",
                bytes: "73000000",
            },
        ],
    );
}

#[test]
fn rem6_run_rejects_unknown_debug_flag() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-unknown", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--execute",
            "--debug-flags",
            "Exec,NoSuchFlag",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported debug flag NoSuchFlag"));
}

#[test]
fn rem6_run_rejects_empty_debug_flag_entries() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-empty", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--execute",
            "--debug-flags",
            "Exec,",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("empty debug flag entry"));
}

#[test]
fn rem6_run_rejects_debug_flags_without_execution() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-no-execute", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "json",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--debug-flags requires --execute"));
}

#[test]
fn rem6_run_rejects_exec_debug_flags_with_text_stats() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-text-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "text",
            "--execute",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--debug-flags requires --stats-format json"));
}

struct ExpectedExecTraceRecord {
    tick: u64,
    pc: &'static str,
    bytes: &'static str,
}

struct ExpectedFetchTraceRecord {
    tick: u64,
    pc: &'static str,
    sequence: u64,
    size: u64,
}

fn stdout_json(stdout: Vec<u8>) -> Value {
    serde_json::from_slice(&stdout)
        .unwrap_or_else(|error| panic!("invalid JSON stdout: {error}; stdout={:?}", stdout))
}

fn assert_exec_trace(json: &Value, expected: &[ExpectedExecTraceRecord]) {
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Exec".to_string())])
    );
    let trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    assert_eq!(trace.len(), expected.len());
    for (record, expected) in trace.iter().zip(expected) {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("tick").and_then(Value::as_u64),
            Some(expected.tick)
        );
        assert_eq!(record.get("pc").and_then(Value::as_str), Some(expected.pc));
        assert_eq!(
            record.get("bytes").and_then(Value::as_str),
            Some(expected.bytes)
        );
        assert_eq!(record.get("retired").and_then(Value::as_bool), Some(true));
    }
}

fn assert_fetch_trace(json: &Value, expected: &[ExpectedFetchTraceRecord]) {
    let trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    assert_eq!(trace.len(), expected.len());
    for (record, expected) in trace.iter().zip(expected) {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("tick").and_then(Value::as_u64),
            Some(expected.tick)
        );
        assert_eq!(record.get("pc").and_then(Value::as_str), Some(expected.pc));
        assert_eq!(
            record.get("sequence").and_then(Value::as_u64),
            Some(expected.sequence)
        );
        assert_eq!(
            record.get("size").and_then(Value::as_u64),
            Some(expected.size)
        );
    }
}

fn assert_fetch_pcs(json: &Value, expected: &[&str]) {
    let trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    let pcs = trace
        .iter()
        .map(|record| record.get("pc").and_then(Value::as_str).unwrap_or(""))
        .collect::<Vec<_>>();
    assert_eq!(pcs, expected);
}

fn memory_trace_unique_requests(trace: &[Value], channel: Option<&str>) -> u64 {
    let mut requests = BTreeSet::new();
    for record in trace {
        if !memory_trace_channel_matches(record, channel) {
            continue;
        }
        let channel = record
            .get("channel")
            .and_then(Value::as_str)
            .expect("memory trace channel");
        let request_agent = record
            .get("request_agent")
            .and_then(Value::as_u64)
            .expect("memory trace request agent");
        let request = record
            .get("request")
            .and_then(Value::as_u64)
            .expect("memory trace request");
        requests.insert((channel, request_agent, request));
    }
    requests.len() as u64
}

fn memory_trace_unique_routes(trace: &[Value], channel: Option<&str>) -> u64 {
    let mut routes = BTreeSet::new();
    for record in trace {
        if !memory_trace_channel_matches(record, channel) {
            continue;
        }
        let channel = record
            .get("channel")
            .and_then(Value::as_str)
            .expect("memory trace channel");
        let route = record
            .get("route")
            .and_then(Value::as_u64)
            .expect("memory trace route");
        routes.insert((channel, route));
    }
    routes.len() as u64
}

fn memory_trace_unique_request_agents(trace: &[Value]) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get("request_agent")
                .and_then(Value::as_u64)
                .expect("memory trace request agent")
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn memory_trace_channel_matches(record: &Value, channel: Option<&str>) -> bool {
    channel.map_or(true, |expected| {
        record.get("channel").and_then(Value::as_str) == Some(expected)
    })
}

fn syscall_trace_unique_u64(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("syscall trace {field}"))
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn syscall_trace_unique_strings(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("syscall trace {field}"))
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn syscall_trace_argument_words(trace: &[Value]) -> u64 {
    trace
        .iter()
        .map(|record| syscall_trace_arguments(record).len() as u64)
        .sum()
}

fn syscall_trace_nonzero_arguments(trace: &[Value]) -> u64 {
    trace
        .iter()
        .map(syscall_trace_record_nonzero_arguments)
        .sum()
}

fn syscall_trace_record_nonzero_arguments(record: &Value) -> u64 {
    syscall_trace_arguments(record)
        .iter()
        .filter(|argument| argument.as_u64().is_some_and(|value| value != 0))
        .count() as u64
}

fn syscall_trace_arguments(record: &Value) -> &[Value] {
    record
        .get("arguments")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .expect("syscall trace arguments")
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SyscallTraceStats {
    records: u64,
    syscall_numbers: BTreeSet<u64>,
    call_sites: BTreeSet<String>,
    cpus: BTreeSet<u64>,
    returns: u64,
    exits: u64,
    blocked: u64,
    argument_words: u64,
    nonzero_arguments: u64,
}

impl SyscallTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        self.syscall_numbers
            .insert(json_record_u64(record, "number"));
        self.call_sites
            .insert(json_record_str(record, "pc").to_string());
        self.cpus.insert(json_record_u64(record, "cpu"));
        self.argument_words = self
            .argument_words
            .saturating_add(syscall_trace_arguments(record).len() as u64);
        self.nonzero_arguments = self
            .nonzero_arguments
            .saturating_add(syscall_trace_record_nonzero_arguments(record));
        match syscall_trace_outcome_kind(record) {
            "return" => self.returns = self.returns.saturating_add(1),
            "exit" => self.exits = self.exits.saturating_add(1),
            "blocked" => self.blocked = self.blocked.saturating_add(1),
            other => panic!("unexpected syscall outcome {other}: {record:?}"),
        }
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("returns", self.returns),
            ("exits", self.exits),
            ("blocked", self.blocked),
            ("syscall_numbers", self.syscall_numbers.len() as u64),
            ("call_sites", self.call_sites.len() as u64),
            ("cpus", self.cpus.len() as u64),
            ("argument_words", self.argument_words),
            ("nonzero_arguments", self.nonzero_arguments),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                "Count",
                value,
                "monotonic",
            );
        }
    }
}

fn assert_syscall_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, SyscallTraceStats>::new();
    let mut numbers = BTreeMap::<u64, SyscallTraceStats>::new();
    let mut call_sites = BTreeMap::<String, SyscallTraceStats>::new();
    let mut outcomes = BTreeMap::<String, SyscallTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let number = json_record_u64(record, "number");
        let call_site = json_record_str(record, "pc").to_string();
        let outcome = syscall_trace_outcome_kind(record).to_string();
        cpus.entry(cpu).or_default().add_record(record);
        numbers.entry(number).or_default().add_record(record);
        call_sites.entry(call_site).or_default().add_record(record);
        outcomes.entry(outcome).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.syscall_trace.cpu.cpu{cpu}"));
    }
    for (number, stats) in numbers {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.syscall_trace.number.syscall{number}"),
        );
    }
    for (call_site, stats) in call_sites {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.syscall_trace.call_site.{}",
                stat_path_segment(&call_site)
            ),
        );
    }
    for (outcome, stats) in outcomes {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.syscall_trace.outcome.{}",
                stat_path_segment(&outcome)
            ),
        );
    }
}

fn syscall_trace_outcome_kind(record: &Value) -> &str {
    record
        .pointer("/outcome/kind")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("syscall trace outcome kind: {record:?}"))
}

fn power_trace_unique_strings(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("power trace {field}"))
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn power_trace_state_count(trace: &[Value], state: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("state").and_then(Value::as_str) == Some(state))
        .count() as u64
}

fn power_trace_sum_u64(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| power_trace_record_u64(record, field))
        .sum()
}

fn power_trace_microwatts(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| power_trace_record_microwatts(record, field))
        .sum()
}

fn power_trace_microwatt_ticks(trace: &[Value], field: &str) -> u64 {
    trace.iter().fold(0u64, |acc, record| {
        acc.saturating_add(power_trace_record_microwatt_ticks(record, field))
    })
}

fn power_trace_record_u64(record: &Value, field: &str) -> u64 {
    record
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("power trace {field}"))
}

fn power_trace_record_state_count(record: &Value, state: &str) -> u64 {
    u64::from(record.get("state").and_then(Value::as_str) == Some(state))
}

fn power_trace_record_microwatts(record: &Value, field: &str) -> u64 {
    let watts = record
        .get(field)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("power trace {field}"));
    watts_to_microwatts(watts)
}

fn power_trace_record_microwatt_ticks(record: &Value, field: &str) -> u64 {
    let residency_ticks = power_trace_record_u64(record, "residency_ticks");
    power_trace_record_microwatts(record, field).saturating_mul(residency_ticks)
}

fn power_trace_target_stat_prefix(target: &str) -> String {
    let target_path = target
        .split('.')
        .map(stat_path_segment)
        .collect::<Vec<_>>()
        .join(".");
    format!("sim.debug.power_trace.target.{target_path}")
}

fn power_trace_record_millicelsius(record: &Value, field: &str) -> u64 {
    let celsius = record
        .get(field)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("power trace {field}"));
    celsius_to_millicelsius(celsius)
}

fn power_trace_max_millicelsius(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| power_trace_record_millicelsius(record, field))
        .max()
        .unwrap_or(0)
}

fn watts_to_microwatts(watts: f64) -> u64 {
    if !watts.is_finite() || watts <= 0.0 {
        0
    } else {
        (watts * 1_000_000.0).round() as u64
    }
}

fn celsius_to_millicelsius(celsius: f64) -> u64 {
    if !celsius.is_finite() || celsius <= 0.0 {
        0
    } else {
        (celsius * 1_000.0).round() as u64
    }
}
