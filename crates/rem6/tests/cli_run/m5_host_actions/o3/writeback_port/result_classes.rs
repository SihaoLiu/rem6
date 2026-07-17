include!("result_classes/support.rs");

const RESULT_MAX_TICK: u64 = 2_000;
const ROUTE_DELAY_CANDIDATES: [u64; 12] = [1, 2, 4, 6, 8, 9, 10, 12, 14, 16, 20, 24];
const ORDINARY_RESOURCES: [&str; 4] = ["cache.data", "transport.data", "fabric", "dram"];
static RESULT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MemoryResultClass {
    FloatLoad,
    LoadReserved,
    Atomic,
    Vector,
    Mmio,
}

const DIRECT_RESULT_CLASSES: [MemoryResultClass; 4] = [
    MemoryResultClass::FloatLoad,
    MemoryResultClass::LoadReserved,
    MemoryResultClass::Atomic,
    MemoryResultClass::Vector,
];
const HIERARCHY_RESULT_CLASSES: [MemoryResultClass; 3] = [
    MemoryResultClass::FloatLoad,
    MemoryResultClass::Atomic,
    MemoryResultClass::Vector,
];

#[test]
fn rem6_run_o3_memory_result_writeback_matrix_direct() {
    run_result_matrix(&DIRECT_RESULT_CLASSES, "direct", 1);
}

#[test]
fn rem6_run_o3_memory_result_writeback_matrix_cache_fabric_dram() {
    run_result_matrix(&HIERARCHY_RESULT_CLASSES, "cache-fabric-dram", 1);
}

#[test]
fn rem6_run_o3_memory_result_writeback_width_two_exact_fit() {
    run_result_matrix(&DIRECT_RESULT_CLASSES, "direct", 2);
}

#[test]
fn rem6_run_o3_memory_result_writeback_readfile_mmio() {
    run_result_matrix(&[MemoryResultClass::Mmio], "direct", 1);
}

fn run_result_matrix(classes: &[MemoryResultClass], memory_system: &str, writeback_width: usize) {
    for &class in classes {
        let fixture = MemoryResultFixture::new(class);
        let route_delay = calibrate_result_collision(&fixture, memory_system);
        assert_eq!(
            route_delay,
            class.expected_route_delay(memory_system),
            "{} {memory_system} route-delay lock changed",
            class.label()
        );
        let completed = fixture.run(memory_system, writeback_width, route_delay, RESULT_MAX_TICK);
        assert_result_collision(&fixture, &completed, writeback_width);
        let admitted = assert_pre_and_post_admission(
            &fixture,
            memory_system,
            writeback_width,
            route_delay,
            &completed,
        );
        match (class, memory_system) {
            (MemoryResultClass::Mmio, _) => assert_mmio_resources(&completed),
            (_, "cache-fabric-dram") => assert_hierarchy_result_resources(&fixture, &admitted),
            _ => assert_direct_result_resources(&fixture, &admitted),
        }
    }
}

struct MemoryResultFixture {
    class: MemoryResultClass,
    binary: std::path::PathBuf,
    readfile: Option<std::path::PathBuf>,
}

