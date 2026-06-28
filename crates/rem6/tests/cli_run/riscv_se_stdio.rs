use std::{
    fs,
    process::{Command, Stdio},
};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_newlib_fgets_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE stdin smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE stdin smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-fgets");
    let source = workspace.join("stdin.c");
    let binary = workspace.join("stdin");
    let input = workspace.join("stdin.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
int main(void) {
    char buffer[32];
    if (fgets(buffer, sizeof(buffer), stdin) == NULL) {
        return 71;
    }
    printf("stdin:%s", buffer);
    return buffer[0] == 'r' ? 23 : 24;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"rem6 stdin\nignored\n").unwrap();

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
        .stdin(Stdio::from(fs::File::open(&input).unwrap()))
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(23),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"stdin:rem6 stdin\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "200000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-stdin",
            input.to_str().unwrap(),
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
    assert!(stdout.contains("\"stop_code\":23"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"stdin:rem6 stdin\\n\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 23, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_fopen_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE file smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-fopen");
    let source = workspace.join("file.c");
    let binary = workspace.join("file");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
int main(void) {
    FILE *file = fopen("guest.txt", "rb");
    if (file == NULL) {
        return 71;
    }

    char buffer[32];
    size_t count = fread(buffer, 1, 18, file);
    if (count != 18) {
        return 72;
    }
    buffer[count] = '\0';
    printf("file:%s", buffer);
    return buffer[0] == 'f' ? 31 : 32;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();

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
    assert!(stdout.contains("\"stop_code\":31"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"file:file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 31, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_file_create_roundtrip() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE file-create smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-file-create");
    let source = workspace.join("file_create.c");
    let binary = workspace.join("file_create");
    fs::write(
        &source,
        r#"#include <errno.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    FILE *file = fopen("created.txt", "w+");
    if (file == NULL) {
        printf("roundtrip:open:%d\n", errno);
        return 51;
    }
    if (fprintf(file, "alpha:%d\n", 17) < 0) {
        printf("roundtrip:write:%d\n", errno);
        fclose(file);
        return 52;
    }
    if (fflush(file) != 0) {
        printf("roundtrip:flush:%d\n", errno);
        fclose(file);
        return 53;
    }
    if (fseek(file, 0, SEEK_SET) != 0) {
        printf("roundtrip:seek:%d\n", errno);
        fclose(file);
        return 54;
    }
    char buffer[32] = {0};
    if (fgets(buffer, sizeof(buffer), file) == NULL) {
        printf("roundtrip:read:%d\n", errno);
        fclose(file);
        return 55;
    }
    fclose(file);
    printf("roundtrip:%s", buffer);
    return strcmp(buffer, "alpha:17\n") == 0 ? 56 : 57;
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
    assert!(stdout.contains("\"stop_code\":56"));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"roundtrip:alpha:17\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 56, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_file_write_read_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw file output smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw file output smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-file-output");
    let source = workspace.join("raw-file-output.c");
    let binary = workspace.join("raw-file-output");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char buffer[64];
    long created = linux_syscall4(56, -100, (long)"out.txt", 01 | 0100 | 01000, 0644);
    long wrote = created < 0 ? created : linux_syscall3(64, created, (long)"file-output:42\n", 15);
    long closed = created < 0 ? -1 : linux_syscall1(57, created);
    long opened = linux_syscall4(56, -100, (long)"out.txt", 0, 0);
    long read_count = opened < 0 ? opened : linux_syscall3(63, opened, (long)buffer, 63);
    long closed_read = opened < 0 ? -1 : linux_syscall1(57, opened);
    if (read_count > 0 && read_count < 64) {
        buffer[read_count] = '\0';
    } else {
        buffer[0] = '\0';
    }
    printf("raw-file-output:%ld:%ld:%ld:%ld:%ld:%ld:%s",
           created, wrote, closed, opened, read_count, closed_read, buffer);
    return created >= 0 &&
           wrote == 15 &&
           closed == 0 &&
           opened >= 0 &&
           read_count == 15 &&
           closed_read == 0 &&
           strcmp(buffer, "file-output:42\n") == 0 ? 57 : 58;
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
        Some(57),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"raw-file-output:3:15:0:3:15:0:file-output:42\n"
    );

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
    assert!(stdout.contains("\"stop_code\":57"));
    assert!(stdout.contains("\"text\":\"raw-file-output:3:15:0:3:15:0:file-output:42\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 57, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_append_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE append smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE append smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-append");
    let qemu_workspace = workspace.join("qemu");
    fs::create_dir(&qemu_workspace).unwrap();
    let source = workspace.join("append.c");
    let binary = workspace.join("append");
    let qemu_input = qemu_workspace.join("guest.txt");
    let rem6_input = workspace.join("rem6-guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char buffer[64];
    long opened = linux_syscall4(56, -100, (long)"guest.txt", 02 | 02000, 0);
    long wrote = opened < 0 ? opened : linux_syscall3(64, opened, (long)"append:new\n", 11);
    long seek = opened < 0 ? -1 : linux_syscall3(62, opened, 0, 0);
    long read_count = seek < 0 ? seek : linux_syscall3(63, opened, (long)buffer, 63);
    if (read_count > 0 && read_count < 64) {
        buffer[read_count] = '\0';
    } else {
        buffer[0] = '\0';
    }
    printf("append:%ld:%ld:%ld:%ld:%s", opened, wrote, seek, read_count, buffer);
    return opened >= 0 &&
           wrote == 11 &&
           seek == 0 &&
           read_count == 16 &&
           strcmp(buffer, "seed\nappend:new\n") == 0 ? 59 : 60;
}
"#,
    )
    .unwrap();
    fs::write(&qemu_input, b"seed\n").unwrap();
    fs::write(&rem6_input, b"seed\n").unwrap();

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
        .current_dir(&qemu_workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(59),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"append:3:11:0:16:seed\nappend:new\n");

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
            &format!("guest.txt={}", rem6_input.display()),
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
    assert!(stdout.contains("\"stop_code\":59"));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"append:3:11:0:16:seed\\n\""));
    assert!(stdout.contains("\"text\":\"append:new\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 59, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_stat_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE stat smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-stat");
    let source = workspace.join("stat.c");
    let binary = workspace.join("stat");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>

int main(void) {
    struct stat st;
    if (stat("guest.txt", &st) != 0) {
        return 71;
    }
    printf("stat:%ld:%lo\n", (long)st.st_size, (unsigned long)(st.st_mode & 0777777));
    return st.st_size == 18 ? 33 : 34;
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
    assert!(stdout.contains("\"stop_code\":33"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"stat:18:100444\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 33, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_lstat_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE lstat smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-lstat");
    let source = workspace.join("lstat.c");
    let binary = workspace.join("lstat");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>

extern int _lstat(const char *path, struct stat *st);

int main(void) {
    struct stat st;
    if (_lstat("guest.txt", &st) != 0) {
        return 71;
    }
    printf("lstat:%ld:%lo\n", (long)st.st_size, (unsigned long)(st.st_mode & 0777777));
    return st.st_size == 18 ? 35 : 36;
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
    assert!(stdout.contains("\"stop_code\":35"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"lstat:18:100444\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 35, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_access_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE access smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-access");
    let source = workspace.join("access.c");
    let binary = workspace.join("access");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <errno.h>
#include <stdio.h>
#include <unistd.h>

extern int _access(const char *path, int mode);

int main(void) {
    errno = 0;
    int present = _access("guest.txt", R_OK);
    int present_errno = errno;
    errno = 0;
    int missing = _access("missing.txt", F_OK);
    int missing_errno = errno;
    printf("access:%d:%d:%d:%d\n", present, present_errno, missing, missing_errno);
    return present == 0 &&
           present_errno == 0 &&
           missing == -1 &&
           missing_errno == ENOENT ? 51 : 52;
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
    assert!(stdout.contains("\"stop_code\":51"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"access:0:0:-1:2\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 51, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_faccessat_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE faccessat smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE faccessat smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-faccessat");
    let source = workspace.join("faccessat.c");
    let binary = workspace.join("faccessat");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <errno.h>
#include <stdio.h>
#include <unistd.h>

extern int _faccessat(int dirfd, const char *path, int mode, int flags);

int main(void) {
    errno = 0;
    int present = _faccessat(-100, "guest.txt", R_OK, 0);
    int present_errno = errno;
    errno = 0;
    int missing = _faccessat(-100, "missing.txt", F_OK, 0);
    int missing_errno = errno;
    printf("faccessat:%d:%d:%d:%d\n", present, present_errno, missing, missing_errno);
    return present == 0 &&
           present_errno == 0 &&
           missing == -1 &&
           missing_errno == ENOENT ? 53 : 54;
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
        .current_dir(&workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(53),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"faccessat:0:0:-1:2\n");

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
    assert!(stdout.contains("\"stop_code\":53"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"faccessat:0:0:-1:2\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 53, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_faccessat2_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw faccessat2 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw faccessat2 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-faccessat2");
    let source = workspace.join("raw-faccessat2.c");
    let binary = workspace.join("raw-faccessat2");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
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

static void finish(int ok) {
    static const char pass[] = "raw-faccessat2:0:0:-2\n";
    static const char fail[] = "raw-faccessat2:fail\n";
    if (ok) {
        linux_syscall3(64, 1, (long)pass, sizeof(pass) - 1);
        linux_syscall3(93, 72, 0, 0);
    } else {
        linux_syscall3(64, 1, (long)fail, sizeof(fail) - 1);
        linux_syscall3(93, 73, 0, 0);
    }
    for (;;) {
    }
}

int main(void) {
    long present = linux_syscall4(439, -100, (long)"guest.txt", 4, 0);
    long present_eaccess = linux_syscall4(439, -100, (long)"guest.txt", 4, 0x200);
    long missing = linux_syscall4(439, -100, (long)"missing.txt", 0, 0);
    finish(present == 0 && present_eaccess == 0 && missing == -2);
    return 74;
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
        .current_dir(&workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(72),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-faccessat2:0:0:-2\n");

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
    assert!(stdout.contains("\"stop_code\":72"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-faccessat2:0:0:-2\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_unlink_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE unlink smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-unlink");
    let source = workspace.join("unlink.c");
    let binary = workspace.join("unlink");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(void) {
    int removed = unlink("guest.txt");
    struct stat st;
    int after = stat("guest.txt", &st);
    printf("unlink:%d:%d\n", removed, after);
    return removed == 0 && after != 0 ? 47 : 48;
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
    assert!(stdout.contains("\"stop_code\":47"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"unlink:0:-1\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_link_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE link smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-link");
    let source = workspace.join("link.c");
    let binary = workspace.join("link");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(void) {
    int made = link("guest.txt", "alias.txt");
    struct stat original;
    struct stat alias;
    int original_stat = stat("guest.txt", &original);
    int alias_stat = stat("alias.txt", &alias);
    printf("link:%d:%d:%d:%ld:%ld:%ld:%ld\n",
           made,
           original_stat,
           alias_stat,
           (long)original.st_size,
           (long)alias.st_size,
           (long)original.st_nlink,
           (long)alias.st_nlink);
    return made == 0 &&
           original_stat == 0 &&
           alias_stat == 0 &&
           alias.st_size == 18 &&
           original.st_ino == alias.st_ino &&
           original.st_nlink == 2 &&
           alias.st_nlink == 2 ? 49 : 50;
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
    assert!(stdout.contains("\"stop_code\":49"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"link:0:0:0:18:18:2:2\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 49, "constant");
}

#[test]
fn rem6_run_riscv_se_opens_registered_guest_file_from_host_bytes() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-registered-guest-file", &elf);
    let input = temp_binary(
        "riscv-se-registered-guest-file-input",
        b"file-backed input\nignored\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "240",
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
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 18, "constant");
}

#[test]
fn rem6_run_riscv_se_reads_stdin_from_manifest_input_resource() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-stdin-resource");
    fs::write(workspace.join("guest.elf"), elf).unwrap();
    fs::write(workspace.join("stdin.bin"), b"Sresource stdin\n").unwrap();
    fs::write(
        workspace.join("resource-acquire.toml"),
        r#"[resource_acquire]
workload_id = "riscv-se-stdin-resource"
boot_entry = 2147483648
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-stdin-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "guest.elf"
artifact_digest = "sha256:riscv-se-stdin-kernel"

[[resource_acquire.resources]]
id = "stdin"
kind = "input"
digest = "sha256:riscv-se-stdin-input"
locator = "resources/stdin.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://stdin"
artifact = "stdin.bin"
artifact_digest = "sha256:riscv-se-stdin-input"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"resource:stdin\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            workspace.join("run.toml").to_str().unwrap(),
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
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("\"stop_code\":83"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_reads_stdin_from_selected_suite_input_resource() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-suite-stdin-resource");
    fs::write(workspace.join("guest-a.elf"), &elf).unwrap();
    fs::write(workspace.join("guest-b.elf"), &elf).unwrap();
    fs::write(workspace.join("stdin-a.bin"), b"A-suite stdin\n").unwrap();
    fs::write(workspace.join("stdin-b.bin"), b"B-suite stdin\n").unwrap();
    fs::write(
        workspace.join("resource-acquire-suite.toml"),
        r#"[resource_acquire]
suite_id = "riscv-se-suite-stdin-resource"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "stdin-a-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-suite-stdin-kernel-a"
locator = "resources/kernel-a.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-a"
artifact = "guest-a.elf"
artifact_digest = "sha256:riscv-se-suite-stdin-kernel-a"

[[resource_acquire.manifests.resources]]
id = "stdin"
kind = "input"
digest = "sha256:riscv-se-suite-stdin-a"
locator = "resources/stdin-a.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://stdin-a"
artifact = "stdin-a.bin"
artifact_digest = "sha256:riscv-se-suite-stdin-a"

[[resource_acquire.manifests]]
workload_id = "stdin-b-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-suite-stdin-kernel-b"
locator = "resources/kernel-b.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-b"
artifact = "guest-b.elf"
artifact_digest = "sha256:riscv-se-suite-stdin-kernel-b"

[[resource_acquire.manifests.resources]]
id = "stdin"
kind = "input"
digest = "sha256:riscv-se-suite-stdin-b"
locator = "resources/stdin-b.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://stdin-b"
artifact = "stdin-b.bin"
artifact_digest = "sha256:riscv-se-suite-stdin-b"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nkernel_resource = \"suite-resource:stdin-b-workload/kernel\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"suite-resource:stdin-b-workload/stdin\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            workspace.join("run.toml").to_str().unwrap(),
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
        json.get("kernel_resource").and_then(Value::as_str),
        Some("suite-resource:stdin-b-workload/kernel")
    );
    assert_eq!(
        json.pointer("/riscv_se_inputs/stdin/source")
            .and_then(Value::as_str),
        Some("suite-resource:stdin-b-workload/stdin")
    );
    assert_eq!(
        json.pointer("/riscv_se_inputs/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":66"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_opens_guest_file_from_suite_input_resource() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-suite-guest-file-resource");
    fs::write(workspace.join("guest.elf"), elf).unwrap();
    fs::write(workspace.join("input.txt"), b"file-backed input\nignored\n").unwrap();
    fs::write(
        workspace.join("resource-acquire-suite.toml"),
        r#"[resource_acquire]
suite_id = "riscv-se-suite-resource"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:riscv-se-suite-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "guest.elf"
artifact_digest = "sha256:riscv-se-suite-kernel"

[[resource_acquire.manifests.resources]]
id = "input"
kind = "input"
digest = "sha256:riscv-se-suite-input"
locator = "resources/input.txt"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input"
artifact = "input.txt"
artifact_digest = "sha256:riscv-se-suite-input"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=suite-resource:boot-workload/input\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            workspace.join("run.toml").to_str().unwrap(),
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
        json.pointer("/riscv_se_inputs/files/0/guest_path")
            .and_then(Value::as_str),
        Some("guest.txt")
    );
    assert_eq!(
        json.pointer("/riscv_se_inputs/files/0/source")
            .and_then(Value::as_str),
        Some("suite-resource:boot-workload/input")
    );
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_toml_guest_file_resolves_from_config_directory() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-relative-guest-file");
    let binary = workspace.join("guest.elf");
    let input = workspace.join("input.txt");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=input.txt\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_toml_guest_file_accepts_explicit_path_prefix() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-prefixed-guest-file");
    let binary = workspace.join("guest.elf");
    let prefixed_dir = workspace.join("suite-resource:boot-workload");
    let input = prefixed_dir.join("input");
    let config = workspace.join("run.toml");
    fs::create_dir_all(&prefixed_dir).unwrap();
    fs::write(&binary, elf).unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=path:suite-resource:boot-workload/input\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_reports_missing_stdin_file() {
    let program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-missing-stdin", &elf);
    let stdin = temp_output("riscv-se-missing-stdin-input");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-stdin",
            stdin.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read RISC-V SE stdin {}",
        stdin.display()
    )));
}

#[test]
fn rem6_run_riscv_se_reports_missing_guest_file() {
    let program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-missing-guest-file", &elf);
    let input = temp_output("riscv-se-missing-guest-file-input");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read RISC-V SE file guest.txt from {}",
        input.display()
    )));
}

#[test]
fn rem6_run_riscv_se_toml_stdin_path_resolves_from_config_directory() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-relative-stdin");
    let binary = workspace.join("guest.elf");
    let stdin = workspace.join("stdin.txt");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&stdin, b"relative stdin\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"stdin.txt\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":114"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_toml_stdin_accepts_explicit_path_prefix() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-prefixed-stdin");
    let binary = workspace.join("guest.elf");
    let stdin = workspace.join("resource:stdin");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&stdin, b"path-prefixed stdin\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"path:resource:stdin\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":112"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

fn registered_guest_file_program() -> Vec<u8> {
    const PATH_OFFSET: usize = 0x80;
    const BUFFER_OFFSET: usize = 0xa0;

    let mut program = riscv64_program(&[
        i_type(-100, 0, 0x0, 10, 0x13), // addi a0, x0, AT_FDCWD
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(PATH_OFFSET as i32 - 4, 11, 0x0, 11, 0x13), // addi a1, a1, path
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, O_RDONLY
        i_type(0, 0, 0x0, 13, 0x13),    // addi a3, x0, 0
        i_type(56, 0, 0x0, 17, 0x13),   // addi a7, x0, 56
        0x0000_0073,                    // ecall
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(BUFFER_OFFSET as i32 - 28, 11, 0x0, 11, 0x13), // addi a1, a1, buffer
        i_type(18, 0, 0x0, 12, 0x13),   // addi a2, x0, 18
        i_type(63, 0, 0x0, 17, 0x13),   // addi a7, x0, 63
        0x0000_0073,                    // ecall
        i_type(1, 0, 0x0, 10, 0x13),    // addi a0, x0, 1
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(BUFFER_OFFSET as i32 - 52, 11, 0x0, 11, 0x13), // addi a1, a1, buffer
        i_type(18, 0, 0x0, 12, 0x13),   // addi a2, x0, 18
        i_type(64, 0, 0x0, 17, 0x13),   // addi a7, x0, 64
        0x0000_0073,                    // ecall
        i_type(18, 0, 0x0, 10, 0x13),   // addi a0, x0, 18
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, 93
        0x0000_0073,                    // ecall
    ]);
    program.resize(PATH_OFFSET, 0);
    program.extend_from_slice(b"guest.txt\0");
    program.resize(BUFFER_OFFSET + 32, 0);
    program
}
