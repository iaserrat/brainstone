//! Table-driven recurrence cases are exempt from function size limits.
#![allow(clippy::too_many_lines)]

use jiff::civil::Time;

use super::{Recurrence, RepeatSpec};
use crate::Error;
use crate::schedule_failure::failure_notice;

const TORONTO: &str = "America/Toronto";

fn eight(days: Option<Vec<i8>>) -> Recurrence {
    Recurrence::new(Time::constant(8, 0, 0, 0), TORONTO, days).unwrap()
}

fn nine(spec: RepeatSpec) -> Recurrence {
    let spec = RepeatSpec {
        time: Some(Time::constant(9, 0, 0, 0)),
        time_zone: Some(TORONTO.to_owned()),
        ..spec
    };
    Recurrence::try_from(spec).unwrap()
}

fn ms(rfc3339: &str) -> i64 {
    rfc3339.parse::<jiff::Timestamp>().unwrap().as_millisecond()
}

/// Legacy daily and weekly rules ignore the anchor; any value works.
const ANY_ANCHOR: i64 = 0;

#[test]
fn next_run_is_strictly_after_now_and_dst_correct() {
    let cases = [
        // Same day, before 08:00 local: today.
        ("2026-03-07T06:00:00-05:00", "2026-03-07T08:00:00-05:00"),
        // Exactly at 08:00: strictly after, so tomorrow; clocks spring forward.
        ("2026-03-07T08:00:00-05:00", "2026-03-08T08:00:00-04:00"),
        // Across the fall-back transition the wall clock still reads 08:00.
        ("2026-10-31T09:00:00-04:00", "2026-11-01T08:00:00-05:00"),
        // Three days of downtime collapse into one catch-up run.
        ("2026-06-10T12:00:00-04:00", "2026-06-11T08:00:00-04:00"),
    ];
    for (after, expected) in cases {
        assert_eq!(
            eight(None).next_run_at(ms(after), ANY_ANCHOR).unwrap(),
            ms(expected),
            "{after}"
        );
    }
}

#[test]
fn a_local_time_inside_the_spring_gap_runs_just_after_it() {
    let half_past_two = Recurrence::new(Time::constant(2, 30, 0, 0), TORONTO, None).unwrap();
    assert_eq!(
        half_past_two
            .next_run_at(ms("2026-03-07T03:00:00-05:00"), ANY_ANCHOR)
            .unwrap(),
        ms("2026-03-08T03:30:00-04:00")
    );
}

#[test]
fn weekday_lists_skip_days_not_listed() {
    // 2026-06-12 is a Friday.
    let weekdays = eight(Some(vec![1, 2, 3, 4, 5]));
    assert_eq!(
        weekdays
            .next_run_at(ms("2026-06-12T09:00:00-04:00"), ANY_ANCHOR)
            .unwrap(),
        ms("2026-06-15T08:00:00-04:00")
    );
    let sundays = eight(Some(vec![7]));
    assert_eq!(
        sundays
            .next_run_at(ms("2026-06-14T08:00:00-04:00"), ANY_ANCHOR)
            .unwrap(),
        ms("2026-06-21T08:00:00-04:00")
    );
}

#[test]
fn an_interval_counts_whole_steps_from_its_anchor() {
    let every_half_hour = Recurrence::try_from(RepeatSpec {
        every_minutes: Some(30),
        ..RepeatSpec::default()
    })
    .unwrap();
    let anchor = ms("2026-06-10T12:00:00Z");
    let cases = [
        // A clock behind the anchor still waits one whole step.
        ("2026-06-10T11:59:00Z", "2026-06-10T12:30:00Z"),
        ("2026-06-10T12:00:00Z", "2026-06-10T12:30:00Z"),
        ("2026-06-10T12:29:59Z", "2026-06-10T12:30:00Z"),
        // Exactly on a step: strictly after, so the next one.
        ("2026-06-10T12:30:00Z", "2026-06-10T13:00:00Z"),
        // Five hours of downtime collapse into one catch-up run, on the grid.
        ("2026-06-10T17:01:00Z", "2026-06-10T17:30:00Z"),
    ];
    for (after, expected) in cases {
        assert_eq!(
            every_half_hour.next_run_at(ms(after), anchor).unwrap(),
            ms(expected),
            "{after}"
        );
    }
}

