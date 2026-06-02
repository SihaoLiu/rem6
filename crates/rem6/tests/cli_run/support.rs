use std::fs;

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
    let sample_end = stdout[path_index..]
        .find('}')
        .map(|offset| path_index + offset + 1)
        .unwrap_or_else(|| panic!("missing stat object end for {path} in {stdout}"));
    &stdout[sample_start..sample_end]
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
