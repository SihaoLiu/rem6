use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_newlib_pipe2_roundtrip_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE pipe2 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE pipe2 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-pipe2");
    let source = workspace.join("pipe2.c");
    let binary = workspace.join("pipe2");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

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

int main(void) {
    int fds[2] = {-1, -1};
    const char *message = "pipe-flow";
    char buffer[16] = {0};

    long pipe_status = linux_syscall2(59, (long)fds, 0);
    long written = pipe_status == 0 ? linux_syscall3(64, fds[1], (long)message, 9) : -1;
    long read_count = pipe_status == 0 ? linux_syscall3(63, fds[0], (long)buffer, sizeof(buffer) - 1) : -1;
    if (pipe_status == 0) {
        linux_syscall1(57, fds[0]);
        linux_syscall1(57, fds[1]);
    }

    int matches = strcmp(buffer, message) == 0;
    printf("pipe2:%ld:%ld:%ld:%d\n", pipe_status, written, read_count, matches);
    return pipe_status == 0 && written == 9 && read_count == 9 && matches ? 45 : 77;
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
        Some(45),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"pipe2:0:9:9:1\n");

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
    assert!(stdout.contains("\"stop_code\":45"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"pipe2:0:9:9:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 45, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_pipe_size_fcntl_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE pipe size fcntl smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE pipe size fcntl smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-pipe-size-fcntl");
    let qemu_workspace = workspace.join("qemu");
    fs::create_dir(&qemu_workspace).unwrap();
    let source = workspace.join("pipe-size-fcntl.c");
    let binary = workspace.join("pipe-size-fcntl");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)
#define O_WRONLY 01
#define O_CREAT 0100
#define O_TRUNC 01000
#define F_SETPIPE_SZ 1031
#define F_GETPIPE_SZ 1032
#define PIPE_PAGE_BYTES 4096

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

