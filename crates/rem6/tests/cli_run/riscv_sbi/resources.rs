use std::{fs, process::Command};

use crate::support::*;

use super::{RISCV_SBI_ENTRY, SBI_LEGACY_CONSOLE_GETCHAR};

#[test]
fn rem6_run_riscv_sbi_console_input_reads_manifest_resource() {
    let program = riscv64_program(&[
        i_type(SBI_LEGACY_CONSOLE_GETCHAR, 0, 0x0, 17, 0x13),
        0x0000_0073,
        i_type(0, 10, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &program);
    let workspace = temp_workspace("riscv-sbi-console-input-resource");
    fs::write(workspace.join("guest.elf"), elf).unwrap();
    fs::write(workspace.join("console.bin"), b"Zresource input\n").unwrap();
    fs::write(
        workspace.join("resource-acquire.toml"),
        r#"[resource_acquire]
workload_id = "riscv-sbi-console-input-resource"
boot_entry = 2147483648
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-sbi-console-input-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "guest.elf"
artifact_digest = "sha256:riscv-sbi-console-input-kernel"

[[resource_acquire.resources]]
id = "console"
kind = "input"
digest = "sha256:riscv-sbi-console-input"
locator = "resources/console.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://console"
artifact = "console.bin"
artifact_digest = "sha256:riscv-sbi-console-input"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--resource-config",
            workspace.join("resource-acquire.toml").to_str().unwrap(),
            "--max-tick",
            "80",
            "--execute",
            "--riscv-sbi",
            "--riscv-sbi-console-input",
            "resource:console",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("\"x5\":\"0x5a\""));
    assert!(stdout.contains("\"riscv_sbi_console\":{\"bytes\":0,\"text\":\"\",\"hex\":\"\"}"));
}

#[test]
fn rem6_run_riscv_sbi_console_input_toml_path_resolves_from_config_directory() {
    let program = riscv64_program(&[
        i_type(SBI_LEGACY_CONSOLE_GETCHAR, 0, 0x0, 17, 0x13),
        0x0000_0073,
        i_type(0, 10, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &program);
    let workspace = temp_workspace("riscv-sbi-console-input-toml-path");
    fs::write(workspace.join("guest.elf"), elf).unwrap();
    fs::write(workspace.join("console.bin"), b"Tconfig input\n").unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 80\nexecute = true\nriscv_sbi = true\nriscv_sbi_console_input = \"console.bin\"\nstats_format = \"json\"\n",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x54\""));
    assert!(stdout.contains("\"riscv_sbi_console\":{\"bytes\":0,\"text\":\"\",\"hex\":\"\"}"));
}
