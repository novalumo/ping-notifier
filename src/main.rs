// リリースビルドの Windows ではコンソールウィンドウを出さない
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
mod config;
mod i18n;
mod icon;
mod monitor;
mod notifier;
mod ping;
mod updater;
mod worker;

use std::path::Path;
use std::process::Command as Process;

use anyhow::{Context, Result};
use semver::Version;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::config::Config;
use crate::i18n::{LanguageSetting, Msg, t};
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

    // 設定を読む前のエラー通知や、初回に作る設定ファイルのテンプレートは OS の言語に合わせる
    i18n::set(LanguageSetting::Auto.resolve());
    let config_path = config::path()?;
    let config = config::load_or_create(&config_path).unwrap_or_else(|e| {
        // 設定に誤りがあっても常駐は続け、修正後に再読み込みしてもらう
        report_error(Msg::ConfigLoadFailedUsingDefaultsTitle, &e);
        Config::default()
    });
    i18n::set(config.language.resolve());

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

    let items = MenuItems::new();
    let menu = Menu::new();
    items.append_to(&menu).context("failed to build the menu")?;

    log(&format!(
        "monitoring {} (settings: {}, language: {:?})",
        config.host,
        config_path.display(),
        i18n::current()
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
    // 自動で置き換えられなかった新バージョンとそのダウンロードページ
    let mut available: Option<(Version, String)> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            // macOS ではイベントループ開始後にトレイアイコンを作る必要がある
            Event::NewEvents(StartCause::Init) => {
                let built = TrayIconBuilder::new()
                    .with_menu(Box::new(menu.take().expect("Init is delivered only once")))
                    .with_tooltip("Ping Notifier")
                    .with_icon(icon::circle(icon::GRAY))
                    .build();
                match built {
                    Ok(t) => tray = Some(t),
                    Err(e) => {
                        report_error(Msg::TrayCreateFailedTitle, &e.into());
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }

            Event::UserEvent(UserEvent::Status(status)) => {
                if last_status.as_ref() == Some(&status) {
                    return;
                }
                let (color, text) = describe(&status);
                items.status.set_text(&text);
                if let Some(tray) = &tray {
                    let _ = tray.set_icon(Some(icon::circle(color)));
                    let _ = tray.set_tooltip(Some(tooltip(&text)));
                }
                last_status = Some(status);
            }

            Event::UserEvent(UserEvent::Update(UpdateEvent::Installed { version, relaunch })) => {
                match updater::relaunch(&relaunch) {
                    Ok(()) => {
                        log(&format!("restarting into v{version}"));
                        tray.take();
                        *control_flow = ControlFlow::Exit;
                    }
                    // 置き換え自体は済んでいるので、次回起動時に新しいバージョンになる
                    Err(e) => report_error(Msg::RelaunchFailedTitle, &e),
                }
            }

            Event::UserEvent(UserEvent::Update(UpdateEvent::Available { version, url })) => {
                items
                    .update
                    .set_text(t(Msg::MenuDownloadUpdate { version: &version }));
                available = Some((version, url));
            }

            Event::UserEvent(UserEvent::Menu(e)) => {
                if e.id == items.quit.id() {
                    tray.take();
                    *control_flow = ControlFlow::Exit;
                } else if e.id == items.pause.id() {
                    let _ = worker.send(Command::SetPaused(items.pause.is_checked()));
                } else if e.id == items.open.id() {
                    if let Err(e) = open_in_editor(&config_path) {
                        report_error(Msg::ConfigOpenFailedTitle, &e);
                    }
                } else if e.id == items.autostart.id() {
                    toggle_autostart(&items.autostart);
                } else if e.id == items.update.id() {
                    match &available {
                        Some((_, url)) => {
                            if let Err(e) = open_url(url) {
                                report_error(Msg::DownloadPageOpenFailedTitle, &e);
                            }
                        }
                        None => {
                            let _ = updater.send(UpdateCommand::CheckNow);
                        }
                    }
                } else if e.id == items.reload.id() {
                    match config::load_or_create(&config_path) {
                        Ok(config) => {
                            log(&format!("reloaded settings: {config:?}"));
                            let lang = config.language.resolve();
                            if lang != i18n::current() {
                                i18n::set(lang);
                                let available = available.as_ref().map(|(v, _)| v);
                                let status = items.relabel(last_status.as_ref(), available);
                                if let Some(tray) = &tray {
                                    let _ = tray.set_tooltip(Some(tooltip(&status)));
                                }
                            }
                            let _ = updater.send(UpdateCommand::SetAuto(config.auto_update));
                            let _ = worker.send(Command::Reload(config));
                        }
                        Err(e) => report_error(Msg::ConfigLoadFailedTitle, &e),
                    }
                }
            }

            _ => {}
        }
    })
}