int main(void) {
    int fds[2] = {-1, -1};
    long pipe_status = linux_syscall2(59, (long)fds, 0);
    long initial_size = pipe_status == 0 ? linux_syscall3(25, fds[0], F_GETPIPE_SZ, 0) : -1;
    long set_size = pipe_status == 0 ? linux_syscall3(25, fds[1], F_SETPIPE_SZ, PIPE_PAGE_BYTES) : -1;
    long after_size = pipe_status == 0 ? linux_syscall3(25, fds[0], F_GETPIPE_SZ, 0) : -1;
    long fd = linux_syscall4(56, AT_FDCWD, (long)"pipe-size-regular.txt",
                             O_WRONLY | O_CREAT | O_TRUNC, 0600);
    long regular_status = fd >= 0 ? linux_syscall3(25, fd, F_GETPIPE_SZ, 0) : fd;
    long bad_fd_status = linux_syscall3(25, 99, F_GETPIPE_SZ, 0);
    long close_status = 0;
    if (pipe_status == 0) {
        close_status |= linux_syscall1(57, fds[0]);
        close_status |= linux_syscall1(57, fds[1]);
    }
    if (fd >= 0) {
        close_status |= linux_syscall1(57, fd);
    }

    int initial_positive = initial_size > 0;
    int set_positive = set_size >= PIPE_PAGE_BYTES;
    int after_matches = after_size == set_size;
    printf("pipe-size:%ld:%d:%d:%d:%ld:%ld:%ld\n",
           pipe_status, initial_positive, set_positive, after_matches,
           regular_status, bad_fd_status, close_status);
    return pipe_status == 0 && initial_positive && set_positive && after_matches &&
           regular_status == -9 && bad_fd_status == -9 && close_status == 0 ? 48 : 80;
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
        .current_dir(&qemu_workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(48),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"pipe-size:0:1:1:1:-9:-9:0\n");

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
    assert!(stdout.contains("\"stop_code\":48"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"pipe-size:0:1:1:1:-9:-9:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 48, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_flock_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE flock smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE flock smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-flock");
    let source = workspace.join("flock.c");
    let binary = workspace.join("flock");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)
#define O_RDWR 02
#define O_CREAT 0100
#define O_TRUNC 01000
#define LOCK_EX 2
#define LOCK_NB 4
#define LOCK_UN 8

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
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long fd = linux_syscall4(56, AT_FDCWD, (long)"locked.txt",
                             O_RDWR | O_CREAT | O_TRUNC, 0600);
    long lock_status = fd >= 0 ? linux_syscall2(32, fd, LOCK_EX | LOCK_NB) : -1;
    long unlock_status = fd >= 0 ? linux_syscall2(32, fd, LOCK_UN) : -1;
    long bad_fd = linux_syscall2(32, 99, LOCK_EX);
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -1;

    printf("flock:%ld:%ld:%ld:%ld:%ld\n",
           fd >= 0 ? 0 : fd, lock_status, unlock_status, bad_fd, close_status);
    return fd >= 0 && lock_status == 0 && unlock_status == 0 &&
           bad_fd == -9 && close_status == 0 ? 46 : 78;
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
    assert_eq!(qemu_output.stdout, b"flock:0:0:0:-9:0\n");

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
    assert!(stdout.contains("\"stop_code\":46"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"flock:0:0:0:-9:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 46, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_fcntl_locks_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE fcntl lock smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE fcntl lock smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-fcntl-lock");
    let source = workspace.join("fcntl-lock.c");
    let binary = workspace.join("fcntl-lock");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)
#define O_RDWR 02
#define O_CREAT 0100
#define O_TRUNC 01000
#define F_GETLK 5
#define F_SETLK 6
#define F_WRLCK 1
#define F_UNLCK 2
#define SEEK_SET 0

struct linux_flock {
    short l_type;
    short l_whence;
    long l_start;
    long l_len;
    int l_pid;
};

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

int main(void) {
    struct linux_flock lock = {F_WRLCK, SEEK_SET, 0, 0, 0};
    long fd = linux_syscall4(56, AT_FDCWD, (long)"fcntl-lock.txt",
                             O_RDWR | O_CREAT | O_TRUNC, 0600);
    long get_status = fd >= 0 ? linux_syscall3(25, fd, F_GETLK, (long)&lock) : -1;
    int get_type = get_status == 0 ? lock.l_type : -1;
    lock.l_type = F_WRLCK;
    long set_status = fd >= 0 ? linux_syscall3(25, fd, F_SETLK, (long)&lock) : -1;
    lock.l_type = F_UNLCK;
    long unlock_status = fd >= 0 ? linux_syscall3(25, fd, F_SETLK, (long)&lock) : -1;
    lock.l_type = 99;
    long bad_type = fd >= 0 ? linux_syscall3(25, fd, F_SETLK, (long)&lock) : -1;
    lock.l_type = F_WRLCK;
    long bad_fd = linux_syscall3(25, 99, F_GETLK, (long)&lock);
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -1;

    printf("fcntl-lock:%ld:%d:%ld:%ld:%ld:%ld:%ld\n",
           fd >= 0 ? 0 : fd, get_type, set_status, unlock_status,
           bad_type, bad_fd, close_status);
    return fd >= 0 && get_status == 0 && get_type == F_UNLCK &&
           set_status == 0 && unlock_status == 0 && bad_type == -22 &&
           bad_fd == -9 && close_status == 0 ? 47 : 79;
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
        Some(47),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"fcntl-lock:0:2:0:0:-22:-9:0\n");

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
    assert!(stdout.contains("\"stop_code\":47"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"fcntl-lock:0:2:0:0:-22:-9:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_sendfile_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE sendfile smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE sendfile smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-sendfile");
    let qemu_workspace = workspace.join("qemu");
    fs::create_dir(&qemu_workspace).unwrap();
    let source = workspace.join("sendfile.c");
    let binary = workspace.join("sendfile");
    let qemu_input = qemu_workspace.join("input.txt");
    let rem6_input = workspace.join("rem6-input.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_RDWR 02
#define O_CREAT 0100
#define O_TRUNC 01000
#define SEEK_SET 0
#define SEEK_CUR 1

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

int main(void) {
    char copied_bytes[8] = {0};
    char next_bytes[8] = {0};
    unsigned long offset = 2;
    long in_fd = linux_syscall4(56, AT_FDCWD, (long)"input.txt", O_RDONLY, 0);
    long out_fd = linux_syscall4(56, AT_FDCWD, (long)"output.txt",
                                 O_RDWR | O_CREAT | O_TRUNC, 0600);
    long copied = in_fd < 0 || out_fd < 0 ? -1 :
        linux_syscall4(71, out_fd, in_fd, (long)&offset, 3);
    long seek = out_fd < 0 ? -1 : linux_syscall3(62, out_fd, 0, SEEK_SET);
    long read_out = seek < 0 ? -1 : linux_syscall3(63, out_fd, (long)copied_bytes, 7);
    long read_in = in_fd < 0 ? -1 : linux_syscall3(63, in_fd, (long)next_bytes, 7);
    if (out_fd >= 0) {
        linux_syscall1(57, out_fd);
    }
    if (in_fd >= 0) {
        linux_syscall1(57, in_fd);
    }

    printf("sendfile:%ld:%ld:%ld:%lu:%ld:%.*s:%ld:%.*s\n",
           in_fd >= 0 ? 0 : in_fd,
           out_fd >= 0 ? 0 : out_fd,
           copied,
           offset,
           read_out,
           (int)(read_out > 0 ? read_out : 0),
           copied_bytes,
           read_in,
           (int)(read_in > 0 ? read_in : 0),
           next_bytes);
    return in_fd >= 0 && out_fd >= 0 &&
           copied == 3 && offset == 5 &&
           read_out == 3 && memcmp(copied_bytes, "cde", 3) == 0 &&
           read_in == 7 && memcmp(next_bytes, "abcdefg", 7) == 0 ? 48 : 79;
}
"#,
    )
    .unwrap();
    fs::write(&qemu_input, b"abcdefgh\n").unwrap();
    fs::write(&rem6_input, b"abcdefgh\n").unwrap();

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
        .current_dir(&qemu_workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(48),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"sendfile:0:0:3:5:3:cde:7:abcdefg\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "450000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("input.txt={}", rem6_input.display()),
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
    assert!(stdout.contains("\"stop_code\":48"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"sendfile:0:0:3:5:3:cde:7:abcdefg\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 48, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_copy_file_range_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE copy_file_range smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE copy_file_range smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-copy-file-range");
    let qemu_workspace = workspace.join("qemu");
    fs::create_dir(&qemu_workspace).unwrap();
    let source = workspace.join("copy-file-range.c");
    let binary = workspace.join("copy-file-range");
    let qemu_input = qemu_workspace.join("input.txt");
    let rem6_input = workspace.join("rem6-input.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)
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
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a7) : "memory");
    return a0;
}

static int bytes_equal(const char *left, const char *right, long len) {
    for (long i = 0; i < len; i++) {
        if (left[i] != right[i]) {
            return 0;
        }
    }
    return 1;
}

int main(void) {
    char copied_bytes[8] = {0};
    char input_bytes[9] = {0};
    unsigned long in_offset = 2;
    unsigned long out_offset = 1;
    long in_fd = linux_syscall4(56, AT_FDCWD, (long)"input.txt", O_RDONLY, 0);
    long out_fd = linux_syscall4(56, AT_FDCWD, (long)"output.txt",
                                 O_RDWR | O_CREAT | O_TRUNC, 0600);
    long seed = out_fd < 0 ? -1 : linux_syscall3(64, out_fd, (long)"XYZ", 3);
    long copied = in_fd < 0 || out_fd < 0 ? -1 :
        linux_syscall6(285, in_fd, (long)&in_offset, out_fd, (long)&out_offset, 3, 0);
    long out_pos = out_fd < 0 ? -1 : linux_syscall3(62, out_fd, 0, SEEK_CUR);
    long seek = out_fd < 0 ? -1 : linux_syscall3(62, out_fd, 0, SEEK_SET);
    long read_out = seek < 0 ? -1 : linux_syscall3(63, out_fd, (long)copied_bytes, 8);
    long read_in = in_fd < 0 ? -1 : linux_syscall3(63, in_fd, (long)input_bytes, 8);
    long bad_flags = in_fd < 0 || out_fd < 0 ? -1 :
        linux_syscall6(285, in_fd, 0, out_fd, 0, 1, 1);
    long bad_fd = out_fd < 0 ? -1 : linux_syscall6(285, -1, 0, out_fd, 0, 1, 0);
    if (out_fd >= 0) {
        linux_syscall1(57, out_fd);
    }
    if (in_fd >= 0) {
        linux_syscall1(57, in_fd);
    }

    return in_fd >= 0 && out_fd >= 0 &&
           seed == 3 &&
           copied == 3 && in_offset == 5 && out_offset == 4 &&
           out_pos == 3 &&
           read_out == 4 && bytes_equal(copied_bytes, "Xcde", 4) &&
           read_in == 8 && bytes_equal(input_bytes, "abcdefgh", 8) &&
           bad_flags == -22 && bad_fd == -9 ? 49 : 79;
}
"#,
    )
    .unwrap();
    fs::write(&qemu_input, b"abcdefgh\n").unwrap();
    fs::write(&rem6_input, b"abcdefgh\n").unwrap();

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
        .current_dir(&qemu_workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(49),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert!(qemu_output.stdout.is_empty());

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "260000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("input.txt={}", rem6_input.display()),
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
    assert!(stdout.contains("\"stop_code\":49"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 49, "constant");
}
