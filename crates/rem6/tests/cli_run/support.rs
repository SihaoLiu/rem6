use std::{
    env, fs,
    path::{Path, PathBuf},
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];

pub(crate) const GEM5_READ_REQ: u32 = 1;
pub(crate) const GEM5_READ_RESP: u32 = 2;
pub(crate) const GEM5_WRITE_REQ: u32 = 4;
pub(crate) const GEM5_WRITE_RESP: u32 = 5;
pub(crate) const GEM5_WRITE_COMPLETE_RESP: u32 = 6;
pub(crate) const GEM5_CLEAN_SHARED_REQ: u32 = 42;
pub(crate) const GEM5_CLEAN_SHARED_RESP: u32 = 43;
pub(crate) const GEM5_INVALIDATE_REQ: u32 = 54;
pub(crate) const GEM5_INVALIDATE_RESP: u32 = 55;
pub(crate) const GEM5_MEM_FENCE_REQ: u32 = 38;
pub(crate) const GEM5_MEM_FENCE_RESP: u32 = 41;
pub(crate) const GEM5_READ_ERROR: u32 = 48;
pub(crate) const GEM5_WRITE_ERROR: u32 = 49;
pub(crate) const GEM5_PRINT_REQ: u32 = 52;
pub(crate) const GEM5_FLUSH_REQ: u32 = 53;
pub(crate) const GEM5_HTM_REQ: u32 = 56;
pub(crate) const GEM5_HTM_REQ_RESP: u32 = 57;
pub(crate) const GEM5_HTM_ABORT: u32 = 58;
pub(crate) const GEM5_TLBI_EXT_SYNC: u32 = 59;

#[derive(Clone, Copy)]
pub(crate) struct PacketFields {
    pub(crate) tick: u64,
    pub(crate) command: u32,
    pub(crate) address: Option<u64>,
    pub(crate) size: Option<u32>,
    pub(crate) packet_id: Option<u64>,
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

pub(crate) fn riscv64_elf(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let payload_offset = 128usize;
    let mut bytes = vec![0; payload_offset + payload.len()];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, 243);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, entry);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 1);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, physical);
    write_u64(&mut bytes, 88, physical);
    write_u64(&mut bytes, 96, payload.len() as u64);
    write_u64(&mut bytes, 104, payload.len() as u64);
    write_u64(&mut bytes, 112, 0x1000);
    bytes[payload_offset..].copy_from_slice(payload);
    bytes
}

pub(crate) fn x86_64_elf(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
    write_u16(&mut bytes, 18, 62);
    bytes
}

pub(crate) fn riscv64_program(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

pub(crate) fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

pub(crate) fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

pub(crate) fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
}

pub(crate) fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
}

pub(crate) fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

pub(crate) fn atomic_type(
    funct5: u32,
    aq: bool,
    rl: bool,
    rs2: u8,
    rs1: u8,
    funct3: u32,
    rd: u8,
) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

pub(crate) fn csr_read(csr: u32, rd: u8) -> u32 {
    (csr << 20) | (0x2 << 12) | (u32::from(rd) << 7) | 0x73
}

pub(crate) fn temp_binary(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.elf", std::process::id()));
    fs::write(&path, bytes).unwrap();
    path
}

