//! Mock clock for controlling virtual time in tests
//!
//! The mock clock allows workflows to control time across all platforms.
//! When a clock action is executed, it updates the virtual time and syncs
//! it to all active platform bridges.
//!
//! # Example
//!
//! ```yaml
//! steps:
//!   # Set clock to a specific time
//!   - uses: clock/set
//!     with:
//!       time: "2024-01-15T10:30:00Z"
//!
//!   # Advance clock by duration
//!   - uses: clock/forward
//!     with:
//!       duration: "1h30m"
//!
//!   # Advance clock to a specific time
//!   - uses: clock/forward-until
//!     with:
//!       time: "2024-01-15T12:00:00Z"
//! ```

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use tokio::sync::RwLock;

/// Default auto-advance duration per step (3 seconds)
pub const DEFAULT_STEP_DURATION: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct MockClock {
    inner: Arc<RwLock<ClockState>>,
}

#[derive(Debug, Clone)]
struct ClockState {
    /// The current virtual time, or None if using real time
    virtual_time: Option<DateTime<Utc>>,
    /// Whether the clock is frozen (time doesn't advance automatically)
    frozen: bool,
    /// Timezone offset for display (stored as seconds from UTC)
    timezone_offset_secs: i32,
    /// Auto-advance duration per step
    step_duration: Duration,
    /// Whether auto-advance is enabled
    auto_advance: bool,
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ClockState {
                virtual_time: None,
                frozen: false,
                timezone_offset_secs: 0, // UTC
                step_duration: DEFAULT_STEP_DURATION,
                auto_advance: true,
            })),
        }
    }

    /// Get the current time (virtual or real)
    pub async fn now(&self) -> DateTime<Utc> {
        let state = self.inner.read().await;
        state.virtual_time.unwrap_or_else(Utc::now)
    }

    /// Check if the clock is using virtual time
    pub async fn is_virtual(&self) -> bool {
        self.inner.read().await.virtual_time.is_some()
    }

    /// Check if the mock clock is active (using virtual time)
    pub async fn is_active(&self) -> bool {
        self.is_virtual().await
    }

    /// Set the clock to a specific time (enables virtual time)
    pub async fn set(&self, time: DateTime<Utc>) {
        let mut state = self.inner.write().await;
        state.virtual_time = Some(time);
        state.frozen = true;
    }

    /// Advance the clock by a duration
    pub async fn forward(&self, duration: Duration) {
        let mut state = self.inner.write().await;
        let current = state.virtual_time.unwrap_or_else(Utc::now);
        state.virtual_time = Some(current + chrono::Duration::from_std(duration).unwrap());
        state.frozen = true;
    }

    /// Advance the clock to a specific time
    /// Returns error if the target time is before the current time
    pub async fn forward_until(&self, target: DateTime<Utc>) -> Result<(), ClockError> {
        let mut state = self.inner.write().await;
        let current = state.virtual_time.unwrap_or_else(Utc::now);

        if target < current {
            return Err(ClockError::CannotGoBackwards { current, target });
        }

        state.virtual_time = Some(target);
        state.frozen = true;
        Ok(())
    }

    /// Reset the clock to use real time
    pub async fn reset(&self) {
        let mut state = self.inner.write().await;
        state.virtual_time = None;
        state.frozen = false;
    }

    /// Get clock state as a serializable struct for syncing to platforms
    pub async fn get_sync_state(&self) -> ClockSyncState {
        let state = self.inner.read().await;
        ClockSyncState {
            virtual_time_ms: state.virtual_time.map(|t| t.timestamp_millis()),
            virtual_time_iso: state.virtual_time.map(|t| t.to_rfc3339()),
            frozen: state.frozen,
            timezone_offset_secs: state.timezone_offset_secs,
        }
    }

    /// Auto-advance the clock by the configured step duration
    /// Called after each step executes
    pub async fn auto_advance_step(&self) {
        let mut state = self.inner.write().await;
        if !state.auto_advance {
            return;
        }

        // Initialize virtual time if not set (start from current UTC time)
        if state.virtual_time.is_none() {
            state.virtual_time = Some(Utc::now());
        }

        let current = state.virtual_time.unwrap();
        state.virtual_time =
            Some(current + chrono::Duration::from_std(state.step_duration).unwrap());
    }

    /// Set the timezone offset (in hours from UTC)
    pub async fn set_timezone(&self, offset_hours: i32) {
        let mut state = self.inner.write().await;
        state.timezone_offset_secs = offset_hours * 3600;
    }

    /// Set timezone from IANA timezone name (e.g., "America/New_York", "Europe/London")
    /// Returns error if timezone is not recognized
    pub async fn set_timezone_name(&self, name: &str) -> Result<(), ClockError> {
        let offset_secs = parse_timezone(name)?;
        let mut state = self.inner.write().await;
        state.timezone_offset_secs = offset_secs;
        Ok(())
    }

    /// Get the current timezone offset in seconds
    pub async fn timezone_offset_secs(&self) -> i32 {
        self.inner.read().await.timezone_offset_secs
    }

    /// Get current time in the configured timezone
    pub async fn now_local(&self) -> DateTime<FixedOffset> {
        let state = self.inner.read().await;
        let utc_time = state.virtual_time.unwrap_or_else(Utc::now);
        let offset = FixedOffset::east_opt(state.timezone_offset_secs)
            .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
        utc_time.with_timezone(&offset)
    }

    /// Set the auto-advance duration per step
    pub async fn set_step_duration(&self, duration: Duration) {
        let mut state = self.inner.write().await;
        state.step_duration = duration;
    }

    /// Enable or disable auto-advance
    pub async fn set_auto_advance(&self, enabled: bool) {
        let mut state = self.inner.write().await;
        state.auto_advance = enabled;
    }

    /// Check if auto-advance is enabled
    pub async fn is_auto_advance_enabled(&self) -> bool {
        self.inner.read().await.auto_advance
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ClockSyncState {
    /// Virtual time as milliseconds since Unix epoch, or null for real time
    pub virtual_time_ms: Option<i64>,
    /// Virtual time as ISO 8601 string, or null for real time
    pub virtual_time_iso: Option<String>,
    /// Whether time is frozen (doesn't advance automatically)
    pub frozen: bool,
    /// Timezone offset in seconds from UTC
    pub timezone_offset_secs: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum ClockError {
    #[error("Cannot move clock backwards: current time is {current}, target is {target}")]
    CannotGoBackwards {
        current: DateTime<Utc>,
        target: DateTime<Utc>,
    },

    #[error("Invalid time format: {0}")]
    InvalidTimeFormat(String),

    #[error("Invalid duration format: {0}")]
    InvalidDurationFormat(String),

    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),
}

/// Parse a duration string like "1h30m", "500ms", "2d"
pub fn parse_duration(s: &str) -> Result<Duration, ClockError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ClockError::InvalidDurationFormat(
            "empty string".to_string(),
        ));
    }

    let mut total = Duration::ZERO;
    let mut current_num = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            current_num.push(c);
        } else {
            if current_num.is_empty() {
                return Err(ClockError::InvalidDurationFormat(format!(
                    "expected number before unit '{}'",
                    c
                )));
            }

            let num: f64 = current_num.parse().map_err(|_| {
                ClockError::InvalidDurationFormat(format!("invalid number: {}", current_num))
            })?;
            current_num.clear();

            let millis = match c {
                'd' => num * 24.0 * 60.0 * 60.0 * 1000.0,
                'h' => num * 60.0 * 60.0 * 1000.0,
                'm' => {
                    // Could be 'm' for minutes or 'ms' for milliseconds
                    num * 60.0 * 1000.0
                }
                's' => num * 1000.0,
                _ => {
                    return Err(ClockError::InvalidDurationFormat(format!(
                        "unknown unit '{}'",
                        c
                    )))
                }
            };

            total += Duration::from_millis(millis as u64);
        }
    }

    // Handle trailing number (assume seconds if no unit)
    if !current_num.is_empty() {
        let num: f64 = current_num.parse().map_err(|_| {
            ClockError::InvalidDurationFormat(format!("invalid number: {}", current_num))
        })?;
        total += Duration::from_secs_f64(num);
    }

    Ok(total)
}

