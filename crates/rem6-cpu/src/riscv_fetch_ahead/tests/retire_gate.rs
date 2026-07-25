use super::*;

#[test]
fn architectural_predecessor_retires_before_younger_branch_speculation() {
    let producer = i_type(7, 0, 0x0, 6, 0x13);
    let branch = j_type(8, 0);
    let core = core_with_completed_fetches([
        (0, 0x8000, producer.to_le_bytes().to_vec()),
        (1, 0x8004, branch.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);
    let pending = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(2),
        Address::new(0x800c),
        AccessSize::new(4).unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        insert_pending_branch_speculation(
            &mut state,
            1,
            Address::new(0x8004),
            Address::new(0x800c),
        );
    }
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(pending));

    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
}
