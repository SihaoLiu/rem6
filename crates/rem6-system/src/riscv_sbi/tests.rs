use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuResetState, CpuTranslationFrontend,
    CpuTranslationFrontendSnapshot,
};
use rem6_kernel::{PartitionId, PartitionSnapshot, SchedulerContext, SchedulerSnapshot};
use rem6_memory::{
    AccessSize, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
    TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
    TranslationQueueSnapshot, TranslationTlbConfig, TranslationTlbEntrySnapshot,
    TranslationTlbSnapshot, TranslationTlbStats,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome, TransportEndpointId,
};

use super::*;

const TEST_HART_START_PENDING: u64 = 2;
const TEST_HART_STOP_PENDING: u64 = 3;
const TEST_HART_SUSPEND_PENDING: u64 = 5;
const TEST_HART_RESUME_PENDING: u64 = 6;

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).expect("valid test endpoint")
}

fn test_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    route: rem6_transport::MemoryRouteId,
    pc: u64,
) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(pc),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                route,
                CacheLineLayout::new(16).expect("valid cache line size"),
                AccessSize::new(4).expect("valid fetch size"),
            ),
        )
        .expect("valid CPU core"),
    )
}

struct TranslatedTestCoreSpec<'a> {
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &'a str,
    fetch_route: rem6_transport::MemoryRouteId,
    data_endpoint: &'a str,
    data_route: rem6_transport::MemoryRouteId,
    pc: u64,
}

fn translated_test_core(
    spec: TranslatedTestCoreSpec<'_>,
    tlb_entries: Vec<TranslationTlbEntrySnapshot>,
) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(spec.cpu),
            PartitionId::new(spec.partition),
            AgentId::new(spec.agent),
            Address::new(spec.pc),
        ),
        CpuFetchConfig::new(
            endpoint(spec.fetch_endpoint),
            spec.fetch_route,
            CacheLineLayout::new(16).expect("valid cache line size"),
            AccessSize::new(4).expect("valid fetch size"),
        ),
    )
    .expect("valid CPU core");
    let queue_config = TranslationQueueConfig::new(4, 0).expect("valid translation queue");
    let tlb_config = TranslationTlbConfig::new(4).expect("valid translation TLB");
    let frontend =
        CpuTranslationFrontend::from_snapshot(&CpuTranslationFrontendSnapshot::new_with_tlb(
            TranslationQueueSnapshot::new(queue_config, Vec::new(), 0),
            Vec::new(),
            TranslationTlbSnapshot::new(tlb_config, tlb_entries, 8, TranslationTlbStats::default()),
        ))
        .expect("valid translated frontend snapshot");

    RiscvCore::with_data_translation(
        core,
        CpuDataConfig::new(
            endpoint(spec.data_endpoint),
            spec.data_route,
            CacheLineLayout::new(16).expect("valid cache line size"),
        ),
        frontend,
    )
}

fn rfence_tlb_entry(virtual_page: u64, physical_page: u64) -> TranslationTlbEntrySnapshot {
    rfence_tlb_entry_in_address_space(
        TranslationAddressSpaceId::global(),
        virtual_page,
        physical_page,
    )
}

fn rfence_tlb_entry_in_address_space(
    address_space: TranslationAddressSpaceId,
    virtual_page: u64,
    physical_page: u64,
) -> TranslationTlbEntrySnapshot {
    TranslationTlbEntrySnapshot::new_in_address_space(
        address_space,
        Address::new(virtual_page),
        Address::new(physical_page),
        TranslationPageSize::new(4096).expect("valid page size"),
        TranslationPagePermissions::read_write_execute(),
        4,
    )
}

fn hsm_request(function: u64, arg0: u64, arg1: u64, arg2: u64) -> RiscvSbiRequest {
    RiscvSbiRequest {
        extension: SBI_HSM_EXTENSION,
        function,
        arg0,
        arg1,
        arg2,
        arg3: 0,
        arg4: 0,
    }
}

