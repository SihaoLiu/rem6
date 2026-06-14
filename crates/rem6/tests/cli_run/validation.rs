use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_rejects_instruction_limit_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("instruction-limit-without-execute", &elf);

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
            "--max-instructions",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--max-instructions requires --execute"));
}

#[test]
fn rem6_run_rejects_riscv_se_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-se-without-execute", &elf);

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
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-se requires --execute"));
}

#[test]
fn rem6_run_rejects_data_cache_protocol_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("data-cache-without-execute", &elf);

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
            "--data-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--data-cache-protocol requires --execute"));
}

#[test]
fn rem6_run_rejects_instruction_cache_protocol_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("instruction-cache-without-execute", &elf);

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
            "--instruction-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--instruction-cache-protocol requires --execute"));
}

#[test]
fn rem6_run_rejects_unsupported_data_cache_protocol() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("unsupported-data-cache-protocol", &elf);

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
            "--execute",
            "--data-cache-protocol",
            "ruby",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid run data cache protocol ruby"));
}

#[test]
fn rem6_run_rejects_unsupported_instruction_cache_protocol() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("unsupported-instruction-cache-protocol", &elf);

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
            "--execute",
            "--instruction-cache-protocol",
            "ruby",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid run instruction cache protocol ruby"));
}

#[test]
fn rem6_run_rejects_unsupported_instruction_cache_protocol_from_toml_config() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let binary = temp_binary("unsupported-instruction-cache-protocol-config-bin", &elf);
    let config = temp_config(
        "unsupported-instruction-cache-protocol-config",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ninstruction_cache_protocol = \"ruby\"\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid run instruction cache protocol ruby"));
}

#[test]
fn rem6_run_rejects_data_cache_protocol_for_non_riscv_isa() {
    let elf = x86_64_elf(0x4000_0000, 0x4000_0000, &[0x90]);
    let path = temp_binary("data-cache-non-riscv", &elf);

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
            "--execute",
            "--data-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--data-cache-protocol requires --isa riscv"));
}

#[test]
fn rem6_run_rejects_instruction_cache_protocol_for_non_riscv_isa() {
    let elf = x86_64_elf(0x4000_0000, 0x4000_0000, &[0x90]);
    let path = temp_binary("instruction-cache-non-riscv", &elf);

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
            "--execute",
            "--instruction-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--instruction-cache-protocol requires --isa riscv"));
}

#[test]
fn rem6_run_rejects_data_cache_protocol_for_multiple_cores() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("data-cache-multiple-cores", &elf);

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
            "--execute",
            "--cores",
            "2",
            "--data-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--data-cache-protocol requires --cores 1, got 2"));
}

#[test]
fn rem6_run_rejects_instruction_cache_protocol_for_multiple_cores() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("instruction-cache-multiple-cores", &elf);

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
            "--execute",
            "--cores",
            "2",
            "--instruction-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--instruction-cache-protocol requires --cores 1, got 2"));
}

#[test]
fn rem6_run_rejects_riscv_se_for_non_riscv_isa() {
    let elf = x86_64_elf(0x4000_0000, 0x4000_0000, &[0x90]);
    let path = temp_binary("riscv-se-non-riscv", &elf);

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
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-se requires --isa riscv"));
}

#[test]
fn rem6_run_rejects_riscv_se_for_multiple_cores() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-se-multiple-cores", &elf);

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
            "--execute",
            "--cores",
            "2",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-se requires --cores 1, got 2"));
}

#[test]
fn rem6_run_rejects_cli_riscv_se_inputs_without_riscv_se() {
    let stdin_path = temp_binary("riscv-se-stdin-input-without-riscv-se", b"stdin");
    let file_path = temp_binary("riscv-se-file-input-without-riscv-se", b"file");
    for (name, flag, value) in [
        (
            "riscv-se-arg-without-riscv-se",
            "--riscv-se-arg",
            "A0".to_string(),
        ),
        (
            "riscv-se-env-without-riscv-se",
            "--riscv-se-env",
            "C=1".to_string(),
        ),
        (
            "riscv-se-stdin-without-riscv-se",
            "--riscv-se-stdin",
            stdin_path.display().to_string(),
        ),
        (
            "riscv-se-file-without-riscv-se",
            "--riscv-se-file",
            format!("guest.txt={}", file_path.display()),
        ),
    ] {
        let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x73, 0, 0, 0]);
        let path = temp_binary(name, &elf);

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
                "--execute",
                flag,
                value.as_str(),
            ])
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "flag {flag} unexpectedly succeeded"
        );
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(&format!("{flag} requires --riscv-se")),
            "stderr for {flag}: {stderr}"
        );
    }
}

#[test]
fn rem6_run_config_scan_treats_riscv_se_stdin_as_value_taking() {
    let bogus_config = temp_output("riscv-se-stdin-prescan-bogus-config");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--riscv-se-stdin",
            "--config",
            bogus_config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&format!("unknown flag {}", bogus_config.display())),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains(&format!("failed to read config {}", bogus_config.display())));
}

#[test]
fn rem6_run_config_scan_treats_riscv_se_file_as_value_taking() {
    let bogus_config = temp_output("riscv-se-file-prescan-bogus-config");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--riscv-se-file",
            "--config",
            bogus_config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid RISC-V SE file mapping --config"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains(&format!("failed to read config {}", bogus_config.display())));
}

