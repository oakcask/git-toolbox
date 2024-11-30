use chrono::{DateTime, FixedOffset, TimeZone};
use git2::Time;

pub struct GitTime(Time);

impl From<Time> for GitTime {
    fn from(value: Time) -> Self {
        GitTime(value)
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
    fn test_gittime_from() {
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
}