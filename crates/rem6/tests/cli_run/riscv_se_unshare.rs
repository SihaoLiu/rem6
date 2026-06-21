use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_unshare_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw unshare smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw unshare smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-unshare");
    let source = workspace.join("raw-unshare.c");
    let binary = workspace.join("raw-unshare");
    fs::write(
        &source,
        r#"#define CLONE_FS 0x00000200L
#define CLONE_FILES 0x00000400L
#define CLONE_SYSVSEM 0x00040000L

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

int main(void) {
    const char *ok = "raw-unshare:ok\n";
    const char *fail = "raw-unshare:fail\n";
    long combined = CLONE_FS | CLONE_FILES | CLONE_SYSVSEM;
    int passed = linux_syscall1(97, 0) == 0 &&
                 linux_syscall1(97, CLONE_FS) == 0 &&
                 linux_syscall1(97, CLONE_FILES) == 0 &&
                 linux_syscall1(97, CLONE_SYSVSEM) == 0 &&
                 linux_syscall1(97, combined) == 0;
    if (passed) {
        linux_syscall3(64, 1, (long)ok, 15);
        linux_syscall1(93, 69);
    } else {
        linux_syscall3(64, 1, (long)fail, 17);
        linux_syscall1(93, 87);
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
        Some(69),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-unshare:ok\n");

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
    assert!(stdout.contains("\"stop_code\":69"));
    assert!(stdout.contains("\"riscv_guest_writes\":["));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-unshare:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
