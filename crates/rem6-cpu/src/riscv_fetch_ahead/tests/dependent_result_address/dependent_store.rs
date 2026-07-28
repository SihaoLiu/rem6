use super::*;

#[test]
fn dependent_scalar_sd_authorizes_addressless_terminal_effect() {
    let head_ld = ld(5, 2, 0);
    let (core, head, younger) = completed_result_pair(head_ld, s_type(16, 6, 5, 0b011));
    let authorization =
        dependent_authorization(&core, &head, &younger).expect("dependent store authority");

    assert_eq!(
        authorization.role(),
        O3MemoryResultWindowRole::YoungerDependentEffect
    );
    assert_eq!(authorization.route(), O3MemoryResultWindowRoute::Memory);
    assert_eq!(authorization.integer_destination(), None);
    assert_eq!(authorization.resolved_range(), None);
    assert_eq!(
        authorization.dependent_source(),
        Some((
            Register::new(5).unwrap(),
            MemoryWidth::Doubleword,
            Immediate::new(16)
        ))
    );

    let head_authorization = resolved_head_authorization(&core, &head);
    let state = core.state.lock().expect("riscv core lock");
    let mut authorizer = detailed_o3::DependentResultAddressAuthorizer::from_head(
        &state,
        &head,
        head_authorization,
        state.o3_runtime.scalar_memory_window_limit(),
    )
    .expect("dependent store authorizer");
    assert!(authorizer.try_authorize_next(&younger).is_some());
    assert_eq!(authorizer.try_authorize_next(&younger), None);
}

#[test]
fn dependent_address_store_rejects_non_exact_widths() {
    for (label, head_raw, younger_bytes) in [
        ("word store", ld(5, 2, 0), bytes(s_type(0, 6, 5, 0b010))),
        (
            "compressed store",
            ld(8, 2, 0),
            0xe004_u16.to_le_bytes().to_vec(),
        ),
    ] {
        let (core, head, younger) = completed_result_with_younger_bytes(head_raw, younger_bytes);
        assert_eq!(
            dependent_authorization(&core, &head, &younger),
            None,
            "{label}"
        );
    }
}

#[test]
fn dependent_address_atomic_head_accepts_only_unordered_amoswap_d() {
    let dependent_ld = ld(6, 5, 0);
    let (core, head, younger) = completed_result_pair(unordered_amo(5, 2, 3), dependent_ld);
    assert_eq!(
        dependent_authorization(&core, &head, &younger)
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );

    for (label, head_raw) in [
        ("atomic add", atomic_type(0x00, false, false, 3, 2, 5)),
        (
            "word atomic swap",
            atomic_type(0x01, false, false, 3, 2, 5) ^ (0b001 << 12),
        ),
        ("acquire atomic", atomic_type(0x01, true, false, 3, 2, 5)),
        ("release atomic", atomic_type(0x01, false, true, 3, 2, 5)),
        ("load reserved", lr(5, 2)),
        ("store conditional", sc(5, 2, 3)),
    ] {
        let (core, head, younger) = completed_result_pair(head_raw, dependent_ld);
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            detailed_o3::dependent_result_address_authorization(
                &state,
                &head,
                &younger,
                synthetic_resolved_head_authorization(),
                state.o3_runtime.scalar_memory_window_limit(),
            ),
            None,
            "{label}"
        );
    }
}