pub(crate) fn temp_output(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

pub(crate) fn temp_config(name: &str, text: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.toml", std::process::id()));
    fs::write(&path, text).unwrap();
    path
}

pub(crate) fn temp_workspace(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

pub(crate) fn temp_trace(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.pb", std::process::id()));
    fs::write(&path, bytes).unwrap();
    path
}

pub(crate) fn find_riscv_tool(name: &str) -> Option<PathBuf> {
    find_tool_on_path(name).or_else(|| {
        let module_candidate =
            Path::new("/mnt/nas0/software/riscv/riscv64-elf-ubuntu-24.04-gcc/bin").join(name);
        module_candidate.is_file().then_some(module_candidate)
    })
}

fn find_tool_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

pub(crate) fn packet_trace_bytes(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut bytes = GEM5_MAGIC.to_vec();
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    for packet in packets {
        let mut message = Vec::new();
        append_key(&mut message, 1, 0);
        append_varint(&mut message, packet.tick);
        append_key(&mut message, 2, 0);
        append_varint(&mut message, u64::from(packet.command));
        if let Some(address) = packet.address {
            append_key(&mut message, 3, 0);
            append_varint(&mut message, address);
        }
        if let Some(size) = packet.size {
            append_key(&mut message, 4, 0);
            append_varint(&mut message, u64::from(size));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        append_record(&mut bytes, &message);
    }
    bytes
}

fn append_record(bytes: &mut Vec<u8>, record: &[u8]) {
    append_varint(bytes, record.len() as u64);
    bytes.extend_from_slice(record);
}

fn append_key(bytes: &mut Vec<u8>, field: u64, wire_type: u8) {
    append_varint(bytes, (field << 3) | u64::from(wire_type));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

pub(crate) fn assert_stat(stdout: &str, path: &str, unit: &str, value: u64, reset_policy: &str) {
    let sample = stat_sample(stdout, path);
    let (scope, name) = stat_scope_and_name(path);
    let scope_json = scope
        .iter()
        .map(|segment| format!("\"{segment}\""))
        .collect::<Vec<_>>()
        .join(",");

    let expected_fields = [
        format!("\"path\":\"{path}\""),
        format!("\"scope\":[{scope_json}]"),
        format!("\"name\":\"{name}\""),
        format!("\"unit\":\"{unit}\""),
        format!("\"value\":{value}"),
        format!("\"reset_policy\":\"{reset_policy}\""),
        "\"description\":null".to_string(),
    ];
    for expected in expected_fields {
        assert!(
            sample.contains(&expected),
            "missing stat field {expected} in {sample}"
        );
    }
}

pub(crate) fn assert_stat_greater_than(
    stdout: &str,
    path: &str,
    unit: &str,
    minimum: u64,
    reset_policy: &str,
) {
    let sample = stat_sample(stdout, path);
    assert!(
        sample.contains(&format!("\"unit\":\"{unit}\"")),
        "missing stat unit {unit} in {sample}"
    );
    assert!(
        sample.contains(&format!("\"reset_policy\":\"{reset_policy}\"")),
        "missing stat reset policy {reset_policy} in {sample}"
    );
    let value = stat_value(stdout, path);
    assert!(
        value > minimum,
        "expected {path} value greater than {minimum}, got {value} in {sample}"
    );
}

pub(crate) fn stat_value(stdout: &str, path: &str) -> u64 {
    let sample = stat_sample(stdout, path);
    let Some(value_tail) = sample.split("\"value\":").nth(1) else {
        panic!("missing stat value in {sample}");
    };
    let value_end = value_tail
        .find(',')
        .or_else(|| value_tail.find('}'))
        .expect("stat value terminator");
    value_tail[..value_end]
        .parse::<u64>()
        .expect("numeric stat value")
}

pub(crate) fn assert_histogram_stat(
    stdout: &str,
    path: &str,
    unit: &str,
    value: u64,
    reset_policy: &str,
    buckets: &[(u64, u64)],
) {
    let sample = stat_sample(stdout, path);
    let expected = [
        "\"kind\":\"histogram\"".to_string(),
        format!("\"unit\":\"{unit}\""),
        format!("\"value\":{value}"),
        format!("\"reset_policy\":\"{reset_policy}\""),
    ];
    for field in expected {
        assert!(
            sample.contains(&field),
            "missing stat field {field} in {sample}"
        );
    }
    for (bucket, count) in buckets {
        let expected_bucket = format!("{{\"bucket\":{bucket},\"count\":{count}}}");
        assert!(
            sample.contains(&expected_bucket),
            "missing histogram bucket {expected_bucket} in {sample}"
        );
    }
}

pub(crate) fn assert_stat_id(stdout: &str, path: &str, id: u64) {
    let sample = stat_sample(stdout, path);
    let expected = format!("\"id\":{id}");
    assert!(
        sample.contains(&expected),
        "missing stat field {expected} in {sample}"
    );
}

fn stat_sample<'a>(stdout: &'a str, path: &str) -> &'a str {
    let path_field = format!("\"path\":\"{path}\"");
    let path_index = stdout
        .find(&path_field)
        .unwrap_or_else(|| panic!("missing stat path {path} in {stdout}"));
    let sample_start = stdout[..path_index]
        .rfind('{')
        .unwrap_or_else(|| panic!("missing stat object start for {path} in {stdout}"));
    let sample_end = json_object_end(stdout, sample_start)
        .unwrap_or_else(|| panic!("missing stat object end for {path} in {stdout}"));
    &stdout[sample_start..sample_end]
}

fn json_object_end(json: &str, start: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in json[start..].bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(start + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn stat_scope_and_name(path: &str) -> (Vec<&str>, &str) {
    let mut segments = path.split('.').collect::<Vec<_>>();
    let name = segments.pop().unwrap_or(path);
    (segments, name)
}

pub(crate) fn assert_transport_stats(
    stdout: &str,
    prefix: &str,
    requests: u64,
    round_trip_ticks: u64,
    max_round_trip_ticks: u64,
) {
    assert_stat(
        stdout,
        &format!("{prefix}.requests"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.request_arrivals"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.responses"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.response_arrivals"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.round_trip_ticks"),
        "Tick",
        round_trip_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_round_trip_ticks"),
        "Tick",
        max_round_trip_ticks,
        "monotonic",
    );
}
