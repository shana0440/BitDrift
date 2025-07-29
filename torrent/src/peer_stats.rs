use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct PeerStats {
    upload: ThroughputRate,
    download: ThroughputRate,
    window: Duration,
}

impl PeerStats {
    pub fn new(window_secs: u64) -> Self {
        Self {
            upload: ThroughputRate::new(window_secs),
            download: ThroughputRate::new(window_secs),
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn record_upload(&mut self, bytes: usize) {
        self.upload.record(bytes);
    }

    pub fn record_download(&mut self, bytes: usize) {
        self.download.record(bytes);
    }

    pub fn upload_rate(&self) -> f64 {
        self.upload.rate()
    }

    pub fn download_rate(&self) -> f64 {
        self.download.rate()
    }
}

struct ThroughputRate {
    log: VecDeque<(Instant, usize)>,
    window: Duration,
}

impl ThroughputRate {
    fn new(window_secs: u64) -> Self {
        Self {
            log: VecDeque::new(),
            window: Duration::from_secs(window_secs),
        }
    }

    fn record(&mut self, bytes: usize) {
        self.log.push_back((Instant::now(), bytes));
        self.cleanup_log();
    }

    fn rate(&self) -> f64 {
        let total: usize = self.log.iter().map(|&(_, b)| b).sum();
        let secs = self.window.as_secs_f64();
        total as f64 / secs
    }

    fn cleanup_log(&mut self) {
        let now = Instant::now();
        while let Some(&(t, _)) = self.log.front() {
            if now.duration_since(t) > self.window {
                self.log.pop_front();
            } else {
                break;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_transfer_rate_record_and_rate() {
        let mut transfer_rate = ThroughputRate::new(2); // 2 second window

        // Initially, rate should be 0
        assert_eq!(transfer_rate.rate(), 0.0);

        // Record 1000 bytes
        transfer_rate.record(1000);
        let rate1 = transfer_rate.rate();
        assert!(rate1 > 0.0);

        // Record another 1000 bytes
        transfer_rate.record(1000);
        let rate2 = transfer_rate.rate();
        assert!(rate2 > rate1);

        // Wait for more than the window, old records should be cleaned up
        sleep(Duration::from_secs(3));
        let rate3 = transfer_rate.rate();
        assert_eq!(rate3, 0.0);
    }

    #[test]
    fn test_transfer_rate_partial_window() {
        let mut transfer_rate = ThroughputRate::new(4); // 4 second window

        transfer_rate.record(400);
        sleep(Duration::from_secs(2));
        transfer_rate.record(600);

        // Both records should be counted
        let rate = transfer_rate.rate();
        assert!((rate - 250.0).abs() < 1e-6); // (400+600)/4 = 250

        sleep(Duration::from_secs(3));
        // Now only the second record should be counted
        let rate = transfer_rate.rate();
        assert!((rate - 150.0).abs() < 1e-6); // 600/4 = 150

        sleep(Duration::from_secs(2));
        // All records should be expired
        let rate = transfer_rate.rate();
        assert_eq!(rate, 0.0);
    }
}
