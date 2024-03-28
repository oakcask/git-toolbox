
use chrono::{DateTime, FixedOffset, TimeZone};
use git2::Time;

pub struct GitTime(Time);

impl From<Time> for GitTime {
    fn from(value: Time) -> Self {
        GitTime(value)
    }
}

impl Into<DateTime<FixedOffset>> for GitTime {
    fn into(self) -> DateTime<FixedOffset> {
        let seconds_from_epoch = self.0.seconds();
        let offset = self.0.offset_minutes() * 60;
        let offset = FixedOffset::west_opt(offset).unwrap();
        offset.timestamp_opt(seconds_from_epoch, 0).unwrap()
    }
}