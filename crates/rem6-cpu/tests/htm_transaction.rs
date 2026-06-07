use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, HtmFailureCause,
    HtmTransactionError, HtmTransactionState, HtmTransactionUid, RiscvCluster,
    RiscvClusterHtmAbortOutcome, RiscvCore,
};
use rem6_isa_riscv::Register;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn core(entry: u64) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                MemoryRouteId::new(0),
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn data_core(cpu: u32, fetch_route: u64, data_route: u64, entry: u64) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(cpu),
            AgentId::new(cpu + 7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint(&format!("cpu{cpu}.ifetch")),
            MemoryRouteId::new(fetch_route),
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    RiscvCore::with_data(
        core,
        CpuDataConfig::new(
            endpoint(&format!("cpu{cpu}.dmem")),
            MemoryRouteId::new(data_route),
            layout(),
        ),
    )
}

#[test]
fn htm_transaction_abort_restores_checkpoint_and_clears_active_state() {
    let mut state = HtmTransactionState::new();
    let checkpoint = vec![0x10, 0x20, 0x30, 0x40];

    let begin = state.begin(checkpoint.clone()).unwrap();
    assert_eq!(begin.uid(), HtmTransactionUid::new(1));
    assert_eq!(begin.depth(), 1);
    assert!(state.in_transaction());
    assert_eq!(state.active_uid(), Some(HtmTransactionUid::new(1)));
    assert_eq!(state.active_depth(), Some(1));

    let abort = state
        .abort(HtmTransactionUid::new(1), HtmFailureCause::Explicit)
        .unwrap();

    assert_eq!(abort.uid(), HtmTransactionUid::new(1));
    assert_eq!(abort.cause(), HtmFailureCause::Explicit);
    assert_eq!(abort.restored_checkpoint(), checkpoint.as_slice());
    assert!(!state.in_transaction());
    assert_eq!(state.active_uid(), None);
    assert_eq!(state.active_depth(), None);
    assert_eq!(state.last_abort(), Some(&abort));
}

#[test]
fn htm_transaction_commit_clears_checkpoint_without_abort_record() {
    let mut state = HtmTransactionState::new();
    let begin = state.begin(vec![0xaa, 0xbb]).unwrap();

    let commit = state.commit(begin.uid()).unwrap();

    assert_eq!(commit.uid(), begin.uid());
    assert_eq!(commit.depth(), 0);
    assert!(!state.in_transaction());
    assert_eq!(state.last_abort(), None);
}

#[test]
fn htm_transaction_rejects_wrong_uid_without_losing_checkpoint() {
    let mut state = HtmTransactionState::new();
    let checkpoint = vec![0xde, 0xad, 0xbe, 0xef];
    state.begin(checkpoint.clone()).unwrap();

    let error = state
        .abort(HtmTransactionUid::new(9), HtmFailureCause::Memory)
        .unwrap_err();

    assert_eq!(
        error,
        HtmTransactionError::TransactionUidMismatch {
            expected: HtmTransactionUid::new(1),
            actual: HtmTransactionUid::new(9),
        }
    );
    assert!(state.in_transaction());
    let abort = state
        .abort(HtmTransactionUid::new(1), HtmFailureCause::Memory)
        .unwrap();
    assert_eq!(abort.restored_checkpoint(), checkpoint.as_slice());
}

#[test]
fn htm_transaction_snapshot_restore_preserves_active_checkpoint_and_next_uid() {
    let mut state = HtmTransactionState::new();
    let begin = state.begin(vec![1, 2, 3]).unwrap();
    let snapshot = state.snapshot();

    let mut restored = HtmTransactionState::restore(snapshot).unwrap();
    let abort = restored
        .abort(begin.uid(), HtmFailureCause::Exception)
        .unwrap();
    assert_eq!(abort.restored_checkpoint(), &[1, 2, 3]);
    assert_eq!(abort.cause(), HtmFailureCause::Exception);

    let next = restored.begin(vec![4, 5, 6]).unwrap();
    assert_eq!(next.uid(), HtmTransactionUid::new(2));
}