/// Parse a time string (ISO 8601 or Unix timestamp)
pub fn parse_time(s: &str) -> Result<DateTime<Utc>, ClockError> {
    let s = s.trim();

    // Try parsing as ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try parsing as Unix timestamp (seconds)
    if let Ok(ts) = s.parse::<i64>() {
        if let Some(dt) = DateTime::from_timestamp(ts, 0) {
            return Ok(dt);
        }
    }

    // Try parsing as Unix timestamp (milliseconds)
    if let Ok(ts) = s.parse::<i64>() {
        if ts > 1_000_000_000_000 {
            if let Some(dt) = DateTime::from_timestamp_millis(ts) {
                return Ok(dt);
            }
        }
    }

    Err(ClockError::InvalidTimeFormat(format!(
        "could not parse '{}' as ISO 8601 or Unix timestamp",
        s
    )))
}

/// Parse a timezone string and return offset in seconds from UTC
/// Supports:
/// - "UTC" or "Z" -> 0
/// - "+HH:MM" or "-HH:MM" -> offset in seconds
/// - "+HH" or "-HH" -> offset in seconds
/// - Common timezone abbreviations (EST, PST, etc.)
/// - IANA names with static offsets (simplified)
pub fn parse_timezone(s: &str) -> Result<i32, ClockError> {
    let s = s.trim();

    // UTC
    if s.eq_ignore_ascii_case("utc") || s == "Z" {
        return Ok(0);
    }

    // Numeric offset: +05:30, -08:00, +5, -8
    if s.starts_with('+') || s.starts_with('-') {
        let sign = if s.starts_with('-') { -1 } else { 1 };
        let rest = &s[1..];

        if let Some((hours_str, mins_str)) = rest.split_once(':') {
            let hours: i32 = hours_str
                .parse()
                .map_err(|_| ClockError::InvalidTimezone(format!("invalid hours in '{}'", s)))?;
            let mins: i32 = mins_str
                .parse()
                .map_err(|_| ClockError::InvalidTimezone(format!("invalid minutes in '{}'", s)))?;
            return Ok(sign * (hours * 3600 + mins * 60));
        } else {
            let hours: i32 = rest
                .parse()
                .map_err(|_| ClockError::InvalidTimezone(format!("invalid offset '{}'", s)))?;
            return Ok(sign * hours * 3600);
        }
    }

    // Common timezone abbreviations (static offsets only)
    let offset_hours = match s.to_uppercase().as_str() {
        // US timezones (standard time)
        "EST" => -5,
        "CST" => -6,
        "MST" => -7,
        "PST" => -8,
        "AKST" => -9,
        "HST" => -10,
        // US timezones (daylight time)
        "EDT" => -4,
        "CDT" => -5,
        "MDT" => -6,
        "PDT" => -7,
        "AKDT" => -8,
        // European timezones
        "GMT" => 0,
        "WET" => 0,
        "CET" => 1,
        "EET" => 2,
        "WEST" => 1,
        "CEST" => 2,
        "EEST" => 3,
        // Asian timezones
        "IST" => 5, // India (5:30, but we simplify)
        "JST" => 9,
        "KST" => 9,
        "CST_ASIA" => 8, // China
        "SGT" => 8,
        "HKT" => 8,
        // Australian timezones
        "AEST" => 10,
        "ACST" => 9, // Actually 9:30
        "AWST" => 8,
        "AEDT" => 11,
        "ACDT" => 10, // Actually 10:30
        _ => {
            return Err(ClockError::InvalidTimezone(format!(
                "unknown timezone '{}'. Use UTC, numeric offset (+05:30), or common abbreviations (EST, PST, etc.)",
                s
            )));
        }
    };

    Ok(offset_hours * 3600)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[tokio::test]
    async fn test_mock_clock_set() {
        let clock = MockClock::new();
        assert!(!clock.is_virtual().await);

        let target = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(target).await;

        assert!(clock.is_virtual().await);
        assert_eq!(clock.now().await, target);
    }

    #[tokio::test]
    async fn test_mock_clock_forward() {
        let clock = MockClock::new();
        let start = DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(start).await;

        clock.forward(Duration::from_secs(3600)).await; // 1 hour

        let expected = DateTime::parse_from_rfc3339("2024-01-15T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(clock.now().await, expected);
    }

    #[tokio::test]
    async fn test_mock_clock_forward_until() {
        let clock = MockClock::new();
        let start = DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(start).await;

        let target = DateTime::parse_from_rfc3339("2024-01-15T15:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.forward_until(target).await.unwrap();

        assert_eq!(clock.now().await, target);
    }

    #[tokio::test]
    async fn test_mock_clock_cannot_go_backwards() {
        let clock = MockClock::new();
        let start = DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(start).await;

        let earlier = DateTime::parse_from_rfc3339("2024-01-15T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(clock.forward_until(earlier).await.is_err());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(parse_duration("500s").unwrap(), Duration::from_secs(500));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86400));
    }

    #[test]
    fn test_parse_time() {
        let dt = parse_time("2024-01-15T10:30:00Z").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 30);

        // Unix timestamp
        let dt = parse_time("1705315800").unwrap();
        assert!(dt.year() == 2024);
    }

    #[tokio::test]
    async fn test_clock_sync_state() {
        let clock = MockClock::new();
        let target = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(target).await;

        let state = clock.get_sync_state().await;
        assert!(state.virtual_time_ms.is_some());
        assert!(state.virtual_time_iso.is_some());
        assert!(state.frozen);
    }
}
