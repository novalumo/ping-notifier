//! バックグラウンドで ping を打ち続け、状態の変化を UI スレッドへ知らせる。

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Instant;

use crate::config::Config;
use crate::monitor::{Event, Monitor};
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
        .expect("監視スレッドを起動できませんでした");
    tx
}

fn run(mut config: Config, rx: &Receiver<Command>, on_status: &dyn Fn(Status)) {
    let mut monitor = Monitor::new(config.threshold);
    let mut paused = false;

    loop {
        let started = Instant::now();

        if paused {
            on_status(Status::Paused);
            // 一時停止中は指示が来るまで待つ
            match rx.recv() {
                Ok(cmd) => apply(cmd, &mut config, &mut monitor, &mut paused),
                Err(_) => return,
            }
            continue;
        }

        on_status(check(&config, &mut monitor));

        // 次の ping までの間も指示を受け付け、届いたらすぐ反映する
        let remaining = (started + config.interval()).saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(cmd) => apply(cmd, &mut config, &mut monitor, &mut paused),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn apply(cmd: Command, config: &mut Config, monitor: &mut Monitor, paused: &mut bool) {
    match cmd {
        Command::SetPaused(p) => *paused = p,
        Command::Reload(new) => *config = new,
    }
    // 設定や一時停止の切り替えをまたいでロス回数を持ち越さない
    *monitor = Monitor::new(config.threshold);
}

fn check(config: &Config, monitor: &mut Monitor) -> Status {
    let host = config.host.clone();
    let success = match ping::ping_once(&host, config.timeout()) {
        Ok(success) => success,
        Err(e) => {
            let message = format!("ping コマンドを実行できません: {e}");
            log(&message);
            return Status::Error { host, message };
        }
    };

    if !success {
        log(&format!("{host}: timeout"));
    }

    match monitor.record(success, Instant::now()) {
        Some(Event::Lost { consecutive }) => {
            let body = format!("{host} から {consecutive} 回連続で応答がありません");
            log(&format!("[通知] {body}"));
            notifier::notify("パケットロスを検知", &body);
        }
        Some(Event::Recovered { lost, downtime }) => {
            let body = format!(
                "{host} への疎通が復旧しました（ロス {lost} 回 / 約 {} 秒）",
                downtime.as_secs()
            );
            log(&format!("[復旧] {body}"));
            if config.notify_recovery {
                notifier::notify("疎通が復旧", &body);
            }
        }
        None => {}
    }

    if monitor.is_down() {
        Status::Down {
            host,
            consecutive: monitor.consecutive_failures(),
        }
    } else {
        Status::Up { host }
    }
}
