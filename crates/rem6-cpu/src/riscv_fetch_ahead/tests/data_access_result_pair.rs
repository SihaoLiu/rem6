use super::*;

fn direct_pair_core(younger: impl IntoIterator<Item = (u64, u64, Vec<u8>)>) -> RiscvCore {
    let float_head = i_type(0, 2, 0b011, 1, 0x07);
    let core = core_with_completed_fetches(
        [(0, 0x8000, float_head.to_le_bytes().to_vec())]
            .into_iter()
            .chain(younger),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 0x9010);
    core
}

fn completed_pair_fetch(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            4,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn atomic_pair_core(second: u32, second_base: u64) -> RiscvCore {
    let atomic = (0x01_u32 << 27) | (3 << 20) | (2 << 15) | (0b011 << 12) | (11 << 7) | 0x2f;
    let core = core_with_completed_fetches([
        (0, 0x8000, atomic.to_le_bytes().to_vec()),
        (1, 0x8004, second.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 7);
    core.write_register(Register::new(4).unwrap(), second_base);
    core.write_register(Register::new(11).unwrap(), second_base);
    core
}

#[test]
fn independent_second_result_authorizes_the_pair_and_fetches_the_scalar_suffix() {
    let second_load = i_type(0, 3, 0b011, 13, 0x03);
    let core = direct_pair_core([(1, 0x8004, second_load.to_le_bytes().to_vec())]);

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
            .get(&request(0))
            .copied()
            .and_then(O3MemoryResultWindowAuthorization::integer_destination),
        None
    );
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .and_then(O3MemoryResultWindowAuthorization::integer_destination),
        Some(Register::new(13).unwrap())
    );
}

#[test]
fn executed_unissued_head_retains_pair_authority_when_the_second_fetch_completes() {
    let core = direct_pair_core([]);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8004))
    );
    assert_eq!(
        core.execute_next_completed_fetch()
            .unwrap()
            .expect("result head executes")
            .fetch_pc(),
        Address::new(0x8000)
    );

    let second_load = i_type(0, 3, 0b011, 13, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(completed_pair_fetch(1, 0x8004, second_load));

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerRead)
    );
}

#[test]
fn third_result_is_not_fetched_but_the_authorized_pair_is_retained() {
    let second_load = i_type(0, 3, 0b011, 13, 0x03);
    let third_float = i_type(8, 3, 0b011, 2, 0x07);
    let core = direct_pair_core([
        (1, 0x8004, second_load.to_le_bytes().to_vec()),
        (2, 0x8008, third_float.to_le_bytes().to_vec()),
    ]);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(0)));
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(1)));
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(2)));
}

#[test]
fn disjoint_unordered_atomic_head_authorizes_a_younger_float_read() {
    let second_float = i_type(0, 4, 0b011, 2, 0x07);
    let core = atomic_pair_core(second_float, 0x9010);

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
            .len(),
        2
    );
}

#[test]
fn executed_unissued_atomic_head_authorizes_a_later_disjoint_float_fetch() {
    let second_float = i_type(0, 4, 0b011, 2, 0x07);
    let core = atomic_pair_core(second_float, 0x9010);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .truncate(1);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8004))
    );
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("atomic head executes");
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(completed_pair_fetch(1, 0x8004, second_float));

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
        Some(O3MemoryResultWindowRole::YoungerRead)
    );
}

#[test]
fn vector_head_writing_v0_blocks_a_masked_younger_vector_result() {
    let head_v0 = (1_u32 << 25) | (2 << 15) | (0b111 << 12) | 0x07;
    let masked_younger = (3_u32 << 15) | (0b111 << 12) | (1 << 7) | 0x07;
    let core = core_with_completed_fetches([
        (0, 0x8000, head_v0.to_le_bytes().to_vec()),
        (1, 0x8004, masked_younger.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 0x9010);
    let mut mask = [0_u8; rem6_isa_riscv::RISCV_VECTOR_REGISTER_BYTES];
    mask[0] = 0b11;
    core.write_vector_register(rem6_isa_riscv::VectorRegister::new(0).unwrap(), mask);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .is_empty());
}

#[test]
fn split_head_and_whole_younger_result_keep_distinct_authorization_identities() {
    let head = i_type(0, 2, 0b011, 1, 0x07).to_le_bytes();
    let younger = i_type(0, 3, 0b011, 13, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, head[..2].to_vec()),
        (1, 0x8002, head[2..].to_vec()),
        (2, 0x8004, younger.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(3).unwrap(), 0x9010);

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
            .get(&request(0))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::Head)
    );
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(1)));
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(2))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerRead)
    );
}

#[test]
fn exact_dependent_scalar_read_uses_dependent_address_role() {
    let second = i_type(0, 11, 0b011, 13, 0x03);
    let core = atomic_pair_core(second, 0x9010);

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
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );
}

#[test]
fn resolved_dependent_or_overlapping_second_result_does_not_open_a_pair() {
    for (label, second, base) in [
        (
            "resolved dependent float base",
            i_type(0, 11, 0b011, 2, 0x07),
            0x9010,
        ),
        (
            "overlapping atomic range",
            i_type(0, 4, 0b011, 2, 0x07),
            0x9000,
        ),
    ] {
        let core = atomic_pair_core(second, base);
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
