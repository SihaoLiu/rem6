use std::{fs, process::Command};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_raw_fadvise64_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw fadvise64 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw fadvise64 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-fadvise64");
    let source = workspace.join("raw-fadvise64.c");
    let binary = workspace.join("raw-fadvise64");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define O_RDWR 02
#define O_CREAT 0100
#define O_TRUNC 01000
#define POSIX_FADV_NORMAL 0
#define POSIX_FADV_WILLNEED 3

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
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

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static void write_stdout(const char *text, long length) {
    linux_syscall3(64, 1, (long)text, length);
}

int main(void) {
    long fd = linux_syscall4(56, AT_FDCWD, (long)"advise.bin",
                             O_RDWR | O_CREAT | O_TRUNC, 0666);
    long write_status = fd >= 0 ? linux_syscall3(64, fd, (long)"advise\n", 7) : -99;
    long normal = fd >= 0 ? linux_syscall4(223, fd, 0, 4096, POSIX_FADV_NORMAL) : -99;
    long willneed = fd >= 0 ? linux_syscall4(223, fd, 0, 4096, POSIX_FADV_WILLNEED) : -99;
    long bad_advice = fd >= 0 ? linux_syscall4(223, fd, 0, 4096, 6) : -99;
    long negative_len = fd >= 0 ? linux_syscall4(223, fd, 0, -1L, POSIX_FADV_NORMAL) : -99;
    long bad_fd = linux_syscall4(223, 99, 0, 4096, POSIX_FADV_NORMAL);
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    int pipefd[2] = {-1, -1};
    long pipe_status = linux_syscall4(59, (long)pipefd, 0, 0, 0);
    long pipe_advice = pipe_status == 0 ?
                       linux_syscall4(223, pipefd[0], 0, 4096, POSIX_FADV_NORMAL) : -99;
    long close_pipe_read = pipe_status == 0 ? linux_syscall1(57, pipefd[0]) : -99;
    long close_pipe_write = pipe_status == 0 ? linux_syscall1(57, pipefd[1]) : -99;

    int ok = fd >= 0 &&
             write_status == 7 &&
             normal == 0 &&
             willneed == 0 &&
             bad_advice == -22 &&
             bad_fd == -9 &&
             negative_len == -22 &&
             close_status == 0 &&
             pipe_status == 0 &&
             pipe_advice == -29 &&
             close_pipe_read == 0 &&
             close_pipe_write == 0;
    if (ok) {
        write_stdout("raw-fadvise64:ok\n", sizeof("raw-fadvise64:ok\n") - 1);
    } else {
        write_stdout("raw-fadvise64:fail\n", sizeof("raw-fadvise64:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 48 : 88);
    __builtin_unreachable();
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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .current_dir(&workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(48),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-fadvise64:ok\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "350000",
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
    assert!(stdout.contains("\"text\":\"raw-fadvise64:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 48, "constant");
}