#[test]
fn every_n_periods_start_at_the_first_occurrence_after_the_anchor() {
    let every_other_day = nine(RepeatSpec {
        interval: Some(2),
        ..RepeatSpec::default()
    });
    // Created Wednesday after 09:00: Thursday is the first run, then every other day.
    let anchor = ms("2026-06-10T10:00:00-04:00");
    let every_other_monday = nine(RepeatSpec {
        interval: Some(2),
        days: Some(vec![1]),
        ..RepeatSpec::default()
    });
    // Created on Tuesday 2026-06-09: the next Monday starts the fortnight.
    let tuesday = ms("2026-06-09T10:00:00-04:00");
    let quarterly_fifteenth = nine(RepeatSpec {
        interval: Some(3),
        month_day: Some(15),
        ..RepeatSpec::default()
    });
    let january_twentieth = ms("2026-01-20T10:00:00-05:00");
    let cases = [
        (
            &every_other_day,
            anchor,
            anchor,
            "2026-06-11T09:00:00-04:00",
        ),
        (
            &every_other_day,
            ms("2026-06-11T09:00:00-04:00"),
            anchor,
            "2026-06-13T09:00:00-04:00",
        ),
        (
            &every_other_monday,
            tuesday,
            tuesday,
            "2026-06-15T09:00:00-04:00",
        ),
        (
            &every_other_monday,
            ms("2026-06-16T09:00:00-04:00"),
            tuesday,
            "2026-06-29T09:00:00-04:00",
        ),
        (
            &quarterly_fifteenth,
            january_twentieth,
            january_twentieth,
            "2026-02-15T09:00:00-05:00",
        ),
        (
            &quarterly_fifteenth,
            ms("2026-02-16T09:00:00-05:00"),
            january_twentieth,
            "2026-05-15T09:00:00-04:00",
        ),
    ];
    for (rule, after, anchor, expected) in cases {
        assert_eq!(
            rule.next_run_at(after, anchor).unwrap(),
            ms(expected),
            "{rule:?} after {after}"
        );
    }
}

#[test]
fn a_month_day_past_the_months_end_runs_on_its_last_day() {
    let thirty_first = nine(RepeatSpec {
        month_day: Some(31),
        ..RepeatSpec::default()
    });
    let january = ms("2026-01-31T10:00:00-05:00");
    assert_eq!(
        thirty_first.next_run_at(january, january).unwrap(),
        ms("2026-02-28T09:00:00-05:00")
    );
    assert_eq!(
        thirty_first
            .next_run_at(ms("2026-02-28T09:00:00-05:00"), january)
            .unwrap(),
        ms("2026-03-31T09:00:00-04:00")
    );
}

#[test]
fn an_nth_weekday_counts_from_either_end_of_the_month() {
    let first_monday = nine(RepeatSpec {
        days: Some(vec![1]),
        nth: Some(1),
        ..RepeatSpec::default()
    });
    let last_friday = nine(RepeatSpec {
        days: Some(vec![5]),
        nth: Some(-1),
        ..RepeatSpec::default()
    });
    // June 2026's first Monday (the 1st) has passed by Tuesday the 2nd.
    let tuesday = ms("2026-06-02T10:00:00-04:00");
    assert_eq!(
        first_monday.next_run_at(tuesday, tuesday).unwrap(),
        ms("2026-07-06T09:00:00-04:00")
    );
    assert_eq!(
        last_friday.next_run_at(tuesday, tuesday).unwrap(),
        ms("2026-06-26T09:00:00-04:00")
    );
}

#[test]
fn invalid_recurrences_are_rejected_with_the_field() {
    let cases: [(Time, &str, Option<Vec<i8>>, &str); 5] = [
        (Time::constant(8, 0, 30, 0), TORONTO, None, "repeat_time"),
        (
            Time::constant(8, 0, 0, 0),
            "Mars/Olympus",
            None,
            "repeat_time_zone",
        ),
        (
            Time::constant(8, 0, 0, 0),
            TORONTO,
            Some(vec![]),
            "repeat_days",
        ),
        (
            Time::constant(8, 0, 0, 0),
            TORONTO,
            Some(vec![0]),
            "repeat_days",
        ),
        (
            Time::constant(8, 0, 0, 0),
            TORONTO,
            Some(vec![1, 8]),
            "repeat_days",
        ),
    ];
    for (time, zone, days, field) in cases {
        assert_eq!(
            Recurrence::new(time, zone, days),
            Err(Error::InvalidInput { field }),
            "{field}"
        );
    }
}

