// リリースビルドの Windows ではコンソールウィンドウを出さない
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
mod config;
mod icon;
mod monitor;
mod notifier;
mod ping;
mod updater;
mod worker;

use std::path::Path;
use std::process::Command as Process;

use anyhow::{Context, Result};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::config::Config;
use crate::updater::{UpdateCommand, UpdateEvent};
use crate::worker::{Command, Status};

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Status(Status),
    Update(UpdateEvent),
}

fn main() -> Result<()> {
    notifier::init();
    updater::cleanup_previous();

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
    let autostart_supported = autostart::is_supported();
    let autostart_item = CheckMenuItem::new(
        if autostart_supported {
            "ログイン時に起動"
        } else {
            "ログイン時に起動（この環境では利用不可）"
        },
        autostart_supported,
        autostart_supported && is_autostart_enabled(),
        None,
    );
    let update_item = MenuItem::new("アップデートを確認", true, None);
    let version_item = MenuItem::new(
        format!("バージョン {}", updater::current_version()),
        false,
        None,
    );
    let quit_item = MenuItem::new("終了", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status_item,
        &PredefinedMenuItem::separator(),
        &pause_item,
        &open_item,
        &reload_item,
        &autostart_item,
        &PredefinedMenuItem::separator(),
        &update_item,
        &version_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .context("メニューを作成できませんでした")?;

    log(&format!(
        "{} を監視します（設定: {}）",
        config.host,
        config_path.display()
    ));
    let auto_update = config.auto_update;
    let proxy = event_loop.create_proxy();
    let worker = worker::spawn(config, move |status| {
        let _ = proxy.send_event(UserEvent::Status(status));
    });
    let proxy = event_loop.create_proxy();
    let updater = updater::spawn(auto_update, move |event| {
        let _ = proxy.send_event(UserEvent::Update(event));
    });

    let mut menu = Some(menu);
    let mut tray: Option<TrayIcon> = None;
    let mut last_status: Option<Status> = None;
    // 自動で置き換えられなかった新バージョンのダウンロードページ
    let mut download_url: Option<String> = None;

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

            Event::UserEvent(UserEvent::Update(UpdateEvent::Installed { version, relaunch })) => {
                match updater::relaunch(&relaunch) {
                    Ok(()) => {
                        log(&format!("v{version} で再起動します"));
                        tray.take();
                        *control_flow = ControlFlow::Exit;
                    }
                    // 置き換え自体は済んでいるので、次回起動時に新しいバージョンになる
                    Err(e) => {
                        report_error("再起動できませんでした。手動で起動し直してください", &e)
                    }
                }
            }

            Event::UserEvent(UserEvent::Update(UpdateEvent::Available { version, url })) => {
                update_item.set_text(format!("v{version} をダウンロード…"));
                download_url = Some(url);
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
                } else if e.id == autostart_item.id() {
                    toggle_autostart(&autostart_item);
                } else if e.id == update_item.id() {
                    match &download_url {
                        Some(url) => {
                            if let Err(e) = open_url(url) {
                                report_error("ダウンロードページを開けませんでした", &e);
                            }
                        }
                        None => {
                            let _ = updater.send(UpdateCommand::CheckNow);
                        }
                    }
                } else if e.id == reload_item.id() {
                    match config::load_or_create(&config_path) {
                        Ok(config) => {
                            log(&format!("設定を再読み込みしました: {config:?}"));
                            let _ = updater.send(UpdateCommand::SetAuto(config.auto_update));
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

fn is_autostart_enabled() -> bool {
    match autostart::status() {
        Ok(status) => status == autostart::Status::Enabled,
        Err(e) => {
            log(&format!("自動起動の状態を取得できませんでした: {e:#}"));
            false
        }
    }
}

/// チェック項目はクリックした時点で表示が切り替わるので、その状態を希望として OS に反映し、
/// 最後に OS 側の実際の状態に表示を合わせる
fn toggle_autostart(item: &CheckMenuItem) {
    let enabled = item.is_checked();
    if let Err(e) = autostart::set_enabled(enabled) {
        report_error("ログイン時の起動を設定できませんでした", &e);
    }

    let status = autostart::status().unwrap_or(autostart::Status::Disabled);
    if enabled && status == autostart::Status::RequiresApproval {
        notifier::notify(
            "ログイン時の起動には許可が必要です",
            "システム設定の「ログイン項目」で Ping Notifier を許可してください",
        );
        autostart::open_system_settings();
    }
    log(&format!("ログイン時の起動: {status:?}"));
    item.set_checked(status == autostart::Status::Enabled);
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

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let program = "xdg-open";
    Process::new(program)
        .arg(url)
        .spawn()
        .context("ブラウザを起動できません")?;
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
