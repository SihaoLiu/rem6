use std::sync::{Arc, Mutex};

use rem6_kernel::{ClockDomain, ClockError, ClockScheduleError, Cycles, EventQueue, ScheduleError};

#[test]
fn event_queue_runs_events_by_tick_then_insertion_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut queue = EventQueue::new();

    let first_at_ten = Arc::clone(&observed);
    queue
        .schedule_at(10, move |tick| {
            first_at_ten.lock().unwrap().push((tick, "first_at_ten"));
        })
        .unwrap();

    let early = Arc::clone(&observed);
    queue
        .schedule_at(5, move |tick| {
            early.lock().unwrap().push((tick, "early"));
        })
        .unwrap();

    let second_at_ten = Arc::clone(&observed);
    queue
        .schedule_at(10, move |tick| {
            second_at_ten.lock().unwrap().push((tick, "second_at_ten"));
        })
        .unwrap();

    queue.run_until_empty();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(5, "early"), (10, "first_at_ten"), (10, "second_at_ten"),]
    );
    assert_eq!(queue.now(), 10);
    assert!(queue.is_empty());
}

#[test]
fn event_queue_rejects_events_scheduled_before_current_tick() {
    let mut queue = EventQueue::new();
    queue.schedule_at(7, |_| {}).unwrap();
    queue.run_until_empty();

    let error = queue.schedule_at(6, |_| {}).unwrap_err();

    assert_eq!(
        error,
        ScheduleError::InThePast {
            now: 7,
            requested: 6
        }
    );
}

#[test]
fn event_queue_schedules_relative_to_current_tick() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut queue = EventQueue::new();
    queue.schedule_at(4, |_| {}).unwrap();
    queue.run_until_empty();

    let relative = Arc::clone(&observed);
    queue
        .schedule_after(3, move |tick| {
            relative.lock().unwrap().push(tick);
        })
        .unwrap();
    queue.run_until_empty();

    assert_eq!(observed.lock().unwrap().as_slice(), &[7]);
    assert_eq!(queue.now(), 7);
}

#[test]
fn event_queue_rejects_relative_delay_overflow_without_mutating_queue() {
    let mut queue = EventQueue::new();
    queue.schedule_at(u64::MAX - 1, |_| {}).unwrap();
    queue.run_until_empty();

    let error = queue.schedule_after(2, |_| {}).unwrap_err();

    assert_eq!(
        error,
        ScheduleError::TickOverflow {
            now: u64::MAX - 1,
            delay: 2,
        }
    );
    assert_eq!(queue.now(), u64::MAX - 1);
    assert!(queue.is_empty());
}

#[test]
fn event_queue_schedules_clock_domain_deadlines() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut queue = EventQueue::new();
    let cpu = ClockDomain::new(2).unwrap();
    let accelerator = ClockDomain::new(5).unwrap();

    let cpu_observed = Arc::clone(&observed);
    queue
        .schedule_at_clock_edge(cpu, Cycles::new(3), move |tick| {
            cpu_observed.lock().unwrap().push((tick, "cpu"));
        })
        .unwrap();

    let accelerator_observed = Arc::clone(&observed);
    queue
        .schedule_at_clock_edge(accelerator, Cycles::new(1), move |tick| {
            accelerator_observed
                .lock()
                .unwrap()
                .push((tick, "accelerator"));
        })
        .unwrap();

    queue.run_until_empty();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(5, "accelerator"), (6, "cpu")]
    );
}

#[test]
fn event_queue_reports_clock_domain_deadline_errors() {
    let mut queue = EventQueue::new();
    let domain = ClockDomain::new(u64::MAX).unwrap();

    let error = queue
        .schedule_at_clock_edge(domain, Cycles::new(2), |_| {})
        .unwrap_err();

    assert_eq!(
        error,
        ClockScheduleError::Clock(ClockError::TickOverflow {
            period: u64::MAX,
            cycles: Cycles::new(2)
        })
    );
}
