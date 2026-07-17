const RESULT_BOUNDARY_MAX_TICK: u64 = 1_200;
const ALL_INACTIVE_SOURCE: u64 = 0x8000_0080;
const ALL_INACTIVE_DESTINATION: u64 = 0x8000_0090;
const ALL_INACTIVE_PC: &str = "0x80000024";

#[derive(Clone, Copy)]
struct ResultBoundaryCase {
    label: &'static str,
    target_pc: &'static str,
    program: fn() -> Vec<u8>,
    dump_address: u64,
    dump_bytes: u64,
    expected_hex: &'static str,
    expected_register: Option<BoundaryRegisterExpectation>,
    trace: BoundaryTraceExpectation,
}

#[derive(Clone, Copy)]
enum BoundaryRegisterExpectation {
    Value(&'static str, &'static str),
    Absent(&'static str),
}

#[derive(Clone, Copy)]
enum BoundaryTraceExpectation {
    Completed(&'static str),
    Absent,
}

const RESULT_BOUNDARY_CASES: [ResultBoundaryCase; 6] = [
    ResultBoundaryCase {
        label: "integer-load-x0",
        target_pc: "0x8000000c",
        program: integer_load_x0_program,
        dump_address: 0x8000_0080,
        dump_bytes: 8,
        expected_hex: "8877665544332211",
        expected_register: None,
        trace: BoundaryTraceExpectation::Completed("load"),
    },
    ResultBoundaryCase {
        label: "load-reserved-x0",
        target_pc: "0x80000010",
        program: load_reserved_x0_program,
        dump_address: 0x8000_0080,
        dump_bytes: 8,
        expected_hex: "2a00000000000000",
        expected_register: Some(BoundaryRegisterExpectation::Absent("x7")),
        trace: BoundaryTraceExpectation::Completed("load_reserved"),
    },
    ResultBoundaryCase {
        label: "atomic-x0",
        target_pc: "0x80000010",
        program: atomic_x0_program,
        dump_address: 0x8000_0080,
        dump_bytes: 8,
        expected_hex: "0e00000000000000",
        expected_register: None,
        trace: BoundaryTraceExpectation::Completed("atomic"),
    },
    ResultBoundaryCase {
        label: "store-conditional",
        target_pc: "0x80000010",
        program: store_conditional_program,
        dump_address: 0x8000_0080,
        dump_bytes: 8,
        expected_hex: "0900000000000000",
        expected_register: Some(BoundaryRegisterExpectation::Value("x7", "0x1")),
        trace: BoundaryTraceExpectation::Completed("store_conditional"),
    },
    ResultBoundaryCase {
        label: "vector-lmul2",
        target_pc: "0x80000014",
        program: vector_lmul2_program,
        dump_address: 0x8000_00d0,
        dump_bytes: 16,
        expected_hex: "08070605040302011817161514131211",
        expected_register: None,
        trace: BoundaryTraceExpectation::Absent,
    },
    ResultBoundaryCase {
        label: "vector-fault-only-first",
        target_pc: "0x80000014",
        program: vector_fault_only_first_program,
        dump_address: 0x8000_00d0,
        dump_bytes: 16,
        expected_hex: "887766554433221100ffeeddccbbaa99",
        expected_register: None,
        trace: BoundaryTraceExpectation::Absent,
    },
];

#[test]
fn rem6_run_o3_memory_result_writeback_rejects_resultless_and_unsupported_shapes() {
    for case in RESULT_BOUNDARY_CASES {
        let path = result_boundary_binary(case.label, (case.program)());
        let json = result_boundary_json(
            &path,
            RESULT_BOUNDARY_MAX_TICK,
            Some((case.dump_address, case.dump_bytes)),
            case.label,
        );

        assert_boundary_stopped_by_host(&json, case.label);
        assert_eq!(
            memory_dump_hex(&json, case.dump_address),
            Some(case.expected_hex),
            "{} architectural memory behavior changed: {json}",
            case.label
        );
        if let Some(expectation) = case.expected_register {
            match expectation {
                BoundaryRegisterExpectation::Value(register, expected) => {
                    assert_register(&json, register, expected)
                }
                BoundaryRegisterExpectation::Absent(register) => {
                    assert_register_absent(&json, register)
                }
            }
        }
        assert_eq!(
            json.pointer("/cores/0/registers/x0"),
            None,
            "{} must not publish x0: {json}",
            case.label
        );
        assert_no_boundary_writeback(&json, case.label);
        assert_empty_boundary_o3_state(&json, case.label);

        match case.trace {
            BoundaryTraceExpectation::Completed(operation) => {
                let event = event_at_pc(&json, case.target_pc);
                assert_eq!(event_str(event, "lsq_operation"), operation);
                assert!(
                    event_u64(event, "issue_tick") <= event_u64(event, "lsq_data_response_tick"),
                    "{} response must not precede issue: {event}",
                    case.label
                );
                assert_eq!(
                    event_u64(event, "writeback_tick"),
                    event_u64(event, "lsq_data_response_tick"),
                    "{} must complete without claiming the memory-result terminal lane: {event}",
                    case.label
                );
            }
            BoundaryTraceExpectation::Absent => assert!(
                event_at_pc_if_present(&json, case.target_pc).is_none(),
                "{} must stay outside the terminal result lane: {json}",
                case.label
            ),
        }
    }
}

#[test]
fn rem6_run_o3_memory_result_writeback_all_inactive_vector_issues_no_request() {
    let path = result_boundary_binary("all-inactive-vector", all_inactive_vector_program());
    let json = result_boundary_json(
        &path,
        RESULT_BOUNDARY_MAX_TICK,
        Some((ALL_INACTIVE_DESTINATION, 16)),
        "all-inactive-vector",
    );

    assert_boundary_stopped_by_host(&json, "all-inactive-vector");
    assert_eq!(
        memory_dump_hex(&json, ALL_INACTIVE_DESTINATION),
        Some("11000000000000001100000000000000"),
        "all-inactive masked load must preserve destination vector bytes: {json}"
    );
    assert_no_data_or_memory_request(&json, ALL_INACTIVE_SOURCE, "all-inactive vector load");
    let event = event_at_pc(&json, ALL_INACTIVE_PC);
    assert_eq!(event_str(event, "lsq_operation"), "none");
    assert_eq!(event_u64(event, "lsq_loads"), 0);
    assert_eq!(event_u64(event, "lsq_stores"), 0);
    assert_eq!(event_u64(event, "lsq_data_response_tick"), 0);
    assert_empty_boundary_o3_state(&json, "all-inactive vector load");
    assert_no_boundary_writeback(&json, "all-inactive vector load");

    let issue_tick = event_u64(event, "issue_tick") + 1;
    let issue = result_boundary_json(&path, issue_tick, None, "all-inactive issue boundary");
    assert_boundary_tick_limit(&issue, issue_tick, "all-inactive issue boundary");
    assert_eq!(
        issue.pointer("/cores/0/pc").and_then(Value::as_str),
        Some(ALL_INACTIVE_PC),
        "all-inactive issue boundary must stop on the vector instruction: {issue}"
    );
    assert_empty_boundary_o3_state(&issue, "all-inactive issue boundary");
    assert_eq!(
        json_u64(&issue, "/cores/0/o3_runtime/writeback_calendar/count"),
        0,
        "all-inactive issue boundary must not reserve writeback: {issue}"
    );
    assert!(
        data_trace(&issue).is_empty(),
        "all-inactive issue boundary emitted CPU data events: {:?}",
        data_trace(&issue)
    );
    let memory = issue
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("all-inactive issue boundary Memory trace missing: {issue}"));
    assert!(
        memory.iter().all(|record| {
            record.pointer("/channel").and_then(Value::as_str) != Some("data")
                || record.pointer("/kind").and_then(Value::as_str) != Some("request_sent")
        }),
        "all-inactive issue boundary emitted a data-channel request: {memory:?}"
    );
}

#[test]
fn rem6_run_o3_memory_result_writeback_denied_amo_traps_before_transport() {
    let denied_path = result_boundary_binary("denied-amo-pmp", pmp_amo_program());
    for (label, control) in [
        ("detached", PmpDeniedAmoGdbControl::Detach),
        ("attached-continue", PmpDeniedAmoGdbControl::Continue),
    ] {
        let artifact = unique_result_boundary_output(&format!("denied-amo-{label}"));
        let output = pmp_denied_amo_output(
            &denied_path,
            RESULT_BOUNDARY_MAX_TICK,
            &[
                "--dump-memory",
                "0x800000c0:8",
                "--output",
                artifact.to_str().unwrap(),
            ],
            control,
            label,
        );
        assert_eq!(output.status.code(), Some(2), "{label} status: {output:?}");
        assert!(output.stdout.is_empty(), "{label} stdout: {output:?}");
        assert_denied_amo_failure_diagnostics(&String::from_utf8(output.stderr).unwrap());
        assert!(!artifact.exists(), "{label} emitted {}", artifact.display());
    }
}

#[test]
fn rem6_run_o3_memory_result_writeback_live_checkpoint_rejects() {
    assert_live_memory_result_action_rejects(LiveResultAction::Checkpoint);
}

#[test]
fn rem6_run_o3_memory_result_writeback_live_mode_switch_rejects() {
    assert_live_memory_result_action_rejects(LiveResultAction::ModeSwitch);
}

#[test]
fn rem6_run_timing_suppresses_o3_memory_result_writeback_surface() {
    let fixture = MemoryResultFixture::new(MemoryResultClass::Atomic);
    let detailed = run_result_fixture_mode(&fixture, "detailed");
    let timing = run_result_fixture_mode(&fixture, "timing");

    assert_boundary_stopped_by_host(&detailed, "detailed AMO result");
    assert_boundary_stopped_by_host(&timing, "timing AMO result");
    assert!(
        detailed
            .pointer("/cores/0/o3_runtime/writeback_port")
            .is_some(),
        "detailed AMO result must expose writeback-port evidence: {detailed}"
    );
    memory_result_event_at_pc(&detailed, fixture.class.pcs()[0]);
    for pointer in ["/cores/0/registers", "/memory"] {
        assert_eq!(
            timing.pointer(pointer),
            detailed.pointer(pointer),
            "timing architecture diverged at {pointer}"
        );
    }
    assert_eq!(
        memory_dump_hex(&timing, 0x8000_0080),
        Some("02000000000000000900000000000000")
    );
    assert!(timing
        .pointer("/cores/0/o3_runtime/writeback_port")
        .is_none());
    assert!(timing
        .pointer("/cores/0/o3_runtime/writeback_calendar/entries")
        .is_none());
    assert!(
        timing
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "timing mode must omit all detailed O3 trace result rows: {timing}"
    );
    for (field, _) in WRITEBACK_PORT_STATS {
        assert_json_stat_absent(&timing, &format!("sim.cpu0.o3.writeback_port.{field}"));
    }
}

#[derive(Clone, Copy)]
enum LiveResultAction {
    Checkpoint,
    ModeSwitch,
}

fn assert_live_memory_result_action_rejects(action: LiveResultAction) {
    let fixture = MemoryResultFixture::new(MemoryResultClass::FloatLoad);
    let baseline = fixture.run("direct", 1, 9, RESULT_MAX_TICK);
    let result = memory_result_event_at_pc(&baseline, fixture.class.pcs()[0]);
    let issue_tick = event_u64(result, "issue_tick");
    let action_tick = issue_tick + 1;
    assert!(
        action_tick < event_u64(result, "lsq_data_response_tick"),
        "live action must occur after FLD issue and before response: {result}"
    );

    let (flag, argument, label) = match action {
        LiveResultAction::Checkpoint => (
            "--host-checkpoint",
            format!("{action_tick}:memory-result-live"),
            "live FLD checkpoint",
        ),
        LiveResultAction::ModeSwitch => (
            "--host-switch-cpu-mode",
            format!("{action_tick}:cpu0:timing"),
            "live FLD mode switch",
        ),
    };
    let artifact = unique_result_boundary_output(label);
    let mut command = result_boundary_command(&fixture.binary, RESULT_MAX_TICK);
    command.args([
        flag,
        argument.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = wait_for_result_boundary(command, label);

    assert_eq!(output.status.code(), Some(2), "{label} status: {output:?}");
    assert!(output.stdout.is_empty(), "{label} stdout: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
        "{label} must fail with the exact cpu0 quiescence error"
    );
    assert!(
        !artifact.exists(),
        "{label} must not emit {}",
        artifact.display()
    );
}

fn run_result_fixture_mode(fixture: &MemoryResultFixture, mode: &str) -> Value {
    let config =
        WritebackRunConfig::direct_detailed_json(1, 9, RESULT_MAX_TICK).with_switch_mode(mode);
    let mut command = writeback_command(&fixture.binary, config);
    command.args(["--host-event-delay", "1"]);
    if let Some((address, bytes)) = fixture.class.dump_range() {
        command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
    }
    let output = wait_for_result_boundary(command, mode);
    assert!(
        output.status.success(),
        "{mode} result fixture stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{mode} result fixture invalid JSON: {error}"))
}

fn result_boundary_json(
    path: &std::path::Path,
    max_tick: u64,
    dump: Option<(u64, u64)>,
    label: &str,
) -> Value {
    let mut command = result_boundary_command(path, max_tick);
    if let Some((address, bytes)) = dump {
        command.args(["--dump-memory", &format!("0x{address:x}:{bytes}")]);
    }
    let output = wait_for_result_boundary(command, label);
    assert!(
        output.status.success(),
        "{label} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{label} invalid stdout JSON: {error}"))
}

fn result_boundary_command(path: &std::path::Path, max_tick: u64) -> Command {
    let mut command = writeback_command(
        path,
        WritebackRunConfig::direct_detailed_json(1, 9, max_tick),
    );
    command.args(["--host-event-delay", "1"]);
    command
}

fn wait_for_result_boundary(mut command: Command, label: &str) -> std::process::Output {
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to spawn {label}: {error}"));
    crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30))
}

