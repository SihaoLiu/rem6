use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::support::{assert_stat, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_newlib_readlink_proc_self_exe_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE proc-self-exe smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE proc-self-exe smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-proc-self-exe")
        .canonicalize()
        .unwrap();
    let source = workspace.join("proc-self-exe.c");
    let binary = workspace.join("proc-self-exe");
    let expected_exe = binary.to_str().unwrap();
    assert!(!expected_exe.contains('\\'));
    assert!(!expected_exe.contains('"'));
    assert!(!expected_exe.contains('\n'));
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char buffer[1024] = {0};
    long len = linux_syscall4(78, -100, (long)"/proc/self/exe", (long)buffer, sizeof(buffer) - 1);
    int matches_expected_path = len > 0 && strcmp(buffer, "EXPECTED_EXE_PATH") == 0;
    printf("proc-self-exe:%d:%d\n", len > 0, matches_expected_path);
    return matches_expected_path ? 44 : 76;
}
"#
        .replace("EXPECTED_EXE_PATH", expected_exe),
    )
    .unwrap();

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

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(44),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"proc-self-exe:1:1\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
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
    assert!(stdout.contains("\"stop_code\":44"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"proc-self-exe:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 44, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prctl_name_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE prctl smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE prctl smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prctl");
    let source = workspace.join("raw-prctl.c");
    let binary = workspace.join("raw-prctl");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char name[16] = {0};
    long set_name = linux_syscall2(167, 15, (long)"rem6-prctl-name-is-long");
    long get_name = linux_syscall2(167, 16, (long)name);
    int matches = strcmp(name, "rem6-prctl-name") == 0;
    printf("prctl-name:%ld:%ld:%s:%d\n", set_name, get_name, name, matches);
    return set_name == 0 && get_name == 0 && matches ? 46 : 78;
}
"#,
    )
    .unwrap();

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

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(46),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"prctl-name:0:0:rem6-prctl-name:1\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
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
    assert!(stdout.contains("\"stop_code\":46"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"prctl-name:0:0:rem6-prctl-name:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 46, "constant");
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
