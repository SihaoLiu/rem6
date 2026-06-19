use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

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
fn rem6_run_riscv_se_runs_static_raw_clock_gettime64_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw clock_gettime64 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw clock_gettime64 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-clock-gettime64");
    let source = workspace.join("raw-clock-gettime64.c");
    let binary = workspace.join("raw-clock-gettime64");
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
    struct timespec64 ts = {-1, -1};
    long ret = linux_syscall2(403, 0, (long)&ts);
    int valid = ret == 0 && ts.tv_sec >= 0 && ts.tv_nsec >= 0 && ts.tv_nsec < 1000000000L;

    printf("raw-clock-gettime64:%ld:%d:%d\n", ret, ts.tv_sec >= 0, ts.tv_nsec >= 0 && ts.tv_nsec < 1000000000L);
    return valid ? 51 : 83;
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
    if qemu_output.status.code() == Some(51) {
        assert_eq!(qemu_output.stdout, b"raw-clock-gettime64:0:1:1\n");
    } else {
        assert_eq!(
            qemu_output.status.code(),
            Some(83),
            "qemu stdout: {}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stdout),
            String::from_utf8_lossy(&qemu_output.stderr)
        );
        assert_eq!(qemu_output.stdout, b"raw-clock-gettime64:-38:0:0\n");
        eprintln!("qemu-riscv64 reports ENOSYS for raw clock_gettime64; checking rem6 SE coverage");
    }

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
    assert!(stdout.contains("\"stop_code\":51"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-clock-gettime64:0:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_legacy_time_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE legacy time smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE legacy time smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-legacy-time");
    let source = workspace.join("raw-legacy-time.c");
    let binary = workspace.join("raw-legacy-time");
    fs::write(
        &source,
        r#"#include <stdio.h>

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long slot = -1;
    long ret = linux_syscall1(1062, (long)&slot);
    long null_ret = linux_syscall1(1062, 0);
    long fault = linux_syscall1(1062, 1);
    int ok = ret == 0 && slot == 0 && null_ret == 0 && fault == -14;

    printf("raw-time:%ld:%ld:%ld:%ld:%d\n", ret, slot, null_ret, fault, ok);
    return ok ? 50 : 82;
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
    let qemu_stdout = String::from_utf8_lossy(&qemu_output.stdout);
    let qemu_matches_rem6_zero =
        qemu_output.status.code() == Some(50) && qemu_stdout == "raw-time:0:0:0:-14:1\n";
    let qemu_reports_enosys =
        qemu_output.status.code() == Some(82) && qemu_stdout == "raw-time:-38:-1:-38:-38:0\n";
    if qemu_reports_enosys {
        eprintln!("qemu-riscv64 reports ENOSYS for raw legacy time; checking rem6 SE coverage");
    } else if !qemu_matches_rem6_zero {
        let fields = qemu_stdout.trim_end().split(':').collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            6,
            "qemu stdout: {qemu_stdout}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stderr)
        );
        assert_eq!(fields[0], "raw-time");
        let ret = fields[1].parse::<i64>().expect("ret field");
        let slot = fields[2].parse::<i64>().expect("slot field");
        let null_ret = fields[3].parse::<i64>().expect("null ret field");
        let fault = fields[4].parse::<i64>().expect("fault field");
        assert!(
            ret >= 0 && slot == ret && null_ret >= ret && fault == -14,
            "qemu stdout: {qemu_stdout}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stderr)
        );
        assert_eq!(
            qemu_output.status.code(),
            Some(82),
            "qemu stdout: {}; qemu stderr: {}",
            qemu_stdout,
            String::from_utf8_lossy(&qemu_output.stderr)
        );
    }

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
    assert!(stdout.contains("\"stop_code\":50"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-time:0:0:0:-14:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_gettimeofday_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw gettimeofday smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw gettimeofday smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-gettimeofday");
    let source = workspace.join("raw-gettimeofday.c");
    let binary = workspace.join("raw-gettimeofday");
    fs::write(
        &source,
        r#"#include <stdio.h>

struct timeval64 {
    long tv_sec;
    long tv_usec;
};

struct timezone32 {
    int tz_minuteswest;
    int tz_dsttime;
};

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    struct timeval64 tv = {-1, -1};
    struct timezone32 tz = {-1, -1};
    long with_tv = linux_syscall2(169, (long)&tv, 0);
    long null_tv = linux_syscall2(169, 0, 0);
    long with_tz = linux_syscall2(169, 0, (long)&tz);
    long bad_tv = linux_syscall2(169, 1, 0);
    int tv_valid = tv.tv_sec >= 0 && tv.tv_usec >= 0 && tv.tv_usec < 1000000;
    int tz_written = tz.tz_minuteswest != -1 && tz.tz_dsttime != -1;
    printf("raw-gettimeofday:%ld:%d:%ld:%ld:%d:%ld\n",
           with_tv, tv_valid, null_tv, with_tz, tz_written, bad_tv);
    return 49;
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
        Some(49),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-gettimeofday:0:1:0:0:1:-14\n");

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
    assert!(stdout.contains("\"stop_code\":49"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(
        stdout.contains("\"text\":\"raw-gettimeofday:0:1:0:0:1:-14\\n\""),
        "{stdout}"
    );
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

#[test]
fn rem6_run_riscv_se_runs_static_raw_interval_timers_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw interval timer smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw interval timer smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-interval-timers");
    let source = workspace.join("raw-interval-timers.c");
    let binary = workspace.join("raw-interval-timers");
    fs::write(
        &source,
        r#"#include <stdio.h>

struct timeval64 {
    long tv_sec;
    long tv_usec;
};

struct itimerval64 {
    struct timeval64 it_interval;
    struct timeval64 it_value;
};

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    struct itimerval64 initial = {{-1, -1}, {-1, -1}};
    struct itimerval64 old_value = {{-1, -1}, {-1, -1}};
    struct itimerval64 zero = {{0, 0}, {0, 0}};
    struct itimerval64 invalid_usec = {{0, 1000000}, {0, 0}};

    long get_initial = linux_syscall2(102, 0, (long)&initial);
    long set_zero = linux_syscall3(103, 1, (long)&zero, (long)&old_value);
    long invalid_which = linux_syscall3(103, 99, (long)&zero, 0);
    long invalid_time = linux_syscall3(103, 0, (long)&invalid_usec, 0);

    int initial_zero = initial.it_interval.tv_sec == 0 &&
        initial.it_interval.tv_usec == 0 &&
        initial.it_value.tv_sec == 0 &&
        initial.it_value.tv_usec == 0;
    int old_zero = old_value.it_interval.tv_sec == 0 &&
        old_value.it_interval.tv_usec == 0 &&
        old_value.it_value.tv_sec == 0 &&
        old_value.it_value.tv_usec == 0;
    int ok = get_initial == 0 &&
        set_zero == 0 &&
        invalid_which == -22 &&
        invalid_time == -22 &&
        initial_zero &&
        old_zero;

    printf("raw-interval-timers:%ld:%ld:%ld:%ld:%d:%d:%d\n",
           get_initial, set_zero, invalid_which, invalid_time,
           initial_zero, old_zero, ok);
    return ok ? 48 : 80;
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
        Some(48),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"raw-interval-timers:0:0:-22:-22:1:1:1\n"
    );

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
    assert!(stdout.contains("\"stop_code\":48"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-interval-timers:0:0:-22:-22:1:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
