use super::*;

pub(super) fn detailed_o3_runtime_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        u_type(0, 5, 0x17),            // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),   // addi x5, x5, data
        i_type(7, 0, 0x0, 11, 0x13),   // addi x11, x0, 7
        i_type(0, 5, 0b010, 12, 0x03), // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),       // sw x12, 4(x5)
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13), // addi x13, x0, 77
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_live_rob_overlap_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        i_type(6, 0, 0x0, 1, 0x13),    // addi x1, x0, 6
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 1, 2, 0x4, 3, 0x33), // div x3, x2, x1
        i_type(5, 0, 0x0, 4, 0x13),    // addi x4, x0, 5
        i_type(11, 4, 0x0, 5, 0x13),   // addi x5, x4, 11
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 3, 12, 0b010),                          // sw x3, 0(x12)
        s_type(4, 5, 12, 0b010),                          // sw x5, 4(x12)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn live_retire_gate_div_witness_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to the CLI-selected mode
        i_type(84, 0, 0x0, 1, 0x13),   // addi x1, x0, 84
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 2, 1, 0x4, 3, 0x33), // div x3, x1, x2
        i_type(-11, 3, 0x0, 4, 0x13),  // addi x4, x3, -11
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 4, 12, 0b010),                          // sw x4, 0(x12)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn live_retire_gate_add_witness_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),         // switch cpu0 to detailed
        i_type(11, 0, 0x0, 1, 0x13), // addi x1, x0, 11
        i_type(14, 1, 0x0, 2, 0x13), // addi x2, x1, 14
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 2, 12, 0b010),                          // sw x2, 0(x12)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_live_rob_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        i_type(6, 0, 0x0, 1, 0x13),    // addi x1, x0, 6
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 1, 2, 0x4, 3, 0x33), // div x3, x2, x1
        i_type(5, 0, 0x0, 4, 0x13),    // addi x4, x0, 5
        i_type(11, 4, 0x0, 5, 0x13),   // addi x5, x4, 11
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 3, 12, 0b010),                          // sw x3, 0(x12)
        s_type(4, 5, 12, 0b010),                          // sw x5, 4(x12)
        m5op(M5_DUMP_STATS),                              // dump live detailed O3 ROB stats
        r_type(1, 1, 3, 0x4, 6, 0x33),                    // div x6, x3, x1 before the later stop
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_scalar_memory_lifecycle_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),                              // auipc x10, 0
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13), // addi x10, x10, data
        i_type(42, 0, 0x0, 11, 0x13),                     // addi x11, x0, 42
        s_type(0, 11, 10, 0b010),                         // sw x11, 0(x10)
        i_type(0, 10, 0b010, 12, 0x03),                   // lw x12, 0(x10)
        i_type(1, 12, 0x0, 13, 0x13),                     // addi x13, x12, 1
        s_type(4, 13, 10, 0b010),                         // sw x13, 4(x10)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_live_rename_pressure_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),        // switch cpu0 to detailed
        i_type(1, 0, 0x0, 1, 0x13), // addi x1, x0, 1
        i_type(2, 0, 0x0, 2, 0x13), // addi x2, x0, 2
        i_type(3, 1, 0x0, 3, 0x13), // addi x3, x1, 3
        i_type(4, 2, 0x0, 4, 0x13), // addi x4, x2, 4
        i_type(5, 3, 0x0, 5, 0x13), // addi x5, x3, 5
        i_type(6, 4, 0x0, 6, 0x13), // addi x6, x4, 6
        i_type(7, 5, 0x0, 7, 0x13), // addi x7, x5, 7
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),                              // auipc x10, 0
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13), // addi x10, x10, data
        s_type(0, 7, 10, 0b010),                          // sw x7, 0(x10)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_live_rename_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),        // switch cpu0 to detailed
        i_type(1, 0, 0x0, 1, 0x13), // addi x1, x0, 1
        i_type(2, 0, 0x0, 2, 0x13), // addi x2, x0, 2
        i_type(3, 1, 0x0, 3, 0x13), // addi x3, x1, 3
        i_type(4, 2, 0x0, 4, 0x13), // addi x4, x2, 4
        i_type(5, 3, 0x0, 5, 0x13), // addi x5, x3, 5
        i_type(6, 4, 0x0, 6, 0x13), // addi x6, x4, 6
        i_type(7, 5, 0x0, 7, 0x13), // addi x7, x5, 7
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 7, 12, 0b010),                          // sw x7, 0(x12)
        m5op(M5_DUMP_STATS),                              // dump live rename pressure
        i_type(1, 7, 0x0, 8, 0x13),                       // addi x8, x7, 1
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 6, 0x17),                             // auipc x6, 0
        i_type(data_start - auipc_pc, 6, 0x0, 6, 0x13), // addi x6, x6, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0, 6, 0b010, 12, 0x03),                  // lw x12, 0(x6)
        s_type(4, 12, 6, 0b010),                        // sw x12, 4(x6)
        m5op(M5_CHECKPOINT),                            // checkpoint cpu1 O3 runtime state
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart_detailed_o3_dump_stats_binary(
    name: &str,
    detailed_hart: u32,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let selected_hart_branch = match detailed_hart {
        0 => b_type(8, 0, 5, 0x0), // beq x5, x0, hart 0 detailed path
        1 => b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        _ => panic!("multicore O3 dump fixture only supports hart 0 or hart 1"),
    };
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        selected_hart_branch, // selected hart enters the detailed path
        b_type(0, 0, 0, 0x0), // other hart spins until selected hart exits
        m5op(M5_SWITCH_CPU),  // switch the selected CPU to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 6, 0x17),                             // auipc x6, 0
        i_type(data_start - auipc_pc, 6, 0x0, 6, 0x13), // addi x6, x6, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0, 6, 0b010, 12, 0x03),                  // lw x12, 0(x6)
        s_type(4, 12, 6, 0b010),                        // sw x12, 4(x6)
        m5op(M5_DUMP_STATS),                            // dump selected CPU O3 runtime aliases
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_float_misc_binary(name: &str) -> std::path::PathBuf {
    let words = vec![
        csr_read(0xf14, 5),                             // csrr x5, mhartid
        b_type(8, 0, 5, 0x1),                           // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0),                           // hart 0: spin until hart 1 exits
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // hart 1: switch cpu1 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_reset_fu_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
    ];
    words.extend(detailed_o3_float_misc_prefix_words());
    words.push(m5op(M5_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_direct_call_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        j_type(8, 1),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        i_type(1, 0, 0x0, 7, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        u_type(0, 5, 0x17),            // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),   // addi x5, x5, data
        i_type(7, 0, 0x0, 14, 0x13),   // addi x14, x0, 7
        i_type(0, 5, 0b010, 12, 0x03), // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),       // sw x12, 4(x5)
        m5op(M5_DUMP_STATS),           // dump live detailed-mode stats
        i_type(99, 0, 0x0, 13, 0x13),  // addi x13, x0, 99 after dump
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        m5op(M5_RESET_STATS),           // reset detailed O3 runtime stats
        i_type(0, 5, 0b010, 12, 0x03),  // lw x12, 0(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_reset_fu_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = detailed_o3_float_misc_prefix_words();
    words.push(m5op(M5_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_dump_reset_fu_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = detailed_o3_float_misc_prefix_words();
    words.push(m5op(M5_DUMP_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_branch_dump_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = detailed_o3_branch_repair_words(data_start);
    words.push(m5op(M5_DUMP_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_branch_predicted_target_match_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(0, 0, 0x0, 9, 0x13),
        b_type(8, 0, 7, 0x1),
        i_type(99, 0, 0x0, 6, 0x13),
        b_type(16, 0, 9, 0x1),
        m5op(M5_SWITCH_CPU),
        i_type(1, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 10, 0x17),
        i_type(data_start - 32, 10, 0x0, 10, 0x13),
        s_type(0, 7, 10, 0b011),
        s_type(8, 9, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
    ];
    append_integer_mul_div_work(&mut words);
    words.extend([i_type(0, 0, 0x0, 10, 0x13), i_type(0, 0, 0x0, 11, 0x13)]);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_indirect_call_wrong_target_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 112_i32;
    let mut words = vec![
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
        u_type(0, 10, 0x17),
        i_type(data_start - 36, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
    ];
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_indirect_jump_wrong_target_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 112_i32;
    let mut words = vec![
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 0, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
        u_type(0, 10, 0x17),
        i_type(data_start - 36, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
    ];
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_indirect_call_wrong_target_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 warmup path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU), // hart 1: switch cpu1 to detailed
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
    ];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - data_auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_indirect_call_wrong_target_dump_reset_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 warmup path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU), // hart 1: switch cpu1 to detailed
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
    ];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - data_auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_RESET_STATS),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_restore_indirect_call_ftq_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 warmup path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU), // hart 1: switch cpu1 to detailed
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
    ];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - data_auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_CHECKPOINT),
        m5op(M5_DUMP_STATS),
        j_type(8, 1),
        i_type(9, 0, 0x0, 7, 0x13),
    ]);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_direct_call_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        j_type(8, 1),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        i_type(1, 0, 0x0, 7, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_return_branch_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        u_type(0, 1, 0x17),
        i_type(16, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 0, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_branch_repair_text_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = detailed_o3_branch_repair_words(data_start);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_return_branch_summary_binary(name: &str) -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - 4, 10, 0x0, 10, 0x13),
        u_type(0, 1, 0x17),
        i_type(16, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 0, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_branch_repair_words(data_start: i32) -> Vec<u32> {
    vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(1, 0, 0x0, 9, 0x13),
        b_type(12, 0, 9, 0x1),
        i_type(11, 0, 0x0, 6, 0x13),
        j_type(16, 0),
        m5op(M5_SWITCH_CPU),
        i_type(0, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 5, 0x17),
        i_type(data_start - 32, 5, 0x0, 5, 0x13),
        s_type(0, 6, 5, 0b011),
        s_type(8, 9, 5, 0b011),
    ]
}

pub(super) fn detailed_o3_lsq_matrix_dump_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0, 5, 0b011, 6, 0x03),                   // ld x6, 0(x5)
        s_type(8, 6, 5, 0b011),                         // sd x6, 8(x5)
        atomic_type(0x02, true, false, 0, 5, 0x3, 7),   // lr.d.aq x7, (x5)
        i_type(3, 0, 0x0, 8, 0x13),                     // addi x8, x0, 3
        atomic_type(0x03, false, true, 8, 5, 0x3, 9),   // sc.d.rl x9, x8, (x5)
        i_type(4, 0, 0x0, 10, 0x13),                    // addi x10, x0, 4
        atomic_type(0x01, true, true, 10, 5, 0x3, 11),  // amoswap.d.aqrl x11, x10, (x5)
        s_type(16, 9, 5, 0b011),                        // sd x9, 16(x5)
        s_type(24, 11, 5, 0b011),                       // sd x11, 24(x5)
        i_type(0, 0, 0x0, 10, 0x13),                    // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),                    // addi x11, x0, 0
        m5op(M5_DUMP_RESET_STATS),
        i_type(32, 5, 0x0, 14, 0x13),   // addi x14, x5, sc-fail data
        i_type(0x2a, 0, 0x0, 13, 0x13), // addi x13, x0, 0x2a
        atomic_type(0x03, false, false, 13, 14, 0x3, 15), // sc.d x15, x13, (x14)
        s_type(40, 15, 5, 0b011),       // sd x15, 40(x5)
        i_type(0, 0, 0x0, 10, 0x13),    // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),    // addi x11, x0, 0
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0, 0, 0, 0, 0, 0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_lsq_forwarding_dump_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        b_type(48, 11, 12, 0x1),                        // bne x12, x11, fail
        m5op(M5_DUMP_RESET_STATS),
        i_type(0x33, 0, 0x0, 13, 0x13), // addi x13, x0, 0x33
        s_type(4, 13, 5, 0b010),        // sw x13, 4(x5)
        i_type(8, 5, 0b010, 14, 0x03),  // lw x14, 8(x5)
        b_type(28, 0, 14, 0x1),         // bne x14, x0, fail
        i_type(0x44, 0, 0x0, 15, 0x13), // addi x15, x0, 0x44
        s_type(12, 15, 5, 0b000),       // sb x15, 12(x5)
        i_type(12, 5, 0b010, 16, 0x03), // lw x16, 12(x5)
        b_type(12, 15, 16, 0x1),        // bne x16, x15, fail
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_lsq_forwarding_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        b_type(48, 11, 12, 0x1),                        // bne x12, x11, fail
        m5op(M5_DUMP_RESET_STATS),
        i_type(0x33, 0, 0x0, 13, 0x13), // addi x13, x0, 0x33
        s_type(4, 13, 5, 0b010),        // sw x13, 4(x5)
        i_type(8, 5, 0b010, 14, 0x03),  // lw x14, 8(x5)
        b_type(28, 0, 14, 0x1),         // bne x14, x0, fail
        i_type(0x44, 0, 0x0, 15, 0x13), // addi x15, x0, 0x44
        s_type(12, 15, 5, 0b000),       // sb x15, 12(x5)
        i_type(12, 5, 0b010, 16, 0x03), // lw x16, 12(x5)
        b_type(12, 15, 16, 0x1),        // bne x16, x15, fail
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_lsq_forwarding_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        b_type(20, 11, 12, 0x1),                        // bne x12, x11, fail
        i_type(0, 0, 0x0, 10, 0x13),                    // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),                    // addi x11, x0, 0
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_float_misc_prefix_words() -> Vec<u32> {
    vec![
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
    ]
}

pub(super) fn append_integer_mul_div_work(words: &mut Vec<u32>) {
    words.extend([
        i_type(42, 0, 0x0, 1, 0x13), // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),  // addi x2, x0, 7
        0x0220_81b3,                 // mul x3, x1, x2
        0x0220_c1b3,                 // div x3, x1, x2
    ]);
}

pub(super) fn detailed_o3_iq_iew_commit_matrix_binary(name: &str) -> std::path::PathBuf {
    let mut words = detailed_o3_float_misc_prefix_words();
    append_integer_mul_div_work(&mut words);
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 160_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),                        // sw x12, 4(x5)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn append_host_stop(words: &mut Vec<u32>) {
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
}

pub(super) fn detailed_o3_lsq_store_load_match_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),  // lw x12, 0(x5)
        b_type(8, 11, 12, 0x1),         // bne x12, x11, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_lsq_store_load_mismatch_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(4, 5, 0b010, 12, 0x03),  // lw x12, 4(x5)
        b_type(8, 0, 12, 0x1),          // bne x12, x0, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_lsq_store_load_partial_overlap_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b000),        // sb x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),  // lw x12, 0(x5)
        b_type(8, 11, 12, 0x1),         // bne x12, x11, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_lsq_store_load_address_mismatch_byte_load_binary(
    name: &str,
) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(4, 5, 0b100, 12, 0x03),  // lbu x12, 4(x5)
        b_type(8, 0, 12, 0x1),          // bne x12, x0, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_ordered_atomic_lsq_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0, 5, 0b011, 6, 0x03),                   // ld x6, 0(x5)
        s_type(8, 6, 5, 0b011),                         // sd x6, 8(x5)
        atomic_type(0x02, true, false, 0, 5, 0x3, 7),   // lr.d.aq x7, (x5)
        i_type(3, 0, 0x0, 8, 0x13),                     // addi x8, x0, 3
        atomic_type(0x03, false, true, 8, 5, 0x3, 9),   // sc.d.rl x9, x8, (x5)
        i_type(4, 0, 0x0, 10, 0x13),                    // addi x10, x0, 4
        atomic_type(0x01, true, true, 10, 5, 0x3, 11),  // amoswap.d.aqrl x11, x10, (x5)
        s_type(16, 9, 5, 0b011),                        // sd x9, 16(x5)
        s_type(24, 11, 5, 0b011),                       // sd x11, 24(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_event_window_ordering_binary(
    name: &str,
    acquire: bool,
    release: bool,
) -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                                // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(4, 0, 0x0, 6, 0x13),                        // addi x6, x0, 4
        atomic_type(0x01, acquire, release, 6, 5, 0x3, 7), // amoswap.d x7, x6, (x5)
        i_type(9, 0, 0x0, 8, 0x13),                        // addi x8, x0, 9
        b_type(20, 7, 8, 0x1),                             // bne x7, x8, fail
        i_type(0, 0, 0x0, 10, 0x13),                       // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),                       // addi x11, x0, 0
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_store_conditional_failure_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x2a, 0, 0x0, 6, 0x13),                  // addi x6, x0, 0x2a
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),  // sc.d x7, x6, (x5)
        s_type(8, 7, 5, 0b011),                         // sd x7, 8(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_float_vector_lsq_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 10, 0x17),                               // auipc x10, 0
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),  // addi x10, x10, data
        i_type(0, 10, 0x3, 1, 0x07),                       // fld f1, 0(x10)
        float_store_type(8, 1, 10, 0x3),                   // fsd f1, 8(x10)
        i_type(16, 10, 0x0, 12, 0x13),                     // addi x12, x10, vector src
        i_type(24, 10, 0x0, 16, 0x13),                     // addi x16, x10, vector dst
        i_type(2, 0, 0x0, 11, 0x13),                       // addi x11, x0, 2
        vsetvli_type(0xd0, 11, 5),                         // e32, m1, vl=2
        vector_unit_stride_load_type(true, 0b110, 12, 1),  // vle v1, (x12)
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse v1, (x16)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&1.0f64.to_bits().to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    program.extend(
        [0x1122_3344, 0x5566_7788, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn functional_store_conditional_failure_binary(name: &str) -> std::path::PathBuf {
    let mut words = Vec::new();
    let auipc_pc = 0_i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x2a, 0, 0x0, 6, 0x13),                  // addi x6, x0, 0x2a
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),  // sc.d x7, x6, (x5)
        s_type(8, 7, 5, 0b011),                         // sd x7, 8(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

pub(super) fn vector_unit_stride_store_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs3: u8,
) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

