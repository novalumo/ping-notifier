//! ping 結果の連続から「パケットロス発生」「復旧」を判定する状態機械。
//!
//! 毎回のロスで通知すると通知センターが埋まってしまうため、
//! 状態が切り替わったタイミングでのみイベントを発生させる。

use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// `threshold` 回連続でタイムアウトした
    Lost { consecutive: u32 },
    /// ロス状態から応答が戻った
    Recovered { lost: u32, downtime: Duration },
}

#[derive(Debug)]
pub struct Monitor {
    threshold: u32,
    consecutive_failures: u32,
    /// ロス状態に入った時刻（最初のタイムアウト時刻）。`None` なら正常状態
    outage_started: Option<Instant>,
    first_failure_at: Option<Instant>,
}

impl Monitor {
    pub fn new(threshold: u32) -> Self {
        Self {
            threshold: threshold.max(1),
            consecutive_failures: 0,
            outage_started: None,
            first_failure_at: None,
        }
    }

    /// ロス状態（通知済みで未復旧）かどうか
    pub fn is_down(&self) -> bool {
        self.outage_started.is_some()
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    pub fn record(&mut self, success: bool, now: Instant) -> Option<Event> {
        if success {
            let lost = self.consecutive_failures;
            self.consecutive_failures = 0;
            self.first_failure_at = None;
            return self.outage_started.take().map(|started| Event::Recovered {
                lost,
                downtime: now.saturating_duration_since(started),
            });
        }

        self.consecutive_failures += 1;
        self.first_failure_at.get_or_insert(now);
        if self.outage_started.is_none() && self.consecutive_failures >= self.threshold {
            self.outage_started = self.first_failure_at;
            return Some(Event::Lost {
                consecutive: self.consecutive_failures,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifies_once_per_outage_and_on_recovery() {
        let t0 = Instant::now();
        let at = |s| t0 + Duration::from_secs(s);
        let mut m = Monitor::new(2);

        assert_eq!(m.record(true, at(0)), None);
        assert_eq!(m.record(false, at(1)), None);
        assert_eq!(m.record(false, at(2)), Some(Event::Lost { consecutive: 2 }));
        assert_eq!(m.record(false, at(3)), None);
        assert_eq!(
            m.record(true, at(4)),
            Some(Event::Recovered {
                lost: 3,
                downtime: Duration::from_secs(3)
            })
        );
        assert_eq!(m.record(true, at(5)), None);
    }

    #[test]
    fn failures_below_threshold_are_ignored() {
        let t0 = Instant::now();
        let mut m = Monitor::new(3);

        assert_eq!(m.record(false, t0), None);
        assert_eq!(m.record(false, t0), None);
        assert_eq!(m.record(true, t0), None);
        assert_eq!(m.record(false, t0), None);
    }

    #[test]
    fn threshold_one_notifies_immediately() {
        let t0 = Instant::now();
        let mut m = Monitor::new(1);

        assert_eq!(m.record(false, t0), Some(Event::Lost { consecutive: 1 }));
    }
}
