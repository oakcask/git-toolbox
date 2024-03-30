
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
        let offset = FixedOffset::west_opt(offset).unwrap();
        offset.timestamp_opt(seconds_from_epoch, 0).unwrap()
    }
}