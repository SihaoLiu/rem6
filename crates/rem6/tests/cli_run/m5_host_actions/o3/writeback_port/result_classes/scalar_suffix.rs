use super::super::result_support::{
    assert_event_order, assert_register, assert_register_absent, assert_resource_counter,
    data_trace, event_str, json_u64, memory_dump_hex, memory_result_event_at_pc,
    result_memory_trace,
};
use super::super::*;
use super::{unique_result_temp_binary, MemoryResultClass, RESULT_MAX_TICK};

const RESULT_PC: &str = "0x80000030";
const DIV_PC: &str = "0x80000034";
const DIV_DEPENDENT_PC: &str = "0x80000038";
const TERMINAL_PC: &str = "0x8000003c";
const DATA_START: u64 = 0x8000_0100;
const DIRECT_CLASSES: [MemoryResultClass; 3] = [
    MemoryResultClass::FloatLoad,
    MemoryResultClass::Atomic,
    MemoryResultClass::Vector,
];

#[test]
fn rem6_run_o3_memory_result_scalar_suffix_matrix_direct() {
    run_suffix_matrix(&DIRECT_CLASSES, "direct", 1);
}

#[test]
fn rem6_run_o3_memory_result_scalar_suffix_matrix_cache_fabric_dram() {
    run_suffix_matrix(&DIRECT_CLASSES, "cache-fabric-dram", 1);
}

#[test]
fn rem6_run_o3_memory_result_scalar_suffix_width_two_exact_fit_direct() {
    run_suffix_matrix(&DIRECT_CLASSES, "direct", 2);
}

#[test]
fn rem6_run_o3_memory_result_scalar_suffix_readfile_mmio() {
    run_suffix_matrix(&[MemoryResultClass::Mmio], "direct", 1);
}

#[test]
fn rem6_run_timing_suppresses_o3_memory_result_scalar_suffix() {
    let fixture = ResultSuffixFixture::new(MemoryResultClass::Atomic);
    let route_delay = collision_delay("direct");
    let detailed = fixture.run("direct", 1, route_delay, RESULT_MAX_TICK, "detailed");
    let timing = fixture.run("direct", 1, route_delay, RESULT_MAX_TICK, "timing");

    assert_final_witness(&fixture, &detailed);
    assert_final_witness(&fixture, &timing);
    assert_eq!(
        timing.pointer("/cores/0/registers"),
        detailed.pointer("/cores/0/registers")
    );
    assert_eq!(timing.pointer("/memory"), detailed.pointer("/memory"));
    assert!(timing
        .pointer("/cores/0/o3_runtime/writeback_port")
        .is_none());
    assert!(timing
        .pointer("/cores/0/o3_runtime/snapshot/rob/count")
        .is_none());
    assert!(detailed
        .pointer("/cores/0/o3_runtime/writeback_port")
        .is_some());
    memory_result_event_at_pc(&detailed, RESULT_PC);
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    for (field, _) in WRITEBACK_PORT_STATS {
        assert_json_stat_absent(&timing, &format!("sim.cpu0.o3.writeback_port.{field}"));
    }
    for stat in ["system.cpu.iew.wbRate", "system.cpu.iew.wbFanout"] {
        assert_json_stat_absent(&timing, stat);
    }
}

fn run_suffix_matrix(classes: &[MemoryResultClass], memory_system: &str, width: usize) {
    for &class in classes {
        let fixture = ResultSuffixFixture::new(class);
        let route_delay = collision_delay(memory_system);
        let completed = fixture.run(
            memory_system,
            width,
            route_delay,
            RESULT_MAX_TICK,
            "detailed",
        );
        assert_completed_window(&fixture, &completed, width);
        assert_pre_admission(&fixture, memory_system, width, route_delay, &completed);
    }
}

struct ResultSuffixFixture {
    class: MemoryResultClass,
    binary: std::path::PathBuf,
    readfile: Option<std::path::PathBuf>,
}

