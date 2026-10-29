//! Property-based tests for timezone conversion (Property 3).
//!
//! **Property 3 – Timezone Conversion Correctness**
//!
//! *For any* valid date string with timezone offset and any valid IANA
//! timezone identifier, converting the date to the target timezone SHALL
//! produce a correctly converted ISO 8601 date string (without timezone
//! suffix).
//!
//! **Validates: Requirements 2.2.1, 2.2.4**

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use proptest::prelude::*;
use spage_engine::timezone::{convert_to_timezone, has_timezone_offset};

// ── Strategies ─────────────────────────────────────────────────────

/// A representative set of valid IANA timezone identifiers covering
/// positive, negative, and zero UTC offsets plus DST-observing zones.
const TIMEZONES: &[&str] = &[
    "UTC",
    "Asia/Tokyo",
    "Asia/Shanghai",
    "Asia/Kolkata",
    "Europe/London",
    "Europe/Berlin",
    "Europe/Moscow",
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "America/Sao_Paulo",
    "Australia/Sydney",
    "Pacific/Auckland",
    "Pacific/Honolulu",
];

fn valid_timezone() -> impl Strategy<Value = &'static str> {
    prop::sample::select(TIMEZONES)
}

/// Generate a valid date component (year, month, day, hour, minute, second).
fn date_components() -> impl Strategy<Value = (i32, u32, u32, u32, u32, u32)> {
    (
        2000..2030i32,  // year
        1..=12u32,      // month
        1..=28u32,      // day (capped at 28 to avoid invalid dates)
        0..=23u32,      // hour
        0..=59u32,      // minute
        0..=59u32,      // second
    )
}

/// Generate a UTC offset in the range -12:00..+14:00 (valid IANA range).
fn offset_hours_minutes() -> impl Strategy<Value = (i32, i32)> {
    (-12..=14i32, prop::sample::select(&[0i32, 15, 30, 45]))
}

/// Build a date string with an explicit timezone offset.
///
/// Produces strings like `2025-01-15T10:30:00+09:00`.
fn date_with_offset() -> impl Strategy<Value = String> {
    (date_components(), offset_hours_minutes()).prop_map(
        |((y, mo, d, h, mi, s), (oh, om))| {
            let sign = if oh >= 0 { '+' } else { '-' };
            let abs_h = oh.unsigned_abs();
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
                y, mo, d, h, mi, s, sign, abs_h, om
            )
        },
    )
}

/// Build a date string ending with `Z` (UTC shorthand).
fn date_with_z() -> impl Strategy<Value = String> {
    date_components().prop_map(|(y, mo, d, h, mi, s)| {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
    })
}

/// Any date string that carries an explicit offset (either `±HH:MM` or `Z`).
fn date_string_with_offset() -> impl Strategy<Value = String> {
    prop_oneof![date_with_offset(), date_with_z(),]
}

// ── Helpers ────────────────────────────────────────────────────────

/// ISO 8601 regex without timezone suffix: `YYYY-MM-DDTHH:MM:SS`
fn is_iso_no_tz(s: &str) -> bool {
    if s.len() != 19 {
        return false;
    }
    let b = s.as_bytes();
    // YYYY-MM-DDTHH:MM:SS
    b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
        && b[11..13].iter().all(|c| c.is_ascii_digit())
        && b[14..16].iter().all(|c| c.is_ascii_digit())
        && b[17..19].iter().all(|c| c.is_ascii_digit())
}

/// Parse the output back into a NaiveDateTime for numeric comparison.
fn parse_output(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .unwrap_or_else(|e| panic!("failed to parse output {s:?}: {e}"))
}

