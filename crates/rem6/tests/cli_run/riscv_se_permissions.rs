use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_capability_syscalls_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw capability smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw capability smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-capability");
    let source = workspace.join("raw-capability.c");
    let binary = workspace.join("raw-capability");
    fs::write(
        &source,
        r#"struct cap_header {
    unsigned int version;
    int pid;
};

struct cap_data {
    unsigned int effective;
    unsigned int permitted;
    unsigned int inheritable;
};

static const char pass_text[] = "raw-capability:ok\n";
static const char fail_text[] = "raw-capability:fail\n";

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

static void finish(int ok) {
    if (ok) {
        linux_syscall3(64, 1, (long)pass_text, sizeof(pass_text) - 1);
        linux_syscall1(93, 53);
    } else {
        linux_syscall3(64, 1, (long)fail_text, sizeof(fail_text) - 1);
        linux_syscall1(93, 84);
    }
    __builtin_unreachable();
}

void _start(void) {
    struct cap_header header = {0x20080522u, 0};
    struct cap_data data[2] = {
        {0xffffffffu, 0xffffffffu, 0xffffffffu},
        {0xffffffffu, 0xffffffffu, 0xffffffffu},
    };
    long capget_ok = linux_syscall2(90, (long)&header, (long)data);

    struct cap_header bad_version = {0x12345678u, 0};
    long capget_bad_version = linux_syscall2(90, (long)&bad_version, (long)data);

    struct cap_header missing_pid = {0x20080522u, 999999};
    long capget_missing_pid = linux_syscall2(90, (long)&missing_pid, (long)data);
    long capget_null_header = linux_syscall2(90, 0, (long)data);

    struct cap_header null_data_header = {0x20080522u, 0};
    long capget_null_data = linux_syscall2(90, (long)&null_data_header, 0);

    struct cap_header capset_header = {0x20080522u, 0};
    struct cap_data zero[2] = {{0, 0, 0}, {0, 0, 0}};
    long capset_zero = linux_syscall2(91, (long)&capset_header, (long)zero);

    struct cap_header nonzero_header = {0x20080522u, 0};
    struct cap_data one[2] = {{1, 0, 0}, {0, 0, 0}};
    long capset_nonzero = linux_syscall2(91, (long)&nonzero_header, (long)one);

    long capset_null_data = linux_syscall2(91, (long)&capset_header, 0);

    struct cap_header capset_bad_version = {0x12345678u, 0};
    long capset_bad_version_status = linux_syscall2(91, (long)&capset_bad_version, (long)zero);

    finish(capget_ok == 0 &&
           header.version == 0x20080522u &&
           header.pid == 0 &&
           data[0].effective == 0 &&
           data[0].permitted == 0 &&
           data[0].inheritable == 0 &&
           data[1].effective == 0 &&
           data[1].permitted == 0 &&
           data[1].inheritable == 0 &&
           capget_bad_version == -22 &&
           bad_version.version == 0x20080522u &&
           capget_missing_pid == -3 &&
           capget_null_header == -14 &&
           capget_null_data == 0 &&
           null_data_header.version == 0x20080522u &&
           capset_zero == 0 &&
           capset_nonzero == -1 &&
           capset_null_data == -14 &&
           capset_bad_version_status == -22 &&
           capset_bad_version.version == 0x20080522u);
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-nostdlib",
            "-nostartfiles",
            "-ffreestanding",
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
        Some(53),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert_eq!(qemu_stdout, "raw-capability:ok\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "350000",
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
    assert!(stdout.contains("\"stop_code\":53"));
    assert!(stdout.contains("\"text\":\"raw-capability:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 53, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_umask_mkdirat_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw umask smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw umask smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-umask");
    let source = workspace.join("raw-umask.c");
    let binary = workspace.join("raw-umask");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)

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

static unsigned int read_u32_le(const unsigned char *bytes, unsigned long offset) {
    return ((unsigned int)bytes[offset]) |
           ((unsigned int)bytes[offset + 1] << 8) |
           ((unsigned int)bytes[offset + 2] << 16) |
           ((unsigned int)bytes[offset + 3] << 24);
}

int main(void) {
    unsigned char stat_bytes[160];
    memset(stat_bytes, 0xa5, sizeof(stat_bytes));

    long old = linux_syscall1(166, 0077);
    long made = linux_syscall3(34, AT_FDCWD, (long)"masked", 0777);
    long stat_status = linux_syscall4(79, AT_FDCWD, (long)"masked", (long)stat_bytes, 0);
    unsigned int mode = stat_status == 0 ? read_u32_le(stat_bytes, 16) & 0777 : 0;
    long second = linux_syscall1(166, old);

    printf("raw-umask:%03lo:%ld:%ld:%03o:%03lo\n",
           (unsigned long)old, made, stat_status, mode, (unsigned long)second);
    return made == 0 && stat_status == 0 && mode == 0700 && second == 0077 ? 47 : 79;
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
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert!(qemu_stdout.starts_with("raw-umask:"));
    assert!(qemu_stdout.ends_with(":0:0:700:077\n"));

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "350000",
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
    assert!(stdout.contains("\"stop_code\":47"));
    assert!(stdout.contains("\"text\":\"raw-umask:000:0:0:700:077\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_umask_openat_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw openat mode smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw openat mode smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-openat-mode");
    let source = workspace.join("raw-openat-mode.c");
    let binary = workspace.join("raw-openat-mode");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)
#define O_WRONLY 01
#define O_CREAT 0100
#define O_TRUNC 01000

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
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

static unsigned int read_u32_le(const unsigned char *bytes, unsigned long offset) {
    return ((unsigned int)bytes[offset]) |
           ((unsigned int)bytes[offset + 1] << 8) |
           ((unsigned int)bytes[offset + 2] << 16) |
           ((unsigned int)bytes[offset + 3] << 24);
}

int main(void) {
    unsigned char path_stat_bytes[160];
    unsigned char fd_stat_bytes[160];
    memset(path_stat_bytes, 0xa5, sizeof(path_stat_bytes));
    memset(fd_stat_bytes, 0xa5, sizeof(fd_stat_bytes));

    long old = linux_syscall1(166, 0027);
    long fd = linux_syscall4(56, AT_FDCWD, (long)"created.txt",
                             O_WRONLY | O_CREAT | O_TRUNC, 0666);
    long path_stat = linux_syscall4(79, AT_FDCWD, (long)"created.txt",
                                    (long)path_stat_bytes, 0);
    long fd_stat = fd >= 0 ? linux_syscall4(80, fd, (long)fd_stat_bytes, 0, 0) : -1;
    unsigned int path_mode = path_stat == 0 ? read_u32_le(path_stat_bytes, 16) & 0777 : 0;
    unsigned int fd_mode = fd_stat == 0 ? read_u32_le(fd_stat_bytes, 16) & 0777 : 0;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -1;
    long second = linux_syscall1(166, old);

    printf("raw-openat-mode:%03lo:%ld:%ld:%ld:%03o:%03o:%ld:%03lo\n",
           (unsigned long)old, fd, path_stat, fd_stat, path_mode, fd_mode,
           close_status, (unsigned long)second);
    return fd >= 0 && path_stat == 0 && fd_stat == 0 &&
           path_mode == 0640 && fd_mode == 0640 &&
           close_status == 0 && second == 0027 ? 52 : 83;
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
        Some(52),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert!(qemu_stdout.starts_with("raw-openat-mode:"));
    assert!(qemu_stdout.ends_with(":0:0:640:640:0:027\n"));

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
    assert!(stdout.contains("\"stop_code\":52"));
    assert!(stdout.contains("\"text\":\"raw-openat-mode:000:3:0:0:640:640:0:027\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 52, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_chmod_family_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw chmod smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw chmod smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-chmod");
    let source = workspace.join("raw-chmod.c");
    let binary = workspace.join("raw-chmod");
    let mapped_input = workspace.join("sub").join("mapped.txt");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_WRONLY 01
#define O_CREAT 0100
#define O_TRUNC 01000
#define O_DIRECTORY 0200000

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

static unsigned int read_u32_le(const unsigned char *bytes, unsigned long offset) {
    return ((unsigned int)bytes[offset]) |
           ((unsigned int)bytes[offset + 1] << 8) |
           ((unsigned int)bytes[offset + 2] << 16) |
           ((unsigned int)bytes[offset + 3] << 24);
}

static void fill_bytes(unsigned char *bytes, unsigned long count, unsigned char value) {
    for (unsigned long index = 0; index < count; ++index) {
        bytes[index] = value;
    }
}

static void write_stdout(const char *text, long length) {
    linux_syscall3(64, 1, (long)text, length);
}

int main(void) {
    unsigned char fd_stat_bytes[160];
    unsigned char path_stat_bytes[160];
    unsigned char child_stat_bytes[160];
    unsigned char fchmodat2_stat_bytes[160];
    fill_bytes(fd_stat_bytes, sizeof(fd_stat_bytes), 0xa5);
    fill_bytes(path_stat_bytes, sizeof(path_stat_bytes), 0xa5);
    fill_bytes(child_stat_bytes, sizeof(child_stat_bytes), 0xa5);
    fill_bytes(fchmodat2_stat_bytes, sizeof(fchmodat2_stat_bytes), 0xa5);

    long fd = linux_syscall4(56, AT_FDCWD, (long)"created.txt",
                             O_WRONLY | O_CREAT | O_TRUNC, 0666);
    long fchmod_status = fd >= 0 ? linux_syscall2(52, fd, 0700) : -99;
    long fd_stat = fd >= 0 ? linux_syscall2(80, fd, (long)fd_stat_bytes) : -99;
    unsigned int fd_mode = fd_stat == 0 ? read_u32_le(fd_stat_bytes, 16) : 0;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;

    long chmod_status = linux_syscall2(1028, (long)"created.txt", 0600);
    long chmod_fallback = chmod_status == -38 ?
        linux_syscall4(53, AT_FDCWD, (long)"created.txt", 0600, 0) : 0;
    long path_stat = linux_syscall4(79, AT_FDCWD, (long)"created.txt",
                                    (long)path_stat_bytes, 0);
    unsigned int path_mode = path_stat == 0 ? read_u32_le(path_stat_bytes, 16) : 0;

    long dirfd = linux_syscall4(56, AT_FDCWD, (long)"sub",
                                O_RDONLY | O_DIRECTORY, 0);
    long fchmodat_status = dirfd >= 0 ? linux_syscall4(53, dirfd, (long)"mapped.txt", 0640, 0) : -99;
    long child_stat = linux_syscall4(79, AT_FDCWD, (long)"sub/mapped.txt",
                                     (long)child_stat_bytes, 0);
    unsigned int child_mode = child_stat == 0 ? read_u32_le(child_stat_bytes, 16) : 0;
    long dir_close_status = dirfd >= 0 ? linux_syscall1(57, dirfd) : -99;

    long fchmodat2_status = linux_syscall4(452, AT_FDCWD, (long)"sub/mapped.txt", 0660, 0);
    long fchmodat2_fallback = fchmodat2_status == -38 ?
        linux_syscall4(53, AT_FDCWD, (long)"sub/mapped.txt", 0660, 0) : 0;
    long fchmodat2_stat = linux_syscall4(79, AT_FDCWD, (long)"sub/mapped.txt",
                                         (long)fchmodat2_stat_bytes, 0);
    unsigned int fchmodat2_mode = fchmodat2_stat == 0 ? read_u32_le(fchmodat2_stat_bytes, 16) : 0;

    int ok = fd >= 0 &&
             fchmod_status == 0 &&
             fd_stat == 0 &&
             fd_mode == 0100700 &&
             close_status == 0 &&
             ((chmod_status == 0 && chmod_fallback == 0) ||
              (chmod_status == -38 && chmod_fallback == 0)) &&
             path_stat == 0 &&
             path_mode == 0100600 &&
             dirfd >= 0 &&
             fchmodat_status == 0 &&
             child_stat == 0 &&
             child_mode == 0100640 &&
             dir_close_status == 0 &&
             ((fchmodat2_status == 0 && fchmodat2_fallback == 0) ||
              (fchmodat2_status == -38 && fchmodat2_fallback == 0)) &&
             fchmodat2_stat == 0 &&
             fchmodat2_mode == 0100660;
    if (ok && chmod_status == 0 && fchmodat2_status == 0) {
        write_stdout("raw-chmod:direct:fchmodat2-direct\n",
                     sizeof("raw-chmod:direct:fchmodat2-direct\n") - 1);
    } else if (ok && chmod_status == -38 && fchmodat2_status == 0) {
        write_stdout("raw-chmod:fallback:fchmodat2-direct\n",
                     sizeof("raw-chmod:fallback:fchmodat2-direct\n") - 1);
    } else if (ok && chmod_status == 0 && fchmodat2_status == -38) {
        write_stdout("raw-chmod:direct:fchmodat2-fallback\n",
                     sizeof("raw-chmod:direct:fchmodat2-fallback\n") - 1);
    } else if (ok && chmod_status == -38 && fchmodat2_status == -38) {
        write_stdout("raw-chmod:fallback:fchmodat2-fallback\n",
                     sizeof("raw-chmod:fallback:fchmodat2-fallback\n") - 1);
    } else {
        write_stdout("raw-chmod:fail\n", sizeof("raw-chmod:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 54 : 85);
    __builtin_unreachable();
}
"#,
    )
    .unwrap();
    fs::create_dir(workspace.join("sub")).unwrap();
    fs::write(&mapped_input, b"mapped input\n").unwrap();

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
        Some(54),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert!(
        qemu_stdout == "raw-chmod:fallback:fchmodat2-direct\n"
            || qemu_stdout == "raw-chmod:fallback:fchmodat2-fallback\n",
        "unexpected qemu stdout: {qemu_stdout}"
    );

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
            "--riscv-se-file",
            &format!("sub/mapped.txt={}", mapped_input.display()),
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
    assert!(stdout.contains("\"stop_code\":54"));
    assert!(stdout.contains("\"text\":\"raw-chmod:direct:fchmodat2-direct\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 54, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_chown_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw chown smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw chown smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-chown");
    let source = workspace.join("raw-chown.c");
    let binary = workspace.join("raw-chown");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define AT_EMPTY_PATH 0x1000L
#define O_RDONLY 0

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

static void write_stdout(const char *bytes, long len) {
    linux_syscall3(64, 1, (long)bytes, len);
}

int main(void) {
    long present = linux_syscall5(54, AT_FDCWD, (long)"guest.txt", -1L, -1L, 0);
    long missing = linux_syscall5(54, AT_FDCWD, (long)"missing.txt", -1L, -1L, 0);
    long bad_flags = linux_syscall5(54, AT_FDCWD, (long)"guest.txt", -1L, -1L, 0x8000);
    long fd = linux_syscall4(56, AT_FDCWD, (long)"guest.txt", O_RDONLY, 0);
    long by_fd = fd >= 0 ? linux_syscall3(55, fd, -1L, -1L) : -99;
    long empty_fd = fd >= 0 ? linux_syscall5(54, fd, (long)"", -1L, -1L, AT_EMPTY_PATH) : -99;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    long bad_fd = linux_syscall5(54, 99, (long)"", -1L, -1L, AT_EMPTY_PATH);
    int ok = present == 0 &&
             missing == -2 &&
             bad_flags == -22 &&
             fd == 3 &&
             by_fd == 0 &&
             empty_fd == 0 &&
             close_status == 0 &&
             bad_fd == -9;
    if (ok) {
        write_stdout("raw-chown:ok\n", sizeof("raw-chown:ok\n") - 1);
    } else {
        write_stdout("raw-chown:fail\n", sizeof("raw-chown:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 55 : 85);
    __builtin_unreachable();
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();

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
        Some(55),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-chown:ok\n");

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
    assert!(stdout.contains("\"stop_code\":55"));
    assert!(stdout.contains("\"text\":\"raw-chown:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 55, "constant");
}
