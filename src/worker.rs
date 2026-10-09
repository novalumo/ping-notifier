//! バックグラウンドで ping を打ち続け、状態の変化を UI スレッドへ知らせる。

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::i18n::{Lang, Msg, t};
use crate::monitor::{Event, LatencyEvent, LatencyMonitor, Monitor};
use crate::{log, notifier, ping};

/// UI スレッドから監視スレッドへの指示
#[derive(Debug)]
pub enum Command {
    SetPaused(bool),
    Reload(Config),
}

/// 監視スレッドから UI スレッドへ知らせる現在の状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Paused,
    Up {
        host: String,
        /// 直近の応答時間（`ping` の出力から読み取れなかった場合は `None`）
        rtt: Option<Duration>,
        /// 応答時間がしきい値を超える状態が続いている
        slow: bool,
    },
    Down {
        host: String,
        consecutive: u32,
    },
    /// ping コマンド自体を実行できない
    Error {
        host: String,
        message: String,
    },
}

pub fn spawn(config: Config, on_status: impl Fn(Status) + Send + 'static) -> Sender<Command> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("ping-worker".into())
        .spawn(move || run(config, &rx, &on_status))
        .expect("failed to spawn the ping worker thread");
    tx
}

fn run(mut config: Config, rx: &Receiver<Command>, on_status: &dyn Fn(Status)) {
    let mut monitors = Monitors::new(&config);
    let mut paused = false;

    loop {
        let started = Instant::now();

        if paused {
            on_status(Status::Paused);
            // 一時停止中は指示が来るまで待つ
            match rx.recv() {
                Ok(cmd) => apply(cmd, &mut config, &mut monitors, &mut paused),
                Err(_) => return,
            }
            continue;
        }

        on_status(check(&config, &mut monitors));

        // 次の ping までの間も指示を受け付け、届いたらすぐ反映する
        let remaining = (started + config.interval()).saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(cmd) => apply(cmd, &mut config, &mut monitors, &mut paused),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// ロスと遅延の判定の状態
struct Monitors {
    loss: Monitor,
    /// 遅延の通知が無効（`latency_threshold_ms` 未設定）なら `None`
    latency: Option<LatencyMonitor>,
}

impl Monitors {
    fn new(config: &Config) -> Self {
        Self {
            loss: Monitor::new(config.threshold),
            latency: latency_monitor(config),
        }
    }
}

fn latency_monitor(config: &Config) -> Option<LatencyMonitor> {
    let threshold = config.latency_threshold()?;
    Some(LatencyMonitor::new(threshold, config.latency_consecutive))
}

fn apply(cmd: Command, config: &mut Config, monitors: &mut Monitors, paused: &mut bool) {
    match cmd {
        Command::SetPaused(p) => *paused = p,
        Command::Reload(new) => *config = new,
    }
    // 設定や一時停止の切り替えをまたいでロス回数や遅延の状態を持ち越さない
    *monitors = Monitors::new(config);
}

fn check(config: &Config, monitors: &mut Monitors) -> Status {
    let host = config.host.clone();
    let reply = match ping::ping_once(&host, config.timeout()) {
        Ok(reply) => reply,
        Err(e) => {
            let message = format!("cannot run ping: {e}");
            log(&message);
            return Status::Error { host, message };
        }
    };

    if reply.is_none() {
        log(&format!("{host}: timeout"));
    }

    let now = Instant::now();
    match monitors.loss.record(reply.is_some(), now) {
        Some(Event::Lost { consecutive }) => {
            let body = Msg::LostBody {
                host: &host,
                consecutive,
            };
            log(&format!("[lost] {}", body.in_lang(Lang::En)));
            notifier::notify(&t(Msg::LostTitle), &t(body));
            // ロスを優先する。遅延の状態は通知せずに捨て、復旧後に判定し直す
            monitors.latency = latency_monitor(config);
        }
        Some(Event::Recovered { lost, downtime }) => {
            let body = Msg::RecoveredBody {
                host: &host,
                lost,
                secs: downtime.as_secs(),
            };
            log(&format!("[recovered] {}", body.in_lang(Lang::En)));
            if config.notify_recovery {
                notifier::notify(&t(Msg::RecoveredTitle), &t(body));
            }
        }
        None => {}
    }

    if monitors.loss.is_down() {
        return Status::Down {
            host,
            consecutive: monitors.loss.consecutive_failures(),
        };
    }

    let rtt = reply.and_then(|r| r.rtt);
    // 応答時間が分からない ping（タイムアウトや出力の読み取り失敗）は遅延の判定に使わない
    if let (Some(latency), Some(rtt)) = (&mut monitors.latency, rtt) {
        match latency.record(rtt, now) {
            Some(LatencyEvent::High { rtt }) => {
                let body = Msg::LatencyHighBody { host: &host, rtt };
                log(&format!("[latency high] {}", body.in_lang(Lang::En)));
                notifier::notify(&t(Msg::LatencyHighTitle), &t(body));
            }
            Some(LatencyEvent::Normal { duration }) => {
                let body = Msg::LatencyNormalBody {
                    host: &host,
                    secs: duration.as_secs(),
                };
                log(&format!("[latency normal] {}", body.in_lang(Lang::En)));
                if config.notify_recovery {
                    notifier::notify(&t(Msg::LatencyNormalTitle), &t(body));
                }
            }
            None => {}
        }
    }

    Status::Up {
        host,
        rtt,
        slow: monitors
            .latency
            .as_ref()
            .is_some_and(LatencyMonitor::is_slow),
    }
}
