use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_timerfd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw timerfd smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw timerfd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-timerfd");
    let source = workspace.join("raw-timerfd.c");
    let binary = workspace.join("raw-timerfd");
    fs::write(
        &source,
        r#"#define CLOCK_MONOTONIC 1
#define TFD_NONBLOCK 0x800
#define TFD_TIMER_ABSTIME 1
#define POLLIN 0x0001

struct timespec {
    long tv_sec;
    long tv_nsec;
};

struct itimerspec {
    struct timespec it_interval;
    struct timespec it_value;
};

struct pollfd {
    int fd;
    short events;
    short revents;
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
    long fd = linux_syscall2(85, CLOCK_MONOTONIC, TFD_NONBLOCK);
    struct itimerspec old_value;
    struct itimerspec current;
    struct itimerspec armed = {{0, 0}, {0, 1}};
    struct timespec zero_timeout = {0, 0};
    struct pollfd poll_fd = {(int)fd, POLLIN, 0};
    unsigned long expirations = 0;

    long invalid_clock = linux_syscall2(85, 99, 0);
    long invalid_flags = linux_syscall2(85, CLOCK_MONOTONIC, 0x40);
    long initial_gettime = fd >= 0 ? linux_syscall2(87, fd, (long)&current) : -99;
    long initial_read = fd >= 0 ? linux_syscall3(63, fd, (long)&expirations, 8) : -99;
    long initial_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                                (long)&zero_timeout, 0, 0) : -99;
    long settime_status = fd >= 0 ? linux_syscall4(86, fd, TFD_TIMER_ABSTIME,
                                                  (long)&armed, (long)&old_value) : -99;
    long post_set_gettime = fd >= 0 ? linux_syscall2(87, fd, (long)&current) : -99;
    poll_fd.revents = 0;
    long ready_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                              (long)&zero_timeout, 0, 0) : -99;
    long read_status = -99;
    for (int attempt = 0; fd >= 0 && attempt < 1024 && read_status != 8; attempt++) {
        read_status = linux_syscall3(63, fd, (long)&expirations, 8);
    }
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    long gettime_after_close = fd >= 0 ? linux_syscall2(87, fd, (long)&current) : -99;

    int ok = fd >= 0 &&
             invalid_clock == -22 &&
             invalid_flags == -22 &&
             initial_gettime == 0 &&
             current.it_interval.tv_sec == 0 &&
             current.it_interval.tv_nsec == 0 &&
             current.it_value.tv_sec == 0 &&
             current.it_value.tv_nsec == 0 &&
             initial_read == -11 &&
             initial_poll == 0 &&
             settime_status == 0 &&
             old_value.it_interval.tv_sec == 0 &&
             old_value.it_interval.tv_nsec == 0 &&
             old_value.it_value.tv_sec == 0 &&
             old_value.it_value.tv_nsec == 0 &&
             post_set_gettime == 0 &&
             current.it_interval.tv_sec == 0 &&
             current.it_interval.tv_nsec == 0 &&
             current.it_value.tv_sec == 0 &&
             current.it_value.tv_nsec == 0 &&
             (ready_poll == 0 || (ready_poll == 1 && (poll_fd.revents & POLLIN) != 0)) &&
             read_status == 8 &&
             expirations >= 1 &&
             close_status == 0 &&
             gettime_after_close == -9;

    if (ok) {
        write_stdout("raw-timerfd:ok\n", sizeof("raw-timerfd:ok\n") - 1);
        linux_syscall1(93, 73);
    }
    write_stdout("raw-timerfd:fail\n", sizeof("raw-timerfd:fail\n") - 1);
    linux_syscall1(93, 89);
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
        Some(73),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-timerfd:ok\n");

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
    assert!(stdout.contains("\"stop_code\":73"));
    assert!(stdout.contains("\"text\":\"raw-timerfd:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
