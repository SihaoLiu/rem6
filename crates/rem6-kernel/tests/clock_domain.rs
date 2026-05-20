use rem6_kernel::{ClockDomain, ClockError, Cycles};

#[test]
fn clock_domain_converts_cycles_to_ticks_with_component_periods() {
    let cpu = ClockDomain::new(3).unwrap();
    let accelerator = ClockDomain::new(5).unwrap();

    assert_eq!(cpu.period(), 3);
    assert_eq!(cpu.cycles_to_ticks(Cycles::new(4)).unwrap(), 12);
    assert_eq!(accelerator.cycles_to_ticks(Cycles::new(4)).unwrap(), 20);
}

#[test]
fn clock_domain_rounds_ticks_up_to_whole_cycles() {
    let domain = ClockDomain::new(4).unwrap();

    assert_eq!(domain.ticks_to_cycles_ceil(0), Cycles::new(0));
    assert_eq!(domain.ticks_to_cycles_ceil(1), Cycles::new(1));
    assert_eq!(domain.ticks_to_cycles_ceil(4), Cycles::new(1));
    assert_eq!(domain.ticks_to_cycles_ceil(5), Cycles::new(2));
}

#[test]
fn clock_domain_finds_current_or_future_clock_edges() {
    let domain = ClockDomain::new(5).unwrap();

    assert_eq!(domain.clock_edge(10, Cycles::new(0)).unwrap(), 10);
    assert_eq!(domain.clock_edge(11, Cycles::new(0)).unwrap(), 15);
    assert_eq!(domain.clock_edge(11, Cycles::new(2)).unwrap(), 25);
}

#[test]
fn clock_domain_rejects_zero_period() {
    let error = ClockDomain::new(0).unwrap_err();

    assert_eq!(error, ClockError::ZeroPeriod);
}

#[test]
fn clock_domain_reports_tick_overflow() {
    let domain = ClockDomain::new(u64::MAX).unwrap();

    let error = domain.cycles_to_ticks(Cycles::new(2)).unwrap_err();

    assert_eq!(
        error,
        ClockError::TickOverflow {
            period: u64::MAX,
            cycles: Cycles::new(2)
        }
    );
}
