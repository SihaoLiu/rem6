use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_xattr_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw xattr smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw xattr smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-xattr");
    let source = workspace.join("raw-xattr.c");
    let binary = workspace.join("raw-xattr");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)
#define O_WRONLY 01
#define O_CREAT 0100
#define O_TRUNC 01000
#define ENODATA 61
#define ENOTSUP 95

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

static long linux_syscall5(long number, long arg0, long arg1, long arg2, long arg3, long arg4) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a7) : "memory");
    return a0;
}

static int list_has_name(const char *list, long bytes, const char *name) {
    long offset = 0;
    while (offset < bytes) {
        const char *entry = list + offset;
        long length = 0;
        while (offset + length < bytes && entry[length] != '\0') {
            length++;
        }
        if (offset + length >= bytes) {
            return 0;
        }
        int equal = 1;
        for (long index = 0; name[index] != '\0' || entry[index] != '\0'; index++) {
            if (name[index] != entry[index]) {
                equal = 0;
                break;
            }
        }
        if (equal) {
            return 1;
        }
        offset += length + 1;
    }
    return 0;
}

int main(void) {
    const char *path = "xattr-target.txt";
    const char *name = "user.rem6";
    const char *value = "value";
    char value_buffer[8] = {0};
    char list_buffer[32] = {0};

    long fd = linux_syscall4(56, AT_FDCWD, (long)path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : fd;
    long set_status = linux_syscall5(5, (long)path, (long)name, (long)value, 5, 0);
    if (set_status == -ENOTSUP) {
        puts("raw-xattr-skip:ENOTSUP");
        return 77;
    }
    long get_status = linux_syscall4(8, (long)path, (long)name, (long)value_buffer, sizeof(value_buffer));
    long list_status = linux_syscall3(11, (long)path, (long)list_buffer, sizeof(list_buffer));
    long list_has = list_status > 0 ? list_has_name(list_buffer, list_status, name) : 0;
    long remove_status = linux_syscall2(14, (long)path, (long)name);
    long get_removed = linux_syscall4(8, (long)path, (long)name, (long)value_buffer, sizeof(value_buffer));

    printf("raw-xattr:%ld:%ld:%ld:%ld:%ld:%ld:%.*s\n",
           fd >= 0 ? 0 : fd, close_status, set_status, get_status,
           list_has, remove_status, (int)(get_status > 0 ? get_status : 0),
           value_buffer);
    return fd >= 0 && close_status == 0 && set_status == 0 && get_status == 5 &&
           list_has == 1 && remove_status == 0 && get_removed == -ENODATA &&
           value_buffer[0] == 'v' && value_buffer[1] == 'a' &&
           value_buffer[2] == 'l' && value_buffer[3] == 'u' &&
           value_buffer[4] == 'e' ? 58 : 82;
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
        .current_dir(&workspace)
        .arg(&binary)
        .output()
        .unwrap();
    if qemu_output.status.code() == Some(77) {
        eprintln!(
            "skipping static RISC-V SE raw xattr smoke: qemu host filesystem returned ENOTSUP"
        );
        return;
    }
    assert_eq!(
        qemu_output.status.code(),
        Some(58),
        "qemu stdout: {}\nqemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-xattr:0:0:0:5:1:0:value\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(&workspace)
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
    assert!(stdout.contains("\"stop_code\":58"));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-xattr:0:0:0:5:1:0:value\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
