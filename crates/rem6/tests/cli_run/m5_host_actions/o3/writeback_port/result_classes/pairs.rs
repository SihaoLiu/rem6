use super::super::result_support::{
    assert_event_order, assert_register, assert_register_absent, assert_resource_counter,
    data_trace, json_u64, memory_dump_hex, memory_result_event_at_pc,
};
use super::super::*;
use super::unique_result_temp_binary;

const FIRST_PC: &str = "0x80000030";
const SECOND_PC: &str = "0x80000034";
const DIV_PC: &str = "0x80000038";
const DEPENDENT_PC: &str = "0x8000003c";
const DATA_START: u64 = 0x8000_0100;
const PAIR_MAX_TICK: u64 = 5_000;
const PAIR_ROUTE_DELAY_CANDIDATES: [u64; 9] = [1, 2, 4, 6, 8, 9, 10, 12, 16];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultPairClass {
    AtomicFloat,
    VectorLoad,
    FloatVector,
}

const RESULT_PAIRS: [ResultPairClass; 3] = [
    ResultPairClass::AtomicFloat,
    ResultPairClass::VectorLoad,
    ResultPairClass::FloatVector,
];

#[test]
fn rem6_run_o3_memory_result_pair_matrix_direct() {
    run_pair_matrix("direct", 1, 40);
}

#[test]
fn rem6_run_o3_memory_result_pair_matrix_cache_fabric_dram() {
    run_pair_matrix("cache-fabric-dram", 1, 4);
}

#[test]
fn rem6_run_o3_memory_result_pair_width_two_exact_fit_direct() {
    let fixture = ResultPairFixture::new(ResultPairClass::AtomicFloat);
    let route_delay = calibrate_pair_collision(&fixture);
    let width_one = fixture.run("direct", 1, route_delay, PAIR_MAX_TICK, "detailed");
    let width_two = fixture.run("direct", 2, route_delay, PAIR_MAX_TICK, "detailed");
    assert_pair_collision(&width_one, 1);
    assert_pair_collision(&width_two, 2);
    assert_final_witness(&fixture, &width_one);
    assert_final_witness(&fixture, &width_two);
}

#[test]
fn rem6_run_o3_memory_result_pair_boundaries() {
    let fixture = ResultPairFixture::dependent_atomic_load();
    let json = fixture.run("direct", 1, 9, PAIR_MAX_TICK, "detailed");
    let first = memory_result_event_at_pc(&json, FIRST_PC);
    let second = memory_result_event_at_pc(&json, SECOND_PC);
    assert!(event_u64(second, "issue_tick") >= event_u64(first, "writeback_tick"));
    assert_register(&json, "x13", "0x33");
    assert_eq!(data_trace(&json).len(), 2);

    for (label, acquire, release) in [("acquire", true, false), ("release", false, true)] {
        let fixture = ResultPairFixture::ordered_atomic_float(label, acquire, release);
        let completed = fixture.run("direct", 1, 9, PAIR_MAX_TICK, "detailed");
        let atomic = memory_result_event_at_pc(&completed, FIRST_PC);
        let float = memory_result_event_at_pc(&completed, SECOND_PC);
        assert!(event_u64(float, "issue_tick") >= event_u64(atomic, "writeback_tick"));
        assert_register(&completed, "x11", "0x9");
        assert_register(&completed, "x12", "0xa");
        assert_eq!(data_trace(&completed).len(), 2);

        let before = fixture.run(
            "direct",
            1,
            9,
            event_u64(atomic, "lsq_data_response_tick").saturating_sub(1),
            "detailed",
        );
        assert_eq!(data_requests_sent(&before), 1, "{label} atomic head");
        assert_resource_counter(&before, "transport.data.activity", 1);
    }

    let fixture = ResultPairFixture::third_result();
    let json = fixture.run("direct", 1, 9, PAIR_MAX_TICK, "detailed");
    let first = memory_result_event_at_pc(&json, FIRST_PC);
    let second = memory_result_event_at_pc(&json, SECOND_PC);
    let third = memory_result_event_at_pc(&json, DIV_PC);
    let earliest_response =
        event_u64(first, "lsq_data_response_tick").min(event_u64(second, "lsq_data_response_tick"));
    assert!(event_u64(first, "issue_tick") < earliest_response);
    assert!(event_u64(second, "issue_tick") < earliest_response);
    assert!(event_u64(third, "issue_tick") >= event_u64(second, "writeback_tick"));
    assert_register(&json, "x13", "0x33");
    assert_register(&json, "x14", "0x34");
    assert_eq!(data_trace(&json).len(), 3);
}

