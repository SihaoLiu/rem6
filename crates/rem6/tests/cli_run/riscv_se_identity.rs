use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_fs_identity_syscalls_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw fs identity smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw fs identity smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-fs-identity");
    let source = workspace.join("raw-fs-identity.c");
    let binary = workspace.join("raw-fs-identity");
    fs::write(
        &source,
        r#"static const char ok_message[] = "raw-fsid:ok\n";
static char bad_message[] = "raw-fsid:bad:?\n";

static long linux_syscall0(long number) {
    register long a0 asm("a0");
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "=r"(a0) : "r"(a7) : "memory");
    return a0;
}

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
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a7) : "memory");
    return a0;
}

__attribute__((noreturn)) static void linux_exit(long code) {
    linux_syscall1(93, code);
    for (;;) {
    }
}

__attribute__((noreturn)) static void fail(char code) {
    bad_message[13] = code;
    linux_syscall4(64, 1, (long)bad_message, sizeof(bad_message) - 1, 0);
    linux_exit(77);
}

void _start(void) {
    long uid = linux_syscall0(174);
    long euid = linux_syscall0(175);
    long gid = linux_syscall0(176);
    long egid = linux_syscall0(177);
    long old_fsuid = linux_syscall1(151, uid);
    long repeat_fsuid = linux_syscall1(151, uid);
    long old_fsgid = linux_syscall1(152, gid);
    long repeat_fsgid = linux_syscall1(152, gid);
    if (!(uid >= 0)) fail('a');
    if (!(euid >= 0)) fail('b');
    if (!(gid >= 0)) fail('c');
    if (!(egid >= 0)) fail('d');
    if (!(old_fsuid == euid)) fail('e');
    if (!(repeat_fsuid == uid)) fail('f');
    if (!(old_fsgid == egid)) fail('g');
    if (!(repeat_fsgid == gid)) fail('h');
    linux_syscall4(64, 1, (long)ok_message, sizeof(ok_message) - 1, 0);
    linux_exit(76);
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O2",
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
    assert_eq!(
        qemu_output.status.code(),
        Some(76),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-fsid:ok\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "1600",
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
    assert!(stdout.contains("\"stop_code\":76"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert!(stdout.contains("raw-fsid:ok"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 76, "constant");
}
