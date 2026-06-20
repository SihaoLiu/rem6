use std::{fs, process::Command};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_raw_splice_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw splice smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw splice smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-splice");
    let source = workspace.join("raw-splice.c");
    let binary = workspace.join("raw-splice");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_RDWR 02
#define O_CREAT 0100
#define O_TRUNC 01000
#define SEEK_SET 0

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

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

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall6(long number, long arg0, long arg1, long arg2,
                           long arg3, long arg4, long arg5) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a5 asm("a5") = arg5;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0)
                  : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a7)
                  : "memory");
    return a0;
}

static int same_bytes(const char *left, const char *right, long length) {
    for (long i = 0; i < length; ++i) {
        if (left[i] != right[i]) {
            return 0;
        }
    }
    return 1;
}

static void write_stdout(const char *text, long length) {
    linux_syscall3(64, 1, (long)text, length);
}

int main(void) {
    int pipe_a[2] = {-1, -1};
    int pipe_b[2] = {-1, -1};
    char first[8] = {0};
    char second[8] = {0};
    long off = 2;

    long fd_in = linux_syscall4(56, AT_FDCWD, (long)"input.txt",
                                O_RDWR | O_CREAT | O_TRUNC, 0600);
    long seed = fd_in >= 0 ? linux_syscall3(64, fd_in, (long)"abcdef", 6) : -99;
    long rewind_in = fd_in >= 0 ? linux_syscall3(62, fd_in, 0, SEEK_SET) : -99;
    long pipe_a_status = linux_syscall2(59, (long)pipe_a, 0);
    long file_to_pipe = pipe_a_status == 0 ?
        linux_syscall6(76, fd_in, (long)&off, pipe_a[1], 0, 3, 0) : -99;
    long read_first = pipe_a_status == 0 ? linux_syscall3(63, pipe_a[0], (long)first, 8) : -99;

    long fd_out = linux_syscall4(56, AT_FDCWD, (long)"output.txt",
                                 O_RDWR | O_CREAT | O_TRUNC, 0600);
    long pipe_b_status = linux_syscall2(59, (long)pipe_b, 0);
    long pipe_seed = pipe_b_status == 0 ? linux_syscall3(64, pipe_b[1], (long)"pipe-data", 9) : -99;
    long pipe_to_file = pipe_b_status == 0 && fd_out >= 0 ?
        linux_syscall6(76, pipe_b[0], 0, fd_out, 0, 4, 0) : -99;
    long rewind_out = fd_out >= 0 ? linux_syscall3(62, fd_out, 0, SEEK_SET) : -99;
    long read_second = fd_out >= 0 ? linux_syscall3(63, fd_out, (long)second, 8) : -99;

    long file_to_file = fd_in >= 0 && fd_out >= 0 ?
        linux_syscall6(76, fd_in, 0, fd_out, 0, 1, 0) : -99;

    int ok = fd_in >= 0 &&
             seed == 6 &&
             rewind_in == 0 &&
             pipe_a_status == 0 &&
             file_to_pipe == 3 &&
             off == 5 &&
             read_first == 3 &&
             same_bytes(first, "cde", 3) &&
             fd_out >= 0 &&
             pipe_b_status == 0 &&
             pipe_seed == 9 &&
             pipe_to_file == 4 &&
             rewind_out == 0 &&
             read_second == 4 &&
             same_bytes(second, "pipe", 4) &&
             file_to_file == -22;

    if (fd_in >= 0) linux_syscall1(57, fd_in);
    if (fd_out >= 0) linux_syscall1(57, fd_out);
    if (pipe_a_status == 0) {
        linux_syscall1(57, pipe_a[0]);
        linux_syscall1(57, pipe_a[1]);
    }
    if (pipe_b_status == 0) {
        linux_syscall1(57, pipe_b[0]);
        linux_syscall1(57, pipe_b[1]);
    }

    if (ok) {
        write_stdout("raw-splice:ok\n", sizeof("raw-splice:ok\n") - 1);
    } else {
        write_stdout("raw-splice:fail\n", sizeof("raw-splice:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 46 : 87);
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
        Some(46),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-splice:ok\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "500000",
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
    assert!(stdout.contains("\"stop_code\":46"));
    assert!(stdout.contains("\"text\":\"raw-splice:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 46, "constant");
}
