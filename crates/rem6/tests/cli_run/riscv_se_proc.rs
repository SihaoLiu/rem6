use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_reads_proc_self_maps_after_mmap_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE proc-self-maps smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE proc-self-maps smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-proc-self-maps");
    let source = workspace.join("proc-self-maps.c");
    let binary = workspace.join("proc-self-maps");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

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

static long linux_syscall6(
    long number,
    long arg0,
    long arg1,
    long arg2,
    long arg3,
    long arg4,
    long arg5
) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a5 asm("a5") = arg5;
    register long a7 asm("a7") = number;
    asm volatile (
        "ecall"
        : "+r"(a0)
        : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a7)
        : "memory"
    );
    return a0;
}

int main(void) {
    char chunk[16385];
    char needle[32];
    char padded[32];
    long mapped = linux_syscall6(222, 0, 8192, 3, 0x22, -1, 0);
    int mmap_ok = mapped > 0;
    snprintf(needle, sizeof(needle), "%lx", (unsigned long)mapped);
    snprintf(padded, sizeof(padded), "%016lx", (unsigned long)mapped);

    long fd = linux_syscall4(56, -100, (long)"/proc/self/maps", 0, 0);
    long total = 0;
    int found = 0;
    if (fd >= 0) {
        for (int i = 0; i < 32 && !found; i++) {
            long count = linux_syscall3(63, fd, (long)chunk, sizeof(chunk) - 1);
            if (count <= 0) {
                break;
            }
            chunk[count] = 0;
            total += count;
            found = strstr(chunk, needle) != 0 || strstr(chunk, padded) != 0;
        }
    }

    printf("proc-self-maps:%d:%d:%d:%d\n", mmap_ok, fd >= 0, total > 0, found);
    return mmap_ok && fd >= 0 && total > 0 && found ? 48 : 79;
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
        Some(48),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"proc-self-maps:1:1:1:1\n");

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
    assert!(stdout.contains("\"stop_code\":48"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"proc-self-maps:1:1:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 48, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_readlink_proc_self_exe_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE proc-self-exe smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE proc-self-exe smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-proc-self-exe")
        .canonicalize()
        .unwrap();
    let source = workspace.join("proc-self-exe.c");
    let binary = workspace.join("proc-self-exe");
    let expected_exe = binary.to_str().unwrap();
    assert!(!expected_exe.contains('\\'));
    assert!(!expected_exe.contains('"'));
    assert!(!expected_exe.contains('\n'));
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

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
    char buffer[1024] = {0};
    long len = linux_syscall4(78, -100, (long)"/proc/self/exe", (long)buffer, sizeof(buffer) - 1);
    int matches_expected_path = len > 0 && strcmp(buffer, "EXPECTED_EXE_PATH") == 0;
    printf("proc-self-exe:%d:%d\n", len > 0, matches_expected_path);
    return matches_expected_path ? 44 : 76;
}
"#
        .replace("EXPECTED_EXE_PATH", expected_exe),
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
        Some(44),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"proc-self-exe:1:1\n");

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
    assert!(stdout.contains("\"stop_code\":44"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"proc-self-exe:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 44, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_readlink_proc_self_fd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE proc-self-fd smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE proc-self-fd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-proc-self-fd");
    let source = workspace.join("proc-self-fd.c");
    let binary = workspace.join("proc-self-fd");
    let guest_file = workspace.join("fd-target.txt");
    fs::write(&guest_file, b"fd target\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

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

static int ends_with(const char *text, const char *suffix) {
    size_t text_len = strlen(text);
    size_t suffix_len = strlen(suffix);
    return text_len >= suffix_len &&
           memcmp(text + text_len - suffix_len, suffix, suffix_len) == 0;
}

int main(void) {
    char proc_path[64] = {0};
    char target[256] = {0};
    long fd = linux_syscall4(56, -100, (long)"fd-target.txt", 0, 0);
    snprintf(proc_path, sizeof(proc_path), "/proc/self/fd/%ld", fd);
    long len = fd >= 0 ? linux_syscall4(78, -100, (long)proc_path, (long)target, sizeof(target) - 1) : -1;
    if (len > 0 && len < (long)sizeof(target)) {
        target[len] = 0;
    }
    long missing = linux_syscall4(78, -100, (long)"/proc/self/fd/99", (long)target, sizeof(target) - 1);
    int suffix = len > 0 && ends_with(target, "fd-target.txt");
    printf("proc-self-fd:%ld:%ld:%d:%ld\n", fd, len, suffix, missing);
    return fd >= 0 && len > 0 && suffix && missing == -2 ? 45 : 77;
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
    assert_eq!(
        qemu_output.status.code(),
        Some(45),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert!(
        qemu_stdout.starts_with("proc-self-fd:3:"),
        "qemu stdout: {qemu_stdout}"
    );
    assert!(
        qemu_stdout.ends_with(":1:-2\n"),
        "qemu stdout: {qemu_stdout}"
    );

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
            "--riscv-se-file",
            &format!("fd-target.txt={}", guest_file.display()),
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
    assert!(stdout.contains("\"text\":\"proc-self-fd:3:"));
    assert!(stdout.contains(":1:-2\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 45, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_readlink_proc_self_cwd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE proc-self-cwd smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE proc-self-cwd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-proc-self-cwd");
    let source = workspace.join("proc-self-cwd.c");
    let binary = workspace.join("proc-self-cwd");
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

static int ends_with(const char *text, const char *suffix) {
    size_t text_len = strlen(text);
    size_t suffix_len = strlen(suffix);
    return text_len >= suffix_len &&
           memcmp(text + text_len - suffix_len, suffix, suffix_len) == 0;
}

int main(void) {
    char target[512] = {0};
    long made = linux_syscall3(34, -100, (long)"work", 0755);
    long changed = linux_syscall1(49, (long)"work");
    long len = linux_syscall4(78, -100, (long)"/proc/self/cwd", (long)target, sizeof(target) - 1);
    if (len > 0 && len < (long)sizeof(target)) {
        target[len] = 0;
    }
    int suffix = len > 0 && ends_with(target, "/work");
    printf("proc-self-cwd:%ld:%ld:%ld:%d\n", made, changed, len, suffix);
    return made == 0 && changed == 0 && len > 0 && suffix ? 47 : 79;
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
    assert_eq!(
        qemu_output.status.code(),
        Some(47),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert!(
        qemu_stdout.starts_with("proc-self-cwd:0:0:"),
        "qemu stdout: {qemu_stdout}"
    );
    assert!(qemu_stdout.ends_with(":1\n"), "qemu stdout: {qemu_stdout}");

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
    assert!(stdout.contains("\"text\":\"proc-self-cwd:0:0:"));
    assert!(stdout.contains(":1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prctl_name_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE prctl smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE prctl smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prctl");
    let source = workspace.join("raw-prctl.c");
    let binary = workspace.join("raw-prctl");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char name[16] = {0};
    long set_name = linux_syscall2(167, 15, (long)"rem6-prctl-name-is-long");
    long get_name = linux_syscall2(167, 16, (long)name);
    int matches = strcmp(name, "rem6-prctl-name") == 0;
    printf("prctl-name:%ld:%ld:%s:%d\n", set_name, get_name, name, matches);
    return set_name == 0 && get_name == 0 && matches ? 46 : 78;
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
        Some(46),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"prctl-name:0:0:rem6-prctl-name:1\n");

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
    assert!(stdout.contains("\"text\":\"prctl-name:0:0:rem6-prctl-name:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 46, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prctl_no_new_privs_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE no-new-privs smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE no-new-privs smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prctl-no-new-privs");
    let source = workspace.join("raw-prctl-no-new-privs.c");
    let binary = workspace.join("raw-prctl-no-new-privs");
    fs::write(
        &source,
        r#"#include <stdio.h>

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

int main(void) {
    long invalid_set = linux_syscall5(167, 38, 0, 0, 0, 0);
    long set = linux_syscall5(167, 38, 1, 0, 0, 0);
    long after = linux_syscall5(167, 39, 0, 0, 0, 0);
    long invalid_get = linux_syscall5(167, 39, 1, 0, 0, 0);

    int ok = invalid_set == -22
        && invalid_get == -22
        && set == 0
        && after == 1;
    printf("prctl-no-new-privs:%ld:%ld:%ld:%ld:%d\n",
           invalid_set, set, after, invalid_get, ok);
    return ok ? 47 : 79;
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
        Some(47),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"prctl-no-new-privs:-22:0:1:-22:1\n");

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
    assert!(stdout.contains("\"text\":\"prctl-no-new-privs:-22:0:1:-22:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prctl_dumpable_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE dumpable smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE dumpable smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prctl-dumpable");
    let source = workspace.join("raw-prctl-dumpable.c");
    let binary = workspace.join("raw-prctl-dumpable");
    fs::write(
        &source,
        r#"#include <stdio.h>

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

int main(void) {
    long initial = linux_syscall5(167, 3, 0, 0, 0, 0);
    long set_zero = linux_syscall5(167, 4, 0, 0, 0, 0);
    long after_zero = linux_syscall5(167, 3, 0, 0, 0, 0);
    long invalid_two = linux_syscall5(167, 4, 2, 0, 0, 0);
    long after_invalid = linux_syscall5(167, 3, 0, 0, 0, 0);
    long set_one = linux_syscall5(167, 4, 1, 0, 0, 0);
    long after_one = linux_syscall5(167, 3, 0, 0, 0, 0);

    int ok = initial == 1
        && set_zero == 0
        && after_zero == 0
        && invalid_two == -22
        && after_invalid == 0
        && set_one == 0
        && after_one == 1;
    printf("prctl-dumpable:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%d\n",
           initial, set_zero, after_zero, invalid_two, after_invalid, set_one, after_one, ok);
    return ok ? 49 : 81;
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
        Some(49),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"prctl-dumpable:1:0:0:-22:0:0:1:1\n");

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
    assert!(stdout.contains("\"stop_code\":49"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"prctl-dumpable:1:0:0:-22:0:0:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 49, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prctl_pdeathsig_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE pdeathsig smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE pdeathsig smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prctl-pdeathsig");
    let source = workspace.join("raw-prctl-pdeathsig.c");
    let binary = workspace.join("raw-prctl-pdeathsig");
    fs::write(
        &source,
        r#"#define PR_SET_PDEATHSIG 1
#define PR_GET_PDEATHSIG 2
#define SIGUSR1 10

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

static void append_char(char **cursor, char value) {
    **cursor = value;
    *cursor = *cursor + 1;
}

static void append_text(char **cursor, const char *text) {
    while (*text != 0) {
        append_char(cursor, *text);
        text++;
    }
}

static void append_long(char **cursor, long value) {
    char digits[24];
    int count = 0;
    unsigned long magnitude;
    if (value < 0) {
        append_char(cursor, '-');
        magnitude = (unsigned long)(-value);
    } else {
        magnitude = (unsigned long)value;
    }
    do {
        digits[count++] = (char)('0' + (magnitude % 10));
        magnitude /= 10;
    } while (magnitude != 0);
    while (count > 0) {
        append_char(cursor, digits[--count]);
    }
}

static void append_field(char **cursor, long value) {
    append_char(cursor, ':');
    append_long(cursor, value);
}

void _start(void) {
    int initial = -1;
    int after_set = -1;
    int after_invalid = -1;
    int after_clear = -1;
    long get_initial = linux_syscall5(167, PR_GET_PDEATHSIG, (long)&initial, 1, 2, 3);
    long set = linux_syscall5(167, PR_SET_PDEATHSIG, SIGUSR1, 1, 2, 3);
    long get_after_set = linux_syscall5(167, PR_GET_PDEATHSIG, (long)&after_set, 0, 0, 0);
    long invalid_negative = linux_syscall5(167, PR_SET_PDEATHSIG, -1, 0, 0, 0);
    long invalid_high = linux_syscall5(167, PR_SET_PDEATHSIG, 65, 0, 0, 0);
    long get_after_invalid = linux_syscall5(167, PR_GET_PDEATHSIG, (long)&after_invalid, 0, 0, 0);
    long clear = linux_syscall5(167, PR_SET_PDEATHSIG, 0, 0, 0, 0);
    long get_after_clear = linux_syscall5(167, PR_GET_PDEATHSIG, (long)&after_clear, 0, 0, 0);
    long get_fault = linux_syscall5(167, PR_GET_PDEATHSIG, 0, 0, 0, 0);

    long ok = get_initial == 0 && initial == 0 &&
        set == 0 && get_after_set == 0 && after_set == SIGUSR1 &&
        invalid_negative == -22 && invalid_high == -22 &&
        get_after_invalid == 0 && after_invalid == SIGUSR1 &&
        clear == 0 && get_after_clear == 0 && after_clear == 0 &&
        get_fault == -14;

    char output[256];
    char *cursor = output;
    append_text(&cursor, "prctl-pdeathsig");
    append_field(&cursor, get_initial);
    append_field(&cursor, initial);
    append_field(&cursor, set);
    append_field(&cursor, get_after_set);
    append_field(&cursor, after_set);
    append_field(&cursor, invalid_negative);
    append_field(&cursor, invalid_high);
    append_field(&cursor, get_after_invalid);
    append_field(&cursor, after_invalid);
    append_field(&cursor, clear);
    append_field(&cursor, get_after_clear);
    append_field(&cursor, after_clear);
    append_field(&cursor, get_fault);
    append_field(&cursor, ok);
    append_char(&cursor, '\n');
    linux_syscall3(64, 1, (long)output, cursor - output);
    linux_syscall1(93, ok ? 48 : 80);
    while (1) {}
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-nostdlib",
            "-static",
            "-fno-builtin",
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
        Some(48),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"prctl-pdeathsig:0:0:0:0:10:-22:-22:0:10:0:0:0:-14:1\n"
    );

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
    assert!(stdout.contains("\"text\":\"prctl-pdeathsig:0:0:0:0:10:-22:-22:0:10:0:0:0:-14:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 48, "constant");
}
