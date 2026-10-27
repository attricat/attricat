//! Values core assigns to an attribute when a record is created, numbered
//! from a gapless per-family counter and rendered through a fixed format.

use attricat_domain::attribute_options::SequenceReset;
use attricat_domain::value_type::ValueType;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

/// Widest zero padding `{n:0W}` accepts; 18 digits still fit a counter.
pub const MAX_GENERATED_WIDTH: usize = 18;
/// Largest number a counter may hand out or be advanced to.
pub const MAX_GENERATED_NUMBER: i64 = 999_999_999_999_999_999;
/// Longest `format`, in characters.
pub const MAX_GENERATED_FORMAT_CHARS: usize = 100;
pub const DEFAULT_GENERATED_START: i64 = 1;
pub const DEFAULT_GENERATED_TIME_ZONE: &str = "UTC";
/// The counter period of attributes whose numbering never restarts.
pub const UNPERIODIC: &str = "";

/// A compiled `generated = { … }` declaration, stored as JSON on the
/// attribute row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeneratedValue {
    /// Absent on integer attributes, which store the number itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub start: i64,
    pub reset: SequenceReset,
    /// IANA zone in which date tokens and periods are read.
    pub time_zone: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Segment {
    Literal(String),
    Number { width: usize },
    Year,
    ShortYear,
    Month,
    Day,
}

impl GeneratedValue {
    /// Validates a declaration for an attribute of `value_type`; the error
    /// says what is wrong in words for the blueprint author.
    pub fn new(
        value_type: ValueType,
        format: Option<String>,
        start: Option<i64>,
        reset: Option<&str>,
        time_zone: Option<String>,
    ) -> Result<Self, String> {
        let start = start.unwrap_or(DEFAULT_GENERATED_START);
        if !(0..=MAX_GENERATED_NUMBER).contains(&start) {
            return Err(format!(
                "start must be between 0 and {MAX_GENERATED_NUMBER}"
            ));
        }
        let reset = match reset {
            None => SequenceReset::Never,
            Some(code) => SequenceReset::parse(code).ok_or_else(|| {
                format!("reset must be one of {}", SequenceReset::codes().join(", "))
            })?,
        };
        match value_type {
            ValueType::String => {}
            ValueType::Integer => {
                if format.is_some() || reset != SequenceReset::Never || time_zone.is_some() {
                    return Err(
                        "integer attributes store the number itself and take no format, reset or time_zone"
                            .into(),
                    );
                }
            }
            _ => return Err("only string and integer attributes can be generated".into()),
        }
        let time_zone = time_zone.unwrap_or_else(|| DEFAULT_GENERATED_TIME_ZONE.to_owned());
        if time_zone.parse::<Tz>().is_err() {
            return Err(format!("time_zone '{time_zone}' is not an IANA time zone"));
        }
        let generated = Self {
            format,
            start,
            reset,
            time_zone,
        };
        if value_type == ValueType::String {
            let format = generated
                .format
                .as_deref()
                .ok_or("string attributes need a format")?;
            if format.chars().count() > MAX_GENERATED_FORMAT_CHARS {
                return Err(format!(
                    "format must be at most {MAX_GENERATED_FORMAT_CHARS} characters"
                ));
            }
            let segments = parse_format(format)?;
            let has = |wanted: &[Segment]| segments.iter().any(|segment| wanted.contains(segment));
            let year = has(&[Segment::Year, Segment::ShortYear]);
            let identifies_period = match reset {
                SequenceReset::Never => true,
                SequenceReset::Yearly => year,
                SequenceReset::Monthly => year && has(&[Segment::Month]),
                SequenceReset::Daily => year && has(&[Segment::Month]) && has(&[Segment::Day]),
            };
            if !identifies_period {
                return Err(format!(
                    "reset = \"{reset}\" needs date tokens that identify the period, or values would repeat"
                ));
            }
        }
        Ok(generated)
    }

    fn zone(&self) -> Tz {
        self.time_zone
            .parse()
            .expect("validated generated time zone")
    }

    /// The counter period `at` falls in: [`UNPERIODIC`] when numbering never
    /// restarts, else `2026`, `2026-10` or `2026-10-27` in the attribute's
    /// time zone.
    pub fn period(&self, at: DateTime<Utc>) -> String {
        self.period_of(at.with_timezone(&self.zone()).date_naive())
    }

    fn period_of(&self, date: NaiveDate) -> String {
        match self.reset {
            SequenceReset::Never => UNPERIODIC.to_owned(),
            SequenceReset::Yearly => format!("{:04}", date.year()),
            SequenceReset::Monthly => format!("{:04}-{:02}", date.year(), date.month()),
            SequenceReset::Daily => {
                format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
            }
        }
    }

