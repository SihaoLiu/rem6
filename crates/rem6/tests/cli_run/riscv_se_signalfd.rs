use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_signalfd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw signalfd smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw signalfd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-signalfd");
    let source = workspace.join("raw-signalfd.c");
    let binary = workspace.join("raw-signalfd");
    fs::write(
        &source,
        r#"#define SIG_BLOCK 0
#define SIGUSR1 10
#define SFD_NONBLOCK 0x800
#define POLLIN 0x0001

struct pollfd {
    int fd;
    short events;
    short revents;
};

struct signalfd_siginfo {
    unsigned int ssi_signo;
    int ssi_errno;
    int ssi_code;
    unsigned int ssi_pid;
    unsigned int ssi_uid;
    int ssi_fd;
    unsigned int ssi_tid;
    unsigned int ssi_band;
    unsigned int ssi_overrun;
    unsigned int ssi_trapno;
    int ssi_status;
    int ssi_int;
    unsigned long ssi_ptr;
    unsigned long ssi_utime;
    unsigned long ssi_stime;
    unsigned long ssi_addr;
    unsigned short ssi_addr_lsb;
    unsigned char pad[46];
};

struct timespec {
    long tv_sec;
    long tv_nsec;
};

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
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a7) : "memory");
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

static void write_stdout(const char *text, long length) {
    linux_syscall3(64, 1, (long)text, length);
}

int main(void) {
    unsigned long mask = 1UL << (SIGUSR1 - 1);
    struct signalfd_siginfo info;
    struct timespec zero_timeout = {0, 0};
    long pid = linux_syscall0(172);
    long block_status = linux_syscall4(135, SIG_BLOCK, (long)&mask, 0, 8);
    long fd = linux_syscall4(74, -1, (long)&mask, 8, SFD_NONBLOCK);
    struct pollfd poll_fd = {(int)fd, POLLIN, 0};
    long initial_read = fd >= 0 ? linux_syscall3(63, fd, (long)&info, sizeof(info)) : -99;
    long initial_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                                (long)&zero_timeout, 0, 0) : -99;
    long kill_status = pid > 0 ? linux_syscall2(129, pid, SIGUSR1) : -99;
    poll_fd.revents = 0;
    long ready_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                              (long)&zero_timeout, 0, 0) : -99;
    short ready_revents = poll_fd.revents;
    long read_status = fd >= 0 ? linux_syscall3(63, fd, (long)&info, sizeof(info)) : -99;
    poll_fd.revents = 0;
    long drained_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                                (long)&zero_timeout, 0, 0) : -99;
    long drained_read = fd >= 0 ? linux_syscall3(63, fd, (long)&info, sizeof(info)) : -99;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    long read_after_close = fd >= 0 ? linux_syscall3(63, fd, (long)&info, sizeof(info)) : -99;

    int ok = pid > 0 &&
             block_status == 0 &&
             fd >= 0 &&
             initial_read == -11 &&
             initial_poll == 0 &&
             kill_status == 0 &&
             ready_poll == 1 &&
             (ready_revents & POLLIN) != 0 &&
             read_status == sizeof(info) &&
             info.ssi_signo == SIGUSR1 &&
             drained_poll == 0 &&
             drained_read == -11 &&
             close_status == 0 &&
             read_after_close == -9;

    if (ok) {
        write_stdout("raw-signalfd:ok\n", sizeof("raw-signalfd:ok\n") - 1);
        linux_syscall1(93, 74);
    }
    write_stdout("raw-signalfd:fail\n", sizeof("raw-signalfd:fail\n") - 1);
    linux_syscall1(93, 91);
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
        Some(74),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-signalfd:ok\n");

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
    assert!(stdout.contains("\"stop_code\":74"));
    assert!(stdout.contains("\"text\":\"raw-signalfd:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
