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

#[cfg(not(target_os = "macos"))]
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
    // 通知に失敗しても監視自体は継続する
    if let Err(e) = notification.show() {
        crate::log(&format!("failed to send a notification: {e}"));
    }
}
