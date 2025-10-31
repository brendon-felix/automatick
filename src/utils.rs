use ticks::tasks::Task;

use crate::tasks;

/// Parse date in US format (MM/DD or MM/DD/YYYY) or ISO format (YYYY-MM-DD)
/// If year is not provided, uses current year or next year for valid future dates
pub fn parse_date_us_format(date_str: &str) -> Result<chrono::NaiveDate, String> {
    use chrono::{Datelike, Local, NaiveDate};

    let date_str = date_str.trim();

    // Try ISO format first (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Ok(date);
    }

    // Determine separator (/ or -)
    let separator = if date_str.contains('/') {
        '/'
    } else if date_str.contains('-') {
        '-'
    } else {
        return Err(format!(
            "Invalid date format. Use MM/DD, MM/DD/YYYY, MM-DD, MM-DD-YY, or YYYY-MM-DD"
        ));
    };

    let parts: Vec<&str> = date_str.split(separator).collect();

    let (month, day, year_opt) = match parts.len() {
        2 => {
            // MM/DD or MM-DD (no year provided)
            let month = parts[0]
                .parse::<u32>()
                .map_err(|_| format!("Invalid month: {}", parts[0]))?;
            let day = parts[1]
                .parse::<u32>()
                .map_err(|_| format!("Invalid day: {}", parts[1]))?;
            (month, day, None)
        }
        3 => {
            // MM/DD/YYYY or MM/DD/YY or MM-DD-YYYY or MM-DD-YY
            let month = parts[0]
                .parse::<u32>()
                .map_err(|_| format!("Invalid month: {}", parts[0]))?;
            let day = parts[1]
                .parse::<u32>()
                .map_err(|_| format!("Invalid day: {}", parts[1]))?;
            let year_part = parts[2]
                .parse::<i32>()
                .map_err(|_| format!("Invalid year: {}", parts[2]))?;

            // Handle 2-digit years
            let year = if year_part < 100 {
                // Assume 2000s for 2-digit years
                2000 + year_part
            } else {
                year_part
            };

            (month, day, Some(year))
        }
        _ => {
            return Err(format!(
                "Invalid date format. Use MM/DD, MM/DD/YYYY, MM-DD, MM-DD-YY, or YYYY-MM-DD"
            ));
        }
    };

    // Validate month and day ranges
    if month < 1 || month > 12 {
        return Err(format!("Month must be between 1 and 12"));
    }
    if day < 1 || day > 31 {
        return Err(format!("Day must be between 1 and 31"));
    }

    // If no year provided, determine current or next year for future date
    let year = if let Some(y) = year_opt {
        y
    } else {
        let today = Local::now().date_naive();
        let current_year = today.year();

        // Try current year first
        if let Some(date) = NaiveDate::from_ymd_opt(current_year, month, day) {
            if date >= today {
                current_year
            } else {
                // Date has passed this year, use next year
                current_year + 1
            }
        } else {
            // Invalid date for current year (e.g., Feb 30), try next year
            current_year + 1
        }
    };

    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("Invalid date: {}/{}/{}", month, day, year))
}

/// Parse time in US format (12-hour with AM/PM)
pub fn parse_time_us_format(time_str: &str) -> Result<chrono::NaiveTime, String> {
    use chrono::NaiveTime;

    let time_str = time_str.trim().to_lowercase();

    // Check if it contains AM or PM
    let (is_pm, time_without_suffix) = if time_str.ends_with("pm") {
        (true, time_str.trim_end_matches("pm").trim())
    } else if time_str.ends_with("am") {
        (false, time_str.trim_end_matches("am").trim())
    } else {
        // Try 24-hour format as fallback
        return NaiveTime::parse_from_str(&time_str, "%H:%M").map_err(|_| {
            format!("Invalid time format. Use formats like '5pm', '5:30 AM', or '17:00'")
        });
    };

    // Parse hour and optional minutes
    let parts: Vec<&str> = time_without_suffix.split(':').collect();

    let (hour_12, minute) = if parts.len() == 1 {
        // No colon, just hour (e.g., "5pm")
        let hour = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid hour: {}", parts[0]))?;
        (hour, 0)
    } else if parts.len() == 2 {
        // Hour and minute (e.g., "5:30pm")
        let hour = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid hour: {}", parts[0]))?;
        let minute = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("Invalid minute: {}", parts[1]))?;
        (hour, minute)
    } else {
        return Err(format!(
            "Invalid time format. Use formats like '5pm' or '5:30 AM'"
        ));
    };

    // Validate ranges
    if hour_12 < 1 || hour_12 > 12 {
        return Err(format!("Hour must be between 1 and 12"));
    }
    if minute > 59 {
        return Err(format!("Minute must be between 0 and 59"));
    }

    // Convert to 24-hour format
    let hour_24 = if hour_12 == 12 {
        if is_pm {
            12
        } else {
            0
        }
    } else {
        if is_pm {
            hour_12 + 12
        } else {
            hour_12
        }
    };

    NaiveTime::from_hms_opt(hour_24, minute, 0)
        .ok_or_else(|| format!("Invalid time: {}:{:02}", hour_24, minute))
}

pub async fn delete_task(task: Task) -> Result<(), String> {
    tasks::delete_task(task).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_us_format() {
        // Test PM times
        assert_eq!(
            parse_time_us_format("5pm").unwrap(),
            chrono::NaiveTime::from_hms_opt(17, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("5PM").unwrap(),
            chrono::NaiveTime::from_hms_opt(17, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("5:30pm").unwrap(),
            chrono::NaiveTime::from_hms_opt(17, 30, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("12pm").unwrap(),
            chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("12:45 pm").unwrap(),
            chrono::NaiveTime::from_hms_opt(12, 45, 0).unwrap()
        );

        // Test AM times
        assert_eq!(
            parse_time_us_format("5am").unwrap(),
            chrono::NaiveTime::from_hms_opt(5, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("5:30 AM").unwrap(),
            chrono::NaiveTime::from_hms_opt(5, 30, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("12am").unwrap(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("12:30 AM").unwrap(),
            chrono::NaiveTime::from_hms_opt(0, 30, 0).unwrap()
        );

        // Test 24-hour format fallback
        assert_eq!(
            parse_time_us_format("17:00").unwrap(),
            chrono::NaiveTime::from_hms_opt(17, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time_us_format("09:30").unwrap(),
            chrono::NaiveTime::from_hms_opt(9, 30, 0).unwrap()
        );

        // Test invalid formats
        assert!(parse_time_us_format("25pm").is_err());
        assert!(parse_time_us_format("13pm").is_err());
        assert!(parse_time_us_format("5:70pm").is_err());
        assert!(parse_time_us_format("0am").is_err());
        assert!(parse_time_us_format("invalid").is_err());
    }
}