// ── Property Tests ─────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // ── P3.1: Output is always valid ISO 8601 without tz suffix ────
    //
    // For any valid date-with-offset and any valid timezone, the output
    // must be exactly `YYYY-MM-DDTHH:MM:SS` (19 chars, no suffix).
    #[test]
    fn output_is_iso8601_no_tz_suffix(
        date in date_string_with_offset(),
        tz in valid_timezone(),
    ) {
        let result = convert_to_timezone(&date, tz);
        // The generator always produces parseable dates, so result
        // should never be empty.
        prop_assert!(!result.is_empty(), "unexpected empty result for date={date:?} tz={tz}");
        prop_assert!(
            is_iso_no_tz(&result),
            "output {result:?} is not ISO 8601 without tz suffix (date={date:?} tz={tz})"
        );
        // Must NOT end with Z or ±offset
        prop_assert!(!result.ends_with('Z'));
        prop_assert!(!has_timezone_offset(&result));
    }

    // ── P3.2: Conversion preserves the instant ─────────────────────
    //
    // Converting a date-with-offset to a timezone and then interpreting
    // the result in that timezone must represent the same UTC instant
    // as the original.
    #[test]
    fn conversion_preserves_instant(
        date in date_string_with_offset(),
        tz_name in valid_timezone(),
    ) {
        let result = convert_to_timezone(&date, tz_name);
        prop_assert!(!result.is_empty());

        // Parse the original date to get its UTC instant.
        let original_utc: DateTime<Utc> = {
            // Try RFC 3339 first, then manual formats.
            if let Ok(dt) = DateTime::parse_from_rfc3339(&date) {
                dt.with_timezone(&Utc)
            } else {
                let formats = [
                    "%Y-%m-%dT%H:%M:%S%:z",
                    "%Y-%m-%d %H:%M:%S%:z",
                    "%Y-%m-%dT%H:%M:%S%z",
                    "%Y-%m-%d %H:%M:%S%z",
                ];
                let mut parsed = None;
                for fmt in formats {
                    if let Ok(dt) = DateTime::parse_from_str(&date, fmt) {
                        parsed = Some(dt.with_timezone(&Utc));
                        break;
                    }
                }
                match parsed {
                    Some(dt) => dt,
                    None => {
                        // If we can't parse the original, skip this case.
                        return Ok(());
                    }
                }
            }
        };

        // Parse the output as a naive datetime, then interpret it in
        // the target timezone to recover the UTC instant.
        let naive_out = parse_output(&result);
        let tz: Tz = tz_name.parse().unwrap();

        // Use the earliest matching local time (handles DST ambiguity).
        let recovered_utc = match tz.from_local_datetime(&naive_out) {
            chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
            chrono::LocalResult::Ambiguous(earliest, latest) => {
                // The output is a naive local time, so a DST fold is inherently lossy: the same
                // wall clock maps to two instants. Either side is a correct round-trip.
                prop_assert!(
                    original_utc == earliest.with_timezone(&Utc)
                        || original_utc == latest.with_timezone(&Utc),
                    "ambiguous local time matched neither offset: date={:?} tz={} result={:?}",
                    date,
                    tz_name,
                    result
                );
                return Ok(());
            }
            chrono::LocalResult::None => {
                // DST gap — the local time doesn't exist. Skip.
                return Ok(());
            }
        };

        prop_assert_eq!(
            original_utc, recovered_utc,
            "instant mismatch: date={:?} tz={} result={:?}", date, tz_name, result
        );
    }

    // ── P3.3: Invalid timezone falls back to UTC ───────────────────
    //
    // When the timezone identifier is invalid, the function must still
    // produce a valid ISO 8601 output that equals the UTC interpretation
    // of the input.
    #[test]
    fn invalid_timezone_falls_back_to_utc(
        date in date_string_with_offset(),
    ) {
        let result = convert_to_timezone(&date, "Invalid/Timezone_XYZ");
        prop_assert!(!result.is_empty());
        prop_assert!(is_iso_no_tz(&result));

        // The result should equal converting to UTC.
        let utc_result = convert_to_timezone(&date, "UTC");
        prop_assert_eq!(
            result, utc_result,
            "invalid tz should fall back to UTC: date={:?}", date
        );
    }

    // ── P3.4: No-offset dates pass through unchanged ───────────────
    //
    // Dates without a timezone offset are treated as UTC and output
    // as-is (no conversion), regardless of the target timezone.
    #[test]
    fn no_offset_passthrough(
        (y, mo, d, h, mi, s) in date_components(),
        tz in valid_timezone(),
    ) {
        let naive_input = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            y, mo, d, h, mi, s
        );
        let result = convert_to_timezone(&naive_input, tz);
        // Output should be the same datetime, just formatted.
        prop_assert_eq!(
            result, naive_input.clone(),
            "no-offset date should pass through: input={:?} tz={}", naive_input, tz
        );
    }

    // ── P3.5: Same timezone is identity ────────────────────────────
    //
    // Converting a date whose offset matches the target timezone should
    // produce the same local time as the input.
    #[test]
    fn same_offset_is_identity(
        (y, mo, d, h, mi, s) in date_components(),
    ) {
        // Build a date at +09:00 and convert to Asia/Tokyo (which is
        // always +09:00, no DST).
        let input = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+09:00",
            y, mo, d, h, mi, s
        );
        let expected = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            y, mo, d, h, mi, s
        );
        let result = convert_to_timezone(&input, "Asia/Tokyo");
        prop_assert_eq!(
            result, expected,
            "same-offset conversion should be identity"
        );
    }

    // ── P3.6: Empty / whitespace input returns empty ───────────────
    #[test]
    fn empty_input_returns_empty(
        tz in valid_timezone(),
    ) {
        prop_assert_eq!(convert_to_timezone("", tz), "");
        prop_assert_eq!(convert_to_timezone("   ", tz), "");
    }
}