#[test]
fn rem6_run_timing_suppresses_o3_memory_result_pairs() {
    let fixture = ResultPairFixture::new(ResultPairClass::VectorLoad);
    let detailed = fixture.run("direct", 1, 40, PAIR_MAX_TICK, "detailed");
    let timing = fixture.run("direct", 1, 40, PAIR_MAX_TICK, "timing");
    assert_final_witness(&fixture, &detailed);
    assert_final_witness(&fixture, &timing);
    assert_eq!(
        timing.pointer("/cores/0/registers"),
        detailed.pointer("/cores/0/registers")
    );
    assert_eq!(timing.pointer("/memory"), detailed.pointer("/memory"));
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
}

fn run_pair_matrix(memory_system: &str, width: usize, route_delay: u64) {
    for class in RESULT_PAIRS {
        let fixture = ResultPairFixture::new(class);
        let completed = fixture.run(memory_system, width, route_delay, PAIR_MAX_TICK, "detailed");
        assert_completed_pair(&fixture, memory_system, &completed);
        assert_pre_response_residency(&fixture, memory_system, width, route_delay, &completed);
    }
}

struct ResultPairFixture {
    class: ResultPairClass,
    binary: std::path::PathBuf,
    dependent_boundary: bool,
}

impl ResultPairFixture {
    fn new(class: ResultPairClass) -> Self {
        Self {
            class,
            binary: pair_binary(class),
            dependent_boundary: false,
        }
    }

    fn dependent_atomic_load() -> Self {
        Self {
            class: ResultPairClass::AtomicFloat,
            binary: dependent_atomic_load_binary(),
            dependent_boundary: true,
        }
    }

    fn ordered_atomic_float(label: &str, acquire: bool, release: bool) -> Self {
        Self {
            class: ResultPairClass::AtomicFloat,
            binary: ordered_atomic_float_binary(label, acquire, release),
            dependent_boundary: true,
        }
    }

    fn third_result() -> Self {
        Self {
            class: ResultPairClass::FloatVector,
            binary: third_result_binary(),
            dependent_boundary: true,
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
        for offset in [0, 16, 32, 48] {
            command.args(["--dump-memory", &format!("0x{:x}:16", DATA_START + offset)]);
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
            "{:?} {memory_system} width {width} delay {route_delay} stderr: {}",
            self.class,
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        if max_tick == PAIR_MAX_TICK {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host"),
                "{:?} did not reach host stop",
                self.class
            );
        } else {
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_at_tick_limit")
            );
        }
        json
    }
}

