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

/// 表示言語。追加するときは `Lang::ALL` にも加え、`Msg::ja` と同様の関数と `Msg::in_lang` の分岐を足す
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    Ja = 1,
    Zh = 2,
    Ko = 3,
}

impl Lang {
    /// 対応しているすべての言語。並びは判別子（`as u8`）の値と一致させる
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Ja, Lang::Zh, Lang::Ko];
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
    Zh,
    Ko,
}

impl LanguageSetting {
    pub fn resolve(self) -> Lang {
        match self {
            Self::Auto => from_locales(sys_locale::get_locales()),
            Self::En => Lang::En,
            Self::Ja => Lang::Ja,
            Self::Zh => Lang::Zh,
            Self::Ko => Lang::Ko,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    Lang::ALL
        .get(CURRENT.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or(Lang::En)
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
        "zh" => is_simplified_chinese(tag).then_some(Lang::Zh),
        "ko" => Some(Lang::Ko),
        _ => None,
    }
}

/// 簡体字の中国語のロケールか。用字（`Hans` / `Hant`）があればそれに従い、なければ地域で判断する。
/// 繁体字を使う台湾・香港・マカオは対象外にして、次の優先言語に進める
fn is_simplified_chinese(tag: &str) -> bool {
    let subtags: Vec<String> = tag
        .split(['-', '_'])
        .skip(1)
        .map(|s| s.to_ascii_lowercase())
        .collect();
    if subtags.iter().any(|s| s == "hans") {
        return true;
    }
    if subtags.iter().any(|s| s == "hant") {
        return false;
    }
    !subtags
        .iter()
        .any(|s| matches!(s.as_str(), "tw" | "hk" | "mo"))
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
    UpdatedBody { from: &'a Version, to: &'a Version },
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
            Lang::Zh => self.zh(),
            Lang::Ko => self.ko(),
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
            Self::UpdatedBody { from, to } => format!("Updated from v{from} to v{to}."),
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
            Self::UpdatedBody { from, to } => format!("v{from} から v{to} に更新しました"),
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

    fn zh(&self) -> String {
        match *self {
            Self::MenuStarting => "正在启动…".into(),
            Self::MenuPause => "暂停".into(),
            Self::MenuOpenConfig => "打开设置文件".into(),
            Self::MenuReloadConfig => "重新加载设置".into(),
            Self::MenuLaunchAtLogin => "登录时启动".into(),
            Self::MenuLaunchAtLoginUnavailable => "登录时启动（当前环境不可用）".into(),
            Self::MenuCheckForUpdates => "检查更新".into(),
            Self::MenuDownloadUpdate { version } => format!("下载 v{version}…"),
            Self::MenuVersion { version } => format!("版本 {version}"),
            Self::MenuQuit => "退出".into(),

            Self::StatusPaused => "已暂停".into(),
            Self::StatusUp { host } => format!("{host}: 正常"),
            Self::StatusDown { host, consecutive } => {
                format!("{host}: 无响应（连续 {consecutive} 次）")
            }
            Self::StatusPingError { host } => format!("{host}: 无法执行 ping"),

            Self::LostTitle => "检测到丢包".into(),
            Self::LostBody { host, consecutive } => {
                format!("{host} 连续 {consecutive} 次无响应")
            }
            Self::RecoveredTitle => "连接已恢复".into(),
            Self::RecoveredBody { host, lost, secs } => {
                format!("与 {host} 的连接已恢复（丢包 {lost} 次，约 {secs} 秒）")
            }

            Self::UpdateCheckFailedTitle => "无法检查更新".into(),
            Self::UpToDateTitle => "已是最新版本".into(),
            Self::UpToDateBody { version } => format!("v{version} 是最新版本。"),
            Self::UpdateAvailableTitle => "有新版本可用".into(),
            Self::UpdateAvailableBody { version } => {
                format!("v{version} 已发布，可从菜单下载。")
            }
            Self::UpdatedTitle => "已更新".into(),
            Self::UpdatedBody { from, to } => format!("已从 v{from} 更新到 v{to}。"),
            Self::UpdateFailedTitle => "更新失败".into(),
            Self::UpdateFailedHint => "可从菜单打开下载页面。".into(),
            Self::RelaunchFailedTitle => "无法重新启动，请手动重新打开应用。".into(),

            Self::AutostartSetFailedTitle => "无法更改登录时启动设置".into(),
            Self::AutostartApprovalTitle => "登录时启动需要你的批准".into(),
            Self::AutostartApprovalBody => {
                "请在“系统设置 > 通用 > 登录项”中允许 Ping Notifier。".into()
            }

            Self::ConfigLoadFailedUsingDefaultsTitle => "无法加载设置（将使用默认值）".into(),
            Self::ConfigLoadFailedTitle => "无法加载设置".into(),
            Self::ConfigOpenFailedTitle => "无法打开设置文件".into(),
            Self::DownloadPageOpenFailedTitle => "无法打开下载页面".into(),
            Self::TrayCreateFailedTitle => "无法创建托盘图标".into(),
        }
    }

    fn ko(&self) -> String {
        match *self {
            Self::MenuStarting => "시작하는 중…".into(),
            Self::MenuPause => "일시 정지".into(),
            Self::MenuOpenConfig => "설정 파일 열기".into(),
            Self::MenuReloadConfig => "설정 다시 불러오기".into(),
            Self::MenuLaunchAtLogin => "로그인 시 실행".into(),
            Self::MenuLaunchAtLoginUnavailable => {
                "로그인 시 실행 (이 환경에서는 사용할 수 없음)".into()
            }
            Self::MenuCheckForUpdates => "업데이트 확인".into(),
            Self::MenuDownloadUpdate { version } => format!("v{version} 다운로드…"),
            Self::MenuVersion { version } => format!("버전 {version}"),
            Self::MenuQuit => "종료".into(),

            Self::StatusPaused => "일시 정지됨".into(),
            Self::StatusUp { host } => format!("{host}: 정상"),
            Self::StatusDown { host, consecutive } => {
                format!("{host}: 응답 없음 ({consecutive}회 연속)")
            }
            Self::StatusPingError { host } => format!("{host}: ping을 실행할 수 없음"),

            Self::LostTitle => "패킷 손실 감지".into(),
            Self::LostBody { host, consecutive } => {
                format!("{host}에서 {consecutive}회 연속으로 응답이 없습니다.")
            }
            Self::RecoveredTitle => "연결 복구됨".into(),
            Self::RecoveredBody { host, lost, secs } => {
                format!("{host} 연결이 복구되었습니다. (손실 {lost}회, 약 {secs}초)")
            }

            Self::UpdateCheckFailedTitle => "업데이트를 확인할 수 없습니다".into(),
            Self::UpToDateTitle => "최신 버전입니다".into(),
            Self::UpToDateBody { version } => {
                format!("사용 중인 버전(v{version})이 최신 버전입니다.")
            }
            Self::UpdateAvailableTitle => "새 버전이 있습니다".into(),
            Self::UpdateAvailableBody { version } => {
                format!("새 버전(v{version})이 공개되었습니다. 메뉴에서 다운로드할 수 있습니다.")
            }
            Self::UpdatedTitle => "업데이트 완료".into(),
            Self::UpdatedBody { from, to } => {
                format!("v{from}에서 v{to} 버전으로 업데이트했습니다.")
            }
            Self::UpdateFailedTitle => "업데이트에 실패했습니다".into(),
            Self::UpdateFailedHint => "메뉴에서 다운로드 페이지를 열 수 있습니다.".into(),
            Self::RelaunchFailedTitle => "다시 시작할 수 없습니다. 앱을 다시 열어 주세요.".into(),

            Self::AutostartSetFailedTitle => "로그인 시 실행 설정을 변경할 수 없습니다".into(),
            Self::AutostartApprovalTitle => "로그인 시 실행하려면 승인이 필요합니다".into(),
            Self::AutostartApprovalBody => {
                "시스템 설정 > 일반 > 로그인 항목에서 Ping Notifier를 허용해 주세요.".into()
            }

            Self::ConfigLoadFailedUsingDefaultsTitle => {
                "설정을 불러올 수 없습니다 (기본값으로 시작합니다)".into()
            }
            Self::ConfigLoadFailedTitle => "설정을 불러올 수 없습니다".into(),
            Self::ConfigOpenFailedTitle => "설정 파일을 열 수 없습니다".into(),
            Self::DownloadPageOpenFailedTitle => "다운로드 페이지를 열 수 없습니다".into(),
            Self::TrayCreateFailedTitle => "트레이 아이콘을 만들 수 없습니다".into(),
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
    fn all_languages_are_ordered_by_discriminant() {
        for (i, lang) in Lang::ALL.into_iter().enumerate() {
            assert_eq!(lang as usize, i, "{lang:?}");
        }
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

        let (from, to) = (Version::new(0, 5, 0), Version::new(0, 5, 1));
        let msg = Msg::UpdatedBody {
            from: &from,
            to: &to,
        };
        assert_eq!(msg.in_lang(Lang::En), "Updated from v0.5.0 to v0.5.1.");
        assert_eq!(msg.in_lang(Lang::Ja), "v0.5.0 から v0.5.1 に更新しました");
    }

    #[test]
    fn supports_simplified_chinese() {
        for tag in [
            "zh",
            "zh-CN",
            "zh_CN",
            "zh-SG",
            "zh-Hans",
            "zh-Hans-CN",
            "zh-Hans-HK",
        ] {
            assert_eq!(from_locale(tag), Some(Lang::Zh), "{tag}");
        }
        for tag in [
            "zh-TW",
            "zh-HK",
            "zh-MO",
            "zh-Hant",
            "zh-Hant-TW",
            "zh-Hant-CN",
        ] {
            assert_eq!(from_locale(tag), None, "{tag}");
        }
        assert_eq!(
            from_locales(locales(&["zh-Hant-TW", "zh-Hans-CN"])),
            Lang::Zh
        );
        assert_eq!(LanguageSetting::Zh.resolve(), Lang::Zh);

        let msg = Msg::StatusDown {
            host: "8.8.8.8",
            consecutive: 3,
        };
        assert_eq!(msg.in_lang(Lang::Zh), "8.8.8.8: 无响应（连续 3 次）");
    }

    #[test]
    fn supports_korean() {
        assert_eq!(from_locale("ko-KR"), Some(Lang::Ko));
        assert_eq!(from_locale("ko"), Some(Lang::Ko));
        assert_eq!(LanguageSetting::Ko.resolve(), Lang::Ko);

        let msg = Msg::StatusDown {
            host: "8.8.8.8",
            consecutive: 3,
        };
        assert_eq!(msg.in_lang(Lang::Ko), "8.8.8.8: 응답 없음 (3회 연속)");
    }
}
