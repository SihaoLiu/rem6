use std::{fs, process::Command};

use serde_json::Value;

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_renameat_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw renameat smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-renameat");
    let source = workspace.join("raw-renameat.c");
    let binary = workspace.join("raw-renameat");
    let input = workspace.join("guest.txt");
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)

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

int main(void) {
    long renamed = linux_syscall5(38, AT_FDCWD, (long)"guest.txt", AT_FDCWD, (long)"renamed.txt", 0x7fff0000L);
    long old_after = linux_syscall4(48, AT_FDCWD, (long)"guest.txt", 0, 0);
    long new_after = linux_syscall4(48, AT_FDCWD, (long)"renamed.txt", 0, 0);

    printf("raw-renameat:%ld:%ld:%ld\n", renamed, old_after, new_after);
    return renamed == 0 && old_after == -2 && new_after == 0 ? 44 : 76;
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
    if qemu_output.status.code() == Some(44) {
        assert_eq!(qemu_output.stdout, b"raw-renameat:0:-2:0\n");
    } else {
        assert_eq!(
            qemu_output.status.code(),
            Some(76),
            "qemu stdout: {}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stdout),
            String::from_utf8_lossy(&qemu_output.stderr),
        );
        assert_eq!(qemu_output.stdout, b"raw-renameat:-38:0:-2\n");
        eprintln!("qemu-riscv64 reports ENOSYS for raw renameat; checking rem6 SE coverage");
    }
    fs::write(&input, b"file-backed input\n").unwrap();

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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":44"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-renameat:0:-2:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_renameat2_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat2 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw renameat2 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-renameat2");
    let source = workspace.join("raw-renameat2.c");
    let binary = workspace.join("raw-renameat2");
    let input = workspace.join("guest.txt");
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)

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

int main(void) {
    long renamed = linux_syscall5(276, AT_FDCWD, (long)"guest.txt", AT_FDCWD, (long)"renamed.txt", 0);
    long old_after = linux_syscall4(48, AT_FDCWD, (long)"guest.txt", 0, 0);
    long new_after = linux_syscall4(48, AT_FDCWD, (long)"renamed.txt", 0, 0);

    printf("raw-renameat2:%ld:%ld:%ld\n", renamed, old_after, new_after);
    return renamed == 0 && old_after == -2 && new_after == 0 ? 43 : 75;
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
        Some(43),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr),
    );
    assert_eq!(qemu_output.stdout, b"raw-renameat2:0:-2:0\n");
    fs::write(&input, b"file-backed input\n").unwrap();

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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":43"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-renameat2:0:-2:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_renameat2_noreplace_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat2 noreplace smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat2 noreplace smoke: qemu-riscv64 not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-renameat2-noreplace");
    let source = workspace.join("raw-renameat2-noreplace.c");
    let binary = workspace.join("raw-renameat2-noreplace");
    let old = workspace.join("old.txt");
    let new = workspace.join("new.txt");
    fs::write(&old, b"old\n").unwrap();
    fs::write(&new, b"new\n").unwrap();
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define RENAME_NOREPLACE 1L

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

static void sep(void) {
    putc(':');
}

