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

#[test]
fn rem6_run_riscv_se_runs_static_raw_clock_nanosleep_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw clock_nanosleep smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw clock_nanosleep smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-clock-nanosleep");
    let source = workspace.join("raw-clock-nanosleep.c");
    let binary = workspace.join("raw-clock-nanosleep");
    fs::write(
        &source,
        r#"#include <stdio.h>

struct timespec64 {
    long tv_sec;
    long tv_nsec;
};

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
    struct timespec64 zero = {0, 0};
    long relative = linux_syscall4(115, 0, 0, (long)&zero, 0);
    long absolute = linux_syscall4(115, 1, 1, (long)&zero, 0);
    long invalid_clock = linux_syscall4(115, 99, 0, (long)&zero, 0);
    long tai = linux_syscall4(115, 11, 0, (long)&zero, 0);
    long process_clock = linux_syscall4(115, 2, 0, (long)&zero, 0);
    long thread_clock = linux_syscall4(115, 3, 0, (long)&zero, 0);
    printf("raw-clock-nanosleep:%ld:%ld:%ld:%ld:%ld:%ld\n",
           relative, absolute, invalid_clock, tai, process_clock, thread_clock);
    return 41;
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
        Some(41),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-clock-nanosleep:0:0:-22:0:0:-95\n");

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
    assert!(stdout.contains("\"stop_code\":41"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-clock-nanosleep:0:0:-22:0:0:-95\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_clock_getres_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw clock_getres smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw clock_getres smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-clock-getres");
    let source = workspace.join("raw-clock-getres.c");
    let binary = workspace.join("raw-clock-getres");
    fs::write(
        &source,
        r#"#include <stdio.h>

struct timespec64 {
    long tv_sec;
    long tv_nsec;
};

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    struct timespec64 realtime = {-1, -1};
    struct timespec64 samples[12];
    for (int i = 0; i < 12; ++i) {
        samples[i].tv_sec = -1;
        samples[i].tv_nsec = -1;
    }
    long rt = linux_syscall2(114, 0, (long)&realtime);
    long clock1 = linux_syscall2(114, 1, (long)&samples[1]);
    long clock2 = linux_syscall2(114, 2, (long)&samples[2]);
    long clock3 = linux_syscall2(114, 3, (long)&samples[3]);
    long clock4 = linux_syscall2(114, 4, (long)&samples[4]);
    long clock5 = linux_syscall2(114, 5, (long)&samples[5]);
    long clock6 = linux_syscall2(114, 6, (long)&samples[6]);
    long clock7 = linux_syscall2(114, 7, (long)&samples[7]);
    long clock11 = linux_syscall2(114, 11, (long)&samples[11]);
    long invalid = linux_syscall2(114, 99, (long)&samples[0]);
    long null_valid = linux_syscall2(114, 0, 0);
    printf("raw-clock-getres:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld\n",
           rt, realtime.tv_sec, realtime.tv_nsec,
           clock1, samples[1].tv_sec, samples[1].tv_nsec,
           clock2, samples[2].tv_nsec,
           clock3, samples[3].tv_nsec,
           clock4, samples[4].tv_nsec,
           clock5, samples[5].tv_nsec,
           clock6, samples[6].tv_nsec,
           clock7, samples[7].tv_nsec,
           clock11, samples[11].tv_nsec,
           invalid, null_valid);
    return rt == 0 && clock1 == 0 && clock2 == 0 && clock3 == 0 &&
           clock4 == 0 && clock5 == 0 && clock6 == 0 && clock7 == 0 &&
           clock11 == 0 && invalid == -22 && null_valid == 0 ? 42 : 43;
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
        Some(42),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let expected =
        b"raw-clock-getres:0:0:1:0:0:1:0:1:0:1:0:1:0:1000000:0:1000000:0:1:0:1:-22:0:0\n";
    assert_eq!(qemu_output.stdout, expected);

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
    assert!(stdout.contains("\"stop_code\":42"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains(
        "\"text\":\"raw-clock-getres:0:0:1:0:0:1:0:1:0:1:0:1:0:1000000:0:1000000:0:1:0:1:-22:0:0\\n\""
    ));
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
