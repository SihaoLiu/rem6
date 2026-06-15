use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use crate::support::temp_workspace;

#[test]
fn rem6_run_riscv_se_runs_static_raw_statx_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw statx smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw statx smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-statx");
    let source = workspace.join("raw-statx.c");
    let binary = workspace.join("raw-statx");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define AT_FDCWD (-100L)
#define AT_EMPTY_PATH 0x1000L
#define AT_NO_AUTOMOUNT 0x800L
#define AT_STATX_SYNC_TYPE 0x6000L
#define O_RDONLY 0
#define O_DIRECTORY 0200000
#define STATX_BASIC_STATS 0x000007ffU
#define STATX_RESERVED 0x80000000U
#define S_IFMT 0170000U
#define S_IFREG 0100000U

struct statx_timestamp {
    int64_t tv_sec;
    uint32_t tv_nsec;
    int32_t __reserved;
};

struct statx {
    uint32_t stx_mask;
    uint32_t stx_blksize;
    uint64_t stx_attributes;
    uint32_t stx_nlink;
    uint32_t stx_uid;
    uint32_t stx_gid;
    uint16_t stx_mode;
    uint16_t __spare0[1];
    uint64_t stx_ino;
    uint64_t stx_size;
    uint64_t stx_blocks;
    uint64_t stx_attributes_mask;
    struct statx_timestamp stx_atime;
    struct statx_timestamp stx_btime;
    struct statx_timestamp stx_ctime;
    struct statx_timestamp stx_mtime;
    uint32_t stx_rdev_major;
    uint32_t stx_rdev_minor;
    uint32_t stx_dev_major;
    uint32_t stx_dev_minor;
    uint64_t stx_mnt_id;
    uint32_t stx_dio_mem_align;
    uint32_t stx_dio_offset_align;
    uint64_t __spare3[12];
};

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
    struct statx st;
    struct statx missing_st;
    memset(&st, 0xa5, sizeof st);
    memset(&missing_st, 0x5a, sizeof missing_st);

    long present = linux_syscall5(291, AT_FDCWD, (long)"guest.txt",
                                  AT_NO_AUTOMOUNT, STATX_BASIC_STATS, (long)&st);
    long missing = linux_syscall5(291, AT_FDCWD, (long)"missing.txt",
                                  0, STATX_BASIC_STATS, (long)&missing_st);
    long invalid_mask = linux_syscall5(291, AT_FDCWD, (long)"guest.txt",
                                       0, STATX_RESERVED, (long)&missing_st);
    long bad_sync = linux_syscall5(291, AT_FDCWD, (long)"guest.txt",
                                   AT_STATX_SYNC_TYPE, STATX_BASIC_STATS, (long)&missing_st);
    long empty_cwd = linux_syscall5(291, AT_FDCWD, (long)"",
                                    AT_EMPTY_PATH, STATX_BASIC_STATS, (long)&missing_st);
    long dirfd = linux_syscall5(56, AT_FDCWD, (long)"sub",
                                O_RDONLY | O_DIRECTORY, 0, 0);
    long dirfd_relative = -99;
    long close_status = -99;
    if (dirfd >= 0) {
        dirfd_relative = linux_syscall5(291, dirfd, (long)"guest.txt",
                                        0, STATX_BASIC_STATS, (long)&missing_st);
        close_status = linux_syscall5(57, dirfd, 0, 0, 0, 0);
    }

    unsigned int mode = st.stx_mode & 0777777U;
    printf("raw-statx:%ld:%llu:%o:%u:%ld\n",
           present, (unsigned long long)st.stx_size, mode, st.stx_nlink, missing);
    printf("raw-statx-edges:%ld:%ld:%ld:%ld:%ld:%ld\n",
           invalid_mask, bad_sync, empty_cwd, dirfd, dirfd_relative, close_status);

    return present == 0 &&
           (st.stx_mask & STATX_BASIC_STATS) == STATX_BASIC_STATS &&
           st.stx_size == 18 &&
           (st.stx_mode & S_IFMT) == S_IFREG &&
           (st.stx_mode & 0777U) == 0444U &&
           st.stx_nlink == 1 &&
           missing == -2 &&
           invalid_mask == -22 &&
           bad_sync == -22 &&
           empty_cwd == 0 &&
           dirfd == 3 &&
           dirfd_relative == 0 &&
           close_status == 0 ? 62 : 63;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o444)).unwrap();
    fs::create_dir(workspace.join("sub")).unwrap();
    let nested_input = workspace.join("sub").join("guest.txt");
    fs::write(&nested_input, b"file-backed input\n").unwrap();
    fs::set_permissions(&nested_input, fs::Permissions::from_mode(0o444)).unwrap();

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
        Some(62),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"raw-statx:0:18:100444:1:-2\nraw-statx-edges:-22:-22:0:3:0:0\n"
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
            &format!("guest.txt={}", input.display()),
            "--riscv-se-file",
            &format!("sub/guest.txt={}", nested_input.display()),
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
    assert!(stdout.contains("\"stop_code\":62"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("raw-statx:0:18:100444:1:-2"));
    assert!(stdout.contains("raw-statx-edges:-22:-22:0:3:0:0"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_utimensat_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw utimensat smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw utimensat smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-utimensat");
    let source = workspace.join("raw-utimensat.c");
    let binary = workspace.join("raw-utimensat");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define AT_EMPTY_PATH 0x1000L
#define O_RDONLY 0
#define UTIME_NOW ((1L << 30) - 1L)
#define UTIME_OMIT ((1L << 30) - 2L)

struct timespec64 {
    long tv_sec;
    long tv_nsec;
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

static void write_stdout(const char *bytes, long len) {
    linux_syscall3(64, 1, (long)bytes, len);
}

int main(void) {
    struct timespec64 valid[2] = {{7, 123456789L}, {0, UTIME_OMIT}};
    struct timespec64 current[2] = {{0, UTIME_NOW}, {0, UTIME_OMIT}};
    struct timespec64 invalid[2] = {{0, 1000000000L}, {0, 0}};
    long present = linux_syscall4(88, AT_FDCWD, (long)"guest.txt", (long)valid, 0);
    long null_times = linux_syscall4(88, AT_FDCWD, (long)"guest.txt", 0, 0);
    long missing = linux_syscall4(88, AT_FDCWD, (long)"missing.txt", (long)valid, 0);
    long bad_flags = linux_syscall4(88, AT_FDCWD, (long)"guest.txt", (long)valid, 0x8000);
    long bad_nsec = linux_syscall4(88, AT_FDCWD, (long)"guest.txt", (long)invalid, 0);
    long fd = linux_syscall4(56, AT_FDCWD, (long)"guest.txt", O_RDONLY, 0);
    long empty_fd = fd >= 0 ? linux_syscall4(88, fd, (long)"", (long)current, AT_EMPTY_PATH) : -99;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    long bad_fd = linux_syscall4(88, 99, (long)"", (long)valid, AT_EMPTY_PATH);
    int ok = present == 0 &&
             null_times == 0 &&
             missing == -2 &&
             bad_flags == -22 &&
             bad_nsec == -22 &&
             fd == 3 &&
             empty_fd == 0 &&
             close_status == 0 &&
             bad_fd == -9;
    if (ok) {
        write_stdout("raw-utimensat:ok\n", sizeof("raw-utimensat:ok\n") - 1);
    } else {
        write_stdout("raw-utimensat:fail\n", sizeof("raw-utimensat:fail\n") - 1);
    }
    linux_syscall1(93, ok ? 46 : 87);
    __builtin_unreachable();
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o444)).unwrap();

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
        Some(46),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-utimensat:ok\n");

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
    assert!(stdout.contains("\"stop_code\":46"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("raw-utimensat:ok"));
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
