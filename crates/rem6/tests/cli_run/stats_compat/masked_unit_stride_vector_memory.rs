use super::*;

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_memory() {
    let vector_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-masked-load-store",
        &masked_unit_stride_vector_memory_program(),
        240,
    );

    assert_eq!(
        stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
        42,
        "masked unit-stride vector memory should retire through the success ecall\nstats:\n{vector_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_suppresses_masked_vector_unit_stride_cross_line_load() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-masked-e32-m1-cross-line-load-suppression",
        &masked_unit_stride_cross_line_suppressed_load_program(),
        180,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        22,
        "masked e32/m1 unit-stride load should not touch an inactive element past the cache-line boundary on the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-masked-e32-m1-cross-line-load-suppression",
        &masked_unit_stride_cross_line_suppressed_load_program(),
        600,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        22,
        "cache-backed masked e32/m1 unit-stride load should not touch an inactive element past the cache-line boundary on the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_suppresses_masked_vector_unit_stride_cross_line_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-masked-e32-m1-cross-line-store-suppression",
        &masked_unit_stride_cross_line_suppressed_store_program(),
        180,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        20,
        "masked e32/m1 unit-stride store should not touch an inactive element past the cache-line boundary on the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-masked-e32-m1-cross-line-store-suppression",
        &masked_unit_stride_cross_line_suppressed_store_program(),
        600,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        20,
        "cache-backed masked e32/m1 unit-stride store should not touch an inactive element past the cache-line boundary on the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_suppresses_noncontiguous_masked_vector_unit_stride_cross_line_store()
{
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-masked-e32-m1-noncontig-cross-line-store-suppression",
        &masked_unit_stride_noncontiguous_cross_line_suppressed_store_program(),
        240,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        26,
        "non-contiguous masked e32/m1 unit-stride store should trim the leading inactive lane while preserving the interior inactive lane on the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-masked-e32-m1-noncontig-cross-line-store-suppression",
        &masked_unit_stride_noncontiguous_cross_line_suppressed_store_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        26,
        "cache-backed non-contiguous masked e32/m1 unit-stride store should trim the leading inactive lane while preserving the interior inactive lane on the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_suppresses_noncontiguous_masked_vector_unit_stride_cross_line_load() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-masked-e32-m1-noncontig-cross-line-load-suppression",
        &masked_unit_stride_noncontiguous_cross_line_suppressed_load_program(),
        240,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "non-contiguous masked e32/m1 unit-stride load should trim the leading inactive lane while preserving the interior inactive lane on the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-masked-e32-m1-noncontig-cross-line-load-suppression",
        &masked_unit_stride_noncontiguous_cross_line_suppressed_load_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed non-contiguous masked e32/m1 unit-stride load should trim the leading inactive lane while preserving the interior inactive lane on the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_suppresses_all_inactive_masked_vector_unit_stride_cross_line_load() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-masked-e32-m1-all-inactive-cross-line-load-suppression",
        &masked_unit_stride_all_inactive_cross_line_suppressed_load_program(),
        240,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "all-inactive masked e32/m1 unit-stride load should not touch memory and should preserve destination bytes on the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.data.loads"),
        10,
        "direct-memory all-inactive masked e32/m1 unit-stride load should not add a data-load transaction beyond setup and byte checks\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.data.stores"),
        1,
        "direct-memory all-inactive masked e32/m1 unit-stride load should leave only the result-store transaction\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-masked-e32-m1-all-inactive-cross-line-load-suppression",
        &masked_unit_stride_all_inactive_cross_line_suppressed_load_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed all-inactive masked e32/m1 unit-stride load should not touch memory and should preserve destination bytes on the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.data.loads"),
        10,
        "cache-backed all-inactive masked e32/m1 unit-stride load should not add a data-load transaction beyond setup and byte checks\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.data.stores"),
        1,
        "cache-backed all-inactive masked e32/m1 unit-stride load should leave only the result-store transaction\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_suppresses_all_inactive_masked_vector_unit_stride_cross_line_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-masked-e32-m1-all-inactive-cross-line-store-suppression",
        &masked_unit_stride_all_inactive_cross_line_suppressed_store_program(),
        240,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        26,
        "all-inactive masked e32/m1 unit-stride store should not touch memory and should preserve store bytes on the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.data.loads"),
        10,
        "direct-memory all-inactive masked e32/m1 unit-stride store should keep only setup and byte-check data loads\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.data.stores"),
        0,
        "direct-memory all-inactive masked e32/m1 unit-stride store should not issue a data-store transaction\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-masked-e32-m1-all-inactive-cross-line-store-suppression",
        &masked_unit_stride_all_inactive_cross_line_suppressed_store_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        26,
        "cache-backed all-inactive masked e32/m1 unit-stride store should not touch memory and should preserve store bytes on the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.data.loads"),
        10,
        "cache-backed all-inactive masked e32/m1 unit-stride store should keep only setup and byte-check data loads\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.data.stores"),
        0,
        "cache-backed all-inactive masked e32/m1 unit-stride store should not issue a data-store transaction\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_memory_element_widths() {
    for (name, program, committed) in [
        (
            "e8",
            masked_unit_stride_vector_memory_width_program(
                0xc0,
                8,
                0b000,
                1,
                &[true, false, true, false, true, false, true, false],
                &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
                &[0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x17, 0x28],
                &[0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58],
            ),
            30,
        ),
        (
            "e16",
            masked_unit_stride_vector_memory_width_program(
                0xc8,
                4,
                0b101,
                2,
                &[true, false, true, false],
                &[0x1111, 0x2222, 0x3333, 0x4444],
                &[0xa1a2, 0xb1b2, 0xc1c2, 0xd1d2],
                &[0x5151, 0x5252, 0x5353, 0x5454],
            ),
            30,
        ),
        (
            "e64",
            masked_unit_stride_vector_memory_width_program(
                0xd8,
                2,
                0b111,
                8,
                &[true, false],
                &[0x1111_1111_1111_1111, 0x2222_2222_2222_2222],
                &[0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8],
                &[0x5151_5151_5151_5151, 0x5252_5252_5252_5252],
            ),
            42,
        ),
    ] {
        let vector_stats = in_order_pipeline_payload_stats_with_max_tick(
            &format!("in-order-vector-unit-stride-masked-load-store-{name}"),
            &program,
            240,
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            committed,
            "{name} masked unit-stride vector memory should retire through the success ecall\nstats:\n{vector_stats}"
        );
    }
}

