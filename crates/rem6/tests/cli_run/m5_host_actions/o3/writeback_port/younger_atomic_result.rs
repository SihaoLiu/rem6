use super::result_classes::unique_result_temp_binary;
use super::result_support::{
    assert_register, assert_register_absent, assert_resource_counter, data_trace, json_u64,
    memory_dump_hex, memory_result_event_at_pc,
};
use super::*;

#[path = "younger_atomic_result/boundaries.rs"]
mod boundaries;

const HEAD_PC: &str = "0x80000030";
const ATOMIC_PC: &str = "0x80000034";
const DIV_PC: &str = "0x80000038";
const DEPENDENT_PC: &str = "0x8000003c";
const DATA_START: u64 = 0x8000_0100;
const MAX_TICK: u64 = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum YoungerAtomicClass {
    ScalarLoad,
    FloatLoad,
    MaskedVectorLoad,
}

const YOUNGER_ATOMIC_CLASSES: [YoungerAtomicClass; 3] = [
    YoungerAtomicClass::ScalarLoad,
    YoungerAtomicClass::FloatLoad,
    YoungerAtomicClass::MaskedVectorLoad,
];

#[test]
fn rem6_run_o3_younger_atomic_result_matrix_direct() {
    run_matrix("direct", 40);
}

#[test]
fn rem6_run_o3_younger_atomic_result_matrix_cache_fabric_dram() {
    run_matrix("cache-fabric-dram", 4);
}

