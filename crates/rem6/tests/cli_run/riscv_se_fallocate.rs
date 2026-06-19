use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_fallocate_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw fallocate smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw fallocate smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-fallocate");
    let source = workspace.join("raw-fallocate.c");
    let binary = workspace.join("raw-fallocate");
    fs::write(
        &source,
        r#"#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_WRONLY 01
#define O_RDWR 02
#define O_CREAT 0100
#define O_TRUNC 01000
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
    const char *path = "fallocate-target.txt";
    const char *payload = "abc";
    unsigned char buffer[9] = {0};
    const char *ok = "raw-fallocate:ok\n";
    const char *fail = "raw-fallocate:fail\n";

    long fd = linux_syscall4(56, AT_FDCWD, (long)path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    long written = fd >= 0 ? linux_syscall3(64, fd, (long)payload, 3) : fd;
    long close_written = fd >= 0 ? linux_syscall1(57, fd) : fd;
    long rw_fd = linux_syscall4(56, AT_FDCWD, (long)path, O_RDWR, 0);
    long allocated = rw_fd >= 0 ? linux_syscall4(47, rw_fd, 0, 5, 4) : rw_fd;
    long seek = rw_fd >= 0 ? linux_syscall3(62, rw_fd, 0, SEEK_SET) : rw_fd;
    long read_count = rw_fd >= 0 ? linux_syscall3(63, rw_fd, (long)buffer, 9) : rw_fd;
    long close_read = rw_fd >= 0 ? linux_syscall1(57, rw_fd) : rw_fd;
    long ro_fd = linux_syscall4(56, AT_FDCWD, (long)path, O_RDONLY, 0);
    long readonly = ro_fd >= 0 ? linux_syscall4(47, ro_fd, 0, 0, 1) : ro_fd;
    long close_ro = ro_fd >= 0 ? linux_syscall1(57, ro_fd) : ro_fd;

    int passed = fd >= 0 && written == 3 && close_written == 0 && rw_fd >= 0 &&
                 allocated == 0 && seek == 0 && read_count == 9 && close_read == 0 &&
                 buffer[0] == 'a' && buffer[1] == 'b' && buffer[2] == 'c' &&
                 buffer[3] == 0 && buffer[4] == 0 && buffer[5] == 0 &&
                 buffer[6] == 0 && buffer[7] == 0 && buffer[8] == 0 &&
                 readonly == -9 && close_ro == 0;
    if (passed) {
        linux_syscall3(64, 1, (long)ok, 17);
        linux_syscall1(93, 67);
    } else {
        linux_syscall3(64, 1, (long)fail, 19);
        linux_syscall1(93, 89);
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
        Some(67),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-fallocate:ok\n");

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
    assert!(stdout.contains("\"stop_code\":67"));
    assert!(stdout.contains("\"riscv_guest_writes\":["));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-fallocate:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