impl MemoryResultFixture {
    fn new(class: MemoryResultClass) -> Self {
        let binary = memory_result_binary(class);
        let readfile = (class == MemoryResultClass::Mmio).then(|| {
            unique_result_temp_binary(
                "o3-memory-result-writeback-mmio-data",
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
        writeback_width: usize,
        route_delay: u64,
        max_tick: u64,
    ) -> Value {
        let config = WritebackRunConfig::detailed_json(
            memory_system,
            writeback_width,
            route_delay,
            max_tick,
        );
        let mut command = writeback_command(&self.binary, config);
        command.args(["--host-event-delay", "1"]);
        if let Some((address, bytes)) = self.class.dump_range() {
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
            "{} {memory_system} width {writeback_width} route delay {route_delay} stderr: {}",
            self.class.label(),
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{} invalid stdout JSON: {error}", self.class.label()));
        if max_tick == RESULT_MAX_TICK {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host")
            );
            assert_eq!(json_u64(&json, "/simulation/stop_code"), 0);
        }
        json
    }
}

impl MemoryResultClass {
    const fn label(self) -> &'static str {
        match self {
            Self::FloatLoad => "FLD",
            Self::LoadReserved => "LR.D",
            Self::Atomic => "AMOSWAP.D",
            Self::Vector => "masked VLE64.V",
            Self::Mmio => "readfile LD",
        }
    }

    const fn pcs(self) -> [&'static str; 3] {
        match self {
            Self::Vector => ["0x80000030", "0x80000034", "0x80000038"],
            Self::Mmio => ["0x80000010", "0x80000014", "0x80000018"],
            _ => ["0x80000018", "0x8000001c", "0x80000020"],
        }
    }

    const fn request_evidence(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::FloatLoad => ("float_load", "load", "0x80000080"),
            Self::LoadReserved => ("load_reserved", "load", "0x80000080"),
            Self::Atomic => ("atomic", "atomic", "0x80000080"),
            Self::Vector => ("vector_load", "load", "0x80000088"),
            Self::Mmio => ("load", "load", "0x10000000"),
        }
    }

    const fn dump_range(self) -> Option<(u64, u64)> {
        match self {
            Self::FloatLoad => Some((0x8000_0088, 8)),
            Self::LoadReserved => Some((0x8000_0080, 8)),
            Self::Atomic => Some((0x8000_0080, 16)),
            Self::Vector => Some((0x8000_0090, 16)),
            Self::Mmio => None,
        }
    }

    fn expected_route_delay(self, memory_system: &str) -> u64 {
        match memory_system {
            "direct" => 9,
            "cache-fabric-dram" if self != Self::LoadReserved && self != Self::Mmio => 8,
            _ => panic!("unsupported route-delay lock"),
        }
    }
}

fn calibrate_result_collision(fixture: &MemoryResultFixture, memory_system: &str) -> u64 {
    let mut collisions = Vec::new();
    let mut observations = Vec::new();
    for route_delay in ROUTE_DELAY_CANDIDATES {
        let json = fixture.run(memory_system, 2, route_delay, RESULT_MAX_TICK);
        let pcs = fixture.class.pcs();
        let result = memory_result_event_at_pc(&json, pcs[0]);
        let div = event_at_pc(&json, pcs[1]);
        let result_raw_ready = event_u64(result, "lsq_data_response_tick") + 1;
        let div_raw_ready = event_u64(div, "issue_tick") + 19;
        observations.push((
            route_delay,
            event_u64(result, "issue_tick"),
            result_raw_ready,
            div_raw_ready,
        ));
        if result_raw_ready == div_raw_ready {
            collisions.push(route_delay);
        }
    }
    assert_eq!(
        collisions.len(),
        1,
        "{} {memory_system} must have exactly one bounded route-delay collision: {observations:?}",
        fixture.class.label()
    );
    collisions[0]
}

fn assert_result_collision(fixture: &MemoryResultFixture, json: &Value, writeback_width: usize) {
    let class = fixture.class;
    let pcs = class.pcs();
    let result = memory_result_event_at_pc(json, pcs[0]);
    let div = event_at_pc(json, pcs[1]);
    let witness = event_at_pc(json, pcs[2]);
    assert_eq!(event_str(div, "fu_latency_class"), "scalar_integer_div");
    assert_eq!(event_u64(div, "fu_latency_cycles"), 19);
    assert_eq!(
        event_str(result, "lsq_operation"),
        class.request_evidence().0
    );
    let result_raw_ready = event_u64(result, "lsq_data_response_tick") + 1;
    let div_raw_ready = event_u64(div, "issue_tick") + 19;
    assert_eq!(
        result_raw_ready,
        div_raw_ready,
        "{} collision",
        class.label()
    );
    assert_eq!(event_u64(result, "writeback_tick"), result_raw_ready);
    assert_eq!(
        event_u64(div, "writeback_tick"),
        div_raw_ready + u64::from(writeback_width == 1)
    );
    assert_event_order([result, div, witness], "sequence", true);
    assert_event_order([result, div, witness], "writeback_tick", false);
    assert_event_order([result, div, witness], "commit_tick", false);
    for event in [result, div, witness] {
        assert!(event_u64(event, "issue_tick") <= event_u64(event, "writeback_tick"));
        assert!(event_u64(event, "writeback_tick") <= event_u64(event, "commit_tick"));
    }
    assert!(event_u64(witness, "issue_tick") >= event_u64(result, "writeback_tick"));
    assert_writeback_port_totals(json, writeback_width);
    assert_final_witness(fixture, json);
}

fn assert_writeback_port_totals(json: &Value, writeback_width: usize) {
    let writeback = writeback_port_artifact(json);
    let deferred = u64::from(writeback_width == 1);
    for (field, value) in [
        ("cycles", 1 + deferred),
        ("admitted_rows", 2),
        ("deferred_rows", deferred),
        ("deferred_row_cycles", deferred),
        ("max_ready_rows_per_cycle", 2),
        ("max_deferred_rows", deferred),
    ] {
        assert_eq!(
            writeback_port_u64(writeback, field),
            value,
            "{field}: {writeback}"
        );
    }
    for (field, unit) in WRITEBACK_PORT_STATS {
        assert_json_stat(
            json,
            &format!("sim.cpu0.o3.writeback_port.{field}"),
            unit,
            writeback_port_u64(writeback, field),
            "monotonic",
        );
    }
}

fn assert_pre_and_post_admission(
    fixture: &MemoryResultFixture,
    memory_system: &str,
    writeback_width: usize,
    route_delay: u64,
    completed: &Value,
) -> Value {
    let completed_result = memory_result_event_at_pc(completed, fixture.class.pcs()[0]);
    let raw_ready_tick = event_u64(completed_result, "lsq_data_response_tick") + 1;
    let admitted_tick = event_u64(completed_result, "writeback_tick");
    let before = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        admitted_tick - 1,
    );
    let sequence = event_u64(completed_result, "sequence");
    let row = rob_entry_at_sequence(&before, sequence);
    let pre_admission_destination = event_u64(row, "destination");
    let reservation = writeback_reservation_at_sequence(&before, sequence);
    assert_eq!(event_u64(reservation, "raw_ready_tick"), raw_ready_tick);
    assert_eq!(event_u64(reservation, "admitted_tick"), admitted_tick);
    assert_eq!(row.pointer("/ready").and_then(Value::as_bool), Some(false));
    assert_eq!(
        row.pointer("/live_staged").and_then(Value::as_bool),
        Some(true)
    );
    assert_pre_admission_witness(fixture, &before);

