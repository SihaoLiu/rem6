use rem6_timer::{
    Mc146818Rtc, RtcDateTime, RtcEncoding, RtcError, RTC_DAY_OF_MONTH_REGISTER,
    RTC_DAY_OF_WEEK_REGISTER, RTC_HOURS_REGISTER, RTC_MINUTES_REGISTER, RTC_MONTH_REGISTER,
    RTC_SECONDS_ALARM_REGISTER, RTC_SECONDS_REGISTER, RTC_STATUS_A_REGISTER, RTC_STATUS_B_REGISTER,
    RTC_STATUS_C_REGISTER, RTC_STATUS_D_REGISTER, RTC_YEARS_REGISTER,
};

#[test]
fn mc146818_rtc_exposes_binary_and_bcd_calendar_registers() {
    let friday = 6;
    let time = RtcDateTime::new(2026, 5, 29, 23, 59, 58, friday).unwrap();

    let binary = Mc146818Rtc::new(time, RtcEncoding::Binary).unwrap();
    assert_eq!(binary.read_register(RTC_SECONDS_REGISTER).unwrap(), 58);
    assert_eq!(binary.read_register(RTC_MINUTES_REGISTER).unwrap(), 59);
    assert_eq!(binary.read_register(RTC_HOURS_REGISTER).unwrap(), 23);
    assert_eq!(
        binary.read_register(RTC_DAY_OF_WEEK_REGISTER).unwrap(),
        friday
    );
    assert_eq!(binary.read_register(RTC_DAY_OF_MONTH_REGISTER).unwrap(), 29);
    assert_eq!(binary.read_register(RTC_MONTH_REGISTER).unwrap(), 5);
    assert_eq!(binary.read_register(RTC_YEARS_REGISTER).unwrap(), 126);
    assert_eq!(binary.read_register(RTC_STATUS_A_REGISTER).unwrap(), 0x26);
    assert_eq!(binary.read_register(RTC_STATUS_B_REGISTER).unwrap(), 0x46);
    assert_eq!(binary.read_register(RTC_STATUS_C_REGISTER).unwrap(), 0);
    assert_eq!(binary.read_register(RTC_STATUS_D_REGISTER).unwrap(), 0);

    let bcd = Mc146818Rtc::new(time, RtcEncoding::Bcd).unwrap();
    assert_eq!(bcd.read_register(RTC_SECONDS_REGISTER).unwrap(), 0x58);
    assert_eq!(bcd.read_register(RTC_MINUTES_REGISTER).unwrap(), 0x59);
    assert_eq!(bcd.read_register(RTC_HOURS_REGISTER).unwrap(), 0x23);
    assert_eq!(bcd.read_register(RTC_DAY_OF_WEEK_REGISTER).unwrap(), 0x06);
    assert_eq!(bcd.read_register(RTC_DAY_OF_MONTH_REGISTER).unwrap(), 0x29);
    assert_eq!(bcd.read_register(RTC_MONTH_REGISTER).unwrap(), 0x05);
    assert_eq!(bcd.read_register(RTC_YEARS_REGISTER).unwrap(), 0x26);
    assert_eq!(bcd.read_register(RTC_STATUS_B_REGISTER).unwrap(), 0x42);
}

#[test]
fn mc146818_rtc_tick_rolls_over_leap_day_and_honors_set_bit() {
    let thursday = 5;
    let friday = 6;
    let time = RtcDateTime::new(2024, 2, 29, 23, 59, 59, thursday).unwrap();
    let mut rtc = Mc146818Rtc::new(time, RtcEncoding::Bcd).unwrap();

    rtc.tick_second().unwrap();

    assert_eq!(rtc.read_register(RTC_SECONDS_REGISTER).unwrap(), 0x00);
    assert_eq!(rtc.read_register(RTC_MINUTES_REGISTER).unwrap(), 0x00);
    assert_eq!(rtc.read_register(RTC_HOURS_REGISTER).unwrap(), 0x00);
    assert_eq!(rtc.read_register(RTC_DAY_OF_MONTH_REGISTER).unwrap(), 0x01);
    assert_eq!(rtc.read_register(RTC_MONTH_REGISTER).unwrap(), 0x03);
    assert_eq!(rtc.read_register(RTC_YEARS_REGISTER).unwrap(), 0x24);
    assert_eq!(rtc.read_register(RTC_DAY_OF_WEEK_REGISTER).unwrap(), friday);

    let status_b = rtc.read_register(RTC_STATUS_B_REGISTER).unwrap();
    rtc.write_register(RTC_STATUS_B_REGISTER, status_b | 0x80)
        .unwrap();
    rtc.tick_second().unwrap();

    assert_eq!(rtc.read_register(RTC_SECONDS_REGISTER).unwrap(), 0x00);
    assert_eq!(rtc.read_register(RTC_DAY_OF_MONTH_REGISTER).unwrap(), 0x01);
}

#[test]
fn mc146818_rtc_snapshot_restores_raw_registers_and_reports_invalid_accesses() {
    let time = RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap();
    let mut source = Mc146818Rtc::new(time, RtcEncoding::Bcd).unwrap();
    source
        .write_register(RTC_SECONDS_ALARM_REGISTER, 0x45)
        .unwrap();
    source.write_register(RTC_SECONDS_REGISTER, 0x56).unwrap();
    let snapshot = source.snapshot();

    let mut target = Mc146818Rtc::new(
        RtcDateTime::new(1999, 12, 31, 23, 59, 59, 6).unwrap(),
        RtcEncoding::Binary,
    )
    .unwrap();
    target.restore(&snapshot).unwrap();

    assert_eq!(
        target.read_register(RTC_SECONDS_ALARM_REGISTER).unwrap(),
        0x45
    );
    assert_eq!(target.read_register(RTC_SECONDS_REGISTER).unwrap(), 0x56);
    assert_eq!(target.read_register(RTC_STATUS_B_REGISTER).unwrap(), 0x42);
    assert_eq!(target.snapshot(), snapshot);

    assert_eq!(
        target
            .write_register(RTC_STATUS_A_REGISTER, 0x16)
            .unwrap_err(),
        RtcError::UnsupportedStatusA {
            register: RTC_STATUS_A_REGISTER,
            value: 0x16,
        }
    );
    assert_eq!(
        target
            .write_register(RTC_STATUS_C_REGISTER, 0xff)
            .unwrap_err(),
        RtcError::ReadOnlyRegister {
            register: RTC_STATUS_C_REGISTER,
        }
    );
    assert_eq!(
        target.read_register(0x80).unwrap_err(),
        RtcError::UnknownRegister { register: 0x80 }
    );
}
