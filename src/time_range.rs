use anyhow::{bail, Result};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};

pub struct Range {
    /// RFC 3339, for the `starting_at` API param.
    pub starting_at: String,
    /// RFC 3339, for the `ending_at` API param.
    pub ending_at: String,
    /// Human-readable first day covered, for display.
    pub since_label: String,
    /// Human-readable last day covered, for display.
    pub until_label: String,
}

pub fn resolve(days: Option<i64>, since: Option<&str>, until: Option<&str>) -> Result<Range> {
    let now = Utc::now();

    let end = match until {
        Some(s) => parse_boundary(s, true)?,
        None => now,
    };
    let start = match since {
        Some(s) => parse_boundary(s, false)?,
        None => {
            let d = days.unwrap_or(7);
            if d <= 0 {
                bail!("--days must be a positive number");
            }
            end - Duration::days(d)
        }
    };

    if start >= end {
        bail!("the range is empty: --since must be before --until");
    }

    let since_label = start.format("%Y-%m-%d").to_string();
    let until_label = (end - Duration::seconds(1)).format("%Y-%m-%d").to_string();

    Ok(Range {
        starting_at: format_rfc3339(start),
        ending_at: format_rfc3339(end),
        since_label,
        until_label,
    })
}

/// Parses either a full RFC 3339 timestamp, or a plain `YYYY-MM-DD` date.
/// A plain date used as the end of a range is treated as the start of the
/// following day, so the whole day is included (the API's `ending_at` bound
/// is exclusive).
fn parse_boundary(s: &str, is_end: bool) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let date = if is_end { date + Duration::days(1) } else { date };
        let naive = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&naive));
    }
    bail!("could not parse '{s}'; use YYYY-MM-DD or a full RFC 3339 timestamp")
}

fn format_rfc3339(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