fn assert_boundary_stopped_by_host(json: &Value, label: &str) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host"),
        "{label} termination: {json}"
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("host_stop"),
        "{label} stop reason: {json}"
    );
    assert_eq!(json_u64(json, "/simulation/stop_code"), 0, "{label}");
}

fn assert_boundary_tick_limit(json: &Value, tick: u64, label: &str) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit"),
        "{label} termination: {json}"
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("tick_limit"),
        "{label} stop reason: {json}"
    );
    assert_eq!(json_u64(json, "/simulation/final_tick"), tick, "{label}");
}

fn assert_no_boundary_writeback(json: &Value, label: &str) {
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/writeback_calendar/count")
            .and_then(Value::as_u64),
        Some(0),
        "{label} must not reserve memory-result writeback: {json}"
    );
    let writeback = writeback_port_artifact(json);
    for (field, _) in WRITEBACK_PORT_STATS {
        assert_eq!(
            writeback_port_u64(writeback, field),
            0,
            "{label} unexpectedly consumed writeback field {field}: {writeback}"
        );
    }
}

fn assert_empty_boundary_o3_state(json: &Value, label: &str) {
    for lane in ["rob", "lsq"] {
        assert_eq!(
            json.pointer(&format!("/cores/0/o3_runtime/snapshot/{lane}/count"))
                .and_then(Value::as_u64),
            Some(0),
            "{label} must not retain a live O3 {lane} row: {json}"
        );
    }
}

