use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_system::{
    GuestEvent, GuestEventChannel, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId,
    HostAction, HostActionRecord, HostEventPolicy, StopRequest, SystemError, SystemRunController,
};

#[test]
fn guest_events_route_from_guest_to_host_partition() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let channel = GuestEventChannel::new(host, 4).unwrap();
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let first_log = Arc::clone(&delivered);
    let second_log = Arc::clone(&delivered);
    scheduler
        .schedule_at(guest, 3, move |context| {
            channel
                .emit(
                    context,
                    GuestEvent::new(
                        GuestEventId::new(10),
                        GuestSourceId::new(7),
                        GuestEventKind::RoiBegin,
                    ),
                    move |delivery| first_log.lock().unwrap().push(delivery),
                )
                .unwrap();
            channel
                .emit(
                    context,
                    GuestEvent::new(
                        GuestEventId::new(11),
                        GuestSourceId::new(7),
                        GuestEventKind::Terminate { code: 0 },
                    ),
                    move |delivery| second_log.lock().unwrap().push(delivery),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 7);
    assert_eq!(
        delivered.lock().unwrap().as_slice(),
        &[
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(10),
                    GuestSourceId::new(7),
                    GuestEventKind::RoiBegin,
                ),
            ),
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(11),
                    GuestSourceId::new(7),
                    GuestEventKind::Terminate { code: 0 },
                ),
            ),
        ]
    );
}

#[test]
fn guest_event_channel_rejects_zero_host_latency() {
    assert_eq!(
        GuestEventChannel::new(PartitionId::new(1), 0).unwrap_err(),
        SystemError::ZeroHostLatency
    );
}

#[test]
fn host_event_policy_maps_structured_events_to_actions() {
    let policy = HostEventPolicy;

    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(1),
            GuestSourceId::new(3),
            GuestEventKind::RoiBegin,
        )),
        vec![HostAction::ResetStats]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(2),
            GuestSourceId::new(3),
            GuestEventKind::RoiEnd,
        )),
        vec![HostAction::DumpStats]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(3),
            GuestSourceId::new(3),
            GuestEventKind::Checkpoint {
                label: "warm-boot".to_string(),
            },
        )),
        vec![HostAction::Checkpoint {
            label: "warm-boot".to_string(),
        }]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(4),
            GuestSourceId::new(3),
            GuestEventKind::Terminate { code: 12 },
        )),
        vec![HostAction::Stop { code: 12 }]
    );
}

#[test]
fn system_run_controller_records_actions_and_stop_request() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(7);
    let mut controller = SystemRunController::new(HostEventPolicy);

    let roi_records = controller.handle_delivery(GuestEventDelivery::new(
        9,
        guest,
        host,
        GuestEvent::new(GuestEventId::new(30), source, GuestEventKind::RoiBegin),
    ));
    assert_eq!(
        roi_records,
        vec![HostActionRecord::new(
            9,
            guest,
            host,
            GuestEventId::new(30),
            source,
            HostAction::ResetStats,
        )]
    );
    assert_eq!(controller.stop_request(), None);

    let stop_records = controller.handle_delivery(GuestEventDelivery::new(
        12,
        guest,
        host,
        GuestEvent::new(
            GuestEventId::new(31),
            source,
            GuestEventKind::Terminate { code: 5 },
        ),
    ));
    assert_eq!(
        stop_records,
        vec![HostActionRecord::new(
            12,
            guest,
            host,
            GuestEventId::new(31),
            source,
            HostAction::Stop { code: 5 },
        )]
    );
    assert_eq!(
        controller.stop_request(),
        Some(&StopRequest::new(12, GuestEventId::new(31), source, 5))
    );
    assert_eq!(controller.deliveries().len(), 2);
    assert_eq!(
        controller.action_records(),
        &[
            HostActionRecord::new(
                9,
                guest,
                host,
                GuestEventId::new(30),
                source,
                HostAction::ResetStats,
            ),
            HostActionRecord::new(
                12,
                guest,
                host,
                GuestEventId::new(31),
                source,
                HostAction::Stop { code: 5 },
            ),
        ]
    );
}