    let at_admission = fixture.run(memory_system, writeback_width, route_delay, admitted_tick);
    assert_admitted_result(
        fixture.class,
        &at_admission,
        sequence,
        admitted_tick,
        pre_admission_destination,
    );
    at_admission
}

fn assert_pre_admission_witness(fixture: &MemoryResultFixture, json: &Value) {
    match fixture.class {
        MemoryResultClass::FloatLoad => {
            assert_eq!(memory_dump_hex(json, 0x8000_0088), Some("0000000000000000"));
        }
        MemoryResultClass::LoadReserved => {
            assert_register_absent(json, "x7");
            assert_register_absent(json, "x8");
        }
        MemoryResultClass::Atomic => {
            assert_register_absent(json, "x11");
            assert_eq!(
                memory_dump_hex(json, 0x8000_0080),
                Some("02000000000000000000000000000000")
            );
        }
        MemoryResultClass::Vector => {
            assert_eq!(
                memory_dump_hex(json, 0x8000_0090),
                Some("11000000000000001100000000000000")
            );
        }
        MemoryResultClass::Mmio => {
            assert_register_absent(json, "x12");
            assert_register_absent(json, "x13");
        }
    }
}

fn assert_admitted_result(
    class: MemoryResultClass,
    json: &Value,
    sequence: u64,
    admitted_tick: u64,
    pre_admission_destination: u64,
) {
    let result = memory_result_event_at_pc(json, class.pcs()[0]);
    assert_eq!(event_u64(result, "sequence"), sequence);
    assert_eq!(event_u64(result, "writeback_tick"), admitted_tick);
    assert_eq!(event_u64(result, "commit_tick"), admitted_tick);
    assert_rob_sequence_absent(json, sequence);
    match class {
        MemoryResultClass::LoadReserved => assert_register(json, "x7", "0x1122334455667788"),
        MemoryResultClass::Atomic => assert_register(json, "x11", "0x9"),
        MemoryResultClass::Mmio => assert_register(json, "x12", "0x123456789abcdef"),
        MemoryResultClass::FloatLoad | MemoryResultClass::Vector => {}
    }
    let register_class = match class {
        MemoryResultClass::FloatLoad => "floating_point",
        MemoryResultClass::Vector => "vector",
        _ => return,
    };
    let entries = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} admitted rename map missing: {json}", class.label()));
    let mut mappings = entries.iter().filter(|entry| {
        entry.pointer("/register_class").and_then(Value::as_str) == Some(register_class)
            && entry.pointer("/architectural").and_then(Value::as_u64) == Some(1)
    });
    let mapping = mappings
        .next()
        .unwrap_or_else(|| panic!("{} destination identity missing: {json}", class.label()));
    assert!(
        mappings.next().is_none(),
        "{} destination identity must be unique: {json}",
        class.label()
    );
    assert_eq!(
        event_u64(mapping, "physical"),
        pre_admission_destination,
        "{} rename identity must match its pre-admission ROB destination",
        class.label()
    );
}

