// リリースビルドの Windows ではコンソールウィンドウを出さない
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
mod config;
mod i18n;
mod icon;
mod monitor;
mod notifier;
mod ping;
mod settings_window;
mod updater;
mod worker;

use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::mpsc::Sender;

use anyhow::{Context, Result};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{ActivationPolicy, App, QuitMode, WindowHandle};
use gpui_component::Root;
use semver::Version;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::config::Config;
use crate::i18n::{LanguageSetting, Msg, t};
use crate::updater::{UpdateCommand, UpdateEvent};
use crate::worker::{Command, Status};

/// メニュー・監視スレッド・更新スレッドから UI（メイン）スレッドへ届くイベント
#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Status(Status),
    Update(UpdateEvent),
    /// 設定画面で設定ファイルを保存した
    SettingsSaved,
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

    // 自動アップデートで再起動された場合は、新しい版から完了を知らせる。
    // 置き換え前の版から送ると、自身の .app が消えた後になるため macOS に拒否される
    if let Some(from) = updater::updated_from(std::env::args().skip(1)) {
        let to = updater::current_version();
        log(&format!("updated from v{from} to v{to}"));
        // 通知の送信は配送の確認で数秒ブロックすることがあるため、起動を待たせないよう別スレッドで送る
        std::thread::spawn(move || {
            notifier::notify(
                &t(Msg::UpdatedTitle),
                &t(Msg::UpdatedBody {
                    from: &from,
                    to: &to,
                }),
            );
        });
    }

    // 別スレッドやメニューのコールバックから届くイベントは、GPUI のメインスレッドで順に処理する
    let (events, mut receiver) = unbounded();
    let sender = events.clone();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = sender.unbounded_send(UserEvent::Menu(e));
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
    let sender = events.clone();
    let worker = worker::spawn(config, move |status| {
        let _ = sender.unbounded_send(UserEvent::Status(status));
    });
    let sender = events.clone();
    let updater = updater::spawn(auto_update, move |event| {
        let _ = sender.unbounded_send(UserEvent::Update(event));
    });

    // 常駐アプリなので、ウィンドウがなくても終了しない（終了はメニューの「終了」と再起動のときだけ）
    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        // 設定画面などで gpui-component が使うアイコン
        .with_assets(gpui_kit_assets::Assets)
        .run(move |cx| {
            // Dock に表示せずメニューバーにだけ常駐する（macOS 以外では何もしない）
            cx.set_activation_policy(ActivationPolicy::Accessory);
            gpui_component::init(cx);
            #[cfg(target_os = "linux")]
            pump_gtk(cx);

            // macOS ではアプリの起動が終わってからトレイアイコンを作る必要がある
            let built = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Ping Notifier")
                .with_icon(icon::circle(icon::GRAY))
                .build();
            let tray = match built {
                Ok(tray) => tray,
                Err(e) => {
                    report_error(Msg::TrayCreateFailedTitle, &e.into());
                    cx.quit();
                    return;
                }
            };

            let mut app = TrayApp {
                items,
                tray: Some(tray),
                last_status: None,
                available: None,
                worker,
                updater,
                config_path,
                events,
                settings_window: None,
            };
            cx.spawn(async move |cx| {
                while let Some(event) = receiver.next().await {
                    cx.update(|cx| app.handle(event, cx));
                }
            })
            .detach();
        });
    Ok(())
}

/// tray-icon は Linux で GTK のメインループを必要とするため、GPUI のループの中で定期的に回す。
/// 対象 OS ではないが、開発時に Linux でも動かせるようにしている
#[cfg(target_os = "linux")]
fn pump_gtk(cx: &mut App) {
    if let Err(e) = gtk::init() {
        log(&format!("failed to initialize GTK: {e}"));
        return;
    }
    cx.spawn(async move |cx| {
        loop {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
            cx.background_executor()
                .timer(std::time::Duration::from_millis(50))
                .await;
        }
    })
    .detach();
}

/// UI スレッドが持つ状態
struct TrayApp {
    items: MenuItems,
    /// 終了時に先に破棄してトレイからアイコンを消す
    tray: Option<TrayIcon>,
    last_status: Option<Status>,
    /// 自動で置き換えられなかった新バージョンとそのダウンロードページ
    available: Option<(Version, String)>,
    worker: Sender<Command>,
    updater: Sender<UpdateCommand>,
    config_path: PathBuf,
    /// 設定画面から UI スレッドへイベントを送るため
    events: UnboundedSender<UserEvent>,
    /// 開いている設定画面。閉じた後も残るが、操作できなければ開き直す
    settings_window: Option<WindowHandle<Root>>,
}

