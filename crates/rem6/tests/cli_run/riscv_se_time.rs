use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::support::temp_workspace;

#[test]
fn rem6_run_riscv_se_runs_static_newlib_times_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE times smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE times smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-times");
    let source = workspace.join("times.c");
    let binary = workspace.join("times");
    fs::write(
        &source,
        r#"#include <errno.h>
#include <stdio.h>
#include <sys/times.h>

int main(void) {
    const clock_t sentinel = (clock_t)-1;
    struct tms sample = {sentinel, sentinel, sentinel, sentinel};
    errno = 0;
    clock_t elapsed = times(&sample);
    if (elapsed == (clock_t)-1) {
        printf("times:fail:%d\n", errno);
        return 70;
    }
    printf("times:ok:%d:%d:%d:%d\n",
           sample.tms_utime != sentinel,
           sample.tms_stime != sentinel,
           sample.tms_cutime != sentinel,
           sample.tms_cstime != sentinel);
    return 38;
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
        Some(38),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"times:ok:1:1:1:1\n");

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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":38"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"times:ok:1:1:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_times_syscall_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw times smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw times smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-times");
    let source = workspace.join("raw-times.c");
    let binary = workspace.join("raw-times");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/times.h>

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

int main(void) {
    const clock_t sentinel = (clock_t)-1;
    struct tms sample = {sentinel, sentinel, sentinel, sentinel};
    long elapsed = linux_syscall1(153, (long)&sample);
    if (elapsed < 0) {
        printf("raw-times:fail:%ld\n", elapsed);
        return 71;
    }
    printf("raw-times:ok:%d:%d:%d:%d\n",
           sample.tms_utime != sentinel,
           sample.tms_stime != sentinel,
           sample.tms_cutime != sentinel,
           sample.tms_cstime != sentinel);
    return 39;
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
        Some(39),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-times:ok:1:1:1:1\n");

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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":39"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-times:ok:1:1:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
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
