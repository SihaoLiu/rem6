use rem6_memory::TranslationRequestId;

use super::*;
use crate::riscv_translation::{PendingDataTranslation, TranslatedDataAccess};

#[path = "translated_mmio_result_pair/fixture.rs"]
mod fixture;

use fixture::*;

#[test]
fn translated_result_pair_without_outstanding_data_is_ordinary() {
    let (_scheduler, _transport, fetch_route, _data_route) = memory_routes();
    let core = RiscvCore::new(cpu_core(fetch_route, HEAD_PC));

    assert_eq!(
        core.translated_result_pair_progress(0),
        O3ResultPairProgress::Ordinary
    );
}

#[test]
fn translated_result_pair_exact_resident_pair_is_ready() {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Ready { issue_tick }
    );
}

#[test]
fn translated_result_pair_memory_width_waits_for_selected_tick() {
    let core = translated_result_pair_with_outstanding_head(1);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::WaitUntil(issue_tick + 1)
    );
}

#[test]
fn translated_result_pair_rejects_unrelated_outstanding_request() {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let (request_id, mut issued) = sole_outstanding(&state);
        state.outstanding_data.remove(&request_id);
        let unrelated_request = fetch_request(100);
        issued.request = unrelated_request;
        issued.fetch_request = fetch_request(99);
        state.outstanding_data.insert(unrelated_request, issued);
    }

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );
}

#[test]
fn translated_result_pair_blocks_multiple_or_unrelated_auxiliary_state() {
    let multiple = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&multiple);
    {
        let mut state = multiple.state.lock().expect("riscv core lock");
        let (_, mut extra) = sole_outstanding(&state);
        extra.request = fetch_request(100);
        extra.fetch_request = fetch_request(99);
        state.outstanding_data.insert(extra.request, extra);
    }
    assert_eq!(
        multiple.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let pending = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&pending);
    {
        let mut state = pending.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        state.pending_data_translations.insert(
            TranslationRequestId::new(AgentId::new(7), 200),
            PendingDataTranslation {
                request_id: fetch_request(200),
                fetch_request: fetch_request(99),
                access: issued.access,
                virtual_address: Address::new(0x6000),
                size: issued.size,
                request_byte_offset: issued.request_byte_offset,
            },
        );
    }
    assert_eq!(
        pending.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let ready = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&ready);
    {
        let mut state = ready.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        state.ready_translated_data.insert(
            fetch_request(99),
            TranslatedDataAccess {
                request_id: fetch_request(200),
                fetch_request: fetch_request(99),
                access: issued.access,
                virtual_address: Address::new(0x6000),
                size: issued.size,
                physical_address: Address::new(0xa000),
                request_byte_offset: issued.request_byte_offset,
            },
        );
    }
    assert_eq!(
        ready.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let buffered = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&buffered);
    {
        let mut state = buffered.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        let issue = OutstandingDataAccess {
            tick: issued.tick,
            partition: issued.partition,
            target: issued.target,
            request_id: fetch_request(200),
            fetch_request: fetch_request(99),
            access: issued.access,
            size: issued.size,
            physical_address: issued.physical_address,
            request_byte_offset: issued.request_byte_offset,
            line_layout: Some(line_layout()),
            forwarded_load_data: None,
            store_load_forwarding_plan: issued.store_load_forwarding_plan,
        };
        let request = issue.memory_request().unwrap();
        state.buffered_o3_effects.insert(
            issue.request_id,
            BufferedO3Effect {
                predecessor: issued.request,
                issue,
                request,
            },
        );
    }
    assert_eq!(
        buffered.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let full = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&full);
    full.set_o3_window_depths(1, 1);
    assert_eq!(
        full.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );
}

#[test]
fn translated_split_gapped_result_pair_is_ready_with_two_memory_slots() {
    let core = translated_split_gapped_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Ready { issue_tick }
    );
}

