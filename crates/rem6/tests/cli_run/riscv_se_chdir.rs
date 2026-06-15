use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_chdir_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw chdir smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw chdir smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-chdir");
    let nested = workspace.join("sub");
    fs::create_dir(&nested).unwrap();
    let source = workspace.join("raw-chdir.c");
    let binary = workspace.join("raw-chdir");
    let input = nested.join("guest.txt");
    fs::write(&input, b"nested input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_DIRECTORY 0200000
#define O_CLOEXEC 02000000

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

static int has_sub_suffix(const char *path) {
    unsigned long len = strlen(path);
    return len >= 4 && strcmp(path + len - 4, "/sub") == 0;
}

int main(void) {
    char cwd[512];
    char first[32];
    char second[32];
    long root = linux_syscall4(56, AT_FDCWD, (long)".", O_DIRECTORY | O_CLOEXEC, 0);
    long cd = linux_syscall1(49, (long)"sub");
    long cwd_status = linux_syscall2(17, (long)cwd, sizeof(cwd));
    long fd = linux_syscall4(56, AT_FDCWD, (long)"guest.txt", O_RDONLY | O_CLOEXEC, 0);
    long bytes = linux_syscall3(63, fd, (long)first, sizeof(first));
    long close_first = linux_syscall1(57, fd);
    long back = linux_syscall1(50, root);
    long fd2 = linux_syscall4(56, AT_FDCWD, (long)"sub/guest.txt", O_RDONLY, 0);
    long bytes2 = linux_syscall3(63, fd2, (long)second, sizeof(second));
    long close_second = linux_syscall1(57, fd2);
    long close_root = linux_syscall1(57, root);

    int ok = root >= 0 && cd == 0 && cwd_status > 0 && has_sub_suffix(cwd) &&
        fd >= 0 && bytes == 13 && memcmp(first, "nested input\n", 13) == 0 &&
        close_first == 0 && back == 0 &&
        fd2 >= 0 && bytes2 == 13 && memcmp(second, "nested input\n", 13) == 0 &&
        close_second == 0 && close_root == 0;
    printf("raw-chdir:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%ld\n",
        root, cd, cwd_status, fd, bytes, back, fd2, bytes2);
    return ok ? 48 : 80;
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
        String::from_utf8_lossy(&qemu_output.stderr),
    );
    assert!(String::from_utf8_lossy(&qemu_output.stdout).starts_with("raw-chdir:"));

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "400000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("sub/guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"text\":\"raw-chdir:"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
