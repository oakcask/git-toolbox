use std::ops::Sub;

use chrono::{DateTime, Days, Months, TimeZone};
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot parse {0}")]
    ParseError(String),
    #[error("range error")]
    RangeError,
}

struct ReltimeBuilder {
    days: u32,
    weeks: u32,
    months: u32,
    years: u32,
}

impl ReltimeBuilder {
    fn normalize(self) -> Result<Self, Error> {
        let months = self
            .years
            .checked_mul(12)
            .ok_or(Error::RangeError)?
            .checked_add(self.months)
            .ok_or(Error::RangeError)?
            .checked_add(self.weeks / 4)
            .ok_or(Error::RangeError)?;
        let weeks = self.weeks % 4;
        let days = weeks
            .checked_mul(7)
            .ok_or(Error::RangeError)?
            .checked_add(self.days)
            .ok_or(Error::RangeError)?;

        Ok(Self {
            days,
            weeks: 0,
            months,
            years: 0,
        })
    }

    fn build(self) -> Result<Reltime, Error> {
        let a = self.normalize()?;
        Ok(Reltime {
            days: Days::new(a.days.into()),
            months: Months::new(a.months),
        })
    }
}

#[derive(Clone)]
pub struct Reltime {
    days: Days,
    months: Months,
}

impl TryFrom<&str> for Reltime {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        static RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(?:(?P<yr>\d+)\s*(?:y|yrs?|years?))?(?:(?P<mo>\d+)\s*(?:mo|months?))?(?:(?P<w>\d+)\s*(?:w|weeks?))?(?:(?P<d>\d+)\s*(?:d|days?))?").unwrap()
        });

        match RE.captures(value) {
            Some(caps) => {
                let years = caps
                    .name("yr")
                    .map_or(Ok(0), |s| s.as_str().parse())
                    .map_err(|_| Error::ParseError(value.to_string()))?;
                let months = caps
                    .name("mo")
                    .map_or(Ok(0), |s| s.as_str().parse())
                    .map_err(|_| Error::ParseError(value.to_string()))?;
                let weeks = caps
                    .name("w")
                    .map_or(Ok(0), |s| s.as_str().parse())
                    .map_err(|_| Error::ParseError(value.to_string()))?;
                let days = caps
                    .name("d")
                    .map_or(Ok(0), |s| s.as_str().parse())
                    .map_err(|_| Error::ParseError(value.to_string()))?;

                Ok(ReltimeBuilder {
                    years,
                    months,
                    weeks,
                    days,
                }
                .build()?)
            }
            None => Err(Error::ParseError(value.to_string())),
        }
    }
}

impl<Tz: TimeZone> Sub<Reltime> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    fn sub(self, rhs: Reltime) -> Self::Output {
        self.checked_sub_months(rhs.months)
            .unwrap()
            .checked_sub_days(rhs.days)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::reltime::Reltime;
    use chrono::DateTime;
    use std::error::Error;

    #[test]
    fn test() -> Result<(), Box<dyn Error>> {
        #[rustfmt::skip]
        let testcases = [
            ("2022-01-01T00:00:00+09:00", "1d",     "2021-12-31T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1day",   "2021-12-31T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1days",  "2021-12-31T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1 d",    "2021-12-31T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1 day",  "2021-12-31T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1 days", "2021-12-31T00:00:00+09:00"),
            // 7 days does not round up to weeks
            ("2022-01-28T00:00:00+09:00", "28 days",  "2021-12-31T00:00:00+09:00"),
            ("2022-03-07T00:00:00+09:00", "1w",       "2022-02-28T00:00:00+09:00"),
            ("2022-03-07T00:00:00+09:00", "1week",    "2022-02-28T00:00:00+09:00"),
            ("2022-03-07T00:00:00+09:00", "1weeks",   "2022-02-28T00:00:00+09:00"),
            ("2022-03-07T00:00:00+09:00", "1 w",      "2022-02-28T00:00:00+09:00"),
            ("2022-03-07T00:00:00+09:00", "1 week",   "2022-02-28T00:00:00+09:00"),
            ("2022-03-07T00:00:00+09:00", "1 weeks",  "2022-02-28T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1mo",      "2021-12-01T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1month",   "2021-12-01T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1months",  "2021-12-01T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1 mo",     "2021-12-01T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1 month",  "2021-12-01T00:00:00+09:00"),
            ("2022-01-01T00:00:00+09:00", "1 months", "2021-12-01T00:00:00+09:00"),
            // 4 weeks will be round up to 1 month
            ("2022-02-28T00:00:00+09:00", "4 weeks", "2022-01-28T00:00:00+09:00"),
            ("2022-02-28T00:00:00+09:00", "8 weeks", "2021-12-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1y",      "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1yr",     "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1yrs",    "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1year",   "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1years",  "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1 y",     "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1 yr",    "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1 yrs",   "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1 year",  "1999-02-28T00:00:00+09:00"),
            ("2000-02-29T00:00:00+09:00", "1 years", "1999-02-28T00:00:00+09:00"),
            // 12 months will be round up to 1 year
            ("2000-02-29T00:00:00+09:00", "12mo", "1999-02-28T00:00:00+09:00"),
        ];

        for (idx, (now, reltime, want)) in testcases.into_iter().enumerate() {
            let dt_now = DateTime::parse_from_rfc3339(now)?;
            let dt_want = DateTime::parse_from_rfc3339(want)?;
            let rt = Reltime::try_from(reltime)?;
            let got = dt_now - rt;

            assert_eq!(
                dt_want, got,
                "wanted {want} from {now} before {reltime} (#{idx})"
            );
        }

        Ok(())
    }
}
