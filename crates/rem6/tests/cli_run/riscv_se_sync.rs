use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_raw_sync_family_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw sync smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw sync smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-sync");
    let source = workspace.join("raw-sync.c");
    let binary = workspace.join("raw-sync");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define O_WRONLY 01
#define O_CREAT 0100
#define O_TRUNC 01000

static long linux_syscall0(long number) {
    register long a7 asm("a7") = number;
    register long a0 asm("a0");
    asm volatile ("ecall" : "=r"(a0) : "r"(a7) : "memory");
    return a0;
}

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

static void write_stdout(const char *text, long length) {
    linux_syscall3(64, 1, (long)text, length);
}

int main(void) {
    long fd = linux_syscall4(56, AT_FDCWD, (long)"created.txt",
                             O_WRONLY | O_CREAT | O_TRUNC, 0666);
    long write_status = fd >= 0 ? linux_syscall3(64, fd, (long)"sync-data\n", 10) : -99;
    long fsync_status = fd >= 0 ? linux_syscall1(82, fd) : -99;
    long fdatasync_status = fd >= 0 ? linux_syscall1(83, fd) : -99;
    long syncfs_status = fd >= 0 ? linux_syscall1(267, fd) : -99;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    long sync_status = linux_syscall0(81);
    long bad_fsync = linux_syscall1(82, 99);
    long bad_fdatasync = linux_syscall1(83, 99);
    long bad_syncfs = linux_syscall1(267, 99);

    int ok = fd >= 0 &&
             write_status == 10 &&
             fsync_status == 0 &&
             fdatasync_status == 0 &&
             syncfs_status == 0 &&
             close_status == 0 &&
             sync_status == 0 &&
             bad_fsync == -9 &&
             bad_fdatasync == -9 &&
             bad_syncfs == -9;
    if (ok) {
        write_stdout("raw-sync:ok\n", sizeof("raw-sync:ok\n") - 1);
    } else {
        write_stdout("raw-sync:fail\n", sizeof("raw-sync:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 45 : 86);
    __builtin_unreachable();
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
        Some(45),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-sync:ok\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "500000",
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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":45"));
    assert!(stdout.contains("\"text\":\"raw-sync:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 45, "constant");
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