fn assert_final_witness(fixture: &MemoryResultFixture, json: &Value) {
    match fixture.class {
        MemoryResultClass::FloatLoad => {
            assert_eq!(memory_dump_hex(json, 0x8000_0088), Some("182d4454fb210940"));
        }
        MemoryResultClass::LoadReserved => {
            assert_register(json, "x7", "0x1122334455667788");
            assert_register(json, "x8", "0x1122334455667789");
        }
        MemoryResultClass::Atomic => {
            assert_register(json, "x11", "0x9");
            assert_eq!(
                memory_dump_hex(json, 0x8000_0080),
                Some("02000000000000000900000000000000")
            );
        }
        MemoryResultClass::Vector => {
            assert_eq!(
                memory_dump_hex(json, 0x8000_0090),
                Some("11000000000000005555444433332222")
            );
            result_data_record(json, MemoryResultClass::Vector);
        }
        MemoryResultClass::Mmio => {
            assert_register(json, "x12", "0x123456789abcdef");
            assert_register(json, "x13", "0x123456789abcdf0");
        }
    }
}

fn assert_result_request_transport<'a>(
    fixture: &MemoryResultFixture,
    json: &'a Value,
) -> (&'a Value, &'a Value, u64, u64) {
    assert_eq!(
        data_trace(json).len(),
        1,
        "witness traffic completed early: {json}"
    );
    let data = result_data_record(json, fixture.class);
    let trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("result memory trace missing: {json}"))
        .iter()
        .filter(|record| event_str(record, "channel") == "data")
        .collect::<Vec<_>>();
    assert_eq!(
        trace.len(),
        3,
        "exact data request/response required: {trace:?}"
    );
    let (sent, arrived, response) = (trace[0], trace[1], trace[2]);
    let request = event_u64(sent, "request");
    let route = event_u64(sent, "route");
    for (record, kind, endpoint) in [
        (sent, "request_sent", "cpu0.dmem"),
        (arrived, "request_arrived", "memory"),
        (response, "response_arrived", "cpu0.dmem"),
    ] {
        assert_eq!(event_str(record, "kind"), kind);
        assert_eq!(event_str(record, "endpoint"), endpoint);
        assert_eq!(event_u64(record, "request"), request);
        assert_eq!(event_u64(record, "route"), route);
    }
    let result = memory_result_event_at_pc(json, fixture.class.pcs()[0]);
    assert_eq!(event_u64(sent, "tick"), event_u64(result, "issue_tick"));
    assert_eq!(
        event_u64(response, "tick"),
        event_u64(result, "lsq_data_response_tick")
    );
    assert_eq!(event_u64(data, "tick"), event_u64(response, "tick"));
    assert_eq!(event_str(response, "response_status"), "completed");
    assert_eq!(
        event_u64(response, "response_latency_ticks"),
        event_u64(response, "tick") - event_u64(sent, "tick")
    );
    assert_resource_counter(json, "transport.data.activity", 1);
    let transport = json.pointer("/memory_resources/transport/data").unwrap();
    for field in ["request_arrivals", "responses", "response_arrivals"] {
        assert_eq!(event_u64(transport, field), 1, "data transport {field}");
    }
    (sent, arrived, request, route)
}