    /// Whether `period` is written exactly as [`Self::period`] writes one of
    /// this attribute's periods.
    pub fn is_period(&self, period: &str) -> bool {
        let first_day = match self.reset {
            SequenceReset::Never => return period == UNPERIODIC,
            SequenceReset::Yearly => format!("{period}-01-01"),
            SequenceReset::Monthly => format!("{period}-01"),
            SequenceReset::Daily => period.to_owned(),
        };
        NaiveDate::parse_from_str(&first_day, "%Y-%m-%d")
            .is_ok_and(|date| self.period_of(date) == period)
    }

    /// The value for counter `number` allocated at `at`: the number itself
    /// for integer attributes, else the rendered format.
    pub fn render(&self, number: i64, at: DateTime<Utc>) -> serde_json::Value {
        let Some(format) = &self.format else {
            return number.into();
        };
        let date = at.with_timezone(&self.zone()).date_naive();
        parse_format(format)
            .expect("validated generated format")
            .into_iter()
            .map(|segment| match segment {
                Segment::Literal(text) => text,
                Segment::Number { width } => format!("{number:0width$}"),
                Segment::Year => format!("{:04}", date.year()),
                Segment::ShortYear => format!("{:02}", date.year().rem_euclid(100)),
                Segment::Month => format!("{:02}", date.month()),
                Segment::Day => format!("{:02}", date.day()),
            })
            .collect::<String>()
            .into()
    }
}