fn rfence_request(
    function: u64,
    hart_mask: u64,
    hart_mask_base: u64,
    start_addr: u64,
    size: u64,
    asid: u64,
) -> RiscvSbiRequest {
    RiscvSbiRequest {
        extension: SBI_RFENCE_EXTENSION,
        function,
        arg0: hart_mask,
        arg1: hart_mask_base,
        arg2: start_addr,
        arg3: size,
        arg4: asid,
    }
}

fn ecall_store(address: u64) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let layout = CacheLineLayout::new(16).expect("valid cache line size");
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout).expect("memory target");
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).expect("valid mapped size"),
        )
        .expect("mapped test memory");
    BootImage::new(Address::new(address))
        .add_segment(
            Address::new(address),
            0x0000_0073_u32.to_le_bytes().to_vec(),
        )
        .expect("ecall segment")
        .load_into_partitioned_store(&mut store, target)
        .expect("loaded ecall");
    Arc::new(Mutex::new(store))
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
    move |delivery, _context| {
        let response = store
            .lock()
            .expect("test memory lock")
            .respond(delivery.request())
            .expect("memory response")
            .response()
            .cloned()
            .expect("completed memory response");
        TargetOutcome::Respond(response)
    }
}

fn registered_hsm_pair() -> (
    PartitionedScheduler,
    MemoryTransport,
    RiscvSbiFirmware,
    RiscvCore,
    RiscvCore,
) {
    let scheduler =
        PartitionedScheduler::with_min_remote_delay(4, 2).expect("valid test scheduler");
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .expect("valid CPU 0 route"),
        )
        .expect("registered CPU 0 route");
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .expect("valid CPU 1 route"),
        )
        .expect("registered CPU 1 route");
    let core0 = test_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = test_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x8800);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).expect("valid cluster");
    let firmware = RiscvSbiFirmware::new();
    firmware
        .register_cluster(&cluster)
        .expect("cluster registers with SBI firmware");
    (scheduler, transport, firmware, core0, core1)
}

fn registered_rfence_pair() -> (
    PartitionedScheduler,
    MemoryTransport,
    RiscvSbiFirmware,
    RiscvCore,
    RiscvCore,
) {
    registered_rfence_pair_with_tlb_entries(vec![rfence_tlb_entry(0x4000, 0x9000)])
}

fn registered_rfence_pair_with_tlb_entries(
    tlb_entries: Vec<TranslationTlbEntrySnapshot>,
) -> (
    PartitionedScheduler,
    MemoryTransport,
    RiscvSbiFirmware,
    RiscvCore,
    RiscvCore,
) {
    let scheduler =
        PartitionedScheduler::with_min_remote_delay(4, 2).expect("valid test scheduler");
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .expect("valid CPU 0 route"),
        )
        .expect("registered CPU 0 route");
    let cpu1_fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .expect("valid CPU 1 fetch route"),
        )
        .expect("registered CPU 1 fetch route");
    let cpu1_data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d"),
                PartitionId::new(2),
                2,
                3,
            )
            .expect("valid CPU 1 data route"),
        )
        .expect("registered CPU 1 data route");
    let core0 = test_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = translated_test_core(
        TranslatedTestCoreSpec {
            cpu: 1,
            partition: 1,
            agent: 8,
            fetch_endpoint: "cpu1.ifetch",
            fetch_route: cpu1_fetch_route,
            data_endpoint: "cpu1.dmem",
            data_route: cpu1_data_route,
            pc: 0x8800,
        },
        tlb_entries,
    );
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).expect("valid cluster");
    let firmware = RiscvSbiFirmware::new();
    firmware
        .register_cluster(&cluster)
        .expect("cluster registers with SBI firmware");
    (scheduler, transport, firmware, core0, core1)
}

