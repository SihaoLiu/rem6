use std::{fs, process::Command};

use serde_json::Value;

use crate::support::{
    b_type, find_riscv_tool, i_type, riscv64_elf, riscv64_program, temp_binary, temp_workspace,
};

const RAW_KNOWN_NI_SYSCALLS: &[i32] = &[
    0,   // io_setup
    1,   // io_destroy
    2,   // io_submit
    3,   // io_cancel
    4,   // io_getevents
    217, // add_key
    218, // request_key
    219, // keyctl
    220, // clone
    241, // perf_event_open
    262, // fanotify_init
    263, // fanotify_mark
    264, // name_to_handle_at
    265, // open_by_handle_at
    272, // kcmp
    277, // seccomp
    280, // bpf
    282, // userfaultfd
    292, // io_pgetevents
    425, // io_uring_setup
    426, // io_uring_enter
    427, // io_uring_register
    428, // open_tree
    429, // move_mount
    430, // fsopen
    431, // fsconfig
    432, // fsmount
    433, // fspick
    435, // clone3
    440, // process_madvise
    442, // mount_setattr
    443, // quotactl_fd
    444, // landlock_create_ruleset
    445, // landlock_add_rule
    446, // landlock_restrict_self
    448, // process_mrelease
    449, // futex_waitv
    450, // set_mempolicy_home_node
    451, // cachestat
    453, // map_shadow_stack
    454, // futex_wake
    455, // futex_wait
    456, // futex_requeue
    457, // statmount
    458, // listmount
    459, // lsm_get_self_attr
    460, // lsm_set_self_attr
    461, // lsm_list_modules
    462, // mseal
];

#[test]
fn rem6_run_riscv_se_runs_static_raw_known_ni_syscalls_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE known no-implementation syscall smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE known no-implementation syscall smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-known-ni");
    let source = workspace.join("known-ni.c");
    let binary = workspace.join("known-ni");
    fs::write(
        &source,
        r#"static long linux_syscall0(long number) {
    register long a0 asm("a0");
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "=r"(a0) : "r"(a7) : "memory");
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

void _start(void) {
    long lookup = linux_syscall3(18, 0, 0, 0);
    long nfs = linux_syscall0(42);
    puts("known-ni:");
    putn(lookup);
    putc(':');
    putn(nfs);
    putc('\n');
    linux_syscall3(93, lookup == -38 && nfs == -38 ? 73 : 74, 0, 0);
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

    let expected_stdout = b"known-ni:-38:-38\n";
    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(73),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, expected_stdout);

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
    assert_eq!(guest_stdout.as_bytes(), expected_stdout);
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_known_ni_clone_and_probe_syscalls() {
    let mut words = vec![i_type(-38, 0, 0x0, 5, 0x13)]; // addi t0, x0, -ENOSYS
    let mut branch_indices = Vec::new();
    for syscall in RAW_KNOWN_NI_SYSCALLS {
        words.push(i_type(*syscall, 0, 0x0, 17, 0x13)); // addi a7, x0, syscall
        words.push(0x0000_0073); // ecall
        branch_indices.push(words.len());
        words.push(0); // patched to bne a0, t0, fail
    }

    words.push(i_type(91, 0, 0x0, 10, 0x13)); // addi a0, x0, 91
    words.push(i_type(93, 0, 0x0, 17, 0x13)); // addi a7, x0, exit
    words.push(0x0000_0073); // ecall

    let fail_index = words.len();
    words.push(i_type(92, 0, 0x0, 10, 0x13)); // addi a0, x0, 92
    words.push(i_type(93, 0, 0x0, 17, 0x13)); // addi a7, x0, exit
    words.push(0x0000_0073); // ecall

    let fail_pc = i32::try_from(fail_index * 4).unwrap();
    for branch_index in branch_indices {
        let branch_pc = i32::try_from(branch_index * 4).unwrap();
        words[branch_index] = b_type(fail_pc - branch_pc, 5, 10, 0x1);
    }

    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-known-ni-clone-probes", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "900",
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
        Some(91)
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