impl ResultSuffixFixture {
    fn new(class: MemoryResultClass) -> Self {
        let binary = suffix_binary(class);
        let readfile = (class == MemoryResultClass::Mmio).then(|| {
            unique_result_temp_binary(
                "o3-memory-result-scalar-suffix-mmio-data",
                &0x0123_4567_89ab_cdef_u64.to_le_bytes(),
            )
        });
        Self {
            class,
            binary,
            readfile,
        }
    }

    fn run(
        &self,
        memory_system: &str,
        width: usize,
        route_delay: u64,
        max_tick: u64,
        switch_mode: &str,
    ) -> Value {
        let config = WritebackRunConfig::detailed_json(memory_system, width, route_delay, max_tick)
            .with_switch_mode(switch_mode);
        let mut command = writeback_command(&self.binary, config);
        command.args([
            "--riscv-o3-scalar-memory-depth",
            "4",
            "--host-event-delay",
            "1",
        ]);
        if let Some((address, bytes)) = dump_range(self.class) {
            command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
        }
        if let Some(readfile) = &self.readfile {
            command.args([
                "--readfile",
                &format!("0x10000000:0x100:{}", readfile.display()),
            ]);
        }
        let child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output =
            crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30));
        assert!(
            output.status.success(),
            "{} {memory_system} width {width} delay {route_delay} stderr: {}",
            self.class.label(),
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!("{} suffix stdout is not JSON: {error}", self.class.label())
        });
        let expected_status = if max_tick == RESULT_MAX_TICK {
            "stopped_by_host"
        } else {
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
            "stopped_at_tick_limit"
        };
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some(expected_status)
        );
        json
    }
}

fn collision_delay(memory_system: &str) -> u64 {
    match memory_system {
        "direct" => 9,
        "cache-fabric-dram" => 4,
        _ => panic!("unsupported scalar-suffix memory system {memory_system}"),
    }
}