fn assert_no_data_or_memory_request(json: &Value, address: u64, label: &str) {
    let address = format!("0x{address:x}");
    assert!(
        data_trace(json)
            .iter()
            .all(|record| event_str(record, "address") != address),
        "{label} emitted a Data request at {address}: {:?}",
        data_trace(json)
    );
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{label} Memory trace missing: {json}"));
    assert!(
        memory.iter().all(|record| {
            record.pointer("/channel").and_then(Value::as_str) != Some("data")
                || record.pointer("/address").and_then(Value::as_str) != Some(address.as_str())
        }),
        "{label} emitted a Memory request at {address}: {memory:?}"
    );
}

fn result_boundary_binary(name: &str, program: Vec<u8>) -> std::path::PathBuf {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    unique_result_temp_binary(&format!("o3-memory-result-boundary-{name}"), &elf)
}

fn unique_result_boundary_output(name: &str) -> std::path::PathBuf {
    let id = RESULT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    temp_output(&format!("o3-memory-result-boundary-{name}-{id}"))
}

fn integer_load_x0_program() -> Vec<u8> {
    scalar_boundary_program([i_type(0, 5, 0b011, 0, 0x03)], &[0x1122_3344_5566_7788])
}

fn load_reserved_x0_program() -> Vec<u8> {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(42, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        atomic_type(0x02, false, false, 0, 5, 0x3, 0),
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),
    ];
    append_host_stop(&mut words);
    finish_result_boundary_program(words, data_start, &[9])
}