/// メニュー項目。表示言語が変わったときに文言を付け直せるようまとめて持つ
struct MenuItems {
    status: MenuItem,
    pause: CheckMenuItem,
    open: MenuItem,
    reload: MenuItem,
    autostart: CheckMenuItem,
    autostart_supported: bool,
    update: MenuItem,
    version: MenuItem,
    quit: MenuItem,
}

impl MenuItems {
    fn new() -> Self {
        let autostart_supported = autostart::is_supported();
        let items = Self {
            status: MenuItem::new("", false, None),
            pause: CheckMenuItem::new("", true, false, None),
            open: MenuItem::new("", true, None),
            reload: MenuItem::new("", true, None),
            autostart: CheckMenuItem::new(
                "",
                autostart_supported,
                autostart_supported && is_autostart_enabled(),
                None,
            ),
            autostart_supported,
            update: MenuItem::new("", true, None),
            version: MenuItem::new("", false, None),
            quit: MenuItem::new("", true, None),
        };
        items.relabel(None, None);
        items
    }

    fn append_to(&self, menu: &Menu) -> tray_icon::menu::Result<()> {
        menu.append_items(&[
            &self.status,
            &PredefinedMenuItem::separator(),
            &self.pause,
            &self.open,
            &self.reload,
            &self.autostart,
            &PredefinedMenuItem::separator(),
            &self.update,
            &self.version,
            &PredefinedMenuItem::separator(),
            &self.quit,
        ])
    }

    /// 現在の表示言語で全項目の文言を設定する。戻り値は状態の文言（ツールチップ用）
    fn relabel(&self, status: Option<&Status>, available: Option<&Version>) -> String {
        let status_text = match status {
            Some(status) => describe(status).1,
            None => t(Msg::MenuStarting),
        };
        self.status.set_text(&status_text);
        self.pause.set_text(t(Msg::MenuPause));
        self.open.set_text(t(Msg::MenuOpenConfig));
        self.reload.set_text(t(Msg::MenuReloadConfig));
        self.autostart.set_text(t(if self.autostart_supported {
            Msg::MenuLaunchAtLogin
        } else {
            Msg::MenuLaunchAtLoginUnavailable
        }));
        self.update.set_text(t(match available {
            Some(version) => Msg::MenuDownloadUpdate { version },
            None => Msg::MenuCheckForUpdates,
        }));
        self.version.set_text(t(Msg::MenuVersion {
            version: &updater::current_version(),
        }));
        self.quit.set_text(t(Msg::MenuQuit));
        status_text
    }
}

fn tooltip(status_text: &str) -> String {
    format!("Ping Notifier - {status_text}")
}

fn is_autostart_enabled() -> bool {
    match autostart::status() {
        Ok(status) => status == autostart::Status::Enabled,
        Err(e) => {
            log(&format!("failed to get the launch-at-login status: {e:#}"));
            false
        }
    }
}

/// チェック項目はクリックした時点で表示が切り替わるので、その状態を希望として OS に反映し、
/// 最後に OS 側の実際の状態に表示を合わせる
fn toggle_autostart(item: &CheckMenuItem) {
    let enabled = item.is_checked();
    if let Err(e) = autostart::set_enabled(enabled) {
        report_error(Msg::AutostartSetFailedTitle, &e);
    }

    let status = autostart::status().unwrap_or(autostart::Status::Disabled);
    if enabled && status == autostart::Status::RequiresApproval {
        notifier::notify(
            &t(Msg::AutostartApprovalTitle),
            &t(Msg::AutostartApprovalBody),
        );
        autostart::open_system_settings();
    }
    log(&format!("launch at login: {status:?}"));
    item.set_checked(status == autostart::Status::Enabled);
}

fn describe(status: &Status) -> ([u8; 3], String) {
    match status {
        Status::Paused => (icon::GRAY, t(Msg::StatusPaused)),
        Status::Up { host } => (icon::GREEN, t(Msg::StatusUp { host })),
        Status::Down { host, consecutive } => (
            icon::RED,
            t(Msg::StatusDown {
                host,
                consecutive: *consecutive,
            }),
        ),
        Status::Error { host, .. } => (icon::ORANGE, t(Msg::StatusPingError { host })),
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
    cmd.spawn().context("cannot launch a text editor")?;
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
        .context("cannot launch a web browser")?;
    Ok(())
}

/// GUI アプリでは標準出力が見えないため、利用者に伝えるべきエラーは通知でも知らせる。
/// 見出しは表示言語、ログと詳細（anyhow のメッセージ）は英語
fn report_error(summary: Msg, error: &anyhow::Error) {
    log(&format!("{}: {error:#}", summary.in_lang(i18n::Lang::En)));
    notifier::notify(&t(summary), &format!("{error:#}"));
}

pub(crate) fn log(message: &str) {
    println!(
        "{} {message}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
}
