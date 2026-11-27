//! The TV's offset from UTC, kept current while Lounge runs for weeks as Home: daylight saving
//! starts or ends, or someone changes the time zone in the TV's Settings. The `time` crate reads
//! the local offset only while a single thread runs (it is unsound on Unix otherwise), so after
//! start-up a worker asks the system (`date +%z`) every two minutes instead.
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use time::UtcOffset;

static SECONDS: AtomicI32 = AtomicI32::new(0);
const EVERY: Duration = Duration::from_secs(120);

/// Starts from the offset read at start-up, then follows the system's.
pub fn start(initial: UtcOffset) {
    SECONDS.store(initial.whole_seconds(), Ordering::Relaxed);
    let _ = std::thread::Builder::new()
        .name("local-offset".into())
        .spawn(|| {
            loop {
                std::thread::sleep(EVERY);
                if let Some(offset) = read() {
                    SECONDS.store(offset.whole_seconds(), Ordering::Relaxed);
                }
            }
        });
}

/// The offset clocks show the local time in, as of the last check.
pub fn offset() -> UtcOffset {
    UtcOffset::from_whole_seconds(SECONDS.load(Ordering::Relaxed)).unwrap_or(UtcOffset::UTC)
}

fn read() -> Option<UtcOffset> {
    let output = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()?;
    parse(std::str::from_utf8(&output.stdout).ok()?.trim())
}

/// "+0530", "-0700": what `date +%z` prints. Anything else is ignored, keeping the last offset.
fn parse(text: &str) -> Option<UtcOffset> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = digits[2..].parse().ok()?;
    if hours > 14 || minutes > 59 {
        return None;
    }
    UtcOffset::from_whole_seconds(sign * (hours * 3600 + minutes * 60)).ok()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_what_date_prints() {
        assert_eq!(parse("+0530").unwrap().whole_seconds(), 19800);
        assert_eq!(parse("-0700").unwrap().whole_seconds(), -25200);
        assert_eq!(parse("+0000").unwrap().whole_seconds(), 0);
        for wrong in ["", "0530", "+530", "+05:30", "+2500", "+05x0", "UTC"] {
            assert!(parse(wrong).is_none(), "{wrong}");
        }
    }
}