fn assert_completed_window(fixture: &ResultSuffixFixture, json: &Value, width: usize) {
    let result = memory_result_event_at_pc(json, RESULT_PC);
    let div = event_at_pc(json, DIV_PC);
    let div_dependent = event_at_pc(json, DIV_DEPENDENT_PC);
    let terminal = event_at_pc(json, TERMINAL_PC);
    let result_raw_ready = event_u64(result, "lsq_data_response_tick") + 1;
    let div_raw_ready = event_u64(div, "issue_tick") + 19;
    assert_eq!(result_raw_ready, div_raw_ready);
    assert_eq!(event_str(div, "fu_latency_class"), "scalar_integer_div");
    assert_eq!(event_u64(result, "writeback_tick"), result_raw_ready);
    assert_eq!(
        event_u64(div, "writeback_tick"),
        div_raw_ready + u64::from(width == 1)
    );
    assert!(event_u64(div, "issue_tick") < event_u64(result, "lsq_data_response_tick"));
    assert!(event_u64(div_dependent, "issue_tick") >= event_u64(div, "writeback_tick"));
    if matches!(
        fixture.class,
        MemoryResultClass::Atomic | MemoryResultClass::Mmio
    ) {
        assert!(event_u64(terminal, "issue_tick") >= event_u64(result, "writeback_tick"));
    } else {
        assert!(event_u64(terminal, "issue_tick") >= event_u64(div_dependent, "writeback_tick"));
    }
    assert_event_order([result, div, div_dependent], "sequence", true);
    for pair in [result, div, div_dependent, terminal].windows(2) {
        assert!(event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick"));
    }
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_final_witness(fixture, json);
}

fn assert_pre_admission(
    fixture: &ResultSuffixFixture,
    memory_system: &str,
    width: usize,
    route_delay: u64,
    completed: &Value,
) {
    let result = memory_result_event_at_pc(completed, RESULT_PC);
    let response_tick = event_u64(result, "lsq_data_response_tick");
    let before = fixture.run(
        memory_system,
        width,
        route_delay,
        response_tick.saturating_sub(1),
        "detailed",
    );
    assert_eq!(
        json_u64(&before, "/cores/0/o3_runtime/snapshot/rob/count"),
        4
    );
    let expected_lsq = 1 + u64::from(fixture.class == MemoryResultClass::Atomic);
    assert_eq!(
        json_u64(&before, "/cores/0/o3_runtime/snapshot/lsq/count"),
        expected_lsq
    );
    let rob = before
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .expect("pre-admission ROB entries");
    let expected = [
        memory_result_event_at_pc(completed, RESULT_PC),
        event_at_pc(completed, DIV_PC),
        event_at_pc(completed, DIV_DEPENDENT_PC),
        event_at_pc(completed, TERMINAL_PC),
    ];
    assert_eq!(rob.len(), expected.len());
    assert_eq!(rob[0].get("ready").and_then(Value::as_bool), Some(false));
    for (entry, event) in rob.iter().zip(expected) {
        assert_eq!(event_u64(entry, "sequence"), event_u64(event, "sequence"));
        assert_eq!(event_str(entry, "pc"), event_str(event, "pc"));
    }
    assert_register_absent(&before, "x3");
    assert_register_absent(&before, "x4");
    assert_register_absent(&before, terminal_register(fixture.class));
    match fixture.class {
        MemoryResultClass::Atomic => assert_register_absent(&before, "x11"),
        MemoryResultClass::Mmio => assert_register_absent(&before, "x12"),
        _ => {}
    }
    assert_pre_admission_memory(fixture.class, &before);
    let routed = fixture.run(memory_system, width, route_delay, response_tick, "detailed");
    assert_result_route(fixture, memory_system, &routed, completed);
}

fn assert_result_route(f: &ResultSuffixFixture, system: &str, json: &Value, done: &Value) {
    assert_eq!(data_trace(json).len(), 1);
    let expected_address = match f.class {
        MemoryResultClass::Vector => "0x80000108",
        MemoryResultClass::Mmio => "0x10000000",
        _ => "0x80000100",
    };
    let expected_kind = if f.class == MemoryResultClass::Atomic {
        "atomic"
    } else {
        "load"
    };
    let data = data_trace(json).iter().find(|record| {
        event_str(record, "kind") == expected_kind
            && event_str(record, "address") == expected_address
            && event_u64(record, "size") == 8
    });
    let data = data.unwrap_or_else(|| panic!("result request missing: {json}"));
    match f.class {
        MemoryResultClass::Mmio => {
            assert_eq!(json_u64(json, "/readfiles/0/bytes"), 8);
            assert!(json
                .pointer("/debug/memory_trace")
                .and_then(Value::as_array)
                .is_some_and(|records| records.iter().all(|record| {
                    record.pointer("/channel").and_then(Value::as_str) != Some("data")
                })));
            for suffix in [
                "cache.data.activity",
                "transport.data.activity",
                "fabric.activity",
                "dram.activity",
            ] {
                assert_resource_counter(json, suffix, 0);
            }
        }
        _ if system == "cache-fabric-dram" => {
            let (sent, arrived, response) = result_memory_trace(json);
            let result = memory_result_event_at_pc(done, RESULT_PC);
            assert_eq!(event_u64(sent, "tick"), event_u64(result, "issue_tick"));
            assert_eq!(event_u64(response, "tick"), event_u64(data, "tick"));
            assert_eq!(
                event_u64(response, "tick"),
                event_u64(result, "lsq_data_response_tick")
            );
            assert_resource_counter(json, "cache.data.activity", 3);
            assert_resource_counter(json, "transport.data.activity", 1);
            assert!(json_u64(json, "/memory_resources/fabric/activity") > 0);
            assert!(json_u64(json, "/memory_resources/dram/activity") > 0);
            let packet = (event_u64(sent, "route") << 48) | event_u64(sent, "request");
            let hop = json
                .pointer("/memory_resources/fabric/hop_activities")
                .and_then(Value::as_array)
                .and_then(|hops| hops.iter().find(|hop| event_u64(hop, "packet") == packet))
                .unwrap_or_else(|| panic!("result fabric packet {packet} missing: {json}"));
            assert_eq!(event_u64(hop, "ready_tick"), event_u64(sent, "tick"));
            assert_eq!(event_u64(hop, "arrival_tick"), event_u64(arrived, "tick"));
            assert_eq!(event_u64(hop, "bytes"), 8);
        }
        _ => {
            let (sent, _arrived, response) = result_memory_trace(json);
            let result = memory_result_event_at_pc(done, RESULT_PC);
            assert_eq!(event_u64(sent, "tick"), event_u64(result, "issue_tick"));
            assert_eq!(event_u64(response, "tick"), event_u64(data, "tick"));
            assert_eq!(
                event_u64(response, "tick"),
                event_u64(result, "lsq_data_response_tick")
            );
            assert_resource_counter(json, "transport.data.activity", 1);
            for suffix in ["cache.data.activity", "fabric.activity", "dram.activity"] {
                assert_resource_counter(json, suffix, 0);
            }
        }
    }
}

fn assert_pre_admission_memory(class: MemoryResultClass, json: &Value) {
    match class {
        MemoryResultClass::FloatLoad => assert_eq!(
            memory_dump_hex(json, DATA_START),
            Some("182d4454fb2109400000000000000000")
        ),
        MemoryResultClass::Atomic => assert_eq!(
            memory_dump_hex(json, DATA_START),
            Some("09000000000000000000000000000000")
        ),
        MemoryResultClass::Vector => assert_eq!(
            memory_dump_hex(json, DATA_START + 16),
            Some("11000000000000001100000000000000")
        ),
        MemoryResultClass::Mmio => {}
        MemoryResultClass::LoadReserved => unreachable!(),
    }
}

fn assert_final_witness(fixture: &ResultSuffixFixture, json: &Value) {
    assert_register(json, "x3", "0x2a");
    assert_register(json, "x4", "0x2b");
    match fixture.class {
        MemoryResultClass::FloatLoad => {
            assert_register(json, "x8", "0x2c");
            assert_eq!(
                memory_dump_hex(json, DATA_START),
                Some("182d4454fb210940182d4454fb210940")
            );
        }
        MemoryResultClass::Atomic => {
            assert_register(json, "x7", "0x9");
            assert_register(json, "x11", "0x2");
            assert_register(json, "x12", "0x3");
            assert_eq!(
                memory_dump_hex(json, DATA_START),
                Some("09000000000000000300000000000000")
            );
        }
        MemoryResultClass::Vector => {
            assert_register(json, "x8", "0x2c");
            assert_eq!(
                memory_dump_hex(json, DATA_START + 16),
                Some("11000000000000005555444433332222")
            );
        }
        MemoryResultClass::Mmio => {
            assert_register(json, "x12", "0x123456789abcdef");
            assert_register(json, "x13", "0x123456789abcdf0");
        }
        MemoryResultClass::LoadReserved => unreachable!(),
    }
}

fn terminal_register(class: MemoryResultClass) -> &'static str {
    match class {
        MemoryResultClass::Atomic => "x12",
        MemoryResultClass::Mmio => "x13",
        MemoryResultClass::FloatLoad | MemoryResultClass::Vector => "x8",
        MemoryResultClass::LoadReserved => unreachable!(),
    }
}

