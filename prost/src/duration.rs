use std::fmt;

/// A Duration represents a signed, fixed-length span of time represented
/// as a count of seconds and fractions of seconds at nanosecond
/// resolution.
///
/// It is independent of any calendar and concepts like "day"
/// or "month".
///
/// # JSON Mapping
///
/// In JSON format, the Duration type is encoded as a string rather than an
/// object, where the string ends in the suffix "s" (indicating seconds) and
/// is preceded by the number of seconds, with nanoseconds expressed as
/// fractional seconds. For example, 3 seconds with 0 nanoseconds should be
/// encoded in JSON format as "3s", while 3 seconds and 1 nanosecond should
/// be expressed in JSON format as "3.000000001s", and 3 seconds and 1
/// microsecond should be expressed in JSON format as "3.000001s".
///
#[derive(
    serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub struct Duration(std::time::Duration);

// {
//     /// Signed seconds of the span of time. Must be from -315,576,000,000
//     /// to +315,576,000,000 inclusive. Note: these bounds are computed from:
//     /// 60 sec/min * 60 min/hr * 24 hr/day * 365.25 days/year * 10000 years
//     // #[prost(uint64, tag = "1")]
//     pub seconds: u64,
//     /// Signed fractions of a second at nanosecond resolution of the span
//     /// of time. Durations less than one second are represented with a 0
//     /// `seconds` field and a positive or negative `nanos` field. For durations
//     /// of one second or more, a non-zero value for the `nanos` field must be
//     /// of the same sign as the `seconds` field. Must be from -999,999,999
//     /// to +999,999,999 inclusive.
//     // #[prost(uint32, tag = "2")]
//     pub nanos: u32,
// }

impl prost::Message for Duration {
    fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
    where
        Self: Sized,
    {
        prost::encoding::uint64::encode(1, &self.0.as_secs(), buf);
        prost::encoding::uint32::encode(2, &self.0.subsec_nanos(), buf);
    }

    fn merge_field(
        &mut self,
        tag: u32,
        wire_type: prost::encoding::wire_type::WireType,
        buf: &mut impl prost::bytes::Buf,
        ctx: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError>
    where
        Self: Sized,
    {
        const STRUCT_NAME: &str = stringify!(Duration);

        match tag {
            // Decode seconds field
            1 => {
                let mut secs = self.0.as_secs();
                let result = prost::encoding::uint64::merge(wire_type, &mut secs, buf, ctx)
                    .map_err(|mut error| {
                        error.push(STRUCT_NAME, stringify!(seconds));
                        error
                    });
                self.0 = std::time::Duration::new(secs, self.0.subsec_nanos());
                result
            }
            // Decode nanos field
            2 => {
                let mut nanos = self.0.subsec_nanos();
                prost::encoding::uint32::merge(wire_type, &mut nanos, buf, ctx).map_err(
                    |mut error| {
                        error.push(STRUCT_NAME, stringify!(nanos));
                        error
                    },
                )?;
                if nanos >= 1_000_000_000 {
                    #[allow(
                        deprecated,
                        reason = "there's no other way to create a DecodeError at the moment"
                    )]
                    return Err(prost::DecodeError::new(
                        "nanos must be less than 1_000_000_000",
                    ));
                }
                self.0 = std::time::Duration::new(self.0.as_secs(), nanos);
                Ok(())
            }
            _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
        }
    }

    fn encoded_len(&self) -> usize {
        prost::encoding::uint64::encoded_len(1, &self.0.as_secs())
            + prost::encoding::uint32::encoded_len(2, &self.0.subsec_nanos())
    }

    fn clear(&mut self) {
        self.0 = std::time::Duration::new(0, 0);
    }
}

impl Duration {
    #[must_use]
    pub const fn from_secs(seconds: u64) -> Self {
        Self(std::time::Duration::from_secs(seconds))
    }

    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self(std::time::Duration::from_millis(millis))
    }

    #[must_use]
    pub const fn inner(&self) -> &std::time::Duration {
        &self.0
    }

    #[must_use]
    pub const fn as_secs(&self) -> u64 {
        self.0.as_secs()
    }

    #[must_use]
    pub const fn as_secs_f32(&self) -> f32 {
        self.0.as_secs_f32()
    }

    #[must_use]
    pub const fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    #[must_use]
    pub const fn as_micros(&self) -> u128 {
        self.0.as_micros()
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_secs = self.0.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        let millis = self.0.subsec_millis() / 100; // tenths of a second

        if hours != 0 {
            write!(f, "{hours}:{minutes:02}:")?;
        } else {
            write!(f, "{minutes}:")?;
        }

        write!(f, "{seconds:02}.{millis:01}")
    }
}

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::iter::Sum for Duration {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|d| d.0).sum())
    }
}

impl From<std::time::Duration> for Duration {
    fn from(duration: std::time::Duration) -> Self {
        Self(duration)
    }
}

impl std::ops::AddAssign<std::time::Duration> for Duration {
    fn add_assign(&mut self, rhs: std::time::Duration) {
        self.0 += rhs;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(Duration(std::time::Duration::new(3661, 0)), "1:01:01.0")]
    #[case(Duration(std::time::Duration::new(0, 1_500_000_000)), "0:01.5")]
    #[case(Duration(std::time::Duration::new(1, 1_000_000_000)), "0:02.0")]
    #[case::zero(Duration::from_secs(0), "0:00.0")]
    #[case::sub_second(Duration::from_millis(100), "0:00.1")]
    #[case::sub_second(Duration::from_millis(101), "0:00.1")]
    #[case::one_second(Duration::from_secs(1), "0:01.0")]
    #[case::one_minute(Duration::from_secs(60), "1:00.0")]
    #[case::one_hour(Duration::from_secs(3600), "1:00:00.0")]
    #[case::one_hour_one_minute_one_second(Duration::from_secs(3661), "1:01:01.0")]
    #[case(Duration::from_secs(3600 + 120 + 1), "1:02:01.0")]
    fn test_display(#[case] input: Duration, #[case] expected: &str) {
        assert_eq!(format!("{}", input), expected);
    }
}