fn execute_sbi_ecall(
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    core: &RiscvCore,
    extension: u64,
    function: u64,
    args: [u64; 5],
) {
    core.write_register(register(17), extension);
    core.write_register(register(16), function);
    for (offset, value) in args.into_iter().enumerate() {
        core.write_register(register(10 + offset as u8), value);
    }
    core.issue_next_fetch(
        scheduler,
        transport,
        MemoryTrace::new(),
        responder(ecall_store(0x8000)),
    )
    .expect("issued ecall fetch");
    scheduler.run_until_idle_conservative();
    core.execute_next_completed_fetch()
        .expect("executed ecall")
        .expect("ecall execution event");
    assert!(core.has_pending_trap());
}

fn execute_hsm_ecall(
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    core: &RiscvCore,
    function: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
) {
    execute_sbi_ecall(
        scheduler,
        transport,
        core,
        SBI_HSM_EXTENSION,
        function,
        [arg0, arg1, arg2, 0, 0],
    );
}

fn execute_rfence_ecall(
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    core: &RiscvCore,
    request: RiscvSbiRequest,
) {
    execute_sbi_ecall(
        scheduler,
        transport,
        core,
        request.extension(),
        request.function(),
        [
            request.arg0(),
            request.arg1(),
            request.arg2(),
            request.arg3(),
            request.arg4(),
        ],
    );
}

fn execute_hsm_hart_start_ecall(
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    core: &RiscvCore,
) {
    execute_hsm_ecall(
        scheduler,
        transport,
        core,
        SBI_HSM_HART_START,
        1,
        0x9000,
        0x55,
    );
}

fn assert_remote_sfence_vma_records_completion(parallel: bool) {
    let (mut scheduler, transport, firmware, core0, core1) = registered_rfence_pair();
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x4000),
        ),
        Some(true)
    );

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_SFENCE_VMA, 0b10, 0, 0, 0, 0),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, parallel)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert!(firmware.rfence_completion_records().is_empty());

    if parallel {
        scheduler
            .run_until_idle_parallel()
            .expect("parallel RFENCE completion event");
    } else {
        scheduler.run_until_idle_conservative();
    }

    assert_eq!(core1.data_translation_tlb_entry_count(), Some(0));
    let completions = firmware.rfence_completion_records();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].source_cpu(), core0.id());
    assert_eq!(completions[0].target_hart(), core1.hart_id());
    assert_eq!(completions[0].function(), SBI_RFENCE_REMOTE_SFENCE_VMA);
    assert_eq!(completions[0].start_addr(), 0);
    assert_eq!(completions[0].size(), 0);
    assert_eq!(completions[0].address_space(), None);
    assert_eq!(completions[0].flushed_entries(), Some(1));
}

#[test]
fn remote_sfence_vma_flushes_target_tlb_when_completion_event_runs() {
    assert_remote_sfence_vma_records_completion(false);
}

#[test]
fn parallel_remote_sfence_vma_records_completion() {
    assert_remote_sfence_vma_records_completion(true);
}

#[test]
fn remote_fence_i_resets_target_fetch_stream_when_completion_event_runs() {
    let (mut scheduler, transport, firmware, core0, core1) = registered_rfence_pair();

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_FENCE_I, 0b10, 0, 0, 0, 0),
    );
    core1
        .issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            responder(ecall_store(0x8800)),
        )
        .expect("issued target fetch");

    assert!(core1.has_pending_fetch());

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert!(core1.has_pending_fetch());

    scheduler.run_until_idle_conservative();

    assert!(!core1.has_pending_fetch());
    assert_eq!(core1.pc(), Address::new(0x8800));
    assert_eq!(core1.inner().pc(), Address::new(0x8800));
    assert!(core1.inner().fetch_events().is_empty());
}

fn assert_send_ipi_sets_target_ssip_when_completion_event_runs(parallel: bool) {
    let (mut scheduler, transport, firmware, core0, core1) = registered_hsm_pair();

    execute_sbi_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_IPI_EXTENSION,
        SBI_IPI_SEND_IPI,
        [0b10, 0, 0, 0, 0],
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, parallel)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(
        firmware.ipi_records(),
        vec![RiscvSbiIpiRecord::new(CpuId::new(0), 0b10, 0, vec![1])]
    );
    assert!(firmware.hsm_wake_records().is_empty());
    assert_eq!(core1.machine_interrupt_pending() & SSIP, 0);

    if parallel {
        scheduler
            .run_until_idle_parallel()
            .expect("parallel IPI event");
    } else {
        scheduler.run_until_idle_conservative();
    }

    assert_eq!(core1.machine_interrupt_pending() & SSIP, SSIP);
    assert!(firmware.hsm_wake_records().is_empty());
}