fn assert_completed_pair(fixture: &ResultPairFixture, system: &str, json: &Value) {
    let first = memory_result_event_at_pc(json, FIRST_PC);
    let second = memory_result_event_at_pc(json, SECOND_PC);
    let div = event_at_pc(json, DIV_PC);
    let dependent = event_at_pc(json, DEPENDENT_PC);
    let first_response = event_u64(first, "lsq_data_response_tick");
    let second_response = event_u64(second, "lsq_data_response_tick");
    let earliest_response = first_response.min(second_response);
    for (label, event) in [("first", first), ("second", second), ("div", div)] {
        assert!(
            event_u64(event, "issue_tick") < earliest_response,
            "{:?} {label} issue {} is not before earliest response {earliest_response}",
            fixture.class,
            event_u64(event, "issue_tick")
        );
    }
    let dependency = match fixture.class {
        ResultPairClass::AtomicFloat => first,
        ResultPairClass::VectorLoad => second,
        ResultPairClass::FloatVector => div,
    };
    assert!(event_u64(dependent, "issue_tick") >= event_u64(dependency, "writeback_tick"));
    assert_event_order([first, second, div], "sequence", true);
    assert!(event_u64(div, "sequence") < event_u64(dependent, "sequence"));
    for pair in [first, second, div, dependent].windows(2) {
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
    assert_eq!(data_trace(json).len(), 5);
    assert_resource_counter(json, "transport.data.activity", 5);
    if system == "cache-fabric-dram" {
        assert!(json_u64(json, "/memory_resources/cache/data/activity") > 0);
        assert!(json_u64(json, "/memory_resources/fabric/activity") > 0);
        assert!(json_u64(json, "/memory_resources/dram/activity") > 0);
    } else {
        for suffix in ["cache.data.activity", "fabric.activity", "dram.activity"] {
            assert_resource_counter(json, suffix, 0);
        }
    }
}

fn assert_pre_response_residency(
    fixture: &ResultPairFixture,
    memory_system: &str,
    width: usize,
    route_delay: u64,
    completed: &Value,
) {
    let first_response = event_u64(
        memory_result_event_at_pc(completed, FIRST_PC),
        "lsq_data_response_tick",
    );
    let second_response = event_u64(
        memory_result_event_at_pc(completed, SECOND_PC),
        "lsq_data_response_tick",
    );
    let before = fixture.run(
        memory_system,
        width,
        route_delay,
        first_response.min(second_response).saturating_sub(1),
        "detailed",
    );
    assert_eq!(
        json_u64(&before, "/cores/0/o3_runtime/snapshot/rob/count"),
        4
    );
    assert_eq!(
        json_u64(&before, "/cores/0/o3_runtime/snapshot/lsq/count"),
        if fixture.class == ResultPairClass::AtomicFloat {
            3
        } else {
            2
        }
    );
    assert_register_absent(&before, "x3");
    assert_register_absent(&before, dependent_register(fixture.class));
    assert!(data_trace(&before).is_empty());
    assert_eq!(data_requests_sent(&before), 2);
    assert_resource_counter(&before, "transport.data.activity", 2);
    if memory_system == "cache-fabric-dram" {
        for pointer in [
            "/memory_resources/cache/data/activity",
            "/memory_resources/fabric/activity",
            "/memory_resources/dram/activity",
        ] {
            assert!(json_u64(&before, pointer) > 0, "{pointer}");
        }
    }
}

fn calibrate_pair_collision(fixture: &ResultPairFixture) -> u64 {
    let matches = PAIR_ROUTE_DELAY_CANDIDATES
        .iter()
        .copied()
        .filter(|delay| {
            let json = fixture.run("direct", 2, *delay, PAIR_MAX_TICK, "detailed");
            let div = event_at_pc(&json, DIV_PC);
            let div_raw = event_u64(div, "issue_tick") + 19;
            [FIRST_PC, SECOND_PC].into_iter().all(|pc| {
                event_u64(
                    memory_result_event_at_pc(&json, pc),
                    "lsq_data_response_tick",
                ) + 1
                    == div_raw
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "pair collision must calibrate uniquely");
    matches[0]
}

fn assert_pair_collision(json: &Value, width: usize) {
    let div = event_at_pc(json, DIV_PC);
    let div_raw = event_u64(div, "issue_tick") + 19;
    let first = memory_result_event_at_pc(json, FIRST_PC);
    let second = memory_result_event_at_pc(json, SECOND_PC);
    assert_event_order([first, second, div], "sequence", true);
    for result in [first, second] {
        assert_eq!(event_u64(result, "lsq_data_response_tick") + 1, div_raw);
    }
    assert_eq!(event_u64(first, "writeback_tick"), div_raw);
    assert_eq!(
        event_u64(second, "writeback_tick"),
        div_raw + u64::from(width == 1)
    );
    assert_eq!(
        event_u64(div, "writeback_tick"),
        div_raw + if width == 1 { 2 } else { 1 }
    );
}

fn assert_final_witness(fixture: &ResultPairFixture, json: &Value) {
    if fixture.dependent_boundary {
        assert_register(json, "x13", "0x33");
        return;
    }
    assert_register(json, "x3", "0x2a");
    match fixture.class {
        ResultPairClass::AtomicFloat => {
            assert_register(json, "x11", "0x9");
            assert_register(json, "x12", "0xa");
        }
        ResultPairClass::VectorLoad => {
            assert_register(json, "x13", "0x33");
            assert_register(json, "x14", "0x34");
        }
        ResultPairClass::FloatVector => assert_register(json, "x4", "0x2b"),
    }
    assert_eq!(
        combined_memory_dump_hex(json),
        Some(expected_memory(fixture.class))
    );
}

fn combined_memory_dump_hex(json: &Value) -> Option<String> {
    [0, 16, 32, 48]
        .into_iter()
        .map(|offset| memory_dump_hex(json, DATA_START + offset))
        .collect::<Option<Vec<_>>>()
        .map(|chunks| chunks.concat())
}

fn data_requests_sent(json: &Value) -> usize {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("memory trace")
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
        })
        .count()
}

fn dependent_register(class: ResultPairClass) -> &'static str {
    match class {
        ResultPairClass::AtomicFloat => "x12",
        ResultPairClass::VectorLoad => "x14",
        ResultPairClass::FloatVector => "x4",
    }
}

fn pair_binary(class: ResultPairClass) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    words.extend(pair_setup(class));
    while words.len() < 12 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.extend(pair_window_words(class));
    words.extend(pair_witness_words(class));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&initial_memory(class));
    unique_result_temp_binary(
        &format!("o3-memory-result-pair-{}", pair_label(class)),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn pair_setup(class: ResultPairClass) -> Vec<u32> {
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(DATA_START as i32 - 0x8000_0004_u64 as i32, 5, 0, 5, 0x13),
        i_type(84, 0, 0, 1, 0x13),
        i_type(2, 0, 0, 2, 0x13),
    ];
    match class {
        ResultPairClass::AtomicFloat => words.push(i_type(3, 0, 0, 7, 0x13)),
        ResultPairClass::VectorLoad | ResultPairClass::FloatVector => {
            words.extend([
                i_type(2, 0, 0, 6, 0x13),
                i_type(17, 0, 0, 7, 0x13),
                i_type(2, 0, 0, 11, 0x13),
                vsetvli_type(0xd8, 11, 0),
                vector_arith_type(0b010111, 0b100, 0, 6, 0),
                vector_arith_type(0b010111, 0b100, 0, 7, 1),
            ]);
            if class == ResultPairClass::FloatVector {
                words.push(i_type(16, 5, 0, 16, 0x13));
            }
        }
    }
    words
}