#[test]
fn translated_split_gapped_result_pair_waits_with_one_memory_slot() {
    let core = translated_split_gapped_result_pair_with_outstanding_head(1);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::WaitUntil(issue_tick + 1)
    );
}

#[test]
fn translated_result_pair_exact_pending_and_ready_keys_preserve_progress() {
    let pending = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&pending);
    install_pending_younger_translation(&pending, false);
    assert_eq!(
        pending.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Ready { issue_tick }
    );

    let ready = translated_result_pair_with_outstanding_head(1);
    let issue_tick = outstanding_issue_tick(&ready);
    install_ready_younger_translation(&ready, false);
    assert_eq!(
        ready.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::WaitUntil(issue_tick + 1)
    );
}

#[test]
fn translated_result_pair_rejects_mismatched_pending_and_ready_map_keys() {
    let pending = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&pending);
    install_pending_younger_translation(&pending, true);
    assert_eq!(
        pending.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let ready = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&ready);
    install_ready_younger_translation(&ready, true);
    assert_eq!(
        ready.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );
}

#[test]
fn translated_result_pair_requires_exact_outstanding_access_identity() {
    assert_outstanding_mutation_blocks("destination", |issued| {
        issued.access = translated_head_access(13, HEAD_VIRTUAL_ADDRESS);
    });
    assert_outstanding_mutation_blocks("virtual address", |issued| {
        issued.access = translated_head_access(11, HEAD_VIRTUAL_ADDRESS + 8);
    });
    assert_outstanding_mutation_blocks("size", |issued| {
        issued.size = AccessSize::new(4).unwrap();
    });
    assert_outstanding_mutation_blocks("request byte offset", |issued| {
        issued.request_byte_offset = 1;
    });
    assert_outstanding_mutation_blocks("target", |issued| {
        issued.target = RiscvDataAccessTarget::Mmio {
            route: MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        };
    });
}

#[test]
fn translated_result_pair_requires_coherent_pending_and_ready_spans() {
    let progress = [
        pending_progress_after_mutation(|pending| {
            pending.access = translated_head_access(12, HEAD_VIRTUAL_ADDRESS);
        }),
        pending_progress_after_mutation(|pending| {
            pending.virtual_address = Address::new(HEAD_VIRTUAL_ADDRESS);
        }),
        pending_progress_after_mutation(|pending| {
            pending.size = AccessSize::new(4).unwrap();
        }),
        pending_progress_after_mutation(|pending| {
            pending.request_byte_offset = 1;
        }),
        ready_progress_after_mutation(|ready| {
            ready.access = translated_head_access(12, HEAD_VIRTUAL_ADDRESS);
        }),
        ready_progress_after_mutation(|ready| {
            ready.virtual_address = Address::new(HEAD_VIRTUAL_ADDRESS);
        }),
        ready_progress_after_mutation(|ready| {
            ready.size = AccessSize::new(4).unwrap();
        }),
        ready_progress_after_mutation(|ready| {
            ready.request_byte_offset = 1;
        }),
    ];

    assert_eq!(progress, [O3ResultPairProgress::Blocked; 8]);
}

fn assert_outstanding_mutation_blocks(label: &str, mutate: impl FnOnce(&mut IssuedDataAccess)) {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);
    mutate_sole_outstanding(&core, mutate);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked,
        "{label}"
    );
}

fn pending_progress_after_mutation(
    mutate: impl FnOnce(&mut PendingDataTranslation),
) -> O3ResultPairProgress {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);
    install_pending_younger_translation(&core, false);
    mutate_sole_pending_translation(&core, mutate);

    core.translated_result_pair_progress(issue_tick)
}

fn ready_progress_after_mutation(
    mutate: impl FnOnce(&mut TranslatedDataAccess),
) -> O3ResultPairProgress {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);
    install_ready_younger_translation(&core, false);
    mutate_sole_ready_translation(&core, mutate);

    core.translated_result_pair_progress(issue_tick)
}
