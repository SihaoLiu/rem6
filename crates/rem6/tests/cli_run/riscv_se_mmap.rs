use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

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