#[test]
fn rem6_run_rejects_toml_riscv_se_inputs_without_riscv_se() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x73, 0, 0, 0]);
    let binary = temp_binary("riscv-se-toml-inputs-without-riscv-se", &elf);
    let stdin = temp_binary("riscv-se-toml-stdin-without-riscv-se", b"stdin");
    let file = temp_binary("riscv-se-toml-file-without-riscv-se", b"file");

    for (name, input, field) in [
        (
            "riscv-se-toml-stdin-without-riscv-se",
            "riscv_se_stdin",
            format!("riscv_se_stdin = \"{}\"\n", stdin.display()),
        ),
        (
            "riscv-se-toml-file-without-riscv-se",
            "riscv_se_files",
            format!("riscv_se_files = [\"guest.txt={}\"]\n", file.display()),
        ),
    ] {
        let config = temp_config(
            name,
            &format!(
                "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\n{}",
                binary.display(),
                field
            ),
        );

        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args(["run", "--config", config.to_str().unwrap()])
            .output()
            .unwrap();

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(&format!("{input} requires --riscv-se")),
            "stderr for {input}: {stderr}"
        );
    }
}

#[test]
fn rem6_run_rejects_zero_instruction_limit() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("zero-instruction-limit", &elf);

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
            "--execute",
            "--max-instructions",
            "0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid max instructions 0"));
}

#[test]
fn rem6_run_rejects_zero_scheduler_min_remote_delay() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("zero-min-remote-delay", &elf);

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
            "--execute",
            "--min-remote-delay",
            "0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid min remote delay 0"));
}

#[test]
fn rem6_run_rejects_zero_memory_route_delay() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("zero-memory-route-delay", &elf);

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
            "--execute",
            "--memory-route-delay",
            "0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid memory route delay 0"));
}

#[test]
fn rem6_run_rejects_unsupported_dram_memory_profile() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("unsupported-dram-memory-profile", &elf);

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
            "--dram-memory",
            "--dram-memory-profile",
            "wideio",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported DRAM memory profile wideio"));
}

#[test]
fn rem6_run_rejects_dram_memory_profile_without_dram_memory() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("dram-memory-profile-without-dram-memory", &elf);

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
            "--dram-memory-profile",
            "hbm",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--dram-memory-profile requires --dram-memory"));
}

#[test]
fn rem6_run_rejects_dram_memory_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("dram-memory-without-execute", &elf);

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
            "--dram-memory",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--dram-memory requires --execute"));
}

#[test]
fn rem6_run_rejects_zero_host_event_delay() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("zero-host-event-delay", &elf);

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
            "--execute",
            "--host-event-delay",
            "0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid host event delay 0"));
}

#[test]
fn rem6_run_rejects_invalid_start_address() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("invalid-start-address", &elf);

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
            "--execute",
            "--start-address",
            "not-an-address",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid start address not-an-address"));
}

#[test]
fn rem6_run_rejects_invalid_riscv_boot_a0() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("invalid-riscv-boot-a0", &elf);

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
            "--execute",
            "--riscv-boot-a0",
            "not-a-value",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid RISC-V boot a0 not-a-value"));
}

#[test]
fn rem6_run_rejects_invalid_riscv_boot_a1() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("invalid-riscv-boot-a1", &elf);

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
            "--execute",
            "--riscv-boot-a1",
            "not-a-value",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid RISC-V boot a1 not-a-value"));
}

#[test]
fn rem6_run_rejects_invalid_load_blob() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("invalid-load-blob", &elf);

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
            "--load-blob",
            "not-a-blob",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid load blob not-a-blob"));
}

#[test]
fn rem6_run_rejects_missing_load_blob_file() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("missing-load-blob", &elf);
    let blob_path = temp_output("missing-load-blob");

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
            "--load-blob",
            &format!("0x80001000:{}", blob_path.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read load blob {}:",
        blob_path.display()
    )));
}

#[test]
fn rem6_run_rejects_empty_load_blob_file() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("empty-load-blob", &elf);
    let blob_path = temp_binary("empty-load-blob-data", &[]);

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
            "--load-blob",
            &format!("0x80001000:{}", blob_path.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!("load blob {} is empty", blob_path.display())));
}

#[test]
fn rem6_run_rejects_load_blob_overlapping_elf_segment() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("overlapping-load-blob", &elf);
    let blob_path = temp_binary("overlapping-load-blob-data", &[0xaa, 0xbb]);

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
            "--load-blob",
            &format!("0x80000000:{}", blob_path.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("overlaps existing region"));
}

#[test]
fn rem6_run_rejects_overlapping_load_blobs() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("overlapping-load-blobs", &elf);
    let first_blob_path = temp_binary("overlapping-load-blobs-first", &[0xaa, 0xbb]);
    let second_blob_path = temp_binary("overlapping-load-blobs-second", &[0xcc, 0xdd]);

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
            "--load-blob",
            &format!("0x80001000:{}", first_blob_path.display()),
            "--load-blob",
            &format!("0x80001001:{}", second_blob_path.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("overlaps existing region"));
}

#[test]
fn rem6_run_rejects_memory_route_delay_below_scheduler_lookahead() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("short-memory-route-delay", &elf);

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
            "--execute",
            "--min-remote-delay",
            "4",
            "--memory-route-delay",
            "2",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("memory route delay 2 is below min remote delay 4"));
}

#[test]
fn rem6_run_rejects_host_event_delay_below_scheduler_lookahead() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("short-host-event-delay", &elf);

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
            "--execute",
            "--min-remote-delay",
            "4",
            "--host-event-delay",
            "2",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("host event delay 2 is below min remote delay 4"));
}
