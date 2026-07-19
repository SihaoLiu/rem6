use super::*;

#[derive(Clone, Copy)]
struct CrossResetMemoryCase {
    name: &'static str,
    memory_system: &'static str,
    operation: CrossResetOperation,
    request_tick: u64,
    reset_tick: u64,
    response_tick: u64,
    response_latency_ticks: u64,
    dump_tick: u64,
}

#[derive(Clone, Copy)]
enum CrossResetOperation {
    LoadReserved,
    AtomicSwap,
}

impl CrossResetOperation {
    const fn snake_case(self) -> &'static str {
        match self {
            Self::LoadReserved => "load_reserved",
            Self::AtomicSwap => "atomic",
        }
    }

    const fn camel_case(self) -> &'static str {
        match self {
            Self::LoadReserved => "loadReserved",
            Self::AtomicSwap => "atomic",
        }
    }
}

#[test]
fn rem6_run_m5_reset_between_o3_lsq_request_and_response_keeps_latency() {
    for case in [
        CrossResetMemoryCase {
            name: "direct",
            memory_system: "direct",
            operation: CrossResetOperation::LoadReserved,
            request_tick: 190,
            reset_tick: 201,
            response_tick: 216,
            response_latency_ticks: 26,
            dump_tick: 246,
        },
        CrossResetMemoryCase {
            name: "cache-fabric-dram",
            memory_system: "cache-fabric-dram",
            operation: CrossResetOperation::AtomicSwap,
            request_tick: 220,
            reset_tick: 229,
            response_tick: 256,
            response_latency_ticks: 36,
            dump_tick: 288,
        },
    ] {
        assert_cross_reset_atomic_response(case);
    }
}