#[test]
fn riscv_core_htm_abort_restores_architectural_checkpoint() {
    let core = core(0x8000);
    core.write_register(reg(5), 0x1111);
    core.redirect_pc(Address::new(0x8010));

    let begin = core.begin_htm_transaction().unwrap();
    core.write_register(reg(5), 0x2222);
    core.redirect_pc(Address::new(0x9000));

    let abort = core
        .abort_htm_transaction(begin.uid(), HtmFailureCause::Memory)
        .unwrap();

    assert_eq!(abort.uid(), begin.uid());
    assert_eq!(abort.cause(), HtmFailureCause::Memory);
    assert_eq!(core.read_register(reg(5)), 0x1111);
    assert_eq!(core.pc(), Address::new(0x8010));
    assert!(!core.in_htm_transaction());
}

#[test]
fn riscv_core_htm_commit_keeps_architectural_updates() {
    let core = core(0x8000);
    core.write_register(reg(6), 0x11);
    let begin = core.begin_htm_transaction().unwrap();
    core.write_register(reg(6), 0x22);
    core.redirect_pc(Address::new(0x8040));

    let commit = core.commit_htm_transaction(begin.uid()).unwrap();

    assert_eq!(commit.depth(), 0);
    assert_eq!(core.read_register(reg(6)), 0x22);
    assert_eq!(core.pc(), Address::new(0x8040));
    assert!(!core.in_htm_transaction());
}

#[test]
fn riscv_core_nested_htm_abort_uses_outer_checkpoint() {
    let core = core(0x8000);
    core.write_register(reg(7), 0x100);

    let outer = core.begin_htm_transaction().unwrap();
    core.write_register(reg(7), 0x200);
    let inner = core.begin_htm_transaction().unwrap();
    core.write_register(reg(7), 0x300);

    assert_eq!(inner.uid(), outer.uid());
    assert_eq!(inner.depth(), 2);
    assert_eq!(core.commit_htm_transaction(inner.uid()).unwrap().depth(), 1);
    assert!(core.in_htm_transaction());

    core.abort_htm_transaction(outer.uid(), HtmFailureCause::Explicit)
        .unwrap();

    assert_eq!(core.read_register(reg(7)), 0x100);
    assert!(!core.in_htm_transaction());
}

#[test]
fn riscv_cluster_htm_abort_by_data_route_restores_matching_core_checkpoint() {
    let core0 = data_core(0, 0, 10, 0x8000);
    let core1 = data_core(1, 1, 11, 0x9000);
    core0.write_register(reg(8), 0x1111);
    let begin = core0.begin_htm_transaction().unwrap();
    core0.write_register(reg(8), 0x2222);

    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let outcome = cluster
        .abort_htm_transaction_for_data_route(MemoryRouteId::new(10), HtmFailureCause::Memory);

    assert!(matches!(
        outcome,
        RiscvClusterHtmAbortOutcome::Aborted { cpu, route, abort }
            if cpu == CpuId::new(0)
                && route == MemoryRouteId::new(10)
                && abort.uid() == begin.uid()
                && abort.cause() == HtmFailureCause::Memory
    ));
    assert_eq!(core0.read_register(reg(8)), 0x1111);
    assert!(!core0.in_htm_transaction());
    assert_eq!(core1.read_register(reg(8)), 0);
}

#[test]
fn riscv_cluster_htm_abort_by_data_route_reports_no_active_transaction() {
    let core = data_core(0, 0, 10, 0x8000);
    let cluster = RiscvCluster::new([core]).unwrap();

    assert_eq!(
        cluster.abort_htm_transaction_for_data_route(
            MemoryRouteId::new(10),
            HtmFailureCause::Explicit,
        ),
        RiscvClusterHtmAbortOutcome::NoActiveTransaction {
            cpu: CpuId::new(0),
            route: MemoryRouteId::new(10),
        }
    );
}
