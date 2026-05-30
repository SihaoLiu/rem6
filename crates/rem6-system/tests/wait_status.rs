use rem6_system::{GuestSignal, GuestWaitStatus, GuestWaitStatusError};

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
