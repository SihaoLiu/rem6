use std::process::Command;

use crate::support::*;

const RISCV_LINUX_EPOLL_CTL_ADD: i32 = 1;
const RISCV_LINUX_EPOLLIN: u32 = 0x0001;

fn epoll_event_bytes(events: u32, data: u64) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&events.to_le_bytes());
    bytes[8..].copy_from_slice(&data.to_le_bytes());
    bytes
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_epoll_pwait2() {
    let mut program = riscv64_program(&[
        i_type(7, 0, 0x0, 10, 0x13),  // addi a0, x0, eventfd initial value
        i_type(0, 0, 0x0, 11, 0x13),  // addi a1, x0, flags
        i_type(19, 0, 0x0, 17, 0x13), // addi a7, x0, eventfd2
        0x0000_0073,                  // ecall
        i_type(0, 10, 0x0, 8, 0x13),  // addi s0, a0, 0
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, flags
        i_type(20, 0, 0x0, 17, 0x13), // addi a7, x0, epoll_create1
        0x0000_0073,                  // ecall
        i_type(0, 10, 0x0, 9, 0x13),  // addi s1, a0, 0
        i_type(0, 9, 0x0, 10, 0x13),  // addi a0, s1, 0
        i_type(RISCV_LINUX_EPOLL_CTL_ADD, 0, 0x0, 11, 0x13), // addi a1, x0, EPOLL_CTL_ADD
        i_type(0, 8, 0x0, 12, 0x13),  // addi a2, s0, 0
        u_type(0x1000, 13, 0x17),     // auipc a3, event data page
        i_type(-0x30, 13, 0x0, 13, 0x13), // addi a3, a3, event data offset
        i_type(21, 0, 0x0, 17, 0x13), // addi a7, x0, epoll_ctl
        0x0000_0073,                  // ecall
        b_type(76, 0, 10, 0x1),       // bne a0, x0, fail
        i_type(0, 9, 0x0, 10, 0x13),  // addi a0, s1, 0
        i_type(0x10, 13, 0x0, 11, 0x13), // addi a1, a3, output data offset
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, maxevents
        i_type(0, 0, 0x0, 13, 0x13),  // addi a3, x0, timeout
        i_type(0, 0, 0x0, 14, 0x13),  // addi a4, x0, sigmask
        i_type(0, 0, 0x0, 15, 0x13),  // addi a5, x0, sigsetsize
        i_type(441, 0, 0x0, 17, 0x13), // addi a7, x0, epoll_pwait2
        0x0000_0073,                  // ecall
        i_type(1, 0, 0x0, 6, 0x13),   // addi x6, x0, 1
        b_type(36, 6, 10, 0x1),       // bne a0, x6, fail
        i_type(0, 11, 0x2, 7, 0x03),  // lw x7, 0(a1)
        b_type(28, 6, 7, 0x1),        // bne x7, x6, fail
        i_type(8, 11, 0x3, 7, 0x03),  // ld x7, 8(a1)
        i_type(0x55, 0, 0x0, 6, 0x13), // addi x6, x0, 0x55
        b_type(16, 6, 7, 0x1),        // bne x7, x6, fail
        i_type(73, 0, 0x0, 10, 0x13), // addi a0, x0, success
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, exit
        0x0000_0073,                  // ecall
        i_type(74, 0, 0x0, 10, 0x13), // addi a0, x0, fail
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, exit
        0x0000_0073,                  // ecall
    ]);
    program.resize(0x1000, 0);
    program.extend_from_slice(&epoll_event_bytes(RISCV_LINUX_EPOLLIN, 0x55));
    program.extend_from_slice(&[0_u8; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-epoll-pwait2", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
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
    assert!(stdout.contains("\"stop_code\":73"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 73, "constant");
}
