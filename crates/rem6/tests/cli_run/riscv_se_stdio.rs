use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_newlib_fgets_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE stdin smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE stdin smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-fgets");
    let source = workspace.join("stdin.c");
    let binary = workspace.join("stdin");
    let input = workspace.join("stdin.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
int main(void) {
    char buffer[32];
    if (fgets(buffer, sizeof(buffer), stdin) == NULL) {
        return 71;
    }
    printf("stdin:%s", buffer);
    return buffer[0] == 'r' ? 23 : 24;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"rem6 stdin\nignored\n").unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .stdin(Stdio::from(fs::File::open(&input).unwrap()))
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(23),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"stdin:rem6 stdin\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "200000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-stdin",
            input.to_str().unwrap(),
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
    assert!(stdout.contains("\"stop_code\":23"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"stdin:rem6 stdin\\n\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 23, "constant");
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

fn find_riscv_tool(name: &str) -> Option<PathBuf> {
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
