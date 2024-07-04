use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

#[derive(Debug)]
pub struct FpsCounter {
    buffer: Mutex<VecDeque<Instant>>,
    fps: AtomicU64,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self {
            // TODO: Replace with im::Vector to get rid of lock
            buffer: Mutex::new(VecDeque::with_capacity(100)),
            fps: AtomicU64::new(0),
        }
    }
}

impl FpsCounter {
    pub fn increment(&self) {
        let mut buf = self.buffer.lock().expect("failed to lock");
        buf.push_front(Instant::now());

        let cap = buf.capacity();
        buf.truncate(cap);

        // At least two data points are needed for an FPS indication.
        if buf.len() < 2 {
            return;
        }

        let mut total_duration = Duration::new(0, 0);
        let mut count = 0;

        for pair in buf.iter().zip(buf.iter().skip(1)) {
            let (first, second) = pair;
            total_duration += second.duration_since(*first);
            count += 1;
        }

        let average_duration = total_duration / count as u32;

        // Construct a time code where the upper 32 bits are seconds and the lower 32 bits are fractional nanoseconds.
        let time = Self::encode(average_duration);
        self.fps.store(time, Ordering::SeqCst);
    }

    pub fn fps(&self) -> Duration {
        let value = self.fps.load(Ordering::SeqCst);
        Self::decode(value)
    }

    fn encode(duration: Duration) -> u64 {
        let seconds = duration.as_secs().min(u32::MAX as _) as u32;
        let sub_nanos = duration.subsec_nanos();

        // Construct a time code where the upper 32 bits are seconds and the lower 32 bits are fractional nanoseconds.
        ((seconds as u64) << 32) | (sub_nanos as u64) & 0xFFFF_FFFF
    }

    fn decode(code: u64) -> Duration {
        let seconds = (code >> 32) & 0xFFFF_FFFF;
        let sub_nanos = code & 0xFFFF_FFFF;
        Duration::new(seconds, sub_nanos as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_encoding() {
        let duration = Duration::from_secs_f64(1.3781738212323123);
        let code = FpsCounter::encode(duration);
        let decoded_duration = FpsCounter::decode(code);
        assert_eq!(duration, decoded_duration);
    }
}
