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
        crate::log(&format!("通知の送り主を {id} に設定できませんでした: {e}"));
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

pub fn notify(summary: &str, body: &str) {
    // 通知に失敗しても監視自体は継続する
    if let Err(e) = Notification::new()
        .appname("Ping Notifier")
        .summary(summary)
        .body(body)
        .show()
    {
        crate::log(&format!("通知の送信に失敗しました: {e}"));
    }
}