pub(super) fn float_store_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = (imm as u32) & 0xfff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x27
}

pub(super) fn detailed_o3_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),         // switch cpu0 to detailed
        i_type(42, 0, 0x0, 1, 0x13), // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),  // addi x2, x0, 7
        0x0220_81b3,                 // mul x3, x1, x2
        0x0220_c1b3,                 // div x3, x1, x2
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13), // addi x13, x0, 77
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_float_misc_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_float_extended_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),                // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),               // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),               // fmv.w.x f2, x8
        fp_r_type(0x78, 0, 8, 0x0, 3),               // fmv.w.x f3, x8
        i_type(2, 0, 0x0, 10, 0x13),                 // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                   // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 8, 1), // vfmv.v.f v1, f8
        vector_arith_type(0b010111, 0b100, 0, 8, 2), // vfmv.v.f v2, f8
        vector_arith_type(0b010111, 0b100, 0, 8, 4), // vfmv.v.f v4, f8
        m5op(M5_SWITCH_CPU),                         // switch cpu0 to detailed
        fp_r_type(0x00, 2, 1, 0x0, 4),               // fadd.s f4, f1, f2
        fp_r4_type(3, 0x0, 2, 1, 0x0, 5, 0x43),      // fmadd.s f5, f1, f2, f3
        fp_r_type(0x2c, 0, 1, 0x0, 6),               // fsqrt.s f6, f1
        vector_arith_type(0b000000, 0b001, 2, 1, 3), // vfadd.vv v3, v2, v1
        vector_arith_type(0b101100, 0b001, 2, 1, 4), // vfmacc.vv v4, v2, v1
        vector_arith_type(0b010011, 0b001, 1, 0, 5), // vfsqrt.v v5, v1
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_vector_integer_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        i_type(2, 0, 0x0, 10, 0x13),                 // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                   // e32, m1, vl=2
        m5op(M5_SWITCH_CPU),                         // switch cpu0 to detailed
        vector_arith_type(0b100101, 0b010, 2, 1, 3), // vmul.vv v3, v2, v1
        vector_arith_type(0b100000, 0b010, 2, 1, 4), // vdivu.vv v4, v2, v1
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_float_misc_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn pre_dump_then_detailed_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_DUMP_STATS),
        m5op(M5_SWITCH_CPU),
        i_type(42, 0, 0x0, 1, 0x13), // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),  // addi x2, x0, 7
        0x0220_81b3,                 // mul x3, x1, x2
        0x0220_c1b3,                 // div x3, x1, x2
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_checkpoint_state_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        m5op(M5_CHECKPOINT),           // baseline O3 runtime state
        u_type(0, 5, 0x17),            // auipc x5, 0
        i_type(48, 5, 0x0, 5, 0x13),   // addi x5, x5, data
        i_type(7, 0, 0x0, 11, 0x13),   // addi x11, x0, 7
        i_type(0, 5, 0b010, 12, 0x03), // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),       // sw x12, 4(x5)
        m5op(M5_CHECKPOINT),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 56 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_scheduled_restore_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13)); // nop
    }
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 704_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
    ]);
    while words.len() < 170 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_restore_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.push(m5op(M5_CHECKPOINT)); // exact baseline for the scheduled restore
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.push(m5op(M5_DUMP_STATS)); // dump restored-baseline stats before O3 work
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 704_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
    ]);
    while words.len() < 170 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn detailed_o3_restore_fu_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        m5op(M5_CHECKPOINT),                            // checkpoint integer FU stats
        m5op(M5_DUMP_STATS),                            // dump checkpoint-era stats
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
    ];
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_restore_fu_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(3, 0, 0x0, 9, 0x13),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 1, 1),
        vector_arith_type(0b010111, 0b100, 0, 2, 2),
        m5op(M5_SWITCH_CPU),
    ];
    append_integer_mul_div_work(&mut words);
    words.extend([
        m5op(M5_CHECKPOINT),
        m5op(M5_DUMP_STATS),
        fp_r_type(0x68, 0, 9, 0x0, 3),
        fp_r_type(0x10, 2, 1, 0x0, 4),
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3),
        vector_arith_type(0b001000, 0b001, 2, 1, 4),
    ]);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn sparse_three_core_detailed_o3_restore_trace_binary(name: &str) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),         // csrr x5, mhartid
        i_type(1, 0, 0x0, 6, 0x13), // addi x6, x0, 1
        b_type(8, 6, 5, 0x1),       // harts 0/2: branch to detailed path
        b_type(0, 0, 0, 0x0),       // hart 1: spin without detailed O3 authority
        m5op(M5_SWITCH_CPU),        // harts 0/2: switch to detailed
    ];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 7, 0x17),                             // auipc x7, 0
        i_type(data_start - auipc_pc, 7, 0x0, 7, 0x13), // addi x7, x7, data
        i_type(2, 5, 0b001, 8, 0x13),                   // slli x8, x5, 2
        r_type(0, 8, 7, 0x0, 7, 0x33),                  // add x7, x7, x8
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        r_type(0, 5, 11, 0x0, 11, 0x33),                // add x11, x11, x5
        s_type(0, 11, 7, 0b010),                        // sw x11, 0(x7)
        i_type(0, 7, 0b010, 12, 0x03),                  // lw x12, 0(x7)
    ]);
    let check_branch_index = words.len();
    words.push(0);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    let fail_index = words.len() - 1;
    words[check_branch_index] = b_type(((fail_index - check_branch_index) * 4) as i32, 11, 12, 0x1);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn multicore_hart1_detailed_o3_restore_lsq_forwarding_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
    ]);
    let first_check_branch_index = words.len();
    words.push(0);
    words.extend([
        i_type(0, 0, 0x0, 10, 0x13),   // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),   // addi x11, x0, 0
        m5op(M5_CHECKPOINT),           // checkpoint one LSQ forwarding match
        m5op(M5_DUMP_STATS),           // dump restored-baseline LSQ counters
        i_type(4, 5, 0b010, 15, 0x03), // lw x15, 4(x5)
    ]);
    let restored_word_branch_index = words.len();
    words.push(0);
    words.extend([
        i_type(0x6b, 0, 0x0, 13, 0x13), // addi x13, x0, 0x6b
        s_type(4, 13, 5, 0b010),        // sw x13, 4(x5)
        i_type(4, 5, 0b010, 14, 0x03),  // lw x14, 4(x5)
    ]);
    let second_check_branch_index = words.len();
    words.push(0);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    let fail_index = words.len() - 1;
    words[first_check_branch_index] = b_type(
        ((fail_index - first_check_branch_index) * 4) as i32,
        11,
        12,
        0x1,
    );
    words[restored_word_branch_index] = b_type(
        ((fail_index - restored_word_branch_index) * 4) as i32,
        0,
        15,
        0x1,
    );
    words[second_check_branch_index] = b_type(
        ((fail_index - second_check_branch_index) * 4) as i32,
        13,
        14,
        0x1,
    );
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn scheduled_host_restore_missing_label_binary(name: &str) -> std::path::PathBuf {
    let mut words = Vec::new();
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn timing_switch_o3_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn timing_switch_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
