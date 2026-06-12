use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::support::temp_workspace;

#[test]
fn rem6_run_riscv_se_runs_static_raw_getdents64_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw getdents64 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw getdents64 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-getdents64");
    let source = workspace.join("raw-getdents64.c");
    let binary = workspace.join("raw-getdents64");
    let input = workspace.join("guest.txt");
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_DIRECTORY 0200000
#define O_CLOEXEC 02000000

struct linux_dirent64 {
    unsigned long long d_ino;
    long long d_off;
    unsigned short d_reclen;
    unsigned char d_type;
    char d_name[];
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
    char buffer[512];
    long fd = linux_syscall4(56, AT_FDCWD, (long)".", O_RDONLY | O_DIRECTORY | O_CLOEXEC, 0);
    if (fd < 0) {
        printf("raw-getdents64:open:%ld\n", fd);
        return 71;
    }
    long bytes = linux_syscall3(61, fd, (long)buffer, sizeof(buffer));
    if (bytes < 0) {
        printf("raw-getdents64:getdents:%ld\n", bytes);
        return 72;
    }

    int seen = 0;
    for (long offset = 0; offset < bytes;) {
        struct linux_dirent64 *entry = (struct linux_dirent64 *)(buffer + offset);
        if (strcmp(entry->d_name, "guest.txt") == 0) {
            seen++;
        }
        offset += entry->d_reclen;
    }
    long eof = linux_syscall3(61, fd, (long)buffer, sizeof(buffer));
    long closed = linux_syscall1(57, fd);

    printf("raw-getdents64:%ld:%d:%ld:%ld\n", bytes, seen, eof, closed);
    return seen == 1 && eof == 0 && closed == 0 ? 47 : 79;
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
        String::from_utf8_lossy(&qemu_output.stderr),
    );
    assert!(String::from_utf8_lossy(&qemu_output.stdout).starts_with("raw-getdents64:"));

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
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":47"));
    assert!(stdout.contains("\"text\":\"raw-getdents64:"));
    assert!(stdout.contains(":1:0:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

fn find_riscv_tool(name: &str) -> Option<PathBuf> {
    find_tool_on_path(name).or_else(|| {
        let module_candidate =
            Path::new("/mnt/nas0/software/riscv/riscv64-elf-ubuntu-24.04-gcc/bin").join(name);
        module_candidate.is_file().then_some(module_candidate)
    })
}

fn find_tool_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}
