use ::std::borrow::Cow;

use ::chrono::{DateTime, TimeDelta, Local, TimeZone as _, Utc};

/// 获取下一个整千秒的时间点
fn next_thousand_second_mark() -> DateTime<Utc> {
    let now = Utc::now();
    let timestamp = now.timestamp();
    let current_thousand = timestamp / 1000;
    let next_thousand_timestamp = (current_thousand + 1) * 1000;

    Utc.timestamp_opt(next_thousand_timestamp, 0)
        .single()
        .expect("valid timestamp")
}

/// 格式化剩余时间为人类可读格式
fn format_duration(duration: TimeDelta) -> Cow<'static, str> {
    let total_seconds = duration.num_seconds();

    if total_seconds <= 0 {
        return Cow::Borrowed("已到达");
    }

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    Cow::Owned(if hours > 0 {
        format!("{hours} 小时 {minutes} 分钟 {seconds} 秒")
    } else if minutes > 0 {
        format!("{minutes} 分钟 {seconds} 秒")
    } else {
        format!("{seconds} 秒")
    })
}

fn main() {
    let now_local = Local::now();
    let next_mark_utc = next_thousand_second_mark();
    let next_mark_local = next_mark_utc.with_timezone(&Local);

    let remaining = next_mark_utc - Utc::now();

    println!("当前时间: {}", now_local.format("%Y-%m-%d %H:%M:%S"));
    println!(
        "下一个整千秒时刻: {}",
        next_mark_local.format("%Y-%m-%d %H:%M:%S")
    );
    println!("距离下一个整千秒还有: {}", format_duration(remaining));

    println!("\n详细信息:");
    println!("- 当前时间戳: {}", now_local.timestamp());
    println!("- 下一个整千秒时间戳: {}", next_mark_utc.timestamp());
}