fn pair_window_words(class: ResultPairClass) -> [u32; 4] {
    match class {
        ResultPairClass::AtomicFloat => [
            atomic_type(0x01, false, false, 7, 5, 0b011, 11),
            i_type(16, 5, 0b011, 1, 0x07),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            i_type(1, 11, 0, 12, 0x13),
        ],
        ResultPairClass::VectorLoad => [
            vector_unit_stride_load_type(false, 0b111, 5, 1),
            i_type(16, 5, 0b011, 13, 0x03),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            i_type(1, 13, 0, 14, 0x13),
        ],
        ResultPairClass::FloatVector => [
            i_type(0, 5, 0b011, 1, 0x07),
            vector_unit_stride_load_type(false, 0b111, 16, 1),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            i_type(1, 3, 0, 4, 0x13),
        ],
    }
}

fn pair_witness_words(class: ResultPairClass) -> Vec<u32> {
    match class {
        ResultPairClass::AtomicFloat => vec![
            float_store_type(24, 1, 5, 0b011),
            s_type(32, 12, 5, 0b011),
            s_type(40, 3, 5, 0b011),
        ],
        ResultPairClass::VectorLoad => vec![
            i_type(32, 5, 0, 16, 0x13),
            vector_unit_stride_store_type(true, 0b111, 16, 1),
            s_type(48, 14, 5, 0b011),
            s_type(56, 3, 5, 0b011),
        ],
        ResultPairClass::FloatVector => vec![
            float_store_type(24, 1, 5, 0b011),
            i_type(32, 5, 0, 17, 0x13),
            vector_unit_stride_store_type(true, 0b111, 17, 1),
            s_type(56, 4, 5, 0b011),
        ],
    }
}

