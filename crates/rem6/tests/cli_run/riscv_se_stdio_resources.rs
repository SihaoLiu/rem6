use std::{fs, process::Command};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_riscv_se_opens_registered_guest_file_from_host_bytes() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-registered-guest-file", &elf);
    let input = temp_binary(
        "riscv-se-registered-guest-file-input",
        b"file-backed input\nignored\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 18, "constant");
}

#[test]
fn rem6_run_riscv_se_reads_stdin_from_manifest_input_resource() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-stdin-resource");
    fs::write(workspace.join("guest.elf"), elf).unwrap();
    fs::write(workspace.join("stdin.bin"), b"Sresource stdin\n").unwrap();
    fs::write(
        workspace.join("resource-acquire.toml"),
        r#"[resource_acquire]
workload_id = "riscv-se-stdin-resource"
boot_entry = 2147483648
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-stdin-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "guest.elf"
artifact_digest = "sha256:riscv-se-stdin-kernel"

[[resource_acquire.resources]]
id = "stdin"
kind = "input"
digest = "sha256:riscv-se-stdin-input"
locator = "resources/stdin.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://stdin"
artifact = "stdin.bin"
artifact_digest = "sha256:riscv-se-stdin-input"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"resource:stdin\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            workspace.join("run.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("\"stop_code\":83"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_reads_stdin_from_selected_suite_input_resource() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-suite-stdin-resource");
    fs::write(workspace.join("guest-a.elf"), &elf).unwrap();
    fs::write(workspace.join("guest-b.elf"), &elf).unwrap();
    fs::write(workspace.join("stdin-a.bin"), b"A-suite stdin\n").unwrap();
    fs::write(workspace.join("stdin-b.bin"), b"B-suite stdin\n").unwrap();
    fs::write(
        workspace.join("resource-acquire-suite.toml"),
        r#"[resource_acquire]
suite_id = "riscv-se-suite-stdin-resource"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "stdin-a-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-suite-stdin-kernel-a"
locator = "resources/kernel-a.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-a"
artifact = "guest-a.elf"
artifact_digest = "sha256:riscv-se-suite-stdin-kernel-a"

[[resource_acquire.manifests.resources]]
id = "stdin"
kind = "input"
digest = "sha256:riscv-se-suite-stdin-a"
locator = "resources/stdin-a.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://stdin-a"
artifact = "stdin-a.bin"
artifact_digest = "sha256:riscv-se-suite-stdin-a"

[[resource_acquire.manifests]]
workload_id = "stdin-b-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-suite-stdin-kernel-b"
locator = "resources/kernel-b.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-b"
artifact = "guest-b.elf"
artifact_digest = "sha256:riscv-se-suite-stdin-kernel-b"

[[resource_acquire.manifests.resources]]
id = "stdin"
kind = "input"
digest = "sha256:riscv-se-suite-stdin-b"
locator = "resources/stdin-b.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://stdin-b"
artifact = "stdin-b.bin"
artifact_digest = "sha256:riscv-se-suite-stdin-b"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nkernel_resource = \"suite-resource:stdin-b-workload/kernel\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"suite-resource:stdin-b-workload/stdin\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            workspace.join("run.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.get("kernel_resource").and_then(Value::as_str),
        Some("suite-resource:stdin-b-workload/kernel")
    );
    assert_eq!(
        json.pointer("/riscv_se_inputs/stdin/source")
            .and_then(Value::as_str),
        Some("suite-resource:stdin-b-workload/stdin")
    );
    assert_eq!(
        json.pointer("/riscv_se_inputs/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":66"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_opens_guest_file_from_suite_input_resource() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-suite-guest-file-resource");
    fs::write(workspace.join("guest.elf"), elf).unwrap();
    fs::write(workspace.join("input.txt"), b"file-backed input\nignored\n").unwrap();
    fs::write(
        workspace.join("resource-acquire-suite.toml"),
        r#"[resource_acquire]
suite_id = "riscv-se-suite-resource"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-suite-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "guest.elf"
artifact_digest = "sha256:riscv-se-suite-kernel"

[[resource_acquire.manifests.resources]]
id = "input"
kind = "input"
digest = "sha256:riscv-se-suite-input"
locator = "resources/input.txt"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input"
artifact = "input.txt"
artifact_digest = "sha256:riscv-se-suite-input"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=suite-resource:boot-workload/input\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            workspace.join("run.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/riscv_se_inputs/files/0/guest_path")
            .and_then(Value::as_str),
        Some("guest.txt")
    );
    assert_eq!(
        json.pointer("/riscv_se_inputs/files/0/source")
            .and_then(Value::as_str),
        Some("suite-resource:boot-workload/input")
    );
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_toml_guest_file_resolves_from_config_directory() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-relative-guest-file");
    let binary = workspace.join("guest.elf");
    let input = workspace.join("input.txt");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=input.txt\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_toml_guest_file_accepts_explicit_path_prefix() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-prefixed-guest-file");
    let binary = workspace.join("guest.elf");
    let prefixed_dir = workspace.join("suite-resource:boot-workload");
    let input = prefixed_dir.join("input");
    let config = workspace.join("run.toml");
    fs::create_dir_all(&prefixed_dir).unwrap();
    fs::write(&binary, elf).unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=path:suite-resource:boot-workload/input\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_reports_missing_stdin_file() {
    let program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-missing-stdin", &elf);
    let stdin = temp_output("riscv-se-missing-stdin-input");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-stdin",
            stdin.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read RISC-V SE stdin {}",
        stdin.display()
    )));
}

#[test]
fn rem6_run_riscv_se_reports_missing_guest_file() {
    let program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-missing-guest-file", &elf);
    let input = temp_output("riscv-se-missing-guest-file-input");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read RISC-V SE file guest.txt from {}",
        input.display()
    )));
}