#[test]
fn handle_pending_core_trap_schedules_ipi_completion_event() {
    assert_send_ipi_sets_target_ssip_when_completion_event_runs(false);
}

#[test]
fn parallel_handle_pending_core_trap_schedules_ipi_completion_event() {
    assert_send_ipi_sets_target_ssip_when_completion_event_runs(true);
}

#[test]
fn parallel_timer_interrupt_records_hsm_wake_for_retentive_suspend() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_sbi_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_TIME_EXTENSION,
        SBI_TIME_SET_TIMER,
        [512, 0, 0, 0, 0],
    );

    let timer_outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, true)
        .expect("handled timer SBI trap")
        .expect("timer SBI outcome");

    assert_eq!(timer_outcome, RiscvSbiOutcome::success(0));
    assert_eq!(firmware.timer_deadline(CpuId::new(0)), Some(512));
    core0.set_hart_suspended();
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_SUSPENDED)
    );
    scheduler
        .run_until_idle_parallel()
        .expect("parallel timer and suspend events");

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
    assert_eq!(core0.machine_interrupt_pending() & STIP, STIP);
    assert_eq!(
        firmware.hsm_wake_records(),
        vec![RiscvSbiHsmWakeRecord::new(CpuId::new(0), 0, STIP)]
    );
}

#[test]
fn hsm_wake_records_are_ordered_for_reproducible_output() {
    let firmware = RiscvSbiFirmware::new();
    firmware
        .hsm_wakes
        .lock()
        .expect("RISC-V SBI HSM wake record lock")
        .extend([
            RiscvSbiHsmWakeRecord::new(CpuId::new(1), 3, SSIP),
            RiscvSbiHsmWakeRecord::new(CpuId::new(0), 2, SSIP),
            RiscvSbiHsmWakeRecord::new(CpuId::new(0), 1, SSIP),
        ]);

    assert_eq!(
        firmware.hsm_wake_records(),
        vec![
            RiscvSbiHsmWakeRecord::new(CpuId::new(0), 1, SSIP),
            RiscvSbiHsmWakeRecord::new(CpuId::new(0), 2, SSIP),
            RiscvSbiHsmWakeRecord::new(CpuId::new(1), 3, SSIP),
        ]
    );
}

#[test]
fn register_cluster_clears_hsm_status_records_for_next_run() {
    let (_scheduler, _transport, firmware, core0, core1) = registered_hsm_pair();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STOPPED)
    );
    assert_eq!(
        firmware.hsm_status_records(),
        vec![RiscvSbiHsmStatusRecord::new(
            CpuId::new(0),
            1,
            SBI_HSM_HART_STOPPED
        )]
    );

    let cluster = RiscvCluster::new([core0, core1]).expect("valid cluster");
    firmware
        .register_cluster(&cluster)
        .expect("re-registered cluster");

    assert!(firmware.hsm_status_records().is_empty());
}

