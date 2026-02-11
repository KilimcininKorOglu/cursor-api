use ::std::borrow::Cow;

use ::chrono::{DateTime, TimeDelta, Local, TimeZone as _, Utc};

/// Get the next thousand-second mark time point
fn next_thousand_second_mark() -> DateTime<Utc> {
    let now = Utc::now();
    let timestamp = now.timestamp();
    let current_thousand = timestamp / 1000;
    let next_thousand_timestamp = (current_thousand + 1) * 1000;

    Utc.timestamp_opt(next_thousand_timestamp, 0)
        .single()
        .expect("valid timestamp")
}

/// Format remaining time to human-readable format
fn format_duration(duration: TimeDelta) -> Cow<'static, str> {
    let total_seconds = duration.num_seconds();

    if total_seconds <= 0 {
        return Cow::Borrowed("Reached");
    }

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    Cow::Owned(if hours > 0 {
        format!("{hours} hours {minutes} minutes {seconds} seconds")
    } else if minutes > 0 {
        format!("{minutes} minutes {seconds} seconds")
    } else {
        format!("{seconds} seconds")
    })
}

fn main() {
    let now_local = Local::now();
    let next_mark_utc = next_thousand_second_mark();
    let next_mark_local = next_mark_utc.with_timezone(&Local);

    let remaining = next_mark_utc - Utc::now();

    println!("Current time: {}", now_local.format("%Y-%m-%d %H:%M:%S"));
    println!(
        "Next thousand-second mark: {}",
        next_mark_local.format("%Y-%m-%d %H:%M:%S")
    );
    println!("Time until next thousand-second: {}", format_duration(remaining));

    println!("\nDetails:");
    println!("- Current timestamp: {}", now_local.timestamp());
    println!("- Next thousand-second timestamp: {}", next_mark_utc.timestamp());
}