#[test]
fn rem6_run_riscv_se_toml_stdin_path_resolves_from_config_directory() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-relative-stdin");
    let binary = workspace.join("guest.elf");
    let stdin = workspace.join("stdin.txt");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&stdin, b"relative stdin\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"stdin.txt\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":114"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_toml_stdin_accepts_explicit_path_prefix() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-prefixed-stdin");
    let binary = workspace.join("guest.elf");
    let stdin = workspace.join("resource:stdin");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&stdin, b"path-prefixed stdin\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"path:resource:stdin\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":112"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

fn registered_guest_file_program() -> Vec<u8> {
    const PATH_OFFSET: usize = 0x80;
    const BUFFER_OFFSET: usize = 0xa0;

    let mut program = riscv64_program(&[
        i_type(-100, 0, 0x0, 10, 0x13), // addi a0, x0, AT_FDCWD
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(PATH_OFFSET as i32 - 4, 11, 0x0, 11, 0x13), // addi a1, a1, path
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, O_RDONLY
        i_type(0, 0, 0x0, 13, 0x13),    // addi a3, x0, 0
        i_type(56, 0, 0x0, 17, 0x13),   // addi a7, x0, 56
        0x0000_0073,                    // ecall
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(BUFFER_OFFSET as i32 - 28, 11, 0x0, 11, 0x13), // addi a1, a1, buffer
        i_type(18, 0, 0x0, 12, 0x13),   // addi a2, x0, 18
        i_type(63, 0, 0x0, 17, 0x13),   // addi a7, x0, 63
        0x0000_0073,                    // ecall
        i_type(1, 0, 0x0, 10, 0x13),    // addi a0, x0, 1
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(BUFFER_OFFSET as i32 - 52, 11, 0x0, 11, 0x13), // addi a1, a1, buffer
        i_type(18, 0, 0x0, 12, 0x13),   // addi a2, x0, 18
        i_type(64, 0, 0x0, 17, 0x13),   // addi a7, x0, 64
        0x0000_0073,                    // ecall
        i_type(18, 0, 0x0, 10, 0x13),   // addi a0, x0, 18
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, 93
        0x0000_0073,                    // ecall
    ]);
    program.resize(PATH_OFFSET, 0);
    program.extend_from_slice(b"guest.txt\0");
    program.resize(BUFFER_OFFSET + 32, 0);
    program
}
