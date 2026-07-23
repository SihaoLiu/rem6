use super::*;

const FIRST_VALUE: u64 = 0x11;
const SECOND_VALUE: u64 = 0x33;
pub(super) const MMIO_PAGE: u64 = 0x1000_0000;
const SETUP_PROBE_OFFSET: i32 = 64;
const CALIBRATION_PAGE_BYTES: usize = SETUP_PROBE_OFFSET as usize + 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TranslatedPairTarget {
    Memory,
    Mmio,
}

pub(super) struct TranslatedMemoryPairFixture {
    target: TranslatedPairTarget,
    binary: std::path::PathBuf,
    readfile: Option<std::path::PathBuf>,
}

impl TranslatedMemoryPairFixture {
    pub(super) fn new() -> Self {
        Self {
            target: TranslatedPairTarget::Memory,
            binary: translated_memory_pair_binary(TranslatedPairTarget::Memory),
            readfile: None,
        }
    }

    pub(super) fn new_mmio() -> Self {
        Self {
            target: TranslatedPairTarget::Mmio,
            binary: translated_memory_pair_binary(TranslatedPairTarget::Mmio),
            readfile: Some(super::super::unique_result_temp_binary(
                "o3-translated-memory-mmio-result-pair-data",
                &SECOND_VALUE.to_le_bytes(),
            )),
        }
    }

    pub(super) fn run(
        &self,
        memory_system: &str,
        writeback_width: usize,
        route_delay: u64,
        max_tick: u64,
    ) -> Value {
        self.run_with_translation(
            memory_system,
            writeback_width,
            2,
            route_delay,
            max_tick,
            true,
            None,
        )
    }

    pub(super) fn run_identity_control(
        &self,
        memory_system: &str,
        writeback_width: usize,
        route_delay: u64,
    ) -> Value {
        self.run_with_translation(
            memory_system,
            writeback_width,
            1,
            route_delay,
            PAIR_MAX_TICK,
            true,
            None,
        )
    }

    pub(super) fn run_calibration(&self, memory_system: &str, route_delay: u64) -> Value {
        self.run_with_translation(memory_system, 2, 2, route_delay, PAIR_MAX_TICK, false, None)
    }

    pub(super) fn run_mixed(&self, memory_system: &str, route_delay: u64, max_tick: u64) -> Value {
        self.run_with_translation(memory_system, 1, 1, route_delay, max_tick, true, None)
    }

    pub(super) fn run_mixed_with_switch(
        &self,
        memory_system: &str,
        route_delay: u64,
        switch_tick: u64,
    ) -> Value {
        self.run_with_translation(
            memory_system,
            1,
            1,
            route_delay,
            PAIR_MAX_TICK,
            true,
            Some(switch_tick),
        )
    }

