use std::{fs, process::Command};

use serde_json::Value;

use crate::support::{
    b_type, find_riscv_tool, i_type, riscv64_elf, riscv64_program, temp_binary, temp_workspace,
};

const RAW_PRIVILEGED_ADMIN_SYSCALLS: &[i32] = &[
    58,  // vhangup
    104, // kexec_load
    105, // init_module
    106, // delete_module
    273, // finit_module
    294, // kexec_file_load
];

#[test]
fn rem6_run_riscv_se_runs_static_raw_admin_syscall_errors_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw admin syscall smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw admin syscall smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-admin");
    let source = workspace.join("raw-admin.c");
    let binary = workspace.join("raw-admin");
    fs::write(
        &source,
        r#"static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
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
    const char path[] = "/";
    const char host[] = "rem6";
    long umount_null = linux_syscall2(39, 0, 0);
    long umount_root = linux_syscall2(39, (long)path, 0);
    long umount_bad_flags = linux_syscall2(39, (long)path, 0x80000000L);
    long mount_null = linux_syscall5(40, 0, 0, 0, 0, 0);
    long mount_root = linux_syscall5(40, (long)path, (long)path, 0, 0, 0);
    long mount_bad_fstype = linux_syscall5(40, (long)path, (long)path, 1, 0, 0);
    long pivot_null = linux_syscall2(41, 0, 0);
    long pivot_root = linux_syscall2(41, (long)path, (long)path);
    long chroot_null = linux_syscall1(51, 0);
    long chroot_root = linux_syscall1(51, (long)path);
    long acct_null = linux_syscall1(89, 0);
    long reboot_null = linux_syscall4(142, 0, 0, 0, 0);
    long sethostname_null = linux_syscall2(161, 0, 4);
    long sethostname_empty_null = linux_syscall2(161, 0, 0);
    long sethostname_name = linux_syscall2(161, (long)host, 4);
    long setdomainname_null = linux_syscall2(162, 0, 4);
    long setdomainname_empty_null = linux_syscall2(162, 0, 0);
    long setdomainname_name = linux_syscall2(162, (long)host, 4);
    long swapon_null = linux_syscall2(224, 0, 0);
    long swapon_root = linux_syscall2(224, (long)path, 0);
    long swapon_bad_flags = linux_syscall2(224, (long)path, 0x80000000L);
    long swapoff_null = linux_syscall1(225, 0);
    long swapoff_root = linux_syscall1(225, (long)path);
    puts("raw-admin:");
    putn(umount_null);
    sep();
    putn(umount_root);
    sep();
    putn(umount_bad_flags);
    sep();
    putn(mount_null);
    sep();
    putn(mount_root);
    sep();
    putn(mount_bad_fstype);
    sep();
    putn(pivot_null);
    sep();
    putn(pivot_root);
    sep();
    putn(chroot_null);
    sep();
    putn(chroot_root);
    sep();
    putn(acct_null);
    sep();
    putn(reboot_null);
    sep();
    putn(sethostname_null);
    sep();
    putn(sethostname_empty_null);
    sep();
    putn(sethostname_name);
    sep();
    putn(setdomainname_null);
    sep();
    putn(setdomainname_empty_null);
    sep();
    putn(setdomainname_name);
    sep();
    putn(swapon_null);
    sep();
    putn(swapon_root);
    sep();
    putn(swapon_bad_flags);
    sep();
    putn(swapoff_null);
    sep();
    putn(swapoff_root);
    putc('\n');
    if (umount_null == -14 &&
        umount_root == -1 &&
        umount_bad_flags == -22 &&
        mount_null == -14 &&
        mount_root == -1 &&
        mount_bad_fstype == -14 &&
        pivot_null == -14 &&
        pivot_root == -1 &&
        chroot_null == -14 &&
        chroot_root == -1 &&
        acct_null == -1 &&
        reboot_null == -1 &&
        sethostname_null == -14 &&
        sethostname_empty_null == -14 &&
        sethostname_name == -1 &&
        setdomainname_null == -14 &&
        setdomainname_empty_null == -14 &&
        setdomainname_name == -1 &&
        swapon_null == -14 &&
        swapon_root == -1 &&
        swapon_bad_flags == -22 &&
        swapoff_null == -14 &&
        swapoff_root == -1) {
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

    let expected_stdout =
        "raw-admin:-14:-1:-22:-14:-1:-14:-14:-1:-14:-1:-1:-1:-14:-14:-1:-14:-14:-1:-14:-1:-22:-14:-1\n";
    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(73),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, expected_stdout.as_bytes());

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
    assert_eq!(guest_stdout, expected_stdout);
    assert_eq!(
        json.pointer("/riscv_unknown_syscalls")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_privileged_admin_syscall_denials() {
    let mut words = vec![i_type(-1, 0, 0x0, 5, 0x13)]; // addi t0, x0, -EPERM
    let mut branch_indices = Vec::new();
    for syscall in RAW_PRIVILEGED_ADMIN_SYSCALLS {
        words.push(i_type(*syscall, 0, 0x0, 17, 0x13)); // addi a7, x0, syscall
        words.push(0x0000_0073); // ecall
        branch_indices.push(words.len());
        words.push(0); // patched to bne a0, t0, fail
    }

    words.push(i_type(75, 0, 0x0, 10, 0x13)); // addi a0, x0, 75
    words.push(i_type(93, 0, 0x0, 17, 0x13)); // addi a7, x0, exit
    words.push(0x0000_0073); // ecall

    let fail_index = words.len();
    words.push(i_type(76, 0, 0x0, 10, 0x13)); // addi a0, x0, 76
    words.push(i_type(93, 0, 0x0, 17, 0x13)); // addi a7, x0, exit
    words.push(0x0000_0073); // ecall

    let fail_pc = i32::try_from(fail_index * 4).unwrap();
    for branch_index in branch_indices {
        let branch_pc = i32::try_from(branch_index * 4).unwrap();
        words[branch_index] = b_type(fail_pc - branch_pc, 5, 10, 0x1);
    }

    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-privileged-admin", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "300",
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
        Some(75)
    );
    let unknown_syscalls = json
        .pointer("/riscv_unknown_syscalls")
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        unknown_syscalls.is_empty(),
        "unexpected unknown syscalls: {unknown_syscalls:?}"
    );
}
