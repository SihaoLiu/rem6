use std::{fs, process::Command};

use serde_json::Value;

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_set_mempolicy_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw set_mempolicy smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw set_mempolicy smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-set-mempolicy");
    let source = workspace.join("raw-set-mempolicy.c");
    let binary = workspace.join("raw-set-mempolicy");
    fs::write(
        &source,
        r#"#include <stdio.h>

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

int main(void) {
    unsigned long mask = 1;
    unsigned long bad_mask = 2;
    int mode = -1;
    int mode_after_bad = -1;
    int reset_mode = -1;
    unsigned long out_mask = ~0UL;
    unsigned long out_after_bad = ~0UL;
    unsigned long reset_mask = ~0UL;

    long set = linux_syscall3(237, 2, (long)&mask, 2);
    long get = linux_syscall5(236, (long)&mode, (long)&out_mask, 64, 0, 0);
    long bad_set = linux_syscall3(237, 2, (long)&bad_mask, 3);
    long get_after_bad = linux_syscall5(236, (long)&mode_after_bad, (long)&out_after_bad, 64, 0, 0);
    long reset = linux_syscall3(237, 0, 0, 0);
    long get_after_reset = linux_syscall5(236, (long)&reset_mode, (long)&reset_mask, 64, 0, 0);

    int ok = set == 0
        && get == 0
        && mode == 2
        && out_mask == 1
        && bad_set == -22
        && get_after_bad == 0
        && mode_after_bad == 2
        && out_after_bad == 1
        && reset == 0
        && get_after_reset == 0
        && reset_mode == 0
        && reset_mask == 0;
    printf("raw-set-mempolicy:%ld:%ld:%d:%lu:%ld:%ld:%d:%lu:%ld:%ld:%d:%lu:%d\n",
           set, get, mode, out_mask, bad_set, get_after_bad, mode_after_bad,
           out_after_bad, reset, get_after_reset, reset_mode, reset_mask, ok);
    return ok ? 71 : 91;
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
    if qemu_output.status.code() == Some(71) {
        assert_eq!(
            qemu_output.stdout,
            b"raw-set-mempolicy:0:0:2:1:-22:0:2:1:0:0:0:0:1\n"
        );
    } else {
        assert_eq!(
            qemu_output.status.code(),
            Some(91),
            "qemu stdout: {}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stdout),
            String::from_utf8_lossy(&qemu_output.stderr)
        );
        assert_eq!(
            qemu_output.stdout,
            b"raw-set-mempolicy:-38:-38:-1:18446744073709551615:-38:-38:-1:18446744073709551615:-38:-38:-1:18446744073709551615:0\n"
        );
        eprintln!(
            "qemu-riscv64 reports ENOSYS for raw set/get_mempolicy; checking rem6 SE coverage"
        );
    }

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
    assert!(stdout.contains("\"stop_code\":71"));
    assert!(stdout.contains("\"text\":\"raw-set-mempolicy:0:0:2:1:-22:0:2:1:0:0:0:0:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_get_mempolicy_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw get_mempolicy smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw get_mempolicy smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-get-mempolicy");
    let source = workspace.join("raw-get-mempolicy.c");
    let binary = workspace.join("raw-get-mempolicy");
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
    int mode = -1;
    int mode_only_value = -1;
    unsigned long mask = ~0UL;
    long full = linux_syscall5(236, (long)&mode, (long)&mask, 64, 0, 0);
    long mode_only = linux_syscall5(236, (long)&mode_only_value, 0, 0, 0, 0);
    long invalid_maxnode = linux_syscall5(236, 0, (long)&mask, 0, 0, 0);
    long invalid_flags = linux_syscall5(236, 0, 0, 0, 0, 1);

    int ok = full == 0
        && mode == 0
        && mask == 0
        && mode_only == 0
        && mode_only_value == 0
        && invalid_maxnode == -22
        && invalid_flags == -22;
    printf("raw-get-mempolicy:%ld:%d:%lu:%ld:%d:%ld:%ld:%d\n",
           full, mode, mask, mode_only, mode_only_value, invalid_maxnode, invalid_flags, ok);
    return ok ? 70 : 90;
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
    if qemu_output.status.code() == Some(70) {
        assert_eq!(
            qemu_output.stdout,
            b"raw-get-mempolicy:0:0:0:0:0:-22:-22:1\n"
        );
    } else {
        assert_eq!(
            qemu_output.status.code(),
            Some(90),
            "qemu stdout: {}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stdout),
            String::from_utf8_lossy(&qemu_output.stderr)
        );
        assert_eq!(
            qemu_output.stdout,
            b"raw-get-mempolicy:-38:-1:18446744073709551615:-38:-1:-38:-38:0\n"
        );
        eprintln!("qemu-riscv64 reports ENOSYS for raw get_mempolicy; checking rem6 SE coverage");
    }

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
    assert!(stdout.contains("\"stop_code\":70"));
    assert!(stdout.contains("\"text\":\"raw-get-mempolicy:0:0:0:0:0:-22:-22:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_madvise_dontneed_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw madvise smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw madvise smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-madvise");
    let source = workspace.join("raw-madvise.c");
    let binary = workspace.join("raw-madvise");
    fs::write(
        &source,
        r#"#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_PRIVATE 2
#define MAP_ANONYMOUS 0x20
#define MADV_DONTNEED 4

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

int main(void) {
    const char *ok = "raw-madvise-dontneed:ok\n";
    const char *fail = "raw-madvise-dontneed:fail\n";
    unsigned char *mapped = (unsigned char *)linux_syscall6(
        222, 0, 8192, PROT_READ | PROT_WRITE,
        MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if ((long)mapped < 0) {
        linux_syscall3(64, 1, (long)fail, 26);
        linux_syscall1(93, 88);
    }

    mapped[0] = 0x5a;
    mapped[4095] = 0x6b;
    mapped[4096] = 0x7c;
    long advised = linux_syscall3(233, (long)mapped, 4096, MADV_DONTNEED);

    if (advised == 0 && mapped[0] == 0 && mapped[4095] == 0 && mapped[4096] == 0x7c) {
        linux_syscall3(64, 1, (long)ok, 24);
        linux_syscall1(93, 68);
    }
    linux_syscall3(64, 1, (long)fail, 26);
    linux_syscall1(93, 88);
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
        Some(68),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-madvise-dontneed:ok\n");

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
    assert!(stdout.contains("\"stop_code\":68"));
    assert!(stdout.contains("\"text\":\"raw-madvise-dontneed:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_restores_file_backed_madvise_dontneed_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE file-backed madvise smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE file-backed madvise smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-file-backed-madvise");
    let source = workspace.join("file-backed-madvise.c");
    let binary = workspace.join("file-backed-madvise");
    let input = workspace.join("guest.txt");
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::write(
        &source,
        r#"#define AT_FDCWD -100
#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_PRIVATE 2
#define MADV_DONTNEED 4

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

int main(void) {
    const char *ok = "raw-madvise-file:ok\n";
    const char *fail = "raw-madvise-file:fail\n";
    int fd = (int)linux_syscall6(56, AT_FDCWD, (long)"guest.txt", 0, 0, 0, 0);
    if (fd < 0) {
        linux_syscall3(64, 1, (long)fail, 22);
        linux_syscall1(93, 89);
    }

    unsigned char *mapped = (unsigned char *)linux_syscall6(
        222, 0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    linux_syscall1(57, fd);
    if ((long)mapped < 0) {
        linux_syscall3(64, 1, (long)fail, 22);
        linux_syscall1(93, 89);
    }

    mapped[0] = 'X';
    mapped[1] = 'Y';
    long advised = linux_syscall3(233, (long)mapped, 4096, MADV_DONTNEED);
    if (advised == 0 && mapped[0] == 'f' && mapped[1] == 'i' && mapped[2] == 'l') {
        linux_syscall3(64, 1, (long)ok, 20);
        linux_syscall1(93, 69);
    }
    linux_syscall3(64, 1, (long)fail, 22);
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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .current_dir(&workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(69),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-madvise-file:ok\n");

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
    assert!(stdout.contains("\"stop_code\":69"));
    assert!(stdout.contains("\"text\":\"raw-madvise-file:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_mlock2_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw mlock2 smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw mlock2 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-mlock2");
    let source = workspace.join("raw-mlock2.c");
    let binary = workspace.join("raw-mlock2");
    fs::write(
        &source,
        r#"static long linux_syscall1(long number, long arg0) {
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

static long linux_syscall6(long number, long arg0, long arg1, long arg2, long arg3, long arg4, long arg5) {
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

static void putc(char c) {
    linux_syscall3(64, 1, (long)&c, 1);
}

static void puts(const char *text) {
    while (*text) {
        putc(*text++);
    }
}

static void putn(long value) {
    char bytes[32];
    int len = 0;
    if (value < 0) {
        putc('-');
        value = -value;
    }
    if (value == 0) {
        putc('0');
        return;
    }
    while (value != 0) {
        bytes[len++] = (char)('0' + (value % 10));
        value /= 10;
    }
    while (len != 0) {
        putc(bytes[--len]);
    }
}

static void print_result(long a, long b, long c, long d, long e, long f) {
    puts("raw-mlock2:");
    putn(a);
    putc(':');
    putn(b);
    putc(':');
    putn(c);
    putc(':');
    putn(d);
    putc(':');
    putn(e);
    putc(':');
    putn(f);
    putc('\n');
}

void _start(void) {
    long mapped = linux_syscall6(222, 0, 8192, 3, 34, -1, 0);
    long lock_now = linux_syscall3(284, mapped, 4096, 0);
    long lock_onfault = linux_syscall3(284, mapped + 4096, 4096, 1);
    long lock_high = linux_syscall3(284, mapped, 0, (1L << 32) | 1);
    long bad_flags = linux_syscall3(284, mapped, 4096, 2);
    long unmapped = linux_syscall3(284, 0x700000000000L, 4096, 1);
    long overflow = linux_syscall3(284, -1, 1, 0);
    print_result(lock_now, lock_onfault, lock_high, bad_flags, unmapped, overflow);
    if (lock_now == 0 && lock_onfault == 0 && lock_high == 0 &&
        bad_flags == -22 && unmapped == -12 && overflow == -22) {
        linux_syscall1(93, 68);
    }
    linux_syscall1(93, 88);
    for (;;) {
    }
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-nostdlib",
            "-fno-builtin",
            "-fno-stack-protector",
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
    if qemu_output.status.code() == Some(68) {
        assert_eq!(qemu_output.stdout, b"raw-mlock2:0:0:0:-22:-12:-22\n");
    } else {
        assert_eq!(
            qemu_output.status.code(),
            Some(88),
            "qemu stdout: {}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stdout),
            String::from_utf8_lossy(&qemu_output.stderr)
        );
        assert_eq!(qemu_output.stdout, b"raw-mlock2:-38:-38:-38:-38:-38:-38\n");
        eprintln!("qemu-riscv64 reports ENOSYS for raw mlock2; checking rem6 SE coverage");
    }

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
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(68)
    );
    let guest_stdout = json
        .pointer("/riscv_guest_writes")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|write| write.get("text").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(guest_stdout, "raw-mlock2:0:0:0:-22:-12:-22\n");
    assert_eq!(
        json.pointer("/riscv_unknown_syscalls")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}
