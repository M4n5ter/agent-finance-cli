use anyhow::{Result, anyhow};
use chrono::{DateTime, SecondsFormat, Utc};
use chrono_tz::Tz;

const FALLBACK_TIMEZONE: &str = "UTC";

pub fn resolve_timezone(value: Option<&str>) -> Result<String> {
    match value {
        Some(value) => parse_timezone_name(value).map(str::to_string),
        None => Ok(system_timezone()),
    }
}

pub fn utc_to_local(value: Option<&str>, timezone: &str) -> Option<String> {
    let value = value?;
    let datetime = DateTime::parse_from_rfc3339(value).ok()?;
    Some(format_local(datetime.with_timezone(&Utc), timezone))
}

pub fn now_local(timezone: &str) -> String {
    format_local(Utc::now(), timezone)
}

pub fn format_local(datetime: DateTime<Utc>, timezone: &str) -> String {
    let timezone = parse_timezone(timezone).expect("timezone must be resolved before formatting");
    datetime
        .with_timezone(&timezone)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn system_timezone() -> String {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|value| parse_timezone_name(&value).ok().map(str::to_string))
        .unwrap_or_else(|| FALLBACK_TIMEZONE.to_string())
}

fn parse_timezone_name(value: &str) -> Result<&str> {
    let value = value.trim();
    if parse_timezone(value).is_ok() {
        Ok(value)
    } else {
        Err(anyhow!("invalid IANA timezone: {value}"))
    }
}

fn parse_timezone(value: &str) -> Result<Tz> {
    value
        .parse::<Tz>()
        .map_err(|_| anyhow!("invalid IANA timezone: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_timezone_overrides_system_timezone() {
        assert_eq!(resolve_timezone(Some("UTC")).unwrap(), "UTC");
    }

    #[test]
    fn invalid_explicit_timezone_is_rejected() {
        assert!(resolve_timezone(Some("Not/AZone")).is_err());
    }

    #[test]
    fn default_timezone_uses_system_timezone_when_available() {
        let expected = iana_time_zone::get_timezone()
            .ok()
            .and_then(|value| parse_timezone_name(&value).ok().map(str::to_string))
            .unwrap_or_else(|| FALLBACK_TIMEZONE.to_string());

        assert_eq!(resolve_timezone(None).unwrap(), expected);
    }

    #[test]
    fn local_format_uses_explicit_timezone() {
        let datetime = DateTime::parse_from_rfc3339("2026-06-21T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(format_local(datetime, "UTC"), "2026-06-21T00:00:00Z");
        assert_eq!(
            format_local(datetime, "America/New_York"),
            "2026-06-20T20:00:00-04:00"
        );
    }
}
