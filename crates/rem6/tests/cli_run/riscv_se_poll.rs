use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_ppoll_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE ppoll smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE ppoll smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-ppoll");
    let source = workspace.join("raw-ppoll.c");
    let binary = workspace.join("raw-ppoll");
    fs::write(
        &source,
        r#"#define POLLIN 0x0001

struct pollfd {
    int fd;
    short events;
    short revents;
};

struct timespec {
    long tv_sec;
    long tv_nsec;
};

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

static long linux_syscall5(long number, long arg0, long arg1, long arg2,
                           long arg3, long arg4) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a4), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    const char *ok = "raw-ppoll:ok\n";
    const char *fail = "raw-ppoll:fail\n";
    struct timespec invalid_timeout = {0, 1000000000L};
    long invalid = linux_syscall5(73, 0, 0, (long)&invalid_timeout, 0, 0);
    struct timespec zero_timeout = {0, 0};
    char sigmask[8] = {0};
    long invalid_sigset = linux_syscall5(73, 0, 0, (long)&zero_timeout,
                                         (long)sigmask, 4);
    struct timespec finite_timeout = {0, 1};
    long timed_out = linux_syscall5(73, 0, 0, (long)&finite_timeout, 0, 0);
    int fds[2] = {-1, -1};
    long pipe_status = linux_syscall2(59, (long)fds, 0);
    long written = pipe_status == 0 ? linux_syscall3(64, fds[1], (long)"x", 1) : -1;
    struct pollfd poll_fd = {fds[0], POLLIN, 0};
    long ready = pipe_status == 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                                   (long)&zero_timeout,
                                                   (long)sigmask, 8) : -1;
    if (pipe_status == 0) {
        linux_syscall1(57, fds[0]);
        linux_syscall1(57, fds[1]);
    }

    if (invalid == -22 && invalid_sigset == -22 && timed_out == 0 &&
        pipe_status == 0 && written == 1 &&
        ready == 1 && (poll_fd.revents & POLLIN) != 0) {
        linux_syscall3(64, 1, (long)ok, 13);
        linux_syscall1(93, 72);
    }
    linux_syscall3(64, 1, (long)fail, 15);
    linux_syscall1(93, 88);
    return 0;
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
        Some(72),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-ppoll:ok\n");

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
    assert!(stdout.contains("\"stop_code\":72"));
    assert!(stdout.contains("\"text\":\"raw-ppoll:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
