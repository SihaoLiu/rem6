use std::fs;
use std::process::Command;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn riscv64_elf(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
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

fn temp_binary(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.elf", std::process::id()));
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn rem6_run_loads_riscv_elf_and_emits_json_stats_artifact() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-run", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.run.v1\""));
    assert!(stdout.contains("\"isa\":\"riscv\""));
    assert!(stdout.contains("\"architecture\":\"riscv64\""));
    assert!(stdout.contains("\"entry\":\"0x80000000\""));
    assert!(stdout.contains("\"status\":\"loaded\""));
    assert!(stdout.contains("\"path\":\"sim.binary.bytes\""));
    assert!(stdout.contains("\"path\":\"sim.elf.load_segments\""));
    assert!(stdout.contains("\"path\":\"sim.max_tick\""));
    assert!(stdout.contains("\"reset_policy\":\"constant\""));
}

#[test]
fn rem6_run_rejects_isa_mismatch_before_emitting_stats() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("isa-mismatch", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "x86",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("requested ISA x86 does not match ELF architecture riscv64"));
}
