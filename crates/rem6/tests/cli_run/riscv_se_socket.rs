use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_socketpair_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE socketpair smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE socketpair smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-socketpair");
    let source = workspace.join("raw-socketpair.c");
    let binary = workspace.join("raw-socketpair");
    fs::write(
        &source,
        r#"#define AF_UNIX 1
#define SOCK_STREAM 1
#define POLLIN 0x0001
#define F_GETPIPE_SZ 1032
#define MSG_DONTWAIT 0x40
#define MSG_NOSIGNAL 0x4000
#define SHUT_RDWR 2

struct pollfd {
    int fd;
    short events;
    short revents;
};

struct sockaddr_un_addr {
    unsigned short family;
    char path[14];
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
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
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

static long linux_syscall6(long number, long arg0, long arg1, long arg2,
                           long arg3, long arg4, long arg5) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a5 asm("a5") = arg5;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a4), "r"(a5), "r"(a7) : "memory");
    return a0;
}

static int bytes_match(const char *left, const char *right, int count) {
    for (int i = 0; i < count; ++i) {
        if (left[i] != right[i]) {
            return 0;
        }
    }
    return 1;
}

int main(void) {
    int fds[2] = {-1, -1};
    char left[16] = {0};
    char right[16] = {0};
    char received[16] = {0};
    struct sockaddr_un_addr name = {0};
    struct sockaddr_un_addr peer = {0};
    unsigned int name_len = sizeof(name);
    unsigned int peer_len = sizeof(peer);
    const char *left_msg = "left";
    const char *right_msg = "right";
    const char *send_msg = "sendto";
    struct pollfd poll_fd = {-1, POLLIN, 0};
    const char *ok = "socketpair:ok\n";
    const char *fail = "socketpair:fail\n";

    long pair_status = linux_syscall4(199, AF_UNIX, SOCK_STREAM, 0, (long)fds);
    long not_pipe = pair_status == 0 ? linux_syscall3(25, fds[0], F_GETPIPE_SZ, 0) : 0;
    long first_write = pair_status == 0 ? linux_syscall3(64, fds[0], (long)left_msg, 4) : -1;
    poll_fd.fd = pair_status == 0 ? fds[1] : -1;
    long ready = pair_status == 0 ? linux_syscall5(73, (long)&poll_fd, 1, 0, 0, 0) : -1;
    long first_read = pair_status == 0 ? linux_syscall3(63, fds[1], (long)right, 4) : -1;
    long second_write = pair_status == 0 ? linux_syscall3(64, fds[1], (long)right_msg, 5) : -1;
    long second_read = pair_status == 0 ? linux_syscall3(63, fds[0], (long)left, 5) : -1;
    long send_status = pair_status == 0 ? linux_syscall6(206, fds[0], (long)send_msg, 6, MSG_NOSIGNAL, 0, 0) : -1;
    long recv_status = pair_status == 0 ? linux_syscall6(207, fds[1], (long)received, 6, MSG_DONTWAIT, 0, 0) : -1;
    long name_status = pair_status == 0 ? linux_syscall3(204, fds[0], (long)&name, (long)&name_len) : -1;
    long peer_status = pair_status == 0 ? linux_syscall3(205, fds[1], (long)&peer, (long)&peer_len) : -1;
    long shutdown_status = pair_status == 0 ? linux_syscall2(210, fds[0], SHUT_RDWR) : -1;
    long left_send_after_shutdown = pair_status == 0 ? linux_syscall6(206, fds[0], (long)send_msg, 1, MSG_NOSIGNAL, 0, 0) : -1;
    long left_read_after_shutdown = pair_status == 0 ? linux_syscall3(63, fds[0], (long)left, 1) : -1;
    long right_send_after_shutdown = pair_status == 0 ? linux_syscall6(206, fds[1], (long)send_msg, 1, MSG_NOSIGNAL, 0, 0) : -1;
    long right_read_after_shutdown = pair_status == 0 ? linux_syscall3(63, fds[1], (long)right, 1) : -1;
    if (pair_status == 0) {
        linux_syscall1(57, fds[0]);
        linux_syscall1(57, fds[1]);
    }

    if (pair_status == 0 && not_pipe == -9 && first_write == 4 &&
        ready == 1 && (poll_fd.revents & POLLIN) != 0 &&
        first_read == 4 && bytes_match(right, left_msg, 4) &&
        second_write == 5 && second_read == 5 && bytes_match(left, right_msg, 5) &&
        send_status == 6 && recv_status == 6 && bytes_match(received, send_msg, 6) &&
        name_status == 0 && peer_status == 0 &&
        name_len == 2 && peer_len == 2 &&
        name.family == AF_UNIX && peer.family == AF_UNIX &&
        shutdown_status == 0 &&
        left_send_after_shutdown == -32 && left_read_after_shutdown == 0 &&
        right_send_after_shutdown == -32 && right_read_after_shutdown == 0) {
        linux_syscall3(64, 1, (long)ok, 14);
        linux_syscall1(93, 74);
    }
    linux_syscall3(64, 1, (long)fail, 16);
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
        Some(74),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"socketpair:ok\n");

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
    assert!(stdout.contains("\"text\":\"socketpair:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