fn dump_range(class: MemoryResultClass) -> Option<(u64, u64)> {
    match class {
        MemoryResultClass::FloatLoad | MemoryResultClass::Atomic => Some((DATA_START, 16)),
        MemoryResultClass::Vector => Some((DATA_START + 16, 16)),
        MemoryResultClass::Mmio => None,
        MemoryResultClass::LoadReserved => unreachable!(),
    }
}

fn suffix_binary(class: MemoryResultClass) -> std::path::PathBuf {
    let mut words = match class {
        MemoryResultClass::Mmio => vec![u_type(0x1000_0000, 5, 0x37)],
        _ => vec![
            u_type(0, 5, 0x17),
            i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 5, 0, 5, 0x13),
        ],
    };
    words.extend([i_type(84, 0, 0, 1, 0x13), i_type(2, 0, 0, 2, 0x13)]);
    match class {
        MemoryResultClass::Atomic => words.push(i_type(9, 0, 0, 7, 0x13)),
        MemoryResultClass::Vector => words.extend([
            i_type(16, 5, 0, 16, 0x13),
            i_type(2, 0, 0, 6, 0x13),
            i_type(17, 0, 0, 7, 0x13),
            i_type(2, 0, 0, 11, 0x13),
            vsetvli_type(0xd8, 11, 0),
            vector_arith_type(0b010111, 0b100, 0, 6, 0),
            vector_arith_type(0b010111, 0b100, 0, 7, 1),
        ]),
        _ => {}
    }
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    match class {
        MemoryResultClass::FloatLoad => words.push(i_type(0, 5, 0b011, 1, 0x07)),
        MemoryResultClass::Atomic => words.push(atomic_type(0x01, false, false, 7, 5, 0b011, 11)),
        MemoryResultClass::Vector => words.push(vector_unit_stride_load_type(false, 0b111, 5, 1)),
        MemoryResultClass::Mmio => words.push(i_type(0, 5, 0b011, 12, 0x03)),
        MemoryResultClass::LoadReserved => unreachable!(),
    }
    words.extend([r_type(0x01, 2, 1, 0b100, 3, 0x33), i_type(1, 3, 0, 4, 0x13)]);
    words.push(match class {
        MemoryResultClass::Atomic => i_type(1, 11, 0, 12, 0x13),
        MemoryResultClass::Mmio => i_type(1, 12, 0, 13, 0x13),
        MemoryResultClass::FloatLoad | MemoryResultClass::Vector => i_type(1, 4, 0, 8, 0x13),
        MemoryResultClass::LoadReserved => unreachable!(),
    });
    match class {
        MemoryResultClass::FloatLoad => {
            words.extend([float_store_type(8, 1, 5, 0b011), s_type(16, 8, 5, 0b011)])
        }
        MemoryResultClass::Atomic => {
            words.extend([s_type(8, 12, 5, 0b011), s_type(16, 4, 5, 0b011)])
        }
        MemoryResultClass::Vector => words.extend([
            vector_unit_stride_store_type(true, 0b111, 16, 1),
            s_type(32, 8, 5, 0b011),
        ]),
        MemoryResultClass::Mmio => {}
        MemoryResultClass::LoadReserved => unreachable!(),
    }
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    match class {
        MemoryResultClass::FloatLoad => {
            program.extend_from_slice(&3.141592653589793_f64.to_bits().to_le_bytes());
            program.extend_from_slice(&[0; 16]);
        }
        MemoryResultClass::Atomic => {
            program.extend_from_slice(&2_u64.to_le_bytes());
            program.extend_from_slice(&[0; 16]);
        }
        MemoryResultClass::Vector => {
            for value in [0xaaaa_aaaa_aaaa_aaaa_u64, 0x2222_3333_4444_5555, 17, 17, 0] {
                program.extend_from_slice(&value.to_le_bytes());
            }
        }
        MemoryResultClass::Mmio => {}
        MemoryResultClass::LoadReserved => unreachable!(),
    }
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    unique_result_temp_binary(
        &format!("o3-memory-result-scalar-suffix-{}", class.label()),
        &elf,
    )
}
