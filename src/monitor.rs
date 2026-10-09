//! ping 結果の連続から「パケットロス発生」「復旧」を判定する状態機械。
//!
//! 毎回のロスで通知すると通知センターが埋まってしまうため、
//! 状態が切り替わったタイミングでのみイベントを発生させる。
//! 応答時間（遅延）の判定も同じ考え方で `LatencyMonitor` が行う。

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

#[derive(Debug, PartialEq, Eq)]
pub enum LatencyEvent {
    /// 応答時間が `consecutive` 回連続でしきい値を超えた
    High { rtt: Duration },
    /// 遅延状態から `consecutive` 回連続でしきい値以下に戻った。`duration` は遅延が続いた時間
    Normal { duration: Duration },
}

/// 応答時間の列から「遅延が大きい状態に入った」「戻った」を判定する状態機械。
///
/// しきい値付近で揺れたときに通知が往復しないよう、入るときも戻るときも
/// `consecutive` 回連続することを条件にしている。
/// ロスの判定は `Monitor` に任せ、ここには応答時間が分かった ping だけを渡す。
#[derive(Debug)]
pub struct LatencyMonitor {
    threshold: Duration,
    consecutive: u32,
    /// 今の状態と食い違う応答（正常時は遅い応答、遅延時は速い応答）が続いている回数
    streak: u32,
    streak_started: Option<Instant>,
    /// 遅延状態に入った時刻（遅い応答が続き始めた時刻）。`None` なら正常状態
    slow_since: Option<Instant>,
}

impl LatencyMonitor {
    pub fn new(threshold: Duration, consecutive: u32) -> Self {
        Self {
            threshold,
            consecutive: consecutive.max(1),
            streak: 0,
            streak_started: None,
            slow_since: None,
        }
    }

    /// 遅延状態（通知済みで未解消）かどうか
    pub fn is_slow(&self) -> bool {
        self.slow_since.is_some()
    }

    pub fn record(&mut self, rtt: Duration, now: Instant) -> Option<LatencyEvent> {
        if (rtt > self.threshold) == self.is_slow() {
            self.streak = 0;
            self.streak_started = None;
            return None;
        }

        self.streak += 1;
        let started = *self.streak_started.get_or_insert(now);
        if self.streak < self.consecutive {
            return None;
        }

        self.streak = 0;
        self.streak_started = None;
        match self.slow_since.take() {
            Some(since) => Some(LatencyEvent::Normal {
                duration: started.saturating_duration_since(since),
            }),
            None => {
                self.slow_since = Some(started);
                Some(LatencyEvent::High { rtt })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn latency_notifies_after_consecutive_slow_replies() {
        let t0 = Instant::now();
        let at = |s| t0 + Duration::from_secs(s);
        let mut m = LatencyMonitor::new(ms(200), 3);

        assert_eq!(m.record(ms(250), at(0)), None);
        assert_eq!(m.record(ms(300), at(1)), None);
        // 途中で速い応答があると数え直す
        assert_eq!(m.record(ms(20), at(2)), None);
        assert_eq!(m.record(ms(250), at(3)), None);
        assert_eq!(m.record(ms(260), at(4)), None);
        assert_eq!(
            m.record(ms(270), at(5)),
            Some(LatencyEvent::High { rtt: ms(270) })
        );
        assert!(m.is_slow());
        // 遅延が続いている間は繰り返さない
        assert_eq!(m.record(ms(400), at(6)), None);
    }

    #[test]
    fn latency_recovers_after_consecutive_fast_replies() {
        let t0 = Instant::now();
        let at = |s| t0 + Duration::from_secs(s);
        let mut m = LatencyMonitor::new(ms(200), 2);

        assert_eq!(m.record(ms(250), at(0)), None);
        assert_eq!(
            m.record(ms(250), at(1)),
            Some(LatencyEvent::High { rtt: ms(250) })
        );
        // 1 回だけ速くても戻らない
        assert_eq!(m.record(ms(20), at(2)), None);
        assert_eq!(m.record(ms(250), at(3)), None);
        assert_eq!(m.record(ms(20), at(4)), None);
        // 遅延の時間は、遅い応答が続き始めてから速い応答が続き始めるまで
        assert_eq!(
            m.record(ms(30), at(5)),
            Some(LatencyEvent::Normal {
                duration: Duration::from_secs(4)
            })
        );
        assert!(!m.is_slow());
    }

    #[test]
    fn latency_equal_to_threshold_is_not_slow() {
        let t0 = Instant::now();
        let mut m = LatencyMonitor::new(ms(200), 1);

        assert_eq!(m.record(ms(200), t0), None);
        assert_eq!(
            m.record(ms(201), t0),
            Some(LatencyEvent::High { rtt: ms(201) })
        );
    }

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