    fn run_with_translation(
        &self,
        memory_system: &str,
        writeback_width: usize,
        memory_issue_width: usize,
        route_delay: u64,
        max_tick: u64,
        translated: bool,
        host_switch_tick: Option<u64>,
    ) -> Value {
        let id = super::super::RESULT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let workspace = temp_workspace(&format!("o3-translated-memory-pair-{id}"));
        let config = workspace.join("run.toml");
        std::fs::write(
            &config,
            translated_pair_config(
                &self.binary,
                memory_system,
                writeback_width,
                memory_issue_width,
                route_delay,
                max_tick,
                translated,
                self.target,
                host_switch_tick,
            ),
        )
        .unwrap();

        let first_head_dump = format!("0x{FIRST_PHYSICAL_PAGE:x}:16");
        let first_tail_dump = format!("0x{:x}:8", FIRST_PHYSICAL_PAGE + 16);
        let second_head_dump = format!("0x{SECOND_PHYSICAL_PAGE:x}:16");
        let second_tail_dump = format!("0x{:x}:8", SECOND_PHYSICAL_PAGE + 16);
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--host-event-delay",
            "1",
            "--dump-memory",
            &first_head_dump,
            "--dump-memory",
            &first_tail_dump,
        ]);
        match self.target {
            TranslatedPairTarget::Memory => {
                command.args([
                    "--dump-memory",
                    &second_head_dump,
                    "--dump-memory",
                    &second_tail_dump,
                ]);
            }
            TranslatedPairTarget::Mmio => {
                command.args([
                    "--dump-memory",
                    &format!("0x{:x}:8", FIRST_PHYSICAL_PAGE + 24),
                    "--dump-memory",
                    &format!("0x{:x}:8", FIRST_PHYSICAL_PAGE + 32),
                ]);
            }
        }
        if let Some(readfile) = &self.readfile {
            command.args([
                "--readfile",
                &format!("0x{MMIO_PAGE:x}:0x100:{}", readfile.display()),
            ]);
        }
        let child = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output =
            crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30));
        let _ = std::fs::remove_dir_all(&workspace);
        assert!(
            output.status.success(),
            "translated pair {memory_system} memory width {memory_issue_width} writeback width {writeback_width} delay {route_delay} max tick {max_tick} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("translated pair invalid stdout JSON: {error}"));
        if max_tick == PAIR_MAX_TICK {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host"),
                "translated pair did not reach host stop: {json}"
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

fn translated_pair_config(
    binary: &std::path::Path,
    memory_system: &str,
    writeback_width: usize,
    memory_issue_width: usize,
    route_delay: u64,
    max_tick: u64,
    translated: bool,
    target: TranslatedPairTarget,
    host_switch_tick: Option<u64>,
) -> String {
    let second_physical_page = match target {
        TranslatedPairTarget::Memory => SECOND_PHYSICAL_PAGE,
        TranslatedPairTarget::Mmio => MMIO_PAGE,
    };
    let translation = translated.then(|| {
        format!(
            r#"
[run.riscv_data_translation]
queue_capacity = 4
latency = 2
tlb_capacity = 4
page_size = 4096

[[run.riscv_data_translation.mappings]]
virtual_base = 16384
physical_base = 2147487744
pages = 1
read = true
write = true

[[run.riscv_data_translation.mappings]]
virtual_base = 20480
physical_base = {second_physical_page}
pages = 1
read = true
write = true
"#
        )
    });
    let host_switch = host_switch_tick
        .map(|tick| format!("host_execution_mode_switches = [\"{tick}:cpu0:timing\"]\n"))
        .unwrap_or_default();
    format!(
        r#"[run]
isa = "riscv"
binary = "{}"
max_tick = {max_tick}
execute = true
stats_format = "json"
debug_flags = ["O3", "Data", "Fetch", "Memory", "HostAction"]
memory_system = "{memory_system}"
memory_route_delay = {route_delay}
m5_switch_cpu_mode = "detailed"
riscv_o3_issue_width = 4
riscv_o3_memory_issue_width = {memory_issue_width}
riscv_o3_writeback_width = {writeback_width}
riscv_o3_scalar_memory_depth = 4
{host_switch}{}
"#,
        binary.display(),
        translation.unwrap_or_default()
    )
}

fn translated_memory_pair_binary(target: TranslatedPairTarget) -> std::path::PathBuf {
    let mut words = vec![
        u_type(FIRST_VIRTUAL_PAGE as i32, 5, 0x37),
        u_type(SECOND_VIRTUAL_PAGE as i32, 6, 0x37),
        i_type(SETUP_PROBE_OFFSET, 5, 0b011, 0, 0x03),
        i_type(SETUP_PROBE_OFFSET, 6, 0b011, 0, 0x03),
        i_type(84, 0, 0, 1, 0x13),
        i_type(2, 0, 0, 2, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend([
        i_type(0, 5, 0b011, 11, 0x03),
        i_type(0, 6, 0b011, 12, 0x03),
        r_type(0x01, 2, 1, 0b100, 3, 0x33),
        i_type(1, 12, 0, 13, 0x13),
        s_type(8, 11, 5, 0b011),
        s_type(16, 3, 5, 0b011),
    ]);
    match target {
        TranslatedPairTarget::Memory => {
            words.extend([s_type(8, 12, 6, 0b011), s_type(16, 13, 6, 0b011)])
        }
        TranslatedPairTarget::Mmio => {
            words.extend([s_type(24, 12, 5, 0b011), s_type(32, 13, 5, 0b011)])
        }
    }
    append_host_stop(&mut words);

    let first_offset = (FIRST_PHYSICAL_PAGE - 0x8000_0000) as usize;
    let second_offset = (SECOND_PHYSICAL_PAGE - 0x8000_0000) as usize;
    let mut payload = riscv64_program(&words);
    let payload_bytes = match target {
        TranslatedPairTarget::Memory => second_offset + SETUP_PROBE_OFFSET as usize + 8,
        TranslatedPairTarget::Mmio => first_offset + SETUP_PROBE_OFFSET as usize + 8,
    };
    payload.resize(payload_bytes, 0);
    write_u64(&mut payload, first_offset, FIRST_VALUE);
    if target == TranslatedPairTarget::Memory {
        write_u64(&mut payload, second_offset, SECOND_VALUE);
    }
    super::super::unique_result_temp_binary(
        "o3-translated-memory-result-pair",
        &translated_pair_elf(&payload),
    )
}

fn translated_pair_elf(payload: &[u8]) -> Vec<u8> {
    const HIGH_OFFSET: usize = 0x1000;
    const FIRST_LOW_OFFSET: usize = 0x4000;
    const SECOND_LOW_OFFSET: usize = 0x5000;
    let mut bytes = vec![0; SECOND_LOW_OFFSET + CALIBRATION_PAGE_BYTES];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, 243);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8000_0000);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 3);
    write_load_segment(
        &mut bytes,
        64,
        HIGH_OFFSET as u64,
        0x8000_0000,
        payload.len() as u64,
        7,
    );
    write_load_segment(
        &mut bytes,
        120,
        FIRST_LOW_OFFSET as u64,
        FIRST_VIRTUAL_PAGE,
        CALIBRATION_PAGE_BYTES as u64,
        6,
    );
    write_load_segment(
        &mut bytes,
        176,
        SECOND_LOW_OFFSET as u64,
        SECOND_VIRTUAL_PAGE,
        CALIBRATION_PAGE_BYTES as u64,
        6,
    );
    bytes[HIGH_OFFSET..HIGH_OFFSET + payload.len()].copy_from_slice(payload);
    write_u64(&mut bytes, FIRST_LOW_OFFSET, FIRST_VALUE);
    write_u64(&mut bytes, SECOND_LOW_OFFSET, SECOND_VALUE);
    bytes
}

fn write_load_segment(
    bytes: &mut [u8],
    offset: usize,
    file_offset: u64,
    address: u64,
    size: u64,
    flags: u32,
) {
    write_u32(bytes, offset, 1);
    write_u32(bytes, offset + 4, flags);
    write_u64(bytes, offset + 8, file_offset);
    write_u64(bytes, offset + 16, address);
    write_u64(bytes, offset + 24, address);
    write_u64(bytes, offset + 32, size);
    write_u64(bytes, offset + 40, size);
    write_u64(bytes, offset + 48, 0x1000);
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn assert_route_resources(
    before: &Value,
    through_pair: &Value,
    memory_system: &str,
    pair: &[PairRequestEvidence<'_>; 2],
) {
    assert_eq!(
        resource_delta(
            before,
            through_pair,
            "/memory_resources/transport/data/activity"
        ),
        2
    );
    match memory_system {
        "direct" => {
            for pointer in [
                "/memory_resources/cache/data/activity",
                "/memory_resources/fabric/activity",
                "/memory_resources/dram/activity",
            ] {
                assert_eq!(
                    resource_delta(before, through_pair, pointer),
                    0,
                    "{pointer}"
                );
            }
        }
        "cache-fabric-dram" => {
            for (pointer, expected) in [
                ("/memory_resources/cache/data/activity", 6),
                ("/memory_resources/cache/data/dram_accesses", 2),
                ("/memory_resources/cache/data/l1/activity", 2),
                ("/memory_resources/cache/data/l2/activity", 2),
                ("/memory_resources/cache/data/l3/activity", 2),
                ("/memory_resources/cache/instruction/dram_accesses", 0),
                ("/memory_resources/fabric/activity", 4),
                ("/memory_resources/dram/activity", 4),
                ("/memory_resources/dram/accesses", 2),
                ("/memory_resources/dram/reads", 2),
            ] {
                assert_eq!(
                    resource_delta(before, through_pair, pointer),
                    expected,
                    "{pointer}"
                );
            }
            for evidence in pair {
                let sent = request_sent_for_identity(through_pair, evidence.identity)
                    .unwrap_or_else(|| panic!("missing pair request {:?}", evidence.identity));
                let arrived =
                    data_record_for_identity(through_pair, "request_arrived", evidence.identity)
                        .unwrap();
                let request_packet = (event_u64(sent, "route") << 48) | evidence.identity.sequence;
                let response_packet = request_packet | (1 << 63);
                let request_hop = fabric_hop(through_pair, request_packet, evidence.pc);
                let response_hop = fabric_hop(through_pair, response_packet, evidence.pc);
                assert_eq!(
                    event_u64(request_hop, "ready_tick"),
                    event_u64(sent, "tick")
                );
                assert_eq!(
                    event_u64(request_hop, "arrival_tick"),
                    event_u64(arrived, "tick")
                );
                assert_eq!(
                    event_u64(response_hop, "arrival_tick"),
                    event_u64(evidence.response, "tick")
                );
                for (hop, network) in [(request_hop, 1), (response_hop, 2)] {
                    assert_eq!(event_u64(hop, "bytes"), 8);
                    assert_eq!(event_u64(hop, "virtual_network"), network);
                }
            }
        }
        _ => unreachable!(),
    }
}

fn fabric_hop<'a>(json: &'a Value, packet: u64, pc: &str) -> &'a Value {
    let matches = json
        .pointer("/memory_resources/fabric/hop_activities")
        .and_then(Value::as_array)
        .expect("hierarchy fabric hops")
        .iter()
        .filter(|hop| event_u64(hop, "packet") == packet)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "exact hierarchy packet {packet} for {pc}");
    matches[0]
}

pub(super) fn resource_delta(before: &Value, after: &Value, pointer: &str) -> u64 {
    json_u64(after, pointer)
        .checked_sub(json_u64(before, pointer))
        .unwrap_or_else(|| panic!("resource counter regressed at {pointer}"))
}
