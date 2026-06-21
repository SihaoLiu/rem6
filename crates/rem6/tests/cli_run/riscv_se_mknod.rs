use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_mknodat_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw mknodat smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw mknodat smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-mknodat");
    let source = workspace.join("raw-mknodat.c");
    let binary = workspace.join("raw-mknodat");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define O_RDWR 02
#define S_IFREG 0100000
#define SEEK_SET 0

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
    const char *path = "node.reg";
    const char *payload = "ok";
    char buffer[3] = {0};
    const char *ok = "raw-mknodat:ok\n";
    const char *fail = "raw-mknodat:fail\n";

    long trailing = linux_syscall4(33, AT_FDCWD, (long)"node.reg/", S_IFREG | 0600, 0);
    long created = linux_syscall4(33, AT_FDCWD, (long)path, S_IFREG | 0600, 0);
    long duplicate = linux_syscall4(33, AT_FDCWD, (long)path, S_IFREG | 0600, 0);
    long fd = linux_syscall4(56, AT_FDCWD, (long)path, O_RDWR, 0);
    long written = fd >= 0 ? linux_syscall3(64, fd, (long)payload, 2) : fd;
    long seek = fd >= 0 ? linux_syscall3(62, fd, 0, SEEK_SET) : fd;
    long read_count = fd >= 0 ? linux_syscall3(63, fd, (long)buffer, 2) : fd;
    long closed = fd >= 0 ? linux_syscall1(57, fd) : fd;

    int passed = trailing == -2 && created == 0 && duplicate == -17 &&
                 fd >= 0 && written == 2 && seek == 0 && read_count == 2 &&
                 buffer[0] == 'o' && buffer[1] == 'k' && closed == 0;
    if (passed) {
        linux_syscall3(64, 1, (long)ok, 15);
        linux_syscall1(93, 68);
    } else {
        linux_syscall3(64, 1, (long)fail, 17);
        linux_syscall1(93, 86);
    }
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
        .current_dir(&workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(68),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-mknodat:ok\n");

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
    assert!(stdout.contains("\"stop_code\":68"));
    assert!(stdout.contains("\"riscv_guest_writes\":["));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-mknodat:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
