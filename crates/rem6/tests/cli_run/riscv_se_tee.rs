use std::{fs, process::Command};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_raw_tee_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw tee smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw tee smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-tee");
    let source = workspace.join("raw-tee.c");
    let binary = workspace.join("raw-tee");
    fs::write(
        &source,
        r#"static long linux_syscall1(long number, long arg0) {
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
    asm volatile ("ecall" : "+r"(a0)
                  : "r"(a1), "r"(a2), "r"(a3), "r"(a7)
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
    char target[8] = {0};
    char source[8] = {0};

    long pipe_a_status = linux_syscall2(59, (long)pipe_a, 0);
    long pipe_b_status = linux_syscall2(59, (long)pipe_b, 0);
    long seed = pipe_a_status == 0 ? linux_syscall3(64, pipe_a[1], (long)"tee-data", 8) : -99;
    long duplicated = pipe_a_status == 0 && pipe_b_status == 0 ?
        linux_syscall4(77, pipe_a[0], pipe_b[1], 8, 0) : -99;
    long same_pipe = pipe_a_status == 0 ?
        linux_syscall4(77, pipe_a[0], pipe_a[1], 1, 0) : -99;
    long read_target = pipe_b_status == 0 ? linux_syscall3(63, pipe_b[0], (long)target, 8) : -99;
    long read_source = pipe_a_status == 0 ? linux_syscall3(63, pipe_a[0], (long)source, 8) : -99;
    long zero = linux_syscall4(77, 99, 98, 0, 0);
    long bad_flags = pipe_a_status == 0 && pipe_b_status == 0 ?
        linux_syscall4(77, pipe_a[0], pipe_b[1], 1, 0x100) : -99;
    long nonblock_empty = pipe_a_status == 0 && pipe_b_status == 0 ?
        linux_syscall4(77, pipe_a[0], pipe_b[1], 1, 0x02) : -99;

    int ok = pipe_a_status == 0 &&
             pipe_b_status == 0 &&
             seed == 8 &&
             duplicated == 8 &&
             same_pipe == -22 &&
             read_target == 8 &&
             same_bytes(target, "tee-data", 8) &&
             read_source == 8 &&
             same_bytes(source, "tee-data", 8) &&
             zero == 0 &&
             bad_flags == -22 &&
             nonblock_empty == -11;

    if (pipe_a_status == 0) {
        linux_syscall1(57, pipe_a[0]);
        linux_syscall1(57, pipe_a[1]);
    }
    if (pipe_b_status == 0) {
        linux_syscall1(57, pipe_b[0]);
        linux_syscall1(57, pipe_b[1]);
    }

    if (ok) {
        write_stdout("raw-tee:ok\n", sizeof("raw-tee:ok\n") - 1);
    } else {
        write_stdout("raw-tee:fail\n", sizeof("raw-tee:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 49 : 87);
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

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(49),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-tee:ok\n");

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
    assert!(stdout.contains("\"stop_code\":49"));
    assert!(stdout.contains("\"text\":\"raw-tee:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 49, "constant");
}