fn assert_direct_result_resources(fixture: &MemoryResultFixture, json: &Value) {
    assert_result_request_transport(fixture, json);
    assert_zero_ordinary_hierarchy(json, false);
}

fn assert_hierarchy_result_resources(fixture: &MemoryResultFixture, json: &Value) {
    let (sent, arrived, request, route) = assert_result_request_transport(fixture, json);
    for (suffix, expected) in [
        ("cache.data.activity", 3),
        ("cache.data.l1.activity", 1),
        ("cache.data.l2.activity", 1),
        ("cache.data.l3.activity", 1),
        ("cache.data.l3.dram_accesses", 1),
    ] {
        assert_resource_counter(json, suffix, expected);
    }
    let packet = (route << 48) | request;
    let hop = json
        .pointer("/memory_resources/fabric/hop_activities")
        .and_then(Value::as_array)
        .and_then(|hops| hops.iter().find(|hop| event_u64(hop, "packet") == packet))
        .unwrap_or_else(|| panic!("data request fabric packet {packet} missing: {json}"));
    assert_eq!(event_u64(hop, "ready_tick"), event_u64(sent, "tick"));
    assert_eq!(event_u64(hop, "arrival_tick"), event_u64(arrived, "tick"));
    assert_eq!(event_u64(hop, "bytes"), 8);
    assert_eq!(event_u64(hop, "virtual_network"), 1);
    for layer in ["fabric", "dram"] {
        let pointer = format!("/memory_resources/{layer}/activity");
        assert!(json_u64(json, &pointer) > 0);
        assert_json_stat_at_least(
            json,
            &format!("sim.memory.resources.{layer}.activity"),
            "Count",
            1,
            "monotonic",
        );
    }
    assert!(json_u64(json, "/memory_resources/dram/reads") > 0);
}

fn assert_mmio_resources(json: &Value) {
    assert_eq!(data_trace(json).len(), 1);
    result_data_record(json, MemoryResultClass::Mmio);
    assert_eq!(json_u64(json, "/readfiles/0/bytes"), 8);
    assert_json_stat(json, "sim.readfiles", "Count", 1, "constant");
    assert_json_stat(json, "sim.readfile_bytes", "Byte", 8, "constant");
    assert_json_stat(json, "sim.readfile0.bytes", "Byte", 8, "constant");
    assert!(json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records
            .iter()
            .all(|record| { record.pointer("/channel").and_then(Value::as_str) != Some("data") })));
    assert_zero_ordinary_hierarchy(json, true);
}

