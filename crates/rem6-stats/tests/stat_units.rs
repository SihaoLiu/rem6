use rem6_stats::{StatUnit, StatUnitKind};

#[test]
fn stat_units_match_gem5_builtin_spellings() {
    let units = [
        (StatUnit::cycle(), "Cycle", StatUnitKind::Cycle),
        (StatUnit::tick(), "Tick", StatUnitKind::Tick),
        (StatUnit::second(), "Second", StatUnitKind::Second),
        (StatUnit::bit(), "Bit", StatUnitKind::Bit),
        (StatUnit::byte(), "Byte", StatUnitKind::Byte),
        (StatUnit::watt(), "Watt", StatUnitKind::Watt),
        (StatUnit::joule(), "Joule", StatUnitKind::Joule),
        (StatUnit::volt(), "Volt", StatUnitKind::Volt),
        (StatUnit::degree_celsius(), "Celsius", StatUnitKind::Celsius),
        (StatUnit::count(), "Count", StatUnitKind::Count),
        (StatUnit::ratio(), "Ratio", StatUnitKind::Ratio),
        (
            StatUnit::unspecified(),
            "Unspecified",
            StatUnitKind::Unspecified,
        ),
    ];

    for (unit, spelling, kind) in units {
        assert_eq!(unit.as_str(), spelling);
        assert_eq!(unit.kind(), &kind);
        assert_eq!(StatUnit::parse(spelling).unwrap(), unit);
    }
}

#[test]
fn stat_units_preserve_gem5_nested_rate_spelling() {
    assert_eq!(
        StatUnit::rate(StatUnit::count(), StatUnit::count()).as_str(),
        "(Count/Count)",
    );
    assert_eq!(
        StatUnit::rate(StatUnit::tick(), StatUnit::second()).as_str(),
        "(Tick/Second)",
    );

    let unit = StatUnit::rate(
        StatUnit::rate(StatUnit::bit(), StatUnit::second()),
        StatUnit::rate(StatUnit::count(), StatUnit::cycle()),
    );

    assert_eq!(unit.as_str(), "((Bit/Second)/(Count/Cycle))");
    assert_eq!(StatUnit::parse(unit.as_str()).unwrap(), unit);
}

#[test]
fn stat_units_keep_celsius_alias_compatible() {
    let celsius = StatUnit::celsius();

    assert_eq!(celsius.as_str(), "Celsius");
    assert_eq!(celsius.kind(), &StatUnitKind::Celsius);
    assert_eq!(StatUnit::parse("Celsius").unwrap(), celsius);
    assert_eq!(StatUnit::parse("DegreeCelsius").unwrap(), celsius);
}