impl TrayApp {
    fn handle(&mut self, event: UserEvent, cx: &mut App) {
        match event {
            UserEvent::Status(status) => self.on_status(status),
            UserEvent::Update(UpdateEvent::Installed { version, relaunch }) => {
                match updater::relaunch(&relaunch) {
                    Ok(()) => {
                        log(&format!("restarting into v{version}"));
                        self.quit(cx);
                    }
                    // 置き換え自体は済んでいるので、次回起動時に新しいバージョンになる。
                    // macOS では自身の .app が既に消えているため、この通知は拒否されることがある
                    Err(e) => report_error(Msg::RelaunchFailedTitle, &e),
                }
            }
            UserEvent::Update(UpdateEvent::Available { version, url }) => {
                self.items
                    .update
                    .set_text(t(Msg::MenuDownloadUpdate { version: &version }));
                self.available = Some((version, url));
            }
            UserEvent::Menu(e) => self.on_menu(e, cx),
            UserEvent::SettingsSaved => self.reload_config(),
        }
    }

    fn open_settings(&mut self, cx: &mut App) {
        // すでに開いていれば前面に出すだけ
        if let Some(handle) = &self.settings_window
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            cx.activate(true);
            return;
        }
        // 画面には設定ファイルの今の内容を出す（手で編集した後でも食い違わないように）
        let config = config::load_or_create(&self.config_path).unwrap_or_else(|e| {
            report_error(Msg::ConfigLoadFailedUsingDefaultsTitle, &e);
            Config::default()
        });
        match settings_window::open(config, self.config_path.clone(), self.events.clone(), cx) {
            Ok(handle) => self.settings_window = Some(handle),
            Err(e) => report_error(Msg::SettingsOpenFailedTitle, &e),
        }
    }

    fn quit(&mut self, cx: &mut App) {
        self.tray.take();
        cx.quit();
    }

    fn on_status(&mut self, status: Status) {
        if self.last_status.as_ref() == Some(&status) {
            return;
        }
        let previous = self.last_status.as_ref().map(describe);
        let (color, text) = describe(&status);
        // 応答時間は毎回変わるので、変わった部分だけ反映する
        if previous.as_ref().map(|p| &p.1) != Some(&text) {
            self.items.status.set_text(&text);
            if let Some(tray) = &self.tray {
                let _ = tray.set_tooltip(Some(tooltip(&text)));
            }
        }
        if previous.map(|p| p.0) != Some(color)
            && let Some(tray) = &self.tray
        {
            let _ = tray.set_icon(Some(icon::circle(color)));
        }
        self.last_status = Some(status);
    }

    fn on_menu(&mut self, e: MenuEvent, cx: &mut App) {
        let items = &self.items;
        if e.id == items.quit.id() {
            self.quit(cx);
        } else if e.id == items.pause.id() {
            let _ = self
                .worker
                .send(Command::SetPaused(items.pause.is_checked()));
        } else if e.id == items.silent.id() {
            let _ = self
                .worker
                .send(Command::SetSilent(items.silent.is_checked()));
        } else if e.id == items.settings.id() {
            self.open_settings(cx);
        } else if e.id == items.open.id() {
            if let Err(e) = open_in_editor(&self.config_path) {
                report_error(Msg::ConfigOpenFailedTitle, &e);
            }
        } else if e.id == items.autostart.id() {
            toggle_autostart(&items.autostart);
        } else if e.id == items.update.id() {
            match &self.available {
                Some((_, url)) => {
                    if let Err(e) = open_url(url) {
                        report_error(Msg::DownloadPageOpenFailedTitle, &e);
                    }
                }
                None => {
                    let _ = self.updater.send(UpdateCommand::CheckNow);
                }
            }
        } else if e.id == items.reload.id() {
            self.reload_config();
        }
    }

    fn reload_config(&mut self) {
        match config::load_or_create(&self.config_path) {
            Ok(config) => {
                log(&format!("reloaded settings: {config:?}"));
                let lang = config.language.resolve();
                if lang != i18n::current() {
                    i18n::set(lang);
                    let available = self.available.as_ref().map(|(v, _)| v);
                    let status = self.items.relabel(self.last_status.as_ref(), available);
                    if let Some(tray) = &self.tray {
                        let _ = tray.set_tooltip(Some(tooltip(&status)));
                    }
                }
                let _ = self
                    .updater
                    .send(UpdateCommand::SetAuto(config.auto_update));
                let _ = self.worker.send(Command::Reload(config));
            }
            Err(e) => report_error(Msg::ConfigLoadFailedTitle, &e),
        }
    }
}

/// メニュー項目。表示言語が変わったときに文言を付け直せるようまとめて持つ
struct MenuItems {
    status: MenuItem,
    pause: CheckMenuItem,
    silent: CheckMenuItem,
    settings: MenuItem,
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
            silent: CheckMenuItem::new("", true, false, None),
            settings: MenuItem::new("", true, None),
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
            &self.silent,
            &self.settings,
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
        self.silent.set_text(t(Msg::MenuSilent));
        self.settings.set_text(t(Msg::MenuSettings));
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
        Status::Up { host, rtt, slow } => {
            let text = match (*slow, *rtt) {
                (false, None) => t(Msg::StatusUp { host }),
                (false, Some(rtt)) => t(Msg::StatusUpRtt { host, rtt }),
                (true, None) => t(Msg::StatusSlow { host }),
                (true, Some(rtt)) => t(Msg::StatusSlowRtt { host, rtt }),
            };
            (if *slow { icon::YELLOW } else { icon::GREEN }, text)
        }
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
