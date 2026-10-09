// リリースビルドの Windows ではコンソールウィンドウを出さない
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod icon;
mod monitor;
mod notifier;
mod ping;
mod worker;

use std::path::Path;
use std::process::Command as Process;

use anyhow::{Context, Result};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::config::Config;
use crate::worker::{Command, Status};

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Status(Status),
}

fn main() -> Result<()> {
    notifier::init();

    let config_path = config::path()?;
    let config = config::load_or_create(&config_path).unwrap_or_else(|e| {
        // 設定に誤りがあっても常駐は続け、修正後に再読み込みしてもらう
        report_error(
            "設定ファイルを読み込めませんでした（既定値で起動します）",
            &e,
        );
        Config::default()
    });

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        // Dock に表示せずメニューバーにだけ常駐する
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Menu(e));
    }));

    let status_item = MenuItem::new("起動中…", false, None);
    let pause_item = CheckMenuItem::new("一時停止", true, false, None);
    let open_item = MenuItem::new("設定ファイルを開く", true, None);
    let reload_item = MenuItem::new("設定を再読み込み", true, None);
    let quit_item = MenuItem::new("終了", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status_item,
        &PredefinedMenuItem::separator(),
        &pause_item,
        &open_item,
        &reload_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .context("メニューを作成できませんでした")?;

    log(&format!(
        "{} を監視します（設定: {}）",
        config.host,
        config_path.display()
    ));
    let proxy = event_loop.create_proxy();
    let worker = worker::spawn(config, move |status| {
        let _ = proxy.send_event(UserEvent::Status(status));
    });

    let mut menu = Some(menu);
    let mut tray: Option<TrayIcon> = None;
    let mut last_status: Option<Status> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            // macOS ではイベントループ開始後にトレイアイコンを作る必要がある
            Event::NewEvents(StartCause::Init) => {
                let built = TrayIconBuilder::new()
                    .with_menu(Box::new(menu.take().expect("Init は 1 度だけ届く")))
                    .with_tooltip("Ping Notifier")
                    .with_icon(icon::circle(icon::GRAY))
                    .build();
                match built {
                    Ok(t) => tray = Some(t),
                    Err(e) => {
                        report_error("トレイアイコンを作成できませんでした", &e.into());
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }

            Event::UserEvent(UserEvent::Status(status)) => {
                if last_status.as_ref() == Some(&status) {
                    return;
                }
                let (color, text) = describe(&status);
                status_item.set_text(&text);
                if let Some(tray) = &tray {
                    let _ = tray.set_icon(Some(icon::circle(color)));
                    let _ = tray.set_tooltip(Some(format!("Ping Notifier - {text}")));
                }
                last_status = Some(status);
            }

            Event::UserEvent(UserEvent::Menu(e)) => {
                if e.id == quit_item.id() {
                    tray.take();
                    *control_flow = ControlFlow::Exit;
                } else if e.id == pause_item.id() {
                    let _ = worker.send(Command::SetPaused(pause_item.is_checked()));
                } else if e.id == open_item.id() {
                    if let Err(e) = open_in_editor(&config_path) {
                        report_error("設定ファイルを開けませんでした", &e);
                    }
                } else if e.id == reload_item.id() {
                    match config::load_or_create(&config_path) {
                        Ok(config) => {
                            log(&format!("設定を再読み込みしました: {config:?}"));
                            let _ = worker.send(Command::Reload(config));
                        }
                        Err(e) => report_error("設定ファイルを読み込めませんでした", &e),
                    }
                }
            }

            _ => {}
        }
    })
}

fn describe(status: &Status) -> ([u8; 3], String) {
    match status {
        Status::Paused => (icon::GRAY, "一時停止中".into()),
        Status::Up { host } => (icon::GREEN, format!("{host}: 正常")),
        Status::Down { host, consecutive } => (
            icon::RED,
            format!("{host}: 応答なし（{consecutive} 回連続）"),
        ),
        Status::Error { host, .. } => (icon::ORANGE, format!("{host}: ping を実行できません")),
    }
}

/// テキストエディタで開く（拡張子 .toml に関連付けがない環境でも開けるように）
fn open_in_editor(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut cmd = Process::new("open");
        cmd.arg("-t").arg(path);
        cmd
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut cmd = Process::new("notepad");
        cmd.arg(path);
        cmd
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = {
        let mut cmd = Process::new("xdg-open");
        cmd.arg(path);
        cmd
    };
    cmd.spawn().context("エディタを起動できません")?;
    Ok(())
}

/// GUI アプリでは標準出力が見えないため、利用者に伝えるべきエラーは通知でも知らせる
fn report_error(summary: &str, error: &anyhow::Error) {
    log(&format!("{summary}: {error:#}"));
    notifier::notify(summary, &format!("{error:#}"));
}

pub(crate) fn log(message: &str) {
    println!(
        "{} {message}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
}