fn assert_cross_reset_atomic_response(case: CrossResetMemoryCase) {
    let path = cross_reset_atomic_response_binary(
        &format!("m5-reset-o3-lsq-response-{}", case.name),
        case.operation,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            case.memory_system,
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "13",
            "--debug-flags",
            "O3,Data,Memory",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{} stderr: {}",
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{} invalid stdout JSON: {error}", case.name));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host"),
        "{} should stop through m5_exit: {json}",
        case.name
    );

    let reset = json
        .pointer("/host_actions/stats_resets/0")
        .unwrap_or_else(|| panic!("{} missing reset action: {json}", case.name));
    assert_eq!(
        reset.pointer("/tick").and_then(Value::as_u64),
        Some(case.reset_tick),
        "{} reset should fire while the atomic response is in flight: {reset}",
        case.name
    );
    assert_eq!(reset.pointer("/epoch").and_then(Value::as_u64), Some(1));

    let dump = json
        .pointer("/host_actions/stats_dumps/0")
        .unwrap_or_else(|| panic!("{} missing post-reset stats dump: {json}", case.name));
    assert_eq!(
        dump.pointer("/tick").and_then(Value::as_u64),
        Some(case.dump_tick),
        "{} dump should run after the atomic response: {dump}",
        case.name
    );
    assert_eq!(dump.pointer("/epoch").and_then(Value::as_u64), Some(1));

    let request = unique_data_memory_trace_event(&json, "request_sent", case.name);
    let response = unique_data_memory_trace_event(&json, "response_arrived", case.name);
    assert_eq!(
        request.pointer("/tick").and_then(Value::as_u64),
        Some(case.request_tick),
        "{} should issue the atomic request before reset: {request}",
        case.name
    );
    assert_eq!(
        response.pointer("/tick").and_then(Value::as_u64),
        Some(case.response_tick),
        "{} should receive the atomic response after reset: {response}",
        case.name
    );
    assert_eq!(
        response
            .pointer("/response_latency_ticks")
            .and_then(Value::as_u64),
        Some(case.response_latency_ticks),
        "{} should retain transport-derived response latency: {response}",
        case.name
    );
    assert!(
        case.request_tick < case.reset_tick && case.reset_tick < case.response_tick,
        "{} fixture must straddle the reset boundary",
        case.name
    );
    assert_eq!(
        request.pointer("/request").and_then(Value::as_u64),
        response.pointer("/request").and_then(Value::as_u64),
        "{} request and response must describe the same transaction",
        case.name
    );

    for path in [
        format!(
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.{}",
            case.operation.snake_case()
        ),
        format!("system.cpu.lsq0.operation.{}", case.operation.camel_case()),
    ] {
        assert_stats_dump_sample(dump, &path, "counter", "Count", 1, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_samples".to_owned(),
        format!(
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.{}_latency_samples",
            case.operation.snake_case()
        ),
        "system.cpu.lsq0.dataResponse.samples".to_owned(),
        format!(
            "system.cpu.lsq0.dataResponse.{}.samples",
            case.operation.camel_case()
        ),
    ] {
        assert_stats_dump_sample(dump, &path, "counter", "Count", 1, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_ticks".to_owned(),
        format!(
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.{}_latency_ticks",
            case.operation.snake_case()
        ),
        "system.cpu.lsq0.dataResponse.totalLatency".to_owned(),
        format!(
            "system.cpu.lsq0.dataResponse.{}.totalLatency",
            case.operation.camel_case()
        ),
    ] {
        assert_stats_dump_sample(
            dump,
            &path,
            "counter",
            "Tick",
            case.response_latency_ticks,
            "resettable",
        );
    }

    for pointer in [
        "/cores/0/o3_runtime/lsq/data_latency/samples".to_owned(),
        format!(
            "/cores/0/o3_runtime/lsq/operation/{}/latency/samples",
            case.operation.snake_case()
        ),
        "/debug/o3_trace/0/lsq/data_latency/samples".to_owned(),
        format!(
            "/debug/o3_trace/0/lsq/operation/{}/latency/samples",
            case.operation.snake_case()
        ),
    ] {
        assert_eq!(
            json.pointer(&pointer).and_then(Value::as_u64),
            Some(1),
            "{} should expose one post-reset response sample at {pointer}: {json}",
            case.name
        );
    }
    for pointer in [
        format!(
            "/cores/0/o3_runtime/lsq/operation/{}/count",
            case.operation.snake_case()
        ),
        format!(
            "/debug/o3_trace/0/lsq/operation/{}/count",
            case.operation.snake_case()
        ),
    ] {
        assert_eq!(
            json.pointer(&pointer).and_then(Value::as_u64),
            Some(1),
            "{} should count the operation that retires after reset at {pointer}: {json}",
            case.name
        );
    }
    let raw_atomic_events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|event| {
            event.pointer("/lsq_operation").and_then(Value::as_str)
                == Some(case.operation.snake_case())
        })
        .count();
    assert_eq!(
        raw_atomic_events, 1,
        "{} should expose one post-reset terminal result event: {json}",
        case.name
    );
}

fn unique_data_memory_trace_event<'a>(json: &'a Value, kind: &str, case_name: &str) -> &'a Value {
    let events = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{case_name} missing memory trace: {json}"))
        .iter()
        .filter(|event| {
            event.pointer("/channel").and_then(Value::as_str) == Some("data")
                && event.pointer("/kind").and_then(Value::as_str) == Some(kind)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        events.len(),
        1,
        "{case_name} should expose one data {kind} event: {events:?}"
    );
    events[0]
}

fn cross_reset_atomic_response_binary(
    name: &str,
    operation: CrossResetOperation,
) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(35, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_RESET_STATS),
    ]);
    match operation {
        CrossResetOperation::LoadReserved => {
            words.push(atomic_type(0x02, false, false, 0, 5, 0x3, 7));
        }
        CrossResetOperation::AtomicSwap => {
            words.push(atomic_type(0x01, false, false, 0, 5, 0x3, 7));
        }
    }
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
