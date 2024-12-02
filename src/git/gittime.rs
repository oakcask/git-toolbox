use chrono::{DateTime, FixedOffset, TimeZone};
use git2::Time;

/// Wrap git2::Time and provides interop between chrono and git2::Time
pub struct GitTime(Time);

impl From<Time> for GitTime {
    fn from(value: Time) -> Self {
        GitTime(value)
    }
}

impl<Tz: TimeZone> From<DateTime<Tz>> for GitTime {
    fn from(datetime: DateTime<Tz>) -> Self {
        let datetime = datetime.fixed_offset();
        let offset = datetime.offset().local_minus_utc();
        let time = datetime.to_utc().timestamp();

        Time::new(time, offset / 60).into()
    }
}

impl AsRef<Time> for GitTime {
    fn as_ref(&self) -> &Time {
        &self.0
    }
}

impl GitTime {
    /// Current time in local time zone
    pub fn now() -> Self {
        Self::from(chrono::Local::now())
    }
}

impl From<GitTime> for DateTime<FixedOffset> {
    fn from(val: GitTime) -> Self {
        let seconds_from_epoch = val.0.seconds();
        let offset = val.0.offset_minutes() * 60;
        let offset = FixedOffset::east_opt(offset).unwrap();
        offset.timestamp_opt(seconds_from_epoch, 0).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, TimeZone};
    use git2::Time;

    use super::GitTime;

    #[test]
    fn test_gittime_from_git_time() {
        // struct git_time consists of time and offset.
        // time is `__time64_t` and that is elapsed seconds from epoch (1970-01-01 00:00:00 UTC).

        // 1970-01-01T09:00:00+0900 = epoch in UTC
        let t = Time::new(0, 9*60);
        let gt: GitTime = t.into();
        let dt: DateTime<_> = gt.into();

        let jst = FixedOffset::east_opt(9*3600).unwrap();
        let nine_o_clock_in_jst = jst.with_ymd_and_hms(1970, 1, 1, 9, 0, 0).unwrap();

        assert_eq!(dt, nine_o_clock_in_jst);
    }

    #[test]
    fn test_gittime_from_datetime() {
        // let's use Go language "Layout"
        let minus7 = FixedOffset::west_opt(7*3600).unwrap();
        let dt = minus7.with_ymd_and_hms(2006, 1, 2, 15, 4, 5).unwrap();

        let gt = GitTime::from(dt);
        assert_eq!((gt.0.seconds(), gt.0.offset_minutes()), (1136239445, -7*60));
    }
}