fn atomic_x0_program() -> Vec<u8> {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(5, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        atomic_type(0x00, false, false, 6, 5, 0x3, 0),
    ];
    append_host_stop(&mut words);
    finish_result_boundary_program(words, data_start, &[9])
}

fn store_conditional_program() -> Vec<u8> {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(42, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),
    ];
    append_host_stop(&mut words);
    finish_result_boundary_program(words, data_start, &[9])
}

fn scalar_boundary_program<const N: usize>(instructions: [u32; N], data: &[u64]) -> Vec<u8> {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        m5op(M5_SWITCH_CPU),
    ];
    words.extend(instructions);
    append_host_stop(&mut words);
    finish_result_boundary_program(words, data_start, data)
}

fn vector_lmul2_program() -> Vec<u8> {
    vector_boundary_program(
        0xd1,
        4,
        16,
        vector_unit_stride_load_type(true, 0b110, 5, 2),
        vector_unit_stride_store_type(true, 0b110, 16, 2),
        &[0x0102_0304_0506_0708, 0x1112_1314_1516_1718],
    )
}

fn vector_fault_only_first_program() -> Vec<u8> {
    vector_boundary_program(
        0xd0,
        4,
        16,
        vector_unit_stride_fault_only_load_type(true, 0b110, 5, 1),
        vector_unit_stride_store_type(true, 0b110, 16, 1),
        &[0x1122_3344_5566_7788, 0x99aa_bbcc_ddee_ff00],
    )
}