#[test]
fn send_ipi_scheduler_error_leaves_no_partial_target_events() {
    let mut scheduler =
        PartitionedScheduler::with_min_remote_delay(2, 2).expect("valid test scheduler");
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .expect("valid CPU 0 route"),
        )
        .expect("registered CPU 0 route");
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .expect("valid CPU 1 route"),
        )
        .expect("registered CPU 1 route");
    let cpu2_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu2.ifetch"),
                PartitionId::new(7),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .expect("valid CPU 2 route"),
        )
        .expect("registered CPU 2 route");
    let core0 = test_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = test_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x8800);
    let core2 = test_core(2, 7, 9, "cpu2.ifetch", cpu2_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core2.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster =
        RiscvCluster::new([core0.clone(), core1.clone(), core2.clone()]).expect("valid cluster");
    let firmware = RiscvSbiFirmware::new();
    firmware
        .register_cluster(&cluster)
        .expect("cluster registers with SBI firmware");
    execute_sbi_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_IPI_EXTENSION,
        SBI_IPI_SEND_IPI,
        [0b110, 0, 0, 0, 0],
    );

    let error = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect_err("unknown target partition rejects IPI scheduling");

    assert_eq!(
        error,
        SystemError::Scheduler(SchedulerError::UnknownPartition {
            partition: PartitionId::new(7),
            partitions: 2,
        })
    );
    assert!(firmware.ipi_records().is_empty());
    assert_eq!(
        scheduler
            .next_pending_tick(PartitionId::new(1))
            .expect("known partition"),
        None
    );
    assert_eq!(core1.machine_interrupt_pending() & SSIP, 0);
    assert_eq!(core2.machine_interrupt_pending() & SSIP, 0);
}

#[test]
fn remote_hfence_gvma_rejects_missing_target_before_scheduling_flush() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_rfence_pair();

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID, 0b100, 0, 0, 0, 0),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::invalid_param());
}

#[test]
fn remote_hfence_gvma_flushes_target_tlb_when_completion_event_runs() {
    let (mut scheduler, transport, firmware, core0, core1) = registered_rfence_pair();
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_HFENCE_GVMA, 0b10, 0, 0, 0, 0),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));

    scheduler.run_until_idle_conservative();

    assert_eq!(core1.data_translation_tlb_entry_count(), Some(0));
}

#[test]
fn remote_hfence_gvma_range_flushes_overlapping_physical_pages_only() {
    let (mut scheduler, transport, firmware, core0, core1) =
        registered_rfence_pair_with_tlb_entries(vec![
            rfence_tlb_entry(0x4000, 0x9000),
            rfence_tlb_entry(0x5000, 0xa000),
            rfence_tlb_entry(0x7000, 0xc000),
        ]);
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(3));

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_HFENCE_GVMA, 0b10, 0, 0x9800, 0x1000, 0),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    scheduler.run_until_idle_conservative();

    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x4000),
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x5000),
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x7000),
        ),
        Some(true)
    );
}

#[test]
fn remote_hfence_gvma_vmid_preserves_non_overlapping_physical_ranges_without_vmid_tag() {
    let address_space11 = TranslationAddressSpaceId::new(11);
    let address_space12 = TranslationAddressSpaceId::new(12);
    let (mut scheduler, transport, firmware, core0, core1) =
        registered_rfence_pair_with_tlb_entries(vec![
            rfence_tlb_entry_in_address_space(address_space11, 0x4000, 0x9000),
            rfence_tlb_entry_in_address_space(address_space12, 0x5000, 0xb000),
        ]);
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(2));

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(
            SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID,
            0b10,
            0,
            0x9000,
            0x1000,
            7,
        ),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    scheduler.run_until_idle_conservative();

    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(address_space11, Address::new(0x4000)),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(address_space12, Address::new(0x5000)),
        Some(true)
    );
}

#[test]
fn remote_hfence_vvma_asid_rejects_invalid_asid_before_scheduling_flush() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_rfence_pair();

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(
            SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID,
            0b10,
            0,
            0,
            0,
            u64::from(u16::MAX) + 1,
        ),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::invalid_param());
}

#[test]
fn remote_hfence_vvma_range_flushes_overlapping_pages_only() {
    let (mut scheduler, transport, firmware, core0, core1) =
        registered_rfence_pair_with_tlb_entries(vec![
            rfence_tlb_entry(0x4000, 0x9000),
            rfence_tlb_entry(0x5000, 0xa000),
            rfence_tlb_entry(0x7000, 0xc000),
        ]);
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(3));

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_HFENCE_VVMA, 0b10, 0, 0x4800, 0x2000, 0),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    scheduler.run_until_idle_conservative();

    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x4000),
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x5000),
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x7000),
        ),
        Some(true)
    );
}

