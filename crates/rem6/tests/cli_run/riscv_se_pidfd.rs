use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_pidfd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw pidfd smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw pidfd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-pidfd");
    let source = workspace.join("raw-pidfd.c");
    let binary = workspace.join("raw-pidfd");
    fs::write(
        &source,
        r#"#define PIDFD_NONBLOCK 0x800

static const char ok_message[] = "pidfd:ok\n";
static char bad_message[] = "pidfd:bad:?\n";

static long linux_syscall0(long number) {
    register long a0 asm("a0");
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "=r"(a0) : "r"(a7) : "memory");
    return a0;
}

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

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a7) : "memory");
    return a0;
}

__attribute__((noreturn)) static void linux_exit(long code) {
    linux_syscall1(93, code);
    for (;;) {
    }
}

__attribute__((noreturn)) static void fail(char code) {
    bad_message[10] = code;
    linux_syscall4(64, 1, (long)bad_message, sizeof(bad_message) - 1, 0);
    linux_exit(83);
}

void _start(void) {
    long pid = linux_syscall0(172);
    long setpgid_status = linux_syscall2(154, 0, 0);
    long pgid = linux_syscall1(155, 0);
    long fd = pid > 0 ? linux_syscall2(434, pid, PIDFD_NONBLOCK) : -99;
    long fd_flags = fd >= 0 ? linux_syscall4(25, fd, 1, 0, 0) : fd;
    long status_flags = fd >= 0 ? linux_syscall4(25, fd, 3, 0, 0) : fd;
    long probe = fd >= 0 ? linux_syscall4(424, fd, 0, 0, 0) : fd;
    long scope_probe = fd >= 0 ? linux_syscall4(424, fd, 0, 0, 4) : fd;
    long bad_scope_flags = fd >= 0 ? linux_syscall4(424, fd, 0, 0, 3) : fd;
    long bad_signal = fd >= 0 ? linux_syscall4(424, fd, 65, 0, 0) : fd;
    long dupfd = fd >= 0 ? linux_syscall4(438, fd, fd, 0, 0) : fd;
    long dup_fd_flags = dupfd >= 0 ? linux_syscall4(25, dupfd, 1, 0, 0) : dupfd;
    long dup_status_flags = dupfd >= 0 ? linux_syscall4(25, dupfd, 3, 0, 0) : dupfd;
    long bad_getfd_flags = fd >= 0 ? linux_syscall4(438, fd, fd, 1, 0) : fd;
    long bad_getfd_target = fd >= 0 ? linux_syscall4(438, fd, 99, 0, 0) : fd;
    long high_dupfd = fd >= 0 ? linux_syscall4(438,
        (1L << 32) | fd, (1L << 32) | fd, 1L << 32, 0) : fd;
    long close_original = fd >= 0 ? linux_syscall1(57, fd) : fd;
    long probe_duplicate = dupfd >= 0 ? linux_syscall4(424, dupfd, 0, 0, 0) : dupfd;
    long close_duplicate = dupfd >= 0 ? linux_syscall1(57, dupfd) : dupfd;
    long close_high_duplicate = high_dupfd >= 0 ? linux_syscall1(57, high_dupfd) : high_dupfd;
    long after_close = dupfd >= 0 ? linux_syscall4(424, dupfd, 0, 0, 0) : dupfd;
    if (!(pid > 0)) fail('a');
    if (!(setpgid_status == 0 || setpgid_status == -1)) fail('b');
    if (!(pgid == pid)) fail('u');
    if (!(fd >= 0)) fail('c');
    if (!(fd_flags == 1)) fail('d');
    if (!(status_flags == 0x8802)) fail('e');
    if (!(probe == 0)) fail('f');
    if (!(scope_probe == 0)) fail('g');
    if (!(bad_scope_flags == -22)) fail('h');
    if (!(bad_signal == -22)) fail('i');
    if (!(dupfd >= 0)) fail('j');
    if (!(dup_fd_flags == 1)) fail('k');
    if (!(dup_status_flags == 0x8802)) fail('l');
    if (!(bad_getfd_flags == -22)) fail('m');
    if (!(bad_getfd_target == -9)) fail('n');
    if (!(high_dupfd >= 0)) fail('o');
    if (!(close_original == 0)) fail('p');
    if (!(probe_duplicate == 0)) fail('q');
    if (!(close_duplicate == 0)) fail('r');
    if (!(close_high_duplicate == 0)) fail('s');
    if (!(after_close == -9)) fail('t');
    linux_syscall4(64, 1, (long)ok_message, sizeof(ok_message) - 1, 0);
    linux_exit(59);
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O2",
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
        Some(59),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"pidfd:ok\n");

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
    assert!(
        stdout.contains("\"status\":\"stopped_by_host\""),
        "rem6 stdout: {stdout}"
    );
    assert!(stdout.contains("\"stop_code\":59"), "rem6 stdout: {stdout}");
    assert!(
        stdout.contains("\"text\":\"pidfd:ok\\n\""),
        "rem6 stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"riscv_unknown_syscalls\":[]"),
        "rem6 stdout: {stdout}"
    );
}