fn assert_zero_ordinary_hierarchy(json: &Value, include_transport: bool) {
    for (index, suffix) in ORDINARY_RESOURCES.into_iter().enumerate() {
        if !include_transport && index == 1 {
            continue;
        }
        assert_resource_counter(json, &format!("{suffix}.activity"), 0);
    }
}

fn memory_result_binary(class: MemoryResultClass) -> std::path::PathBuf {
    if class == MemoryResultClass::Vector {
        return vector_memory_result_binary();
    }
    if class == MemoryResultClass::Mmio {
        return mmio_memory_result_binary();
    }
    let data_start = 128_i32;
    let mut words = Vec::new();
    let auipc_pc = 0_i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        m5op(M5_SWITCH_CPU),
    ]);
    match class {
        MemoryResultClass::FloatLoad => words.extend([
            i_type(0, 5, 0b011, 1, 0x07),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            float_store_type(8, 1, 5, 0b011),
        ]),
        MemoryResultClass::LoadReserved => words.extend([
            atomic_type(0x02, false, false, 0, 5, 0x3, 7),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            i_type(1, 7, 0x0, 8, 0x13),
        ]),
        MemoryResultClass::Atomic => words.extend([
            atomic_type(0x01, false, false, 2, 5, 0x3, 11),
            r_type(0x01, 2, 1, 0b100, 3, 0x33),
            s_type(8, 11, 5, 0b011),
        ]),
        MemoryResultClass::Vector | MemoryResultClass::Mmio => unreachable!(),
    }
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    match class {
        MemoryResultClass::FloatLoad => {
            let mut program = riscv64_program(&words);
            program.extend_from_slice(&3.141592653589793f64.to_bits().to_le_bytes());
            program.extend_from_slice(&0_u64.to_le_bytes());
            return result_temp_binary(class, &program);
        }
        MemoryResultClass::LoadReserved => words.extend([0x5566_7788, 0x1122_3344]),
        MemoryResultClass::Atomic => words.extend([9, 0, 0, 0]),
        MemoryResultClass::Vector | MemoryResultClass::Mmio => unreachable!(),
    }
    result_temp_binary(class, &riscv64_program(&words))
}

fn vector_memory_result_binary() -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(16, 5, 0x0, 16, 0x13),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        i_type(2, 0, 0x0, 6, 0x13),
        i_type(17, 0, 0x0, 7, 0x13),
        i_type(2, 0, 0x0, 11, 0x13),
        vsetvli_type(0xd8, 11, 0),
        vector_arith_type(0b010111, 0b100, 0, 6, 0),
        vector_arith_type(0b010111, 0b100, 0, 7, 1),
        m5op(M5_SWITCH_CPU),
        vector_unit_stride_load_type(false, 0b111, 5, 1),
        r_type(0x01, 2, 1, 0b100, 3, 0x33),
        vector_unit_stride_store_type(true, 0b111, 16, 1),
    ];
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    for value in [0xaaaa_aaaa_aaaa_aaaa_u64, 0x2222_3333_4444_5555, 17, 17] {
        program.extend_from_slice(&value.to_le_bytes());
    }
    result_temp_binary(MemoryResultClass::Vector, &program)
}

fn mmio_memory_result_binary() -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x1000_0000, 5, 0x37),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(0, 5, 0b011, 12, 0x03),
        r_type(0x01, 2, 1, 0b100, 3, 0x33),
        i_type(1, 12, 0x0, 13, 0x13),
    ];
    append_host_stop(&mut words);
    result_temp_binary(MemoryResultClass::Mmio, &riscv64_program(&words))
}

fn result_temp_binary(class: MemoryResultClass, program: &[u8]) -> std::path::PathBuf {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, program);
    unique_result_temp_binary(
        &format!("o3-memory-result-writeback-{}", class.label()),
        &elf,
    )
}

fn unique_result_temp_binary(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let id = RESULT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    temp_binary(&format!("{name}-{id}"), bytes)
}
