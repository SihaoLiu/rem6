use super::*;

use rem6_isa_riscv::{RiscvTrap, RiscvTrapKind};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::MemoryResponse;
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome};

#[test]
fn producer_forwarded_target_response_issues_descendant_after_o3_wake() {
    let (core, _) = live_same_link_core(false);
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("producer-forwarded target decision");
    let authority = producer_forwarded_target(&decision);
    assert_eq!(decision.pc(), Address::new(0x9000));

    let prepared = core.prepare_fetch_ahead_speculation(&decision).unwrap();
    core.set_fetch_ahead_pc(decision.pc());
    core.core
        .advance_sequence_past(authority.last_fetch_request());

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(route, core.core.fetch_route());

    let descendant = i_type(0, 1, 0x0, 13, 0x13);
    core.issue_next_fetch_with_prepared_fetch_ahead(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| TargetOutcome::RespondAfter {
            delay: 30,
            response: MemoryResponse::completed(
                delivery.request(),
                Some(descendant.to_le_bytes().to_vec()),
            )
            .unwrap(),
        },
        prepared,
    )
    .unwrap();
    scheduler.run_until_idle_conservative();

    let response_tick = core
        .core
        .fetch_events()
        .into_iter()
        .find(|event| {
            event.kind() == crate::CpuFetchEventKind::Completed
                && event.request_id().sequence() > authority.last_fetch_request().sequence()
                && event.pc() == Address::new(0x9000)
        })
        .expect("completed target fetch")
        .tick();
    assert!(fire_requested_o3_writeback_wakes(&core).contains(&response_tick));
    let snapshot = core.o3_runtime_snapshot();
    let target = snapshot
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(0x9000))
        .expect("target response must stage its descendant immediately");
    assert!(target.is_live_staged());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .o3_runtime
            .producer_forwarded_scalar_issue_tick_for_test(),
        Some(response_tick)
    );
}

#[test]
fn producer_forwarded_target_response_respects_frontend_gates_and_exact_completion() {
    let (core, _) = live_same_link_core(false);
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("producer-forwarded target decision");
    let authority = producer_forwarded_target(&decision);
    let prepared = core.prepare_fetch_ahead_speculation(&decision).unwrap();
    core.set_fetch_ahead_pc(decision.pc());
    core.core
        .advance_sequence_past(authority.last_fetch_request());
    core.state.lock().expect("riscv core lock").pending_trap =
        Some(RiscvTrap::new(RiscvTrapKind::Interrupt { code: 1 }, 0x9000));

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let descendant = i_type(0, 1, 0x0, 13, 0x13);
    core.issue_next_fetch_with_prepared_fetch_ahead(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| TargetOutcome::RespondAfter {
            delay: 30,
            response: MemoryResponse::completed(
                delivery.request(),
                Some(descendant.to_le_bytes().to_vec()),
            )
            .unwrap(),
        },
        prepared,
    )
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(core
        .o3_runtime_snapshot()
        .reorder_buffer()
        .iter()
        .all(|entry| entry.pc() != Address::new(0x9000)));
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .producer_forwarded_scalar_issue_tick_for_test(),
        None
    );

    core.state.lock().expect("riscv core lock").pending_trap = None;
    let unrelated = i_type(0, 0, 0x0, 0, 0x13);
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(unrelated.to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(core
        .o3_runtime_snapshot()
        .reorder_buffer()
        .iter()
        .all(|entry| entry.pc() != Address::new(0x9000)));
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .producer_forwarded_scalar_issue_tick_for_test(),
        None
    );
}