/// Splits a format into literal text and tokens: exactly one `{n}` or
/// `{n:0W}`, any of `{yyyy}`, `{yy}`, `{mm}` and `{dd}`, and `{{` / `}}` for
/// literal braces. Every other placeholder is rejected so tokens can be added
/// later without changing what an existing format means.
fn parse_format(format: &str) -> Result<Vec<Segment>, String> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut numbers = 0;
    let mut chars = format.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '}' => {
                return Err("format has an unmatched '}'; write '}}' for a literal brace".into());
            }
            '{' => {
                let mut token = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(character) => token.push(character),
                        None => {
                            return Err(
                                "format has an unclosed '{'; write '{{' for a literal brace".into(),
                            );
                        }
                    }
                }
                let segment = match token.as_str() {
                    "n" => Segment::Number { width: 0 },
                    "yyyy" => Segment::Year,
                    "yy" => Segment::ShortYear,
                    "mm" => Segment::Month,
                    "dd" => Segment::Day,
                    _ => match token.strip_prefix("n:0").map(str::parse::<usize>) {
                        Some(Ok(width)) if (1..=MAX_GENERATED_WIDTH).contains(&width) => {
                            Segment::Number { width }
                        }
                        _ => {
                            return Err(format!(
                                "format has unknown token '{{{token}}}'; use {{n}}, {{n:0W}} with W from 1 to {MAX_GENERATED_WIDTH}, {{yyyy}}, {{yy}}, {{mm}} or {{dd}}"
                            ));
                        }
                    },
                };
                if matches!(segment, Segment::Number { .. }) {
                    numbers += 1;
                }
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(segment);
            }
            _ => literal.push(character),
        }
    }
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    if numbers != 1 {
        return Err("format must contain exactly one {n} or {n:0W} counter token".into());
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn string(
        format: &str,
        reset: Option<&str>,
        time_zone: Option<&str>,
    ) -> Result<GeneratedValue, String> {
        GeneratedValue::new(
            ValueType::String,
            Some(format.to_owned()),
            None,
            reset,
            time_zone.map(str::to_owned),
        )
    }

    #[test]
    fn renders_padded_numbers_dates_and_escaped_braces() {
        let at = Utc.with_ymd_and_hms(2026, 3, 7, 12, 0, 0).unwrap();
        let generated = string("{{PRD}}-{yyyy}/{yy}{mm}{dd}-{n:05}", None, None).unwrap();
        assert_eq!(generated.render(42, at), "{PRD}-2026/260307-00042");
        assert_eq!(string("N{n}", None, None).unwrap().render(7, at), "N7");
        // Wider numbers are not truncated.
        assert_eq!(
            string("{n:03}", None, None).unwrap().render(1000, at),
            "1000"
        );
    }

    #[test]
    fn integer_attributes_store_the_number() {
        let generated =
            GeneratedValue::new(ValueType::Integer, None, Some(100), None, None).unwrap();
        assert_eq!(generated.start, 100);
        assert_eq!(generated.render(105, Utc::now()), 105);
        assert_eq!(generated.period(Utc::now()), UNPERIODIC);
        for (format, reset, time_zone) in [
            (Some("{n}"), None, None),
            (None, Some("yearly"), None),
            (None, None, Some("UTC")),
        ] {
            assert!(
                GeneratedValue::new(
                    ValueType::Integer,
                    format.map(str::to_owned),
                    None,
                    reset,
                    time_zone.map(str::to_owned),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_invalid_formats_and_options() {
        for format in [
            "",
            "PRD",
            "{n}{n}",
            "{n:00}",
            "{n:019}",
            "{n:5}",
            "{x}",
            "{n",
            "a}b{n}",
            "{category}-{n}",
        ] {
            assert!(string(format, None, None).is_err(), "{format}");
        }
        assert!(
            string(
                &format!("{{n}}{}", "x".repeat(MAX_GENERATED_FORMAT_CHARS)),
                None,
                None
            )
            .is_err()
        );
        assert!(string("{n}", Some("hourly"), None).is_err());
        assert!(string("{n}", None, Some("Mars/Base")).is_err());
        assert!(
            GeneratedValue::new(ValueType::String, None, None, None, None).is_err(),
            "string attributes need a format"
        );
        assert!(
            GeneratedValue::new(ValueType::Number, None, None, None, None).is_err(),
            "only string and integer attributes"
        );
        for start in [-1, MAX_GENERATED_NUMBER + 1] {
            assert!(
                GeneratedValue::new(ValueType::Integer, None, Some(start), None, None).is_err()
            );
        }
    }

    #[test]
    fn resets_need_tokens_that_identify_the_period() {
        assert!(string("{n}", Some("yearly"), None).is_err());
        assert!(string("{yy}-{n}", Some("yearly"), None).is_ok());
        assert!(string("{yyyy}-{n}", Some("monthly"), None).is_err());
        assert!(string("{yyyy}{mm}-{n}", Some("monthly"), None).is_ok());
        assert!(string("{yyyy}{mm}-{n}", Some("daily"), None).is_err());
        assert!(string("{yyyy}{mm}{dd}-{n}", Some("daily"), None).is_ok());
        // Printing a date without resetting is allowed.
        assert!(string("{yyyy}-{n}", Some("never"), None).is_ok());
    }

    #[test]
    fn periods_and_dates_follow_the_time_zone() {
        let at = Utc.with_ymd_and_hms(2025, 12, 31, 23, 30, 0).unwrap();
        let utc = string("{yyyy}-{n}", Some("yearly"), None).unwrap();
        assert_eq!(utc.period(at), "2025");
        assert_eq!(utc.render(1, at), "2025-1");
        let warsaw = string("{yyyy}{mm}{dd}-{n}", Some("daily"), Some("Europe/Warsaw")).unwrap();
        assert_eq!(warsaw.period(at), "2026-01-01");
        assert_eq!(warsaw.render(3, at), "20260101-3");
        let monthly = string("{yyyy}{mm}-{n}", Some("monthly"), Some("Europe/Warsaw")).unwrap();
        assert_eq!(monthly.period(at), "2026-01");
    }

    #[test]
    fn periods_are_recognised_only_in_their_own_spelling() {
        let never = string("{n}", None, None).unwrap();
        assert!(never.is_period(""));
        assert!(!never.is_period("2026"));
        let yearly = string("{yyyy}-{n}", Some("yearly"), None).unwrap();
        assert!(yearly.is_period("2026"));
        assert!(!yearly.is_period("26") && !yearly.is_period("2026-01") && !yearly.is_period(""));
        let monthly = string("{yyyy}{mm}-{n}", Some("monthly"), None).unwrap();
        assert!(monthly.is_period("2026-02"));
        assert!(!monthly.is_period("2026-2") && !monthly.is_period("2026-13"));
        let daily = string("{yyyy}{mm}{dd}-{n}", Some("daily"), None).unwrap();
        assert!(daily.is_period("2028-02-29"));
        assert!(!daily.is_period("2026-02-29") && !daily.is_period("2026-2-1"));
    }

    #[test]
    fn stored_json_round_trips() {
        let generated = string("INV-{yyyy}-{n:05}", Some("yearly"), Some("Europe/Warsaw")).unwrap();
        let json = serde_json::to_value(&generated).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "format": "INV-{yyyy}-{n:05}",
                "start": 1,
                "reset": "yearly",
                "time_zone": "Europe/Warsaw"
            })
        );
        assert_eq!(
            serde_json::from_value::<GeneratedValue>(json).unwrap(),
            generated
        );
    }
}
