use super::*;

fn atomic_type(operation: u32, acquire: bool, release: bool, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (operation << 27)
        | (u32::from(acquire) << 26)
        | (u32::from(release) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b011 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

#[test]
fn float_head_authorizes_a_disjoint_unordered_younger_amo_effect() {
    let float_head = i_type(0, 2, 0b011, 1, 0x07);
    let atomic = atomic_type(0x01, false, false, 3, 4, 11);
    let core = core_with_completed_fetches([
        (0, 0x8000, float_head.to_le_bytes().to_vec()),
        (1, 0x8004, atomic.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 7);
    core.write_register(Register::new(4).unwrap(), 0x9010);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerBufferedEffect)
    );
}

#[test]
fn masked_vector_head_authorizes_a_disjoint_unordered_younger_amo_effect() {
    let vector_head = (2_u32 << 15) | (0b111 << 12) | (1 << 7) | 0x07;
    let atomic = atomic_type(0x00, false, false, 3, 4, 11);
    let core = core_with_completed_fetches([
        (0, 0x8000, vector_head.to_le_bytes().to_vec()),
        (1, 0x8004, atomic.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 7);
    core.write_register(Register::new(4).unwrap(), 0x9010);
    let mut mask = [0_u8; rem6_isa_riscv::RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b11;
    core.write_vector_register(rem6_isa_riscv::VectorRegister::new(0).unwrap(), mask);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerBufferedEffect)
    );
}

#[test]
fn scalar_load_head_selects_the_effect_lane_for_an_adjacent_atomic() {
    let load_head = i_type(0, 2, 0b011, 5, 0x03);
    let atomic = atomic_type(0x01, false, false, 3, 4, 11);
    let core = core_with_completed_fetches([
        (0, 0x8000, load_head.to_le_bytes().to_vec()),
        (1, 0x8004, atomic.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 7);
    core.write_register(Register::new(4).unwrap(), 0x9010);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(0))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::Head)
    );
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerBufferedEffect)
    );
}

#[test]
fn younger_amo_effect_rejects_ordering_overlap_dependency_and_zero_destination() {
    let float_head = i_type(0, 2, 0b011, 1, 0x07);
    let load_head = i_type(0, 2, 0b011, 5, 0x03);
    for (label, head, atomic, atomic_base) in [
        (
            "acquire",
            float_head,
            atomic_type(0x01, true, false, 3, 4, 11),
            0x9010,
        ),
        (
            "release",
            float_head,
            atomic_type(0x01, false, true, 3, 4, 11),
            0x9010,
        ),
        (
            "overlap",
            float_head,
            atomic_type(0x01, false, false, 3, 4, 11),
            0x9000,
        ),
        (
            "dependent base",
            load_head,
            atomic_type(0x01, false, false, 3, 5, 11),
            0x9010,
        ),
        (
            "zero destination",
            float_head,
            atomic_type(0x01, false, false, 3, 4, 0),
            0x9010,
        ),
    ] {
        let core = core_with_completed_fetches([
            (0, 0x8000, head.to_le_bytes().to_vec()),
            (1, 0x8004, atomic.to_le_bytes().to_vec()),
        ]);
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_scalar_memory_depth(4);
        core.write_register(Register::new(2).unwrap(), 0x9000);
        core.write_register(Register::new(3).unwrap(), 7);
        core.write_register(Register::new(4).unwrap(), atomic_base);
        core.write_register(Register::new(5).unwrap(), atomic_base);

        assert_eq!(core.next_fetch_ahead_before_retire(), None, "{label}");
        assert!(
            core.state
                .lock()
                .expect("riscv core lock")
                .memory_result_window_authorizations
                .is_empty(),
            "{label}"
        );
    }
}

#[test]
fn ordinary_scalar_load_suffix_keeps_scalar_memory_authority() {
    let load_head = i_type(0, 2, 0b011, 5, 0x03);
    let addi = i_type(1, 0, 0, 6, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load_head.to_le_bytes().to_vec()),
        (1, 0x8004, addi.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .is_empty());
}
