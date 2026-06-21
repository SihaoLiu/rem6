use std::{fs, process::Command};

use serde_json::Value;

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_syslog_errors_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw syslog smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw syslog smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-syslog");
    let source = workspace.join("raw-syslog.c");
    let binary = workspace.join("raw-syslog");
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

static void print_result(long a, long b, long c, long d, long e, long f,
                         long g, long h, long i) {
    puts("raw-syslog:");
    putn(a);
    sep();
    putn(b);
    sep();
    putn(c);
    sep();
    putn(d);
    sep();
    putn(e);
    sep();
    putn(f);
    sep();
    putn(g);
    sep();
    putn(h);
    sep();
    putn(i);
    putc('\n');
}

void _start(void) {
    long close_log = linux_syscall3(116, 0, 0, 0);
    long open_log = linux_syscall3(116, 1, 0, 0);
    long clear = linux_syscall3(116, 5, 0, 0);
    long console_off = linux_syscall3(116, 6, 0, 0);
    long console_on = linux_syscall3(116, 7, 0, 0);
    long console_level = linux_syscall3(116, 8, 0, 0);
    long size_unread = linux_syscall3(116, 9, 0, 0);
    long size_buffer = linux_syscall3(116, 10, 0, 0);
    long bad_type = linux_syscall3(116, 99, 0, 0);
    print_result(close_log, open_log, clear, console_off, console_on,
                 console_level, size_unread, size_buffer, bad_type);
    if (close_log == -1 &&
        open_log == -1 &&
        clear == -1 &&
        console_off == -1 &&
        console_on == -1 &&
        console_level == -1 &&
        size_unread == -1 &&
        size_buffer == -1 &&
        bad_type == -22) {
        linux_syscall1(93, 73);
    }
    linux_syscall1(93, 74);
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
    assert_eq!(
        qemu_output.status.code(),
        Some(73),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"raw-syslog:-1:-1:-1:-1:-1:-1:-1:-1:-22\n"
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
    assert!(stdout.contains("\"stop_code\":73"));
    let guest_stdout = json
        .pointer("/riscv_guest_writes")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|write| write.get("text").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(guest_stdout, "raw-syslog:-1:-1:-1:-1:-1:-1:-1:-1:-22\n");
    assert_eq!(
        json.pointer("/riscv_unknown_syscalls")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}
