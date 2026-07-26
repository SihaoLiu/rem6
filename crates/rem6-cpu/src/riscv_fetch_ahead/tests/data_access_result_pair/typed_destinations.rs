use super::*;

fn float_mul_d(source: u8) -> u32 {
    r_type(0b0001001, source, 4, 0, 5, 0x53)
}

fn addi(source: u8) -> u32 {
    i_type(1, source, 0, 7, 0x13)
}

fn vector_move_to_scalar(source: u8) -> u32 {
    (0b010000_u32 << 26) | (1 << 25) | (u32::from(source) << 20) | (0b010 << 12) | (7 << 7) | 0x57
}

fn assert_pair_retained(core: &RiscvCore) {
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(0)));
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(1)));
}

fn direct_fp_pair_core(second_result: u32, suffix: u32) -> RiscvCore {
    direct_pair_core([
        (1, 0x8004, second_result.to_le_bytes().to_vec()),
        (2, 0x8008, suffix.to_le_bytes().to_vec()),
    ])
}

fn assert_direct_pair_boundary(second_result: u32, matching: u32, nonmatching: u32) {
    let matching = direct_fp_pair_core(second_result, matching);
    assert_eq!(matching.next_fetch_ahead_before_retire(), None);
    assert_pair_retained(&matching);

    assert_eq!(
        direct_fp_pair_core(second_result, nonmatching)
            .next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}

fn atomic_fp_pair_core(suffix: u32) -> RiscvCore {
    let second_float = i_type(0, 4, 0b011, 2, 0x07);
    let core = atomic_pair_core(second_float, 0x9010);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(completed_pair_fetch(2, 0x8008, suffix));
    core
}

fn vector_pair_core(suffix: u32) -> RiscvCore {
    let vector_head = (1_u32 << 25) | (2 << 15) | (0b111 << 12) | (2 << 7) | 0x07;
    let second_load = i_type(0, 3, 0b011, 13, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, vector_head.to_le_bytes().to_vec()),
        (1, 0x8004, second_load.to_le_bytes().to_vec()),
        (2, 0x8008, suffix.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 0x9010);
    core
}

#[test]
fn typed_fp_head_pair_window_blocks_matching_suffix_and_keeps_controls_live() {
    let second_result = i_type(0, 3, 0b011, 13, 0x03);
    assert_direct_pair_boundary(second_result, float_mul_d(1), float_mul_d(6));
}

#[test]
fn typed_same_fp_destination_pair_blocks_matching_suffix_and_keeps_control_live() {
    let younger_fld_f1 = i_type(0, 3, 0b011, 1, 0x07);
    assert_direct_pair_boundary(younger_fld_f1, float_mul_d(1), float_mul_d(6));
}

#[test]
fn typed_same_index_cross_class_pair_retains_both_authorities() {
    let younger_ld_x1 = i_type(0, 3, 0b011, 1, 0x03);
    for (matching, nonmatching) in [(float_mul_d(1), float_mul_d(6)), (addi(1), addi(6))] {
        assert_direct_pair_boundary(younger_ld_x1, matching, nonmatching);
    }

    let core = direct_fp_pair_core(younger_ld_x1, float_mul_d(6));
    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .and_then(O3MemoryResultWindowAuthorization::integer_destination),
        Some(Register::new(1).unwrap())
    );
}

#[test]
fn typed_younger_fp_pair_window_blocks_matching_suffix_and_keeps_control_live() {
    let matching = atomic_fp_pair_core(float_mul_d(2));
    assert_eq!(matching.next_fetch_ahead_before_retire(), None);
    assert_pair_retained(&matching);

    assert_eq!(
        atomic_fp_pair_core(float_mul_d(6))
            .next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}

#[test]
fn typed_vector_pair_window_rejects_matching_vector_to_scalar_suffix() {
    let matching = vector_pair_core(vector_move_to_scalar(2));
    assert_eq!(matching.next_fetch_ahead_before_retire(), None);
    assert_pair_retained(&matching);

    assert_eq!(
        vector_pair_core(vector_move_to_scalar(3))
            .next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}
