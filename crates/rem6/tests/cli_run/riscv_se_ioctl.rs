use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_ioctl_winsize_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw ioctl smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw ioctl smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-ioctl-winsize");
    let source = workspace.join("raw-ioctl-winsize.c");
    let binary = workspace.join("raw-ioctl-winsize");
    fs::write(
        &source,
        r#"static long linux_syscall1(long number, long arg0) {
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

static void write_stdout(const char *text, long bytes) {
    linux_syscall3(64, 1, (long)text, bytes);
}

static void exit_with(long code) {
    linux_syscall1(93, code);
    __builtin_unreachable();
}

void _start(void) {
    unsigned short stdout_winsize[4] = {0xffff, 0xffff, 0xffff, 0xffff};
    unsigned short pipe_winsize[4] = {0xffff, 0xffff, 0xffff, 0xffff};
    int pipe_fds[2] = {-1, -1};
    long stdout_ioctl = linux_syscall3(29, 1, 0x5413, (long)stdout_winsize);
    long pipe_status = linux_syscall3(59, (long)pipe_fds, 0, 0);
    long pipe_ioctl = pipe_status == 0
        ? linux_syscall3(29, pipe_fds[0], 0x5413, (long)pipe_winsize)
        : -999;
    if (pipe_status == 0) {
        linux_syscall1(57, pipe_fds[0]);
        linux_syscall1(57, pipe_fds[1]);
    }

    if (stdout_ioctl == 0 &&
        stdout_winsize[0] == 24 &&
        stdout_winsize[1] == 80 &&
        stdout_winsize[2] == 0 &&
        stdout_winsize[3] == 0 &&
        pipe_ioctl == -25) {
        write_stdout("raw-ioctl-winsz:tty\n", sizeof("raw-ioctl-winsz:tty\n") - 1);
        exit_with(61);
    }
    if ((stdout_ioctl == 0 || stdout_ioctl == -25) && pipe_ioctl == -25) {
        write_stdout("raw-ioctl-winsz:qemu-boundary\n",
                     sizeof("raw-ioctl-winsz:qemu-boundary\n") - 1);
        exit_with(61);
    }
    write_stdout("raw-ioctl-winsz:fail\n", sizeof("raw-ioctl-winsz:fail\n") - 1);
    exit_with(62);
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-nostdlib",
            "-nostartfiles",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-Wl,-e,_start",
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
        Some(61),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    let qemu_stdout = String::from_utf8(qemu_output.stdout).unwrap();
    assert!(
        qemu_stdout.starts_with("raw-ioctl-winsz:"),
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
            "200000",
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
    assert!(stdout.contains("\"stop_code\":61"));
    assert!(stdout.contains("\"text\":\"raw-ioctl-winsz:tty\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 61, "constant");
}