#[test]
fn remote_hfence_vvma_asid_preserves_other_address_spaces() {
    let address_space11 = TranslationAddressSpaceId::new(11);
    let address_space12 = TranslationAddressSpaceId::new(12);
    let (mut scheduler, transport, firmware, core0, core1) =
        registered_rfence_pair_with_tlb_entries(vec![
            rfence_tlb_entry_in_address_space(address_space11, 0x4000, 0x9000),
            rfence_tlb_entry_in_address_space(address_space12, 0x5000, 0xa000),
        ]);
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(2));

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(
            SBI_RFENCE_REMOTE_HFENCE_VVMA_ASID,
            0b10,
            0,
            0,
            0,
            u64::from(address_space11.get()),
        ),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    scheduler.run_until_idle_conservative();

    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(address_space11, Address::new(0x4000)),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(address_space12, Address::new(0x5000)),
        Some(true)
    );
}

#[test]
fn remote_hfence_gvma_vmid_rejects_invalid_vmid_before_scheduling_flush() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_rfence_pair();

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_HFENCE_GVMA_VMID, 0b10, 0, 0, 0, 1 << 14),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::invalid_param());
}

#[test]
fn remote_hfence_gvma_rejects_invalid_range_before_scheduling_flush() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_rfence_pair();

    execute_rfence_ecall(
        &mut scheduler,
        &transport,
        &core0,
        rfence_request(SBI_RFENCE_REMOTE_HFENCE_GVMA, 0b10, 0, u64::MAX - 1, 4, 0),
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::invalid_address());
}

#[test]
fn hart_start_reports_start_pending_before_entry_event_runs() {
    let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();

    let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

    assert_eq!(start, RiscvSbiOutcome::success(0));
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_START_PENDING)
    );
    assert_eq!(core1.pc(), Address::new(0x8800));
}

#[test]
fn hart_start_reports_already_available_for_started_target() {
    let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();
    core1.set_hart_started();

    let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

    assert_eq!(start, RiscvSbiOutcome::already_available());
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
    assert_eq!(core1.pc(), Address::new(0x8800));
}

#[test]
fn hart_start_reports_already_available_for_suspended_target() {
    let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();
    core1.set_hart_suspended();

    let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

    assert_eq!(start, RiscvSbiOutcome::already_available());
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_SUSPENDED)
    );
    assert_eq!(core1.pc(), Address::new(0x8800));
}

#[test]
fn hart_start_rejects_pending_target_states() {
    for (name, set_state, expected_status) in [
        (
            "start-pending",
            RiscvCore::set_hart_start_pending as fn(&RiscvCore),
            TEST_HART_START_PENDING,
        ),
        (
            "stop-pending",
            RiscvCore::set_hart_stop_pending as fn(&RiscvCore),
            TEST_HART_STOP_PENDING,
        ),
        (
            "suspend-pending",
            RiscvCore::set_hart_suspend_pending as fn(&RiscvCore),
            TEST_HART_SUSPEND_PENDING,
        ),
        (
            "resume-pending",
            RiscvCore::set_hart_resume_pending as fn(&RiscvCore),
            TEST_HART_RESUME_PENDING,
        ),
    ] {
        let (_scheduler, _transport, firmware, _core0, core1) = registered_hsm_pair();
        set_state(&core1);

        let start = firmware.hart_start(hsm_request(SBI_HSM_HART_START, 1, 0x9000, 0x55));

        assert_eq!(start, RiscvSbiOutcome::invalid_param(), "{name}");
        assert_eq!(
            firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
            RiscvSbiOutcome::success(expected_status),
            "{name}",
        );
        assert_eq!(core1.pc(), Address::new(0x8800), "{name}");
    }
}