fn masked_unit_stride_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const WORDS_PER_VECTOR: usize = 4;
    const VECTOR_BYTES: i32 = (WORDS_PER_VECTOR * 4) as i32;

    let fail_instruction_index = 42;
    let mut words = vec![
        u_type(0, 10, 0x17),                                 // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),      // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                      // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),           // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),       // addi x14, x10, source vector
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),       // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),       // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),       // addi x19, x10, expected load result
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),       // addi x20, x10, expected store result
        i_type(WORDS_PER_VECTOR as i32, 0, 0b000, 11, 0x13), // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                           // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 1),    // vle32.v v1, (x12)
        vector_vi_type(0b011000, 1, 0, 0),                   // vmseq.vi v0, v1, 0
        vector_unit_stride_load_type(true, 0b110, 13, 2),    // vle32.v v2, (x13)
        vector_unit_stride_load_type(false, 0b110, 14, 2),   // vle32.v v2, (x14), v0.t
        vector_unit_stride_store_type(true, 0b110, 15, 2),   // vse32.v v2, (x15)
        vector_unit_stride_store_type(false, 0b110, 16, 2),  // vse32.v v2, (x16), v0.t
    ];

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, store result
        words.push(i_type(offset, 20, 0b010, 18, 0x03)); // lw x18, expected store result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let vectors: [[u32; WORDS_PER_VECTOR]; 7] = [
        [0, 1, 0, 1],
        [0x1111_1111, 0x2222_2222, 0x3333_3333, 0x4444_4444],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0, 0, 0, 0],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0x5454_5454],
        [0xa1a2_a3a4, 0x2222_2222, 0xc1c2_c3c4, 0x4444_4444],
        [0xa1a2_a3a4, 0x5252_5252, 0xc1c2_c3c4, 0x5454_5454],
    ];

    let mut program = riscv64_program(&words);
    for vector in vectors {
        for word in vector {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_cross_line_suppressed_load_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const SOURCE_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const SOURCE_TAIL_OFFSET_BYTES: i32 = SOURCE_OFFSET_BYTES + 12;
    const LOAD_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 3;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const FAIL_INSTRUCTION_INDEX: i32 = 22;

    let mut words = vec![
        u_type(0, 10, 0x17),                                     // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),          // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),          // addi x12, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),       // addi x13, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),        // addi x14, x10, source block
        i_type(SOURCE_TAIL_OFFSET_BYTES, 10, 0b000, 15, 0x13),   // addi x15, x10, source tail
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, load result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected load
        i_type(2, 0, 0b000, 11, 0x13),                           // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                               // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8),        // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                       // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 2),        // vle32.v v2, (x13)
        vector_unit_stride_load_type(false, 0b110, 15, 2),       // vle32.v v2, (x15), v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 2),       // vse32.v v2, (x16)
    ];

    for word_index in 0..2 {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, observed load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 5] = [
        [0, 1, 0xeeee_eeee, 0xeeee_eeee],
        [0x1111_1111, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0, 0, 0xeeee_eeee, 0xeeee_eeee],
        [0xd1d2_d3d4, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_cross_line_suppressed_store_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const STORE_TAIL_OFFSET_BYTES: i32 = STORE_OFFSET_BYTES + 12;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const FAIL_INSTRUCTION_INDEX: i32 = 20;

    let mut words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),           // addi x12, x10, mask data
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source vector
        i_type(STORE_OFFSET_BYTES, 10, 0b000, 16, 0x13),          // addi x16, x10, store block
        i_type(STORE_TAIL_OFFSET_BYTES, 10, 0b000, 15, 0x13),     // addi x15, x10, store tail
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8), // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 2), // vle32.v v2, (x14)
        vector_unit_stride_store_type(false, 0b110, 15, 2), // vse32.v v2, (x15), v0.t
    ];

    for word_index in 0..2 {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, observed store tail
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected store tail
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 5] = [
        [0, 1, 0xeeee_eeee, 0xeeee_eeee],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xeeee_eeee, 0xeeee_eeee],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0x5454_5454],
        [0x6161_6161, 0x6262_6262, 0x6363_6363, 0x6464_6464],
        [0xa1a2_a3a4, 0x6161_6161, 0xeeee_eeee, 0xeeee_eeee],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_noncontiguous_cross_line_suppressed_store_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const STORE_TAIL_OFFSET_BYTES: i32 = STORE_OFFSET_BYTES + 12;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const FAIL_INSTRUCTION_INDEX: i32 = 26;

    let mut words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),           // addi x12, x10, mask data
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source vector
        i_type(STORE_OFFSET_BYTES, 10, 0b000, 16, 0x13),          // addi x16, x10, store block
        i_type(STORE_TAIL_OFFSET_BYTES, 10, 0b000, 15, 0x13),     // addi x15, x10, store tail
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected
        i_type(4, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8), // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 2), // vle32.v v2, (x14)
        vector_unit_stride_store_type(false, 0b110, 15, 2), // vse32.v v2, (x15), v0.t
    ];

    for word_index in 0..4 {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, observed store tail
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected store tail
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 5] = [
        [1, 0, 1, 0],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0x5454_5454],
        [0x6161_6161, 0x6262_6262, 0x6363_6363, 0x6464_6464],
        [0x5454_5454, 0xb1b2_b3b4, 0x6262_6262, 0xd1d2_d3d4],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_noncontiguous_cross_line_suppressed_load_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const SOURCE_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const SOURCE_TAIL_OFFSET_BYTES: i32 = SOURCE_OFFSET_BYTES + 12;
    const LOAD_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = BLOCK_BYTES * 5;
    const FAIL_INSTRUCTION_INDEX: i32 = 28;

    let mut words = vec![
        u_type(0, 10, 0x17),                                     // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),          // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),          // addi x12, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),       // addi x13, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),        // addi x14, x10, source block
        i_type(SOURCE_TAIL_OFFSET_BYTES, 10, 0b000, 15, 0x13),   // addi x15, x10, source tail
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, load result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected load
        i_type(4, 0, 0b000, 11, 0x13),                           // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                               // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8),        // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                       // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 2),        // vle32.v v2, (x13)
        vector_unit_stride_load_type(false, 0b110, 15, 2),       // vle32.v v2, (x15), v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 2),       // vse32.v v2, (x16)
    ];

    for word_index in 0..4 {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, observed load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 6] = [
        [1, 0, 1, 0],
        [0xa1a2_a3a4, 0x5151_5151, 0xc1c2_c3c4, 0x5353_5353],
        [0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee, 0x4141_4141],
        [0xb1b2_b3b4, 0x9999_9999, 0xd1d2_d3d4, 0xeeee_eeee],
        [0, 0, 0, 0],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_all_inactive_cross_line_suppressed_load_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const SOURCE_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const SOURCE_TAIL_OFFSET_BYTES: i32 = SOURCE_OFFSET_BYTES + 12;
    const LOAD_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = BLOCK_BYTES * 5;
    const FAIL_INSTRUCTION_INDEX: i32 = 28;

    let mut words = vec![
        u_type(0, 10, 0x17),                                     // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),          // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),          // addi x12, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),       // addi x13, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),        // addi x14, x10, source block
        i_type(SOURCE_TAIL_OFFSET_BYTES, 10, 0b000, 15, 0x13),   // addi x15, x10, source tail
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, load result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected load
        i_type(4, 0, 0b000, 11, 0x13),                           // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                               // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8),        // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                       // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 2),        // vle32.v v2, (x13)
        vector_unit_stride_load_type(false, 0b110, 15, 2),       // vle32.v v2, (x15), v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 2),       // vse32.v v2, (x16)
    ];

    for word_index in 0..4 {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, observed load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 6] = [
        [1, 1, 1, 1],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee, 0x4141_4141],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0xeeee_eeee],
        [0, 0, 0, 0],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_all_inactive_cross_line_suppressed_store_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const STORE_TAIL_OFFSET_BYTES: i32 = STORE_OFFSET_BYTES + 12;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const FAIL_INSTRUCTION_INDEX: i32 = 26;

    let mut words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),           // addi x12, x10, mask data
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source vector
        i_type(STORE_OFFSET_BYTES, 10, 0b000, 16, 0x13),          // addi x16, x10, store block
        i_type(STORE_TAIL_OFFSET_BYTES, 10, 0b000, 15, 0x13),     // addi x15, x10, store tail
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected
        i_type(4, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8), // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 2), // vle32.v v2, (x14)
        vector_unit_stride_store_type(false, 0b110, 15, 2), // vse32.v v2, (x15), v0.t
    ];

    for word_index in 0..4 {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, observed store tail
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected store tail
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 5] = [
        [1, 1, 1, 1],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0x5454_5454],
        [0x6161_6161, 0x6262_6262, 0x6363_6363, 0x6464_6464],
        [0x5454_5454, 0x6161_6161, 0x6262_6262, 0x6363_6363],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_vector_memory_width_program(
    vtype: u32,
    avl: i32,
    width: u32,
    element_bytes: usize,
    active_lanes: &[bool],
    initial_lanes: &[u64],
    source_lanes: &[u64],
    store_lanes: &[u64],
) -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;

    assert_eq!(active_lanes.len(), initial_lanes.len());
    assert_eq!(active_lanes.len(), source_lanes.len());
    assert_eq!(active_lanes.len(), store_lanes.len());
    assert!(element_bytes.is_power_of_two());
    assert!(element_bytes <= 8);

    let byte_len = active_lanes.len() * element_bytes;
    assert_eq!(byte_len % 4, 0);
    let vector_bytes = byte_len as i32;
    let compare_words = byte_len / 4;
    let fail_instruction_index = 18 + compare_words as i32 * 6;

    let mut words = vec![
        u_type(0, 10, 0x17),                            // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                 // addi x12, x10, mask data
        i_type(vector_bytes, 10, 0b000, 13, 0x13),      // addi x13, x10, initial vector
        i_type(vector_bytes * 2, 10, 0b000, 14, 0x13),  // addi x14, x10, source vector
        i_type(vector_bytes * 3, 10, 0b000, 15, 0x13),  // addi x15, x10, load result
        i_type(vector_bytes * 4, 10, 0b000, 16, 0x13),  // addi x16, x10, store result
        i_type(vector_bytes * 5, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load result
        i_type(vector_bytes * 6, 10, 0b000, 20, 0x13),  // addi x20, x10, expected store result
        i_type(avl, 0, 0b000, 11, 0x13),                // addi x11, x0, vl
        vsetvli_type(vtype, 11, 5),                     // vsetvli x5, x11, e*, m1, ta, ma
        vector_unit_stride_load_type(true, width, 12, 1),
        vector_vi_type(0b011000, 1, 0, 0), // vmseq.vi v0, v1, 0
        vector_unit_stride_load_type(true, width, 13, 2),
        vector_unit_stride_load_type(false, width, 14, 2),
        vector_unit_stride_store_type(true, width, 15, 2),
        vector_unit_stride_store_type(false, width, 16, 2),
    ];

    for word_index in 0..compare_words {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    for word_index in 0..compare_words {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, store result
        words.push(i_type(offset, 20, 0b010, 18, 0x03)); // lw x18, expected store result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mask_lanes: Vec<u64> = active_lanes
        .iter()
        .copied()
        .map(|active| if active { 0 } else { 1 })
        .collect();
    let expected_load_lanes: Vec<u64> = active_lanes
        .iter()
        .zip(initial_lanes)
        .zip(source_lanes)
        .map(|((active, initial), source)| if *active { *source } else { *initial })
        .collect();
    let expected_store_lanes: Vec<u64> = active_lanes
        .iter()
        .zip(store_lanes)
        .zip(source_lanes)
        .map(|((active, store), source)| if *active { *source } else { *store })
        .collect();

    let blocks = [
        vector_lanes_to_bytes(element_bytes, &mask_lanes),
        vector_lanes_to_bytes(element_bytes, initial_lanes),
        vector_lanes_to_bytes(element_bytes, source_lanes),
        vec![0; byte_len],
        vector_lanes_to_bytes(element_bytes, store_lanes),
        vector_lanes_to_bytes(element_bytes, &expected_load_lanes),
        vector_lanes_to_bytes(element_bytes, &expected_store_lanes),
    ];

    let mut program = riscv64_program(&words);
    for bytes in blocks {
        assert_eq!(bytes.len(), byte_len);
        program.extend_from_slice(&bytes);
    }
    program
}

fn vector_lanes_to_bytes(element_bytes: usize, lanes: &[u64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(element_bytes * lanes.len());
    for lane in lanes {
        bytes.extend_from_slice(&lane.to_le_bytes()[..element_bytes]);
    }
    bytes
}
