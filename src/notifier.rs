//! OS の通知を送る。

use notify_rust::Notification;

/// `Cargo.toml` の `[package.metadata.bundle] identifier` と一致させること
#[cfg(target_os = "macos")]
pub(crate) const BUNDLE_ID: &str = "com.novalumo.ping-notifier";

/// 通知の送り主となるアプリを設定する。他の通知より前に 1 度だけ呼ぶこと。
///
/// notify-rust は macOS で送り主を指定しないと Finder 名義で送ろうとし、
/// 新しい macOS ではそれが黙って破棄される。
/// `.app` として起動していれば自身の Bundle ID を、`cargo run` などで直接起動した場合は
/// 開発用にターミナル.app の名義を使う。
#[cfg(target_os = "macos")]
pub fn init() {
    let id = if running_in_app_bundle() {
        BUNDLE_ID
    } else {
        "com.apple.Terminal"
    };
    if let Err(e) = notify_rust::set_application(id) {
        crate::log(&format!(
            "failed to set the notification sender to {id}: {e}"
        ));
    }
}

/// 通知の送り主となるアプリ（AppUserModelID）をレジストリに登録する。他の通知より前に 1 度だけ呼ぶこと。
///
/// notify-rust は Windows で AppUserModelID を指定しないと PowerShell 名義で送る。
/// 登録に失敗したときは、通知が届かなくならないよう PowerShell 名義のままにする。
#[cfg(target_os = "windows")]
pub fn init() {
    match windows::register() {
        Ok(()) => windows::REGISTERED.store(true, std::sync::atomic::Ordering::Relaxed),
        Err(e) => crate::log(&format!(
            "failed to register the notification sender, using PowerShell instead: {e:#}"
        )),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn init() {}

#[cfg(target_os = "macos")]
pub(crate) fn running_in_app_bundle() -> bool {
    std::env::current_exe()
        .map(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

/// パケットロスの通知に付ける音。音の名前の体系は OS ごとに違う
/// - macOS: `/System/Library/Sounds` のシステムサウンド名
/// - Windows: トースト通知の音（`tauri-winrt-notification` の `Sound` の名前）
/// - Linux: freedesktop.org のサウンド名
#[cfg(target_os = "macos")]
const ALERT_SOUND: &str = "Basso";
#[cfg(target_os = "windows")]
const ALERT_SOUND: &str = "Reminder";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const ALERT_SOUND: &str = "dialog-warning";

/// 音なしで通知する。notify-rust は音を指定しないと macOS でも Windows でも無音で送る
pub fn notify(summary: &str, body: &str) {
    send(summary, body, false);
}

/// 音付きで通知する。音は OS の通知設定（アプリごとの通知音、集中モードなど）に従う
pub fn notify_with_sound(summary: &str, body: &str) {
    send(summary, body, true);
}

fn send(summary: &str, body: &str, sound: bool) {
    let mut notification = Notification::new();
    notification
        .appname("Ping Notifier")
        .summary(summary)
        .body(body);
    if sound {
        notification.sound_name(ALERT_SOUND);
    }
    #[cfg(target_os = "windows")]
    if windows::REGISTERED.load(std::sync::atomic::Ordering::Relaxed) {
        notification.app_id(windows::APP_USER_MODEL_ID);
    }
    // 通知に失敗しても監視自体は継続する
    if let Err(e) = notification.show() {
        crate::log(&format!("failed to send a notification: {e}"));
    }
}

/// Windows の通知の送り主（AppUserModelID）の登録。
///
/// パッケージ化していないアプリでも、`HKCU\Software\Classes\AppUserModelId\<ID>` に
/// 表示名とアイコンを書けば、その ID で送った通知にアプリ名とアイコンが表示される
/// （スタートメニューのショートカットは要らない）。
#[cfg(target_os = "windows")]
mod windows {
    use std::fs;
    use std::sync::atomic::AtomicBool;

    use anyhow::{Context, Result};
    use windows_registry::CURRENT_USER;

    /// 変えると Windows は別のアプリとみなし、通知の設定（オン・オフなど）が引き継がれない
    pub(super) const APP_USER_MODEL_ID: &str = "Novalumo.PingNotifier";
    const DISPLAY_NAME: &str = "Ping Notifier";
    /// 通知に表示するアイコン。exe に埋め込んだアイコンは使えず、画像ファイルのパスで指定する
    const ICON: &[u8] = include_bytes!("../icons/icon.ico");

    /// 登録できたか。できていなければ PowerShell 名義で送る
    pub(super) static REGISTERED: AtomicBool = AtomicBool::new(false);

    pub(super) fn register() -> Result<()> {
        let dir = dirs::data_local_dir()
            .context("cannot determine the local app data directory")?
            .join("ping-notifier");
        fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let icon = dir.join("notification-icon.ico");
        // 起動のたびに書き換えないよう、中身が違うときだけ書く
        if fs::read(&icon).ok().as_deref() != Some(ICON) {
            fs::write(&icon, ICON).with_context(|| format!("cannot write {}", icon.display()))?;
        }
        let icon = icon.to_str().context("the icon path is not valid UTF-8")?;

        let key = CURRENT_USER
            .create(format!(
                r"Software\Classes\AppUserModelId\{APP_USER_MODEL_ID}"
            ))
            .context("cannot open the AppUserModelId registry key")?;
        key.set_string("DisplayName", DISPLAY_NAME)
            .context("cannot set the display name")?;
        key.set_string("IconUri", icon)
            .context("cannot set the icon")?;
        Ok(())
    }
}
