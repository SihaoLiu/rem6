use rem6_system::{
    GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestSignal, GuestWaitOptions,
    GuestWaitOutcome, GuestWaitQueue, GuestWaitSelector, GuestWaitStatus, GuestWaitStatusError,
};

#[test]
fn guest_wait_status_encodes_normal_exit_for_wait4() {
    let status = GuestWaitStatus::exited(42);

    assert_eq!(status.raw_wait_status(), 42 << 8);
    assert!(status.is_exited());
    assert_eq!(status.exit_code(), Some(42));
    assert!(!status.is_signaled());
    assert_eq!(status.terminating_signal(), None);
}

#[test]
fn guest_wait_status_encodes_signal_exit_without_losing_signal_number() {
    let status = GuestWaitStatus::signaled(GuestSignal::new(6).unwrap(), false);

    assert_eq!(status.raw_wait_status(), 6);
    assert!(status.is_signaled());
    assert_eq!(status.terminating_signal().unwrap().number(), 6);
    assert!(!status.core_dumped());
    assert_eq!(status.exit_code(), None);

    let core_dumped = GuestWaitStatus::signaled(GuestSignal::new(6).unwrap(), true);
    assert_eq!(core_dumped.raw_wait_status(), 0x86);
    assert!(core_dumped.core_dumped());
}

#[test]
fn guest_wait_status_encodes_stopped_and_continued_children() {
    let stopped = GuestWaitStatus::stopped(GuestSignal::new(19).unwrap());
    assert_eq!(stopped.raw_wait_status(), (19 << 8) | 0x7f);
    assert!(stopped.is_stopped());
    assert_eq!(stopped.stop_signal().unwrap().number(), 19);

    let continued = GuestWaitStatus::continued();
    assert_eq!(continued.raw_wait_status(), 0xffff);
    assert!(continued.is_continued());
}

#[test]
fn guest_wait_status_rejects_invalid_guest_signals() {
    assert_eq!(
        GuestSignal::new(0).unwrap_err(),
        GuestWaitStatusError::InvalidSignal { signal: 0 }
    );
    assert_eq!(
        GuestSignal::new(128).unwrap_err(),
        GuestWaitStatusError::InvalidSignal { signal: 128 }
    );
}

#[test]
fn guest_wait_selectors_reject_invalid_guest_ids() {
    assert_eq!(
        GuestProcessId::new(0).unwrap_err(),
        GuestWaitStatusError::InvalidProcessId { pid: 0 }
    );
    assert_eq!(
        GuestProcessGroupId::new(0).unwrap_err(),
        GuestWaitStatusError::InvalidProcessGroupId { pgid: 0 }
    );
    assert_eq!(
        GuestWaitSelector::from_wait4_pid(i32::MIN).unwrap_err(),
        GuestWaitStatusError::InvalidWaitPid { pid: i32::MIN }
    );
}

#[test]
fn guest_wait_queue_selects_exact_child_and_consumes_it() {
    let current_group = GuestProcessGroupId::new(10).unwrap();
    let other_group = GuestProcessGroupId::new(11).unwrap();
    let first = GuestChildStatus::new(
        GuestProcessId::new(100).unwrap(),
        other_group,
        GuestWaitStatus::exited(1),
    );
    let selected = GuestChildStatus::new(
        GuestProcessId::new(101).unwrap(),
        current_group,
        GuestWaitStatus::signaled(GuestSignal::new(6).unwrap(), false),
    );
    let mut queue = GuestWaitQueue::new(current_group);
    queue.push(first);
    queue.push(selected);

    assert_eq!(
        queue.wait(
            GuestWaitSelector::Process(selected.pid()),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(selected)
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(selected.status().raw_wait_status(), 6);
    assert_eq!(
        queue.wait(GuestWaitSelector::AnyChild, GuestWaitOptions::blocking()),
        GuestWaitOutcome::Ready(first)
    );
    assert!(queue.is_empty());
}

#[test]
fn guest_wait_queue_applies_wait4_process_group_and_any_child_rules() {
    let current_group = GuestProcessGroupId::new(20).unwrap();
    let other_group = GuestProcessGroupId::new(21).unwrap();
    let other_first = GuestChildStatus::new(
        GuestProcessId::new(200).unwrap(),
        other_group,
        GuestWaitStatus::exited(2),
    );
    let current_child = GuestChildStatus::new(
        GuestProcessId::new(201).unwrap(),
        current_group,
        GuestWaitStatus::exited(3),
    );
    let other_second = GuestChildStatus::new(
        GuestProcessId::new(202).unwrap(),
        other_group,
        GuestWaitStatus::exited(4),
    );
    let mut queue = GuestWaitQueue::new(current_group);
    queue.push(other_first);
    queue.push(current_child);
    queue.push(other_second);

    assert_eq!(
        queue.wait(
            GuestWaitSelector::from_wait4_pid(0).unwrap(),
            GuestWaitOptions::nonblocking()
        ),
        GuestWaitOutcome::Ready(current_child)
    );
    assert_eq!(
        queue.wait(
            GuestWaitSelector::from_wait4_pid(-21).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(other_first)
    );
    assert_eq!(
        queue.wait(
            GuestWaitSelector::from_wait4_pid(-1).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(other_second)
    );
    assert!(queue.is_empty());
}

#[test]
fn guest_wait_queue_reports_no_ready_or_retry_without_consuming_children() {
    let current_group = GuestProcessGroupId::new(30).unwrap();
    let child = GuestChildStatus::new(
        GuestProcessId::new(300).unwrap(),
        current_group,
        GuestWaitStatus::exited(5),
    );
    let mut queue = GuestWaitQueue::new(current_group);
    queue.push(child);

    assert_eq!(
        queue.wait(
            GuestWaitSelector::from_wait4_pid(999).unwrap(),
            GuestWaitOptions::nonblocking()
        ),
        GuestWaitOutcome::NoReady
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue.wait(
            GuestWaitSelector::from_wait4_pid(999).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Retry
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue.wait(GuestWaitSelector::AnyChild, GuestWaitOptions::blocking()),
        GuestWaitOutcome::Ready(child)
    );
}

#[test]
fn guest_wait_queue_snapshot_restore_preserves_selector_behavior() {
    let current_group = GuestProcessGroupId::new(40).unwrap();
    let other_group = GuestProcessGroupId::new(41).unwrap();
    let first = GuestChildStatus::new(
        GuestProcessId::new(400).unwrap(),
        other_group,
        GuestWaitStatus::exited(7),
    );
    let selected = GuestChildStatus::new(
        GuestProcessId::new(401).unwrap(),
        current_group,
        GuestWaitStatus::signaled(GuestSignal::new(11).unwrap(), true),
    );
    let mut queue = GuestWaitQueue::new(current_group);
    queue.push(first);
    queue.push(selected);
    let snapshot = queue.snapshot();

    let mut restored = GuestWaitQueue::from_snapshot(snapshot.clone());

    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.wait(
            GuestWaitSelector::from_wait4_pid(0).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(selected)
    );
    assert_eq!(
        restored.wait(
            GuestWaitSelector::from_wait4_pid(-41).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(first)
    );
    assert!(restored.is_empty());
}
