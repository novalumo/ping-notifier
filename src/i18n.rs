//! 表示言語の決定と、利用者に見せる文言。
//!
//! 文言は `Msg` のバリアントとして定義し、言語ごとの関数で網羅的に `match` する。
//! 文言や言語を追加したときに訳し漏れがあるとコンパイルエラーになる。
//!
//! 翻訳するのはメニュー・ツールチップ・通知の見出しと案内文だけ。
//! ログとエラーの詳細（anyhow のメッセージ）は、Issue などで共有されても読めるよう英語に固定している。

use std::sync::atomic::{AtomicU8, Ordering};

use semver::Version;
use serde::Deserialize;

/// 表示言語。追加するときは `Msg::ja` と同様の関数と `Msg::in_lang` の分岐を足す
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    Ja = 1,
}

/// 設定ファイルの `language`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageSetting {
    /// OS の言語に合わせる。対応していなければ英語
    #[default]
    Auto,
    En,
    Ja,
}

impl LanguageSetting {
    pub fn resolve(self) -> Lang {
        match self {
            Self::Auto => from_locales(sys_locale::get_locales()),
            Self::En => Lang::En,
            Self::Ja => Lang::Ja,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Ja,
        _ => Lang::En,
    }
}

/// OS の優先言語の並び（例: `["ja-JP", "en-US"]`）から、最初に対応している言語を選ぶ
fn from_locales(locales: impl IntoIterator<Item = String>) -> Lang {
    locales
        .into_iter()
        .find_map(|tag| from_locale(&tag))
        .unwrap_or(Lang::En)
}

fn from_locale(tag: &str) -> Option<Lang> {
    // "ja-JP" / "ja_JP" / "ja" のいずれの形式でも、主言語の部分だけで判定する
    let primary = tag.split(['-', '_']).next()?.to_ascii_lowercase();
    match primary.as_str() {
        "en" => Some(Lang::En),
        "ja" => Some(Lang::Ja),
        _ => None,
    }
}

/// 現在の表示言語で文言を返す
pub fn t(msg: Msg) -> String {
    msg.in_lang(current())
}

/// 利用者に見せる文言
#[derive(Debug, Clone, Copy)]
pub enum Msg<'a> {
    // メニュー
    MenuStarting,
    MenuPause,
    MenuOpenConfig,
    MenuReloadConfig,
    MenuLaunchAtLogin,
    MenuLaunchAtLoginUnavailable,
    MenuCheckForUpdates,
    MenuDownloadUpdate { version: &'a Version },
    MenuVersion { version: &'a Version },
    MenuQuit,

    // 監視の状態（メニュー先頭とツールチップ）
    StatusPaused,
    StatusUp { host: &'a str },
    StatusDown { host: &'a str, consecutive: u32 },
    StatusPingError { host: &'a str },

    // 通知: 監視
    LostTitle,
    LostBody { host: &'a str, consecutive: u32 },
    RecoveredTitle,
    RecoveredBody { host: &'a str, lost: u32, secs: u64 },

    // 通知: アップデート
    UpdateCheckFailedTitle,
    UpToDateTitle,
    UpToDateBody { version: &'a Version },
    UpdateAvailableTitle,
    UpdateAvailableBody { version: &'a Version },
    UpdatedTitle,
    UpdatedBody { version: &'a Version },
    UpdateFailedTitle,
    UpdateFailedHint,
    RelaunchFailedTitle,

    // 通知: ログイン時の起動
    AutostartSetFailedTitle,
    AutostartApprovalTitle,
    AutostartApprovalBody,

    // 通知: その他のエラー
    ConfigLoadFailedUsingDefaultsTitle,
    ConfigLoadFailedTitle,
    ConfigOpenFailedTitle,
    DownloadPageOpenFailedTitle,
    TrayCreateFailedTitle,
}

impl Msg<'_> {
    pub fn in_lang(&self, lang: Lang) -> String {
        match lang {
            Lang::En => self.en(),
            Lang::Ja => self.ja(),
        }
    }

    fn en(&self) -> String {
        match *self {
            Self::MenuStarting => "Starting…".into(),
            Self::MenuPause => "Pause".into(),
            Self::MenuOpenConfig => "Open Settings File".into(),
            Self::MenuReloadConfig => "Reload Settings".into(),
            Self::MenuLaunchAtLogin => "Launch at Login".into(),
            Self::MenuLaunchAtLoginUnavailable => "Launch at Login (unavailable here)".into(),
            Self::MenuCheckForUpdates => "Check for Updates".into(),
            Self::MenuDownloadUpdate { version } => format!("Download v{version}…"),
            Self::MenuVersion { version } => format!("Version {version}"),
            Self::MenuQuit => "Quit".into(),

            Self::StatusPaused => "Paused".into(),
            Self::StatusUp { host } => format!("{host}: OK"),
            Self::StatusDown { host, consecutive } => {
                format!("{host}: no response ({consecutive} in a row)")
            }
            Self::StatusPingError { host } => format!("{host}: cannot run ping"),

            Self::LostTitle => "Packet loss detected".into(),
            Self::LostBody { host, consecutive } => {
                format!("No response from {host} ({consecutive} in a row)")
            }
            Self::RecoveredTitle => "Connection restored".into(),
            Self::RecoveredBody { host, lost, secs } => {
                format!("{host} is reachable again (lost {lost}, about {secs}s)")
            }

            Self::UpdateCheckFailedTitle => "Couldn't check for updates".into(),
            Self::UpToDateTitle => "You're up to date".into(),
            Self::UpToDateBody { version } => format!("v{version} is the latest version."),
            Self::UpdateAvailableTitle => "A new version is available".into(),
            Self::UpdateAvailableBody { version } => {
                format!("v{version} is available. You can download it from the menu.")
            }
            Self::UpdatedTitle => "Updated".into(),
            Self::UpdatedBody { version } => format!("Updated to v{version}. Restarting…"),
            Self::UpdateFailedTitle => "Update failed".into(),
            Self::UpdateFailedHint => "You can open the download page from the menu.".into(),
            Self::RelaunchFailedTitle => "Couldn't restart. Please reopen the app.".into(),

            Self::AutostartSetFailedTitle => "Couldn't change Launch at Login".into(),
            Self::AutostartApprovalTitle => "Launch at Login needs your approval".into(),
            Self::AutostartApprovalBody => {
                "Allow Ping Notifier in System Settings > General > Login Items.".into()
            }

            Self::ConfigLoadFailedUsingDefaultsTitle => {
                "Couldn't load settings (using defaults)".into()
            }
            Self::ConfigLoadFailedTitle => "Couldn't load settings".into(),
            Self::ConfigOpenFailedTitle => "Couldn't open the settings file".into(),
            Self::DownloadPageOpenFailedTitle => "Couldn't open the download page".into(),
            Self::TrayCreateFailedTitle => "Couldn't create the tray icon".into(),
        }
    }

    fn ja(&self) -> String {
        match *self {
            Self::MenuStarting => "起動中…".into(),
            Self::MenuPause => "一時停止".into(),
            Self::MenuOpenConfig => "設定ファイルを開く".into(),
            Self::MenuReloadConfig => "設定を再読み込み".into(),
            Self::MenuLaunchAtLogin => "ログイン時に起動".into(),
            Self::MenuLaunchAtLoginUnavailable => "ログイン時に起動（この環境では利用不可）".into(),
            Self::MenuCheckForUpdates => "アップデートを確認".into(),
            Self::MenuDownloadUpdate { version } => format!("v{version} をダウンロード…"),
            Self::MenuVersion { version } => format!("バージョン {version}"),
            Self::MenuQuit => "終了".into(),

            Self::StatusPaused => "一時停止中".into(),
            Self::StatusUp { host } => format!("{host}: 正常"),
            Self::StatusDown { host, consecutive } => {
                format!("{host}: 応答なし（{consecutive} 回連続）")
            }
            Self::StatusPingError { host } => format!("{host}: ping を実行できません"),

            Self::LostTitle => "パケットロスを検知".into(),
            Self::LostBody { host, consecutive } => {
                format!("{host} から {consecutive} 回連続で応答がありません")
            }
            Self::RecoveredTitle => "疎通が復旧".into(),
            Self::RecoveredBody { host, lost, secs } => {
                format!("{host} への疎通が復旧しました（ロス {lost} 回 / 約 {secs} 秒）")
            }

            Self::UpdateCheckFailedTitle => "アップデートを確認できませんでした".into(),
            Self::UpToDateTitle => "最新版です".into(),
            Self::UpToDateBody { version } => format!("v{version} は最新版です"),
            Self::UpdateAvailableTitle => "新しいバージョンがあります".into(),
            Self::UpdateAvailableBody { version } => {
                format!("v{version} が公開されています。メニューからダウンロードできます")
            }
            Self::UpdatedTitle => "アップデートしました".into(),
            Self::UpdatedBody { version } => format!("v{version} に更新しました。再起動します"),
            Self::UpdateFailedTitle => "アップデートに失敗しました".into(),
            Self::UpdateFailedHint => "メニューからダウンロードページを開けます".into(),
            Self::RelaunchFailedTitle => {
                "再起動できませんでした。手動で起動し直してください".into()
            }

            Self::AutostartSetFailedTitle => "ログイン時の起動を設定できませんでした".into(),
            Self::AutostartApprovalTitle => "ログイン時の起動には許可が必要です".into(),
            Self::AutostartApprovalBody => {
                "システム設定の「ログイン項目」で Ping Notifier を許可してください".into()
            }

            Self::ConfigLoadFailedUsingDefaultsTitle => {
                "設定ファイルを読み込めませんでした（既定値で起動します）".into()
            }
            Self::ConfigLoadFailedTitle => "設定ファイルを読み込めませんでした".into(),
            Self::ConfigOpenFailedTitle => "設定ファイルを開けませんでした".into(),
            Self::DownloadPageOpenFailedTitle => "ダウンロードページを開けませんでした".into(),
            Self::TrayCreateFailedTitle => "トレイアイコンを作成できませんでした".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locales(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_language_from_locale_tags() {
        assert_eq!(from_locale("ja-JP"), Some(Lang::Ja));
        assert_eq!(from_locale("ja_JP"), Some(Lang::Ja));
        assert_eq!(from_locale("ja"), Some(Lang::Ja));
        assert_eq!(from_locale("en-US"), Some(Lang::En));
        assert_eq!(from_locale("EN-gb"), Some(Lang::En));
        assert_eq!(from_locale("fr-FR"), None);
        assert_eq!(from_locale("zh-Hant-TW"), None);
    }

    #[test]
    fn picks_first_supported_language_in_preference_order() {
        assert_eq!(from_locales(locales(&["ja-JP", "en-US"])), Lang::Ja);
        assert_eq!(from_locales(locales(&["en-US", "ja-JP"])), Lang::En);
        assert_eq!(from_locales(locales(&["fr-FR", "ja-JP"])), Lang::Ja);
    }

    #[test]
    fn falls_back_to_english() {
        assert_eq!(from_locales(locales(&["fr-FR", "de-DE"])), Lang::En);
        assert_eq!(from_locales(locales(&[])), Lang::En);
    }

    #[test]
    fn explicit_setting_overrides_os_language() {
        assert_eq!(LanguageSetting::En.resolve(), Lang::En);
        assert_eq!(LanguageSetting::Ja.resolve(), Lang::Ja);
    }

    #[test]
    fn formats_messages_with_arguments() {
        let version = Version::new(1, 2, 3);
        let msg = Msg::MenuDownloadUpdate { version: &version };
        assert_eq!(msg.in_lang(Lang::En), "Download v1.2.3…");
        assert_eq!(msg.in_lang(Lang::Ja), "v1.2.3 をダウンロード…");

        let msg = Msg::StatusDown {
            host: "8.8.8.8",
            consecutive: 3,
        };
        assert_eq!(msg.in_lang(Lang::En), "8.8.8.8: no response (3 in a row)");
        assert_eq!(msg.in_lang(Lang::Ja), "8.8.8.8: 応答なし（3 回連続）");
    }
}