#[test]
fn handle_pending_core_trap_schedules_hart_start_completion() {
    let (mut scheduler, transport, firmware, core0, core1) = registered_hsm_pair();
    execute_hsm_hart_start_ecall(&mut scheduler, &transport, &core0);

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_START_PENDING)
    );
    assert_eq!(core1.pc(), Address::new(0x8800));

    scheduler.run_until_idle_conservative();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
    assert_eq!(core1.pc(), Address::new(0x9000));
    assert_eq!(core1.read_register(register(10)), 1);
    assert_eq!(core1.read_register(register(11)), 0x55);
}

fn assert_hart_stop_pending_until_event_runs(parallel: bool) {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_STOP,
        44,
        55,
        0,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, parallel)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::Stopped);
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_STOP_PENDING)
    );
    assert_eq!(core0.read_register(register(10)), 44);
    assert_eq!(core0.read_register(register(11)), 55);

    if parallel {
        scheduler
            .run_until_idle_parallel()
            .expect("parallel stop event");
    } else {
        scheduler.run_until_idle_conservative();
    }

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STOPPED)
    );
    assert_eq!(core0.read_register(register(31)), 0);
}

#[test]
fn handle_pending_core_trap_reports_hart_stop_pending_until_event_runs() {
    assert_hart_stop_pending_until_event_runs(false);
}

#[test]
fn parallel_handle_pending_core_trap_reports_hart_stop_pending_until_event_runs() {
    assert_hart_stop_pending_until_event_runs(true);
}

#[test]
fn scheduled_hart_stop_does_not_complete_after_state_changes() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_STOP,
        44,
        55,
        0,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::Stopped);
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_STOP_PENDING)
    );
    core0.set_hart_started();
    scheduler.run_until_idle_conservative();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
}

fn assert_retentive_suspend_pending_until_event_runs(parallel: bool) {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_SUSPEND,
        SBI_HSM_DEFAULT_RETENTIVE_SUSPEND,
        0x9000,
        0x55,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, parallel)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_SUSPEND_PENDING)
    );
    assert_eq!(
        core0.read_register(register(10)),
        SBI_HSM_DEFAULT_RETENTIVE_SUSPEND
    );
    assert_eq!(core0.read_register(register(11)), 0x9000);

    if parallel {
        scheduler
            .run_until_idle_parallel()
            .expect("parallel suspend event");
    } else {
        scheduler.run_until_idle_conservative();
    }

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_SUSPENDED)
    );
    assert_eq!(core0.read_register(register(31)), 0);
}

#[test]
fn handle_pending_core_trap_reports_retentive_suspend_pending_until_event_runs() {
    assert_retentive_suspend_pending_until_event_runs(false);
}

#[test]
fn parallel_handle_pending_core_trap_reports_retentive_suspend_pending_until_event_runs() {
    assert_retentive_suspend_pending_until_event_runs(true);
}

#[test]
fn scheduled_hart_suspend_does_not_complete_after_state_changes() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_SUSPEND,
        SBI_HSM_DEFAULT_RETENTIVE_SUSPEND,
        0x9000,
        0x55,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_SUSPEND_PENDING)
    );
    core0.set_hart_started();
    scheduler.run_until_idle_conservative();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
}

fn assert_nonretentive_resume_pending_until_resume_event_runs(parallel: bool) {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_SUSPEND,
        SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND,
        0x9000,
        0x55,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, parallel)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::Resumed);
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_RESUME_PENDING)
    );
    assert_ne!(core0.pc(), Address::new(0x9000));

    if parallel {
        scheduler
            .run_until_idle_parallel()
            .expect("parallel non-retentive resume event");
    } else {
        scheduler.run_until_idle_conservative();
    }

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
    assert_eq!(core0.pc(), Address::new(0x9000));
    assert_eq!(core0.read_register(register(10)), 0);
    assert_eq!(core0.read_register(register(11)), 0x55);
}

#[test]
fn handle_pending_core_trap_reports_nonretentive_resume_pending_until_resume_event_runs() {
    assert_nonretentive_resume_pending_until_resume_event_runs(false);
}