#[test]
fn rem6_run_timing_suppresses_o3_younger_atomic_result() {
    for class in YOUNGER_ATOMIC_CLASSES {
        let fixture = YoungerAtomicFixture::new(class);
        let detailed = fixture.run("direct", 40, MAX_TICK, "detailed");
        let timing = fixture.run("direct", 40, MAX_TICK, "timing");
        assert_eq!(
            timing.pointer("/cores/0/registers"),
            detailed.pointer("/cores/0/registers"),
            "{class:?} timing registers"
        );
        assert_eq!(
            timing.pointer("/memory"),
            detailed.pointer("/memory"),
            "{class:?} timing memory"
        );
        assert!(timing.pointer("/cores/0/o3_runtime").is_none());
        assert!(timing
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
    }
}

fn run_matrix(memory_system: &str, route_delay: u64) {
    for class in YOUNGER_ATOMIC_CLASSES {
        let fixture = YoungerAtomicFixture::new(class);
        let completed = fixture.run(memory_system, route_delay, MAX_TICK, "detailed");
        assert_completed(&fixture, memory_system, &completed);
        assert_pre_response(&fixture, memory_system, route_delay, &completed);
    }
}

struct YoungerAtomicFixture {
    class: YoungerAtomicClass,
    binary: std::path::PathBuf,
    expected_memory: String,
}

impl YoungerAtomicFixture {
    fn new(class: YoungerAtomicClass) -> Self {
        let initial = initial_memory(class);
        let expected_memory = expected_memory(class, &initial);
        Self {
            class,
            binary: younger_atomic_binary(class, &initial),
            expected_memory,
        }
    }

    fn run(
        &self,
        memory_system: &str,
        route_delay: u64,
        max_tick: u64,
        switch_mode: &str,
    ) -> Value {
        let mut command = self.command(memory_system, route_delay, max_tick, switch_mode);
        let child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output =
            crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30));
        assert!(
            output.status.success(),
            "{:?} younger atomic {memory_system} stderr: {}",
            self.class,
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        if max_tick == MAX_TICK {
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

    fn command(
        &self,
        memory_system: &str,
        route_delay: u64,
        max_tick: u64,
        switch_mode: &str,
    ) -> std::process::Command {
        let config = WritebackRunConfig::detailed_json(memory_system, 1, route_delay, max_tick)
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
        command
    }
}

fn assert_completed(fixture: &YoungerAtomicFixture, memory_system: &str, json: &Value) {
    let head = memory_result_event_at_pc(json, HEAD_PC);
    let atomic = memory_result_event_at_pc(json, ATOMIC_PC);
    let div = event_at_pc(json, DIV_PC);
    let dependent = event_at_pc(json, DEPENDENT_PC);
    let head_response = event_u64(head, "lsq_data_response_tick");
    assert!(event_u64(atomic, "issue_tick") < head_response);
    assert!(event_u64(div, "issue_tick") < head_response);
    assert!(event_u64(head, "writeback_tick") <= event_u64(atomic, "writeback_tick"));
    assert!(event_u64(dependent, "issue_tick") >= event_u64(atomic, "writeback_tick"));
    assert!(event_u64(head, "commit_tick") <= event_u64(atomic, "commit_tick"));
    assert!(event_u64(atomic, "commit_tick") <= event_u64(div, "commit_tick"));
    assert!(event_u64(div, "commit_tick") <= event_u64(dependent, "commit_tick"));

    let sent = data_requests_sent(json);
    assert_eq!(sent.len(), 5, "exact head/atomic/witness requests");
    assert!(event_u64(sent[1], "tick") >= head_response);
    assert_register(json, "x11", "0x9");
    assert_register(json, "x12", "0xa");
    assert_register(json, "x3", "0x2a");
    if fixture.class == YoungerAtomicClass::ScalarLoad {
        assert_register(json, "x13", "0x22");
    }
    assert_eq!(
        combined_memory_dump_hex(json),
        Some(fixture.expected_memory.clone())
    );
    assert_eq!(data_trace(json).len(), 5);
    assert_resource_counter(json, "transport.data.activity", 5);
    if memory_system == "cache-fabric-dram" {
        for pointer in [
            "/memory_resources/cache/data/activity",
            "/memory_resources/fabric/activity",
            "/memory_resources/dram/activity",
        ] {
            assert!(json_u64(json, pointer) > 0, "{pointer}");
        }
    } else {
        for suffix in ["cache.data.activity", "fabric.activity", "dram.activity"] {
            assert_resource_counter(json, suffix, 0);
        }
    }
}

fn assert_pre_response(
    fixture: &YoungerAtomicFixture,
    memory_system: &str,
    route_delay: u64,
    completed: &Value,
) {
    let head_response = event_u64(
        memory_result_event_at_pc(completed, HEAD_PC),
        "lsq_data_response_tick",
    );
    let before = fixture.run(
        memory_system,
        route_delay,
        head_response.saturating_sub(1),
        "detailed",
    );
    assert_eq!(
        json_u64(&before, "/cores/0/o3_runtime/snapshot/rob/count"),
        4
    );
    assert_eq!(
        json_u64(&before, "/cores/0/o3_runtime/snapshot/lsq/count"),
        3
    );
    for register in ["x3", "x11", "x12"] {
        assert_register_absent(&before, register);
    }
    if fixture.class == YoungerAtomicClass::ScalarLoad {
        assert_register_absent(&before, "x13");
    }
    assert!(data_trace(&before).is_empty());
    assert_eq!(data_requests_sent(&before).len(), 1);
    assert_resource_counter(&before, "transport.data.activity", 1);
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

fn data_requests_sent(json: &Value) -> Vec<&Value> {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("memory trace")
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
        })
        .collect()
}

fn combined_memory_dump_hex(json: &Value) -> Option<String> {
    [0, 16, 32, 48]
        .into_iter()
        .map(|offset| memory_dump_hex(json, DATA_START + offset))
        .collect::<Option<Vec<_>>>()
        .map(|chunks| chunks.concat())
}

fn younger_atomic_binary(class: YoungerAtomicClass, initial: &[u8]) -> std::path::PathBuf {
    let mut words = setup_words(class);
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend(window_words(class));
    words.extend(witness_words(class));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(initial);
    unique_result_temp_binary(
        &format!("o3-younger-atomic-result-{}", class.label()),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn setup_words(class: YoungerAtomicClass) -> Vec<u32> {
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 5, 0, 5, 0x13),
        i_type(84, 0, 0, 1, 0x13),
        i_type(2, 0, 0, 2, 0x13),
    ];
    match class {
        YoungerAtomicClass::ScalarLoad | YoungerAtomicClass::FloatLoad => {
            words.extend([i_type(16, 5, 0, 6, 0x13), i_type(3, 0, 0, 7, 0x13)]);
        }
        YoungerAtomicClass::MaskedVectorLoad => {
            words.extend([
                i_type(17, 0, 0, 7, 0x13),
                i_type(5, 0, 0, 8, 0x13),
                vsetvli_type(0xd8, 2, 0),
                vector_arith_type(0b010111, 0b100, 0, 2, 0),
                vector_arith_type(0b010111, 0b100, 0, 7, 1),
                i_type(16, 5, 0, 16, 0x13),
            ]);
        }
    }
    words
}

fn window_words(class: YoungerAtomicClass) -> [u32; 4] {
    let head = match class {
        YoungerAtomicClass::ScalarLoad => i_type(0, 5, 0b011, 13, 0x03),
        YoungerAtomicClass::FloatLoad => i_type(0, 5, 0b011, 1, 0x07),
        YoungerAtomicClass::MaskedVectorLoad => vector_unit_stride_load_type(false, 0b111, 5, 1),
    };
    let (operation, rs2, rs1) = match class {
        YoungerAtomicClass::ScalarLoad | YoungerAtomicClass::FloatLoad => (0x01, 7, 6),
        YoungerAtomicClass::MaskedVectorLoad => (0x00, 8, 16),
    };
    [
        head,
        atomic_type(operation, false, false, rs2, rs1, 11),
        r_type(0x01, 2, 1, 0b100, 3, 0x33),
        i_type(1, 11, 0, 12, 0x13),
    ]
}

fn witness_words(class: YoungerAtomicClass) -> Vec<u32> {
    match class {
        YoungerAtomicClass::ScalarLoad => vec![
            s_type(32, 13, 5, 0b011),
            s_type(48, 12, 5, 0b011),
            s_type(56, 3, 5, 0b011),
        ],
        YoungerAtomicClass::FloatLoad => vec![
            float_store_type(32, 1, 5, 0b011),
            s_type(48, 12, 5, 0b011),
            s_type(56, 3, 5, 0b011),
        ],
        YoungerAtomicClass::MaskedVectorLoad => vec![
            i_type(32, 5, 0, 17, 0x13),
            vector_unit_stride_store_type(true, 0b111, 17, 1),
            s_type(48, 12, 5, 0b011),
            s_type(56, 3, 5, 0b011),
        ],
    }
}

fn initial_memory(class: YoungerAtomicClass) -> Vec<u8> {
    let mut data = vec![0; 64];
    match class {
        YoungerAtomicClass::ScalarLoad => write_u64(&mut data, 0, 0x22),
        YoungerAtomicClass::FloatLoad => {
            write_u64(&mut data, 0, 3.141592653589793_f64.to_bits());
        }
        YoungerAtomicClass::MaskedVectorLoad => {
            write_u64(&mut data, 0, 0xaaaa_aaaa_aaaa_aaaa);
            write_u64(&mut data, 8, 0x2222_3333_4444_5555);
        }
    }
    write_u64(&mut data, 16, 9);
    data
}

fn expected_memory(class: YoungerAtomicClass, initial: &[u8]) -> String {
    let mut data = initial.to_vec();
    write_u64(
        &mut data,
        16,
        if class == YoungerAtomicClass::MaskedVectorLoad {
            14
        } else {
            3
        },
    );
    match class {
        YoungerAtomicClass::ScalarLoad => write_u64(&mut data, 32, 0x22),
        YoungerAtomicClass::FloatLoad => {
            write_u64(&mut data, 32, 3.141592653589793_f64.to_bits());
        }
        YoungerAtomicClass::MaskedVectorLoad => {
            write_u64(&mut data, 32, 17);
            write_u64(&mut data, 40, 0x2222_3333_4444_5555);
        }
    }
    write_u64(&mut data, 48, 10);
    write_u64(&mut data, 56, 42);
    hex(&data)
}

fn atomic_type(operation: u32, acquire: bool, release: bool, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (operation << 27)
        | (u32::from(acquire) << 26)
        | (u32::from(release) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b011 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

impl YoungerAtomicClass {
    fn label(self) -> &'static str {
        match self {
            Self::ScalarLoad => "scalar-load",
            Self::FloatLoad => "float-load",
            Self::MaskedVectorLoad => "masked-vector-load",
        }
    }
}

fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}