fn initial_memory(class: ResultPairClass) -> Vec<u8> {
    let mut data = vec![0; 64];
    match class {
        ResultPairClass::AtomicFloat => {
            write_u64(&mut data, 0, 9);
            write_u64(&mut data, 16, 3.141592653589793_f64.to_bits());
        }
        ResultPairClass::VectorLoad => {
            write_u64(&mut data, 0, 0xaaaa_aaaa_aaaa_aaaa);
            write_u64(&mut data, 8, 0x2222_3333_4444_5555);
            write_u64(&mut data, 16, 0x33);
        }
        ResultPairClass::FloatVector => {
            write_u64(&mut data, 0, 3.141592653589793_f64.to_bits());
            write_u64(&mut data, 16, 0xaaaa_aaaa_aaaa_aaaa);
            write_u64(&mut data, 24, 0x5555_6666_7777_8888);
        }
    }
    data
}

fn expected_memory(class: ResultPairClass) -> String {
    let mut data = initial_memory(class);
    match class {
        ResultPairClass::AtomicFloat => {
            write_u64(&mut data, 0, 3);
            write_u64(&mut data, 24, 3.141592653589793_f64.to_bits());
            write_u64(&mut data, 32, 10);
            write_u64(&mut data, 40, 42);
        }
        ResultPairClass::VectorLoad => {
            write_u64(&mut data, 32, 17);
            write_u64(&mut data, 40, 0x2222_3333_4444_5555);
            write_u64(&mut data, 48, 0x34);
            write_u64(&mut data, 56, 42);
        }
        ResultPairClass::FloatVector => {
            write_u64(&mut data, 24, 3.141592653589793_f64.to_bits());
            write_u64(&mut data, 32, 17);
            write_u64(&mut data, 40, 0x5555_6666_7777_8888);
            write_u64(&mut data, 56, 43);
        }
    }
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn dependent_atomic_load_binary() -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 5, 0, 5, 0x13),
        i_type(3, 0, 0, 7, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.extend([
        m5op(M5_SWITCH_CPU),
        atomic_type(0x01, false, false, 7, 5, 0b011, 11),
        i_type(0, 11, 0b011, 13, 0x03),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut data = vec![0; 64];
    write_u64(&mut data, 0, DATA_START + 16);
    write_u64(&mut data, 16, 0x33);
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&data);
    unique_result_temp_binary(
        "o3-memory-result-pair-dependent-atomic-load",
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn ordered_atomic_float_binary(label: &str, acquire: bool, release: bool) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    words.extend(pair_setup(ResultPairClass::AtomicFloat));
    while words.len() < 12 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.extend([
        atomic_type(0x01, acquire, release, 7, 5, 0b011, 11),
        i_type(16, 5, 0b011, 1, 0x07),
        i_type(1, 11, 0, 12, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&initial_memory(ResultPairClass::AtomicFloat));
    unique_result_temp_binary(
        &format!("o3-memory-result-pair-{label}-atomic-float"),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn third_result_binary() -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(DATA_START as i32 - 0x8000_0004_u64 as i32, 5, 0, 5, 0x13),
    ];
    while words.len() < 12 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.extend([
        i_type(0, 5, 0b011, 1, 0x07),
        i_type(16, 5, 0b011, 13, 0x03),
        i_type(32, 5, 0b011, 2, 0x07),
        i_type(1, 13, 0, 14, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut data = vec![0; 64];
    write_u64(&mut data, 0, 3.141592653589793_f64.to_bits());
    write_u64(&mut data, 16, 0x33);
    write_u64(&mut data, 32, 2.718281828459045_f64.to_bits());
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&data);
    unique_result_temp_binary(
        "o3-memory-result-pair-third-result",
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn pair_label(class: ResultPairClass) -> &'static str {
    match class {
        ResultPairClass::AtomicFloat => "atomic-float",
        ResultPairClass::VectorLoad => "vector-load",
        ResultPairClass::FloatVector => "float-vector",
    }
}
