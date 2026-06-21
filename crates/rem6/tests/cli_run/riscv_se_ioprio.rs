use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_ioprio_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw ioprio smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw ioprio smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-ioprio");
    let source = workspace.join("raw-ioprio.c");
    let binary = workspace.join("raw-ioprio");
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

static void write_message(const char *message, long bytes) {
    linux_syscall3(64, 1, (long)message, bytes);
}

__attribute__((noreturn)) static void linux_exit(long code) {
    linux_syscall1(93, code);
    for (;;) {
    }
}

void _start(void) {
    const char *ok = "raw-ioprio:ok\n";
    const char *fail = "raw-ioprio:fail\n";
    long initial = linux_syscall2(31, 1, 0);
    long set_idle = linux_syscall3(30, 1, 0, (3L << 13));
    long after_idle = linux_syscall2(31, 1, 0);
    long set_be = linux_syscall3(30, 1, 0, (2L << 13) | 5);
    long after_be = linux_syscall2(31, 1, 0);
    long set_high_bits = linux_syscall3(30, 1, 0, (1L << 40) | (1L << 16) | (2L << 13) | 5);
    long after_high_bits = linux_syscall2(31, 1, 0);
    long bad_which_get = linux_syscall2(31, 99, 0);
    long bad_which_set = linux_syscall3(30, 99, 0, 0);
    long bad_class = linux_syscall3(30, 1, 0, (4L << 13));
    long bad_class_unknown_who = linux_syscall3(30, 1, 999999, (4L << 13));
    long none_with_data = linux_syscall3(30, 1, 0, 5);
    long realtime = linux_syscall3(30, 1, 0, (1L << 13));
    long unknown_who = linux_syscall2(31, 1, 999999);
    int passed = initial == 0 && set_idle == 0 && after_idle == (3L << 13) &&
                 set_be == 0 && after_be == ((2L << 13) | 5) &&
                 set_high_bits == 0 && after_high_bits == ((2L << 13) | 5) &&
                 bad_which_get == -22 && bad_which_set == -22 &&
                 bad_class == -22 && bad_class_unknown_who == -22 &&
                 none_with_data == -22 && realtime == -1 && unknown_who == -3;
    if (passed) {
        write_message(ok, 14);
        linux_exit(74);
    }
    write_message(fail, 16);
    linux_exit(75);
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-nostdlib",
            "-fno-builtin",
            "-fno-stack-protector",
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
        Some(74),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-ioprio:ok\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "2000",
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
    assert!(stdout.contains("\"stop_code\":74"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-ioprio:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