#[test]
fn parallel_handle_pending_core_trap_reports_nonretentive_resume_pending_until_resume_event_runs() {
    assert_nonretentive_resume_pending_until_resume_event_runs(true);
}

#[test]
fn scheduled_nonretentive_resume_does_not_complete_after_state_changes() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_SUSPEND,
        SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND,
        0x9000,
        0x55,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::Resumed);
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_RESUME_PENDING)
    );
    core0.set_hart_started();
    scheduler.run_until_idle_conservative();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
    assert_ne!(core0.pc(), Address::new(0x9000));
}

#[test]
fn nonretentive_suspend_scheduler_error_leaves_no_hsm_record() {
    let near_end = u64::MAX - 1;
    let mut scheduler =
        PartitionedScheduler::with_min_remote_delay(1, 2).expect("valid test scheduler");
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(0),
                2,
                3,
            )
            .expect("valid CPU route"),
        )
        .expect("registered CPU route");
    let core0 = test_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone()]).expect("valid cluster");
    let firmware = RiscvSbiFirmware::new();
    firmware
        .register_cluster(&cluster)
        .expect("cluster registers with SBI firmware");
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_SUSPEND,
        SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND,
        0x9000,
        0x55,
    );
    scheduler
        .restore_quiescent(&SchedulerSnapshot::new(
            near_end,
            2,
            vec![PartitionSnapshot::quiescent(
                PartitionId::new(0),
                near_end,
                0,
                0,
            )],
        ))
        .expect("restore near-overflow scheduler");

    let error = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect_err("tick overflow rejects non-retentive resume scheduling");

    assert_eq!(
        error,
        SystemError::Scheduler(SchedulerError::TickOverflow {
            now: near_end,
            delay: 2,
        })
    );
    assert!(firmware.hsm_records().is_empty());
}

#[test]
fn pending_interrupt_wakes_retentive_suspend_before_suspend_event_completes() {
    let (mut scheduler, transport, firmware, core0, _core1) = registered_hsm_pair();
    execute_hsm_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_HSM_HART_SUSPEND,
        SBI_HSM_DEFAULT_RETENTIVE_SUSPEND,
        0x9000,
        0x55,
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(TEST_HART_SUSPEND_PENDING)
    );
    core0.set_machine_interrupt_pending_bits(SSIP);
    scheduler.run_until_idle_conservative();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 0, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STARTED)
    );
    assert_eq!(core0.machine_interrupt_pending() & SSIP, SSIP);
}

#[test]
fn scheduled_hart_start_does_not_complete_after_state_changes() {
    let (mut scheduler, transport, firmware, core0, core1) = registered_hsm_pair();
    execute_hsm_hart_start_ecall(&mut scheduler, &transport, &core0);

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    core1.set_hart_stopped();
    scheduler.run_until_idle_conservative();

    assert_eq!(
        firmware.hart_get_status(hsm_request(SBI_HSM_HART_GET_STATUS, 1, 0, 0)),
        RiscvSbiOutcome::success(SBI_HSM_HART_STOPPED)
    );
    assert_eq!(core1.pc(), Address::new(0x8800));
}

#[test]
fn debug_console_clears_when_cluster_registers_for_next_run() {
    let (mut scheduler, transport, firmware, core0, core1) = registered_hsm_pair();
    execute_sbi_ecall(
        &mut scheduler,
        &transport,
        &core0,
        SBI_DEBUG_CONSOLE_EXTENSION,
        SBI_DEBUG_CONSOLE_WRITE_BYTE,
        [u64::from(b'R'), 0, 0, 0, 0],
    );

    let outcome = firmware
        .handle_pending_core_trap(&mut scheduler, &core0, false)
        .expect("handled SBI trap")
        .expect("SBI outcome");

    assert_eq!(outcome, RiscvSbiOutcome::success(0));
    assert_eq!(firmware.debug_console_bytes(), b"R");

    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).expect("valid cluster");
    firmware
        .register_cluster(&cluster)
        .expect("cluster registers with SBI firmware");

    assert!(firmware.debug_console_bytes().is_empty());
}
