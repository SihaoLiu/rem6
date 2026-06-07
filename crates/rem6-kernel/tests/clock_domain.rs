use rem6_kernel::{ClockDomain, ClockDomainId, ClockError, Cycles, SourceClockDomain};

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

#[test]
fn source_clock_domain_switches_between_valid_performance_points() {
    let mut domain = SourceClockDomain::new(Some(ClockDomainId::new(7)), vec![2, 4, 8], 1).unwrap();

    assert_eq!(domain.domain_id(), Some(ClockDomainId::new(7)));
    assert_eq!(domain.performance_level(), 1);
    assert_eq!(domain.performance_point_count(), 3);
    assert_eq!(domain.period(), 4);
    assert_eq!(domain.clock_domain(), ClockDomain::new(4).unwrap());
    assert_eq!(domain.period_at_performance_level(0).unwrap(), 2);
    assert_eq!(domain.frequency_hz(1_000).unwrap(), 250);

    assert!(!domain.set_performance_level(1).unwrap());
    assert!(domain.set_performance_level(0).unwrap());
    assert_eq!(domain.performance_level(), 0);
    assert_eq!(domain.period(), 2);
    assert_eq!(
        domain
            .clock_domain()
            .cycles_to_ticks(Cycles::new(3))
            .unwrap(),
        6
    );
}

#[test]
fn source_clock_domain_rejects_invalid_performance_tables() {
    assert_eq!(
        SourceClockDomain::new(None, Vec::new(), 0).unwrap_err(),
        ClockError::EmptyPerformancePoints
    );
    assert_eq!(
        SourceClockDomain::new(None, vec![2, 0], 0).unwrap_err(),
        ClockError::ZeroPeriod
    );
    assert_eq!(
        SourceClockDomain::new(None, vec![4, 2], 0).unwrap_err(),
        ClockError::UnsortedPerformancePoints
    );
    assert_eq!(
        SourceClockDomain::new(None, vec![2, 4], 2).unwrap_err(),
        ClockError::InvalidPerformanceLevel { level: 2, count: 2 }
    );
}

#[test]
fn clock_domain_derives_divided_child_domains() {
    let source = SourceClockDomain::new(None, vec![3, 6], 0).unwrap();

    let derived = source.derived_clock_domain(4).unwrap();

    assert_eq!(derived.period(), 12);
    assert_eq!(derived.cycles_to_ticks(Cycles::new(2)).unwrap(), 24);
    assert_eq!(
        source.derived_clock_domain(0).unwrap_err(),
        ClockError::ZeroClockDivider
    );
    assert_eq!(
        ClockDomain::new(u64::MAX).unwrap().derived(2).unwrap_err(),
        ClockError::DerivedClockOverflow {
            period: u64::MAX,
            divider: 2
        }
    );
}