void _start(void) {
    long existing = linux_syscall5(276, AT_FDCWD, (long)"old.txt", AT_FDCWD, (long)"new.txt", RENAME_NOREPLACE);
    long old_after_existing = linux_syscall4(48, AT_FDCWD, (long)"old.txt", 0, 0);
    long new_after_existing = linux_syscall4(48, AT_FDCWD, (long)"new.txt", 0, 0);
    long missing = linux_syscall5(276, AT_FDCWD, (long)"old.txt", AT_FDCWD, (long)"third.txt", RENAME_NOREPLACE);
    long old_after_missing = linux_syscall4(48, AT_FDCWD, (long)"old.txt", 0, 0);
    long third_after_missing = linux_syscall4(48, AT_FDCWD, (long)"third.txt", 0, 0);
    long new_after_missing = linux_syscall4(48, AT_FDCWD, (long)"new.txt", 0, 0);

    puts("raw-renameat2-noreplace:");
    putn(existing);
    sep();
    putn(old_after_existing);
    sep();
    putn(new_after_existing);
    sep();
    putn(missing);
    sep();
    putn(old_after_missing);
    sep();
    putn(third_after_missing);
    sep();
    putn(new_after_missing);
    putc('\n');
    if (existing == -17 && old_after_existing == 0 && new_after_existing == 0 &&
        missing == 0 && old_after_missing == -2 && third_after_missing == 0 &&
        new_after_missing == 0) {
        linux_syscall1(93, 42);
    }
    linux_syscall1(93, 77);
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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .current_dir(&workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(42),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr),
    );
    let expected_stdout = "raw-renameat2-noreplace:-17:0:0:0:-2:0:0\n";
    assert_eq!(qemu_output.stdout, expected_stdout.as_bytes());
    fs::write(&old, b"old\n").unwrap();
    fs::write(&new, b"new\n").unwrap();

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
            &format!("old.txt={}", old.display()),
            "--riscv-se-file",
            &format!("new.txt={}", new.display()),
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":42"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    let guest_stdout = json
        .pointer("/riscv_guest_writes")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|write| write.get("text").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(guest_stdout, expected_stdout);
    assert_eq!(
        json.pointer("/riscv_unknown_syscalls")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_renameat2_exchange_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat2 exchange smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw renameat2 exchange smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-renameat2-exchange");
    let source = workspace.join("raw-renameat2-exchange.c");
    let binary = workspace.join("raw-renameat2-exchange");
    let old = workspace.join("old.txt");
    let new = workspace.join("new.txt");
    fs::write(&old, b"old\n").unwrap();
    fs::write(&new, b"new\n").unwrap();
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define RENAME_EXCHANGE 2L

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

static int same4(const char *a, const char *b) {
    return a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3];
}

static long read4(const char *path, char *buffer) {
    long fd = linux_syscall4(56, AT_FDCWD, (long)path, 0, 0);
    if (fd < 0) {
        return fd;
    }
    long count = linux_syscall3(63, fd, (long)buffer, 4);
    linux_syscall1(57, fd);
    return count;
}

static void sep(void) {
    putc(':');
}

void _start(void) {
    char old_buffer[4] = {0, 0, 0, 0};
    char new_buffer[4] = {0, 0, 0, 0};
    long exchanged = linux_syscall5(276, AT_FDCWD, (long)"old.txt", AT_FDCWD, (long)"new.txt", RENAME_EXCHANGE);
    long old_read = read4("old.txt", old_buffer);
    long new_read = read4("new.txt", new_buffer);
    long old_has_new = same4(old_buffer, "new\n");
    long new_has_old = same4(new_buffer, "old\n");

    puts("raw-renameat2-exchange:");
    putn(exchanged);
    sep();
    putn(old_read);
    sep();
    putn(new_read);
    sep();
    putn(old_has_new);
    sep();
    putn(new_has_old);
    putc('\n');
    if (exchanged == 0 && old_read == 4 && new_read == 4 && old_has_new == 1 && new_has_old == 1) {
        linux_syscall1(93, 46);
    }
    linux_syscall1(93, 78);
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
        String::from_utf8_lossy(&qemu_output.stderr),
    );
    let expected_stdout = "raw-renameat2-exchange:0:4:4:1:1\n";
    assert_eq!(qemu_output.stdout, expected_stdout.as_bytes());
    fs::write(&old, b"old\n").unwrap();
    fs::write(&new, b"new\n").unwrap();

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
            &format!("old.txt={}", old.display()),
            "--riscv-se-file",
            &format!("new.txt={}", new.display()),
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":46"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    let guest_stdout = json
        .pointer("/riscv_guest_writes")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|write| write.get("text").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(guest_stdout, expected_stdout);
    assert_eq!(
        json.pointer("/riscv_unknown_syscalls")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}