fn vector_boundary_program(
    vtype: u32,
    avl: i32,
    bytes: i32,
    load: u32,
    store: u32,
    data: &[u64],
) -> Vec<u8> {
    let data_start = 192_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(bytes, 5, 0x0, 16, 0x13),
        i_type(avl, 0, 0x0, 11, 0x13),
        vsetvli_type(vtype, 11, 0),
        m5op(M5_SWITCH_CPU),
        load,
        store,
    ];
    append_host_stop(&mut words);
    let mut program = finish_result_boundary_program(words, data_start, data);
    program.resize(program.len() + bytes as usize, 0);
    program
}

fn all_inactive_vector_program() -> Vec<u8> {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(16, 5, 0x0, 16, 0x13),
        i_type(2, 0, 0x0, 11, 0x13),
        i_type(17, 0, 0x0, 6, 0x13),
        vsetvli_type(0xd8, 11, 0),
        vector_arith_type(0b010111, 0b100, 0, 6, 1),
        vector_arith_type(0b010111, 0b100, 0, 0, 0),
        m5op(M5_SWITCH_CPU),
        vector_unit_stride_load_type(false, 0b111, 5, 1),
        vector_unit_stride_store_type(true, 0b111, 16, 1),
    ];
    append_host_stop(&mut words);
    let mut program = finish_result_boundary_program(
        words,
        data_start,
        &[0xaaaa_aaaa_aaaa_aaaa, 0xbbbb_bbbb_bbbb_bbbb],
    );
    program.resize(program.len() + 16, 0);
    program
}

fn pmp_amo_program() -> Vec<u8> {
    let data_start = 192_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(5, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        atomic_type(0x00, false, false, 6, 5, 0x3, 10),
    ];
    append_host_stop(&mut words);
    finish_result_boundary_program(words, data_start, &[9])
}

fn finish_result_boundary_program(mut words: Vec<u32>, data_start: i32, data: &[u64]) -> Vec<u8> {
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    for value in data {
        program.extend_from_slice(&value.to_le_bytes());
    }
    program
}

fn vector_unit_stride_fault_only_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (0b10000 << 20)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

include!("result_boundaries/support.rs");
