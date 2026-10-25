use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::settings::{Settings, TzSetting};

/// The study day `now` belongs to: the local date of `now` minus `rollover_hour` hours,
/// so late-night study counts toward the previous day.
pub fn study_day<Tz: TimeZone>(now: DateTime<Utc>, tz: &Tz, rollover_hour: u32) -> NaiveDate {
    // Subtract wall-clock hours, not elapsed time, so DST transition days roll over at the same local hour.
    (now.with_timezone(tz).naive_local() - Duration::hours(rollover_hour as i64)).date()
}

/// A local wall-clock time as UTC; in a DST gap, the earliest valid reading (or UTC as a fallback).
fn local_to_utc<Tz: TimeZone>(tz: &Tz, local: NaiveDateTime) -> DateTime<Utc> {
    tz.from_local_datetime(&local)
        .earliest()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc.from_utc_datetime(&local))
}

impl Settings {
    pub fn study_day(&self, now: DateTime<Utc>) -> NaiveDate {
        match self.timezone {
            TzSetting::System => study_day(now, &chrono::Local, self.day_rollover_hour),
            TzSetting::Named(tz) => study_day(now, &tz, self.day_rollover_hour),
        }
    }

    /// The instant study day `day` ends: the next day's rollover hour, local time.
    pub fn day_end(&self, day: NaiveDate) -> DateTime<Utc> {
        let next = (day + Duration::days(1)).and_hms_opt(self.day_rollover_hour, 0, 0).expect("valid hour");
        match self.timezone {
            TzSetting::System => local_to_utc(&chrono::Local, next),
            TzSetting::Named(tz) => local_to_utc(&tz, next),
        }
    }

    /// Local calendar date of `now` (no rollover), used for naming backups.
    pub fn local_date(&self, now: DateTime<Utc>) -> NaiveDate {
        match self.timezone {
            TzSetting::System => now.with_timezone(&chrono::Local).date_naive(),
            TzSetting::Named(tz) => now.with_timezone(&tz).date_naive(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};

    fn utc(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn before_rollover_counts_as_previous_day() {
        let la = chrono_tz::America::Los_Angeles;
        // 10:59Z = 03:59 PDT → before 04:00 → study day 2026-10-22
        assert_eq!(study_day(utc("2026-10-23T10:59:00Z"), &la, 4), date(2026, 10, 22));
        assert_eq!(study_day(utc("2026-10-23T11:00:00Z"), &la, 4), date(2026, 10, 23));
    }

    #[test]
    fn dst_transitions() {
        let la = chrono_tz::America::Los_Angeles;
        // DST ends 2026-11-01: 11:30Z = 03:30 PST
        assert_eq!(study_day(utc("2026-11-01T11:30:00Z"), &la, 4), date(2026, 10, 31));
        assert_eq!(study_day(utc("2026-11-01T12:00:00Z"), &la, 4), date(2026, 11, 1));
        // DST starts 2026-03-08: 10:30Z = 03:30 PDT
        assert_eq!(study_day(utc("2026-03-08T10:30:00Z"), &la, 4), date(2026, 3, 7));
    }

    #[test]
    fn zero_rollover_is_midnight() {
        assert_eq!(study_day(utc("2026-10-23T00:30:00Z"), &Utc, 0), date(2026, 10, 23));
    }
}
