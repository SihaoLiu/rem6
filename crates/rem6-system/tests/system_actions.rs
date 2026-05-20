use rem6_kernel::PartitionId;
use rem6_stats::{StatSample, StatSnapshot, StatsRegistry, StatsResetRecord};
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, StopRequest, SystemActionExecutor,
    SystemActionOutcome,
};

#[test]
fn system_action_executor_applies_stats_reset_and_dump_actions() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(4);
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    stats.increment(insts, 9).unwrap();
    let mut executor = SystemActionExecutor::new(stats);

    let reset = HostActionRecord::new(
        10,
        guest,
        host,
        GuestEventId::new(1),
        source,
        HostAction::ResetStats,
    );
    assert_eq!(
        executor.apply(&reset).unwrap(),
        SystemActionOutcome::StatsReset(StatsResetRecord::new(10, 1, vec![(insts, 9)]))
    );

    executor.stats_mut().increment(insts, 3).unwrap();

    let dump = HostActionRecord::new(
        14,
        guest,
        host,
        GuestEventId::new(2),
        source,
        HostAction::DumpStats,
    );
    assert_eq!(
        executor.apply(&dump).unwrap(),
        SystemActionOutcome::StatsSnapshot(StatSnapshot::new(
            14,
            1,
            10,
            vec![StatSample::new(insts, "cpu0.committed_insts", "count", 3)],
        ))
    );
}

#[test]
fn system_action_executor_records_non_stats_control_outcomes() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(8);
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());

    let command = HostActionRecord::new(
        20,
        guest,
        host,
        GuestEventId::new(3),
        source,
        HostAction::InjectCommand {
            command: "dump-device-tree".to_string(),
        },
    );
    assert_eq!(
        executor.apply(&command).unwrap(),
        SystemActionOutcome::InjectedCommand {
            tick: 20,
            event: GuestEventId::new(3),
            source,
            command: "dump-device-tree".to_string(),
        }
    );

    let checkpoint = HostActionRecord::new(
        24,
        guest,
        host,
        GuestEventId::new(4),
        source,
        HostAction::Checkpoint {
            label: "after-boot".to_string(),
        },
    );
    assert_eq!(
        executor.apply(&checkpoint).unwrap(),
        SystemActionOutcome::Checkpoint {
            tick: 24,
            event: GuestEventId::new(4),
            source,
            label: "after-boot".to_string(),
        }
    );

    let stop = HostActionRecord::new(
        30,
        guest,
        host,
        GuestEventId::new(5),
        source,
        HostAction::Stop { code: 0 },
    );
    assert_eq!(
        executor.apply(&stop).unwrap(),
        SystemActionOutcome::Stop(StopRequest::new(30, GuestEventId::new(5), source, 0))
    );
}