#[test]
fn invalid_rule_combinations_name_the_offending_field() {
    let nine_toronto = RepeatSpec {
        time: Some(Time::constant(9, 0, 0, 0)),
        time_zone: Some(TORONTO.to_owned()),
        ..RepeatSpec::default()
    };
    let cases = [
        (RepeatSpec::default(), "repeat_time"),
        (
            RepeatSpec {
                time: Some(Time::constant(9, 0, 0, 0)),
                ..RepeatSpec::default()
            },
            "repeat_time_zone",
        ),
        (
            RepeatSpec {
                every_minutes: Some(0),
                ..RepeatSpec::default()
            },
            "repeat_every_minutes",
        ),
        (
            RepeatSpec {
                every_minutes: Some(30),
                ..nine_toronto.clone()
            },
            "repeat_every_minutes",
        ),
        (
            RepeatSpec {
                every_minutes: Some(30),
                interval: Some(2),
                ..RepeatSpec::default()
            },
            "repeat_every_minutes",
        ),
        (
            RepeatSpec {
                interval: Some(0),
                ..nine_toronto.clone()
            },
            "repeat_interval",
        ),
        (
            RepeatSpec {
                interval: Some(53),
                ..nine_toronto.clone()
            },
            "repeat_interval",
        ),
        (
            RepeatSpec {
                month_day: Some(0),
                ..nine_toronto.clone()
            },
            "repeat_month_day",
        ),
        (
            RepeatSpec {
                month_day: Some(32),
                ..nine_toronto.clone()
            },
            "repeat_month_day",
        ),
        (
            RepeatSpec {
                month_day: Some(1),
                days: Some(vec![1]),
                ..nine_toronto.clone()
            },
            "repeat_month_day",
        ),
        (
            RepeatSpec {
                nth: Some(1),
                ..nine_toronto.clone()
            },
            "repeat_nth",
        ),
        (
            RepeatSpec {
                nth: Some(5),
                days: Some(vec![1]),
                ..nine_toronto.clone()
            },
            "repeat_nth",
        ),
        (
            RepeatSpec {
                nth: Some(-2),
                days: Some(vec![1]),
                ..nine_toronto.clone()
            },
            "repeat_nth",
        ),
        (
            RepeatSpec {
                nth: Some(1),
                days: Some(vec![1, 2]),
                ..nine_toronto
            },
            "repeat_days",
        ),
    ];
    for (spec, field) in cases {
        assert_eq!(
            Recurrence::try_from(spec.clone()),
            Err(Error::InvalidInput { field }),
            "{spec:?}"
        );
    }
}

#[test]
fn a_rule_reads_back_as_the_fields_it_was_written_with() {
    let weekly = serde_json::to_value(eight(Some(vec![1, 3]))).unwrap();
    assert_eq!(
        weekly,
        serde_json::json!({"time": "08:00:00", "time_zone": TORONTO, "days": [1, 3]})
    );
    let first_monday = serde_json::json!({
        "time": "09:00:00", "time_zone": TORONTO, "days": [1], "nth": 1, "interval": 2
    });
    let decoded: Recurrence = serde_json::from_value(first_monday.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), first_monday);
    let interval = serde_json::json!({"every_minutes": 90});
    let decoded: Recurrence = serde_json::from_value(interval.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), interval);
    let unknown = serde_json::json!({"every_minutes": 90, "count": 3});
    assert!(serde_json::from_value::<Recurrence>(unknown).is_err());
    let invalid = serde_json::json!({"every_minutes": 0});
    assert!(serde_json::from_value::<Recurrence>(invalid).is_err());
}

#[test]
fn the_failure_notice_appears_at_three_and_reports_the_cancel_at_fourteen() {
    assert_eq!(failure_notice(None), None);
    assert_eq!(failure_notice(Some(2)), None);
    let nudge = failure_notice(Some(3)).unwrap();
    assert!(nudge.contains("3 runs in a row"), "{nudge}");
    assert!(nudge.contains("CancelSchedule"), "{nudge}");
    let cancelled = failure_notice(Some(14)).unwrap();
    assert!(cancelled.contains("has been cancelled"), "{cancelled}");
}
