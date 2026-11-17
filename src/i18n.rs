//! 表示言語の決定と、利用者に見せる文言。
//!
//! 文言は `Msg` のバリアントとして定義し、言語ごとの関数で網羅的に `match` する。
//! 文言や言語を追加したときに訳し漏れがあるとコンパイルエラーになる。
//!
//! 翻訳するのはメニュー・ツールチップ・通知の見出しと案内文だけ。
//! ログとエラーの詳細（anyhow のメッセージ）は、Issue などで共有されても読めるよう英語に固定している。

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

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
    Eo = 4,
    De = 5,
}

impl Lang {
    /// 対応しているすべての言語。並びは判別子（`as u8`）の値と一致させる
    pub const ALL: [Lang; 6] = [Lang::En, Lang::Ja, Lang::Zh, Lang::Ko, Lang::Eo, Lang::De];
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
    Eo,
    De,
}

impl LanguageSetting {
    /// 設定画面の選択肢の並び
    pub const ALL: [LanguageSetting; 7] = [
        Self::Auto,
        Self::En,
        Self::Ja,
        Self::Zh,
        Self::Ko,
        Self::Eo,
        Self::De,
    ];

    /// 設定ファイルに書く値（`Deserialize` の `rename_all = "lowercase"` と一致させる）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::En => "en",
            Self::Ja => "ja",
            Self::Zh => "zh",
            Self::Ko => "ko",
            Self::Eo => "eo",
            Self::De => "de",
        }
    }

    pub fn resolve(self) -> Lang {
        match self {
            Self::Auto => from_locales(sys_locale::get_locales()),
            Self::En => Lang::En,
            Self::Ja => Lang::Ja,
            Self::Zh => Lang::Zh,
            Self::Ko => Lang::Ko,
            Self::Eo => Lang::Eo,
            Self::De => Lang::De,
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
        "eo" => Some(Lang::Eo),
        "de" => Some(Lang::De),
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

/// 応答時間の表示（`12 ms`、1 ms 未満は `<1 ms`）。単位の表記は全言語で共通
fn rtt_text(rtt: Duration) -> String {
    if rtt < Duration::from_millis(1) {
        "<1 ms".into()
    } else {
        format!("{} ms", (rtt.as_secs_f64() * 1000.0).round())
    }
}

/// 利用者に見せる文言
#[derive(Debug, Clone, Copy)]
pub enum Msg<'a> {
    // メニュー
    MenuStarting,
    MenuPause,
    MenuSilent,
    MenuOpenConfig,
    MenuReloadConfig,
    MenuLaunchAtLogin,
    MenuLaunchAtLoginUnavailable,
    MenuCheckForUpdates,
    MenuDownloadUpdate { version: &'a Version },
    MenuVersion { version: &'a Version },
    MenuQuit,

    // 設定画面
    MenuSettings,
    SettingsTitle,
    SettingsPageMonitoring,
    SettingsPageNotifications,
    SettingsPageGeneral,
    SettingsHost,
    SettingsHostDescription,
    SettingsInterval,
    SettingsTimeout,
    SettingsTimeoutDescription,
    SettingsThreshold,
    SettingsNotifyRecovery,
    SettingsLossSound,
    SettingsLatency,
    SettingsLatencyDescription,
    SettingsLatencyThreshold,
    SettingsLatencyConsecutive,
    SettingsLanguage,
    SettingsLanguageAuto,
    SettingsAutoUpdate,
    SettingsSave,
    SettingsCancel,
    SettingsSaveFailed,
    SettingsOpenFailedTitle,

    // 監視の状態（メニュー先頭とツールチップ）
    StatusPaused,
    StatusUp { host: &'a str },
    StatusUpRtt { host: &'a str, rtt: Duration },
    StatusSlow { host: &'a str },
    StatusSlowRtt { host: &'a str, rtt: Duration },
    StatusDown { host: &'a str, consecutive: u32 },
    StatusPingError { host: &'a str },

    // 通知: 監視
    LostTitle,
    LostBody { host: &'a str, consecutive: u32 },
    RecoveredTitle,
    RecoveredBody { host: &'a str, lost: u32, secs: u64 },
    LatencyHighTitle,
    LatencyHighBody { host: &'a str, rtt: Duration },
    LatencyNormalTitle,
    LatencyNormalBody { host: &'a str, secs: u64 },

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
            Lang::Eo => self.eo(),
            Lang::De => self.de(),
        }
    }

    fn en(&self) -> String {
        match *self {
            Self::MenuStarting => "Starting…".into(),
            Self::MenuPause => "Pause".into(),
            Self::MenuSilent => "Silent Mode (No Notifications)".into(),
            Self::MenuOpenConfig => "Open Settings File".into(),
            Self::MenuReloadConfig => "Reload Settings".into(),
            Self::MenuLaunchAtLogin => "Launch at Login".into(),
            Self::MenuLaunchAtLoginUnavailable => "Launch at Login (unavailable here)".into(),
            Self::MenuCheckForUpdates => "Check for Updates".into(),
            Self::MenuDownloadUpdate { version } => format!("Download v{version}…"),
            Self::MenuVersion { version } => format!("Version {version}"),
            Self::MenuQuit => "Quit".into(),
            Self::MenuSettings => "Settings…".into(),
            Self::SettingsTitle => "Ping Notifier Settings".into(),
            Self::SettingsPageMonitoring => "Monitoring".into(),
            Self::SettingsPageNotifications => "Notifications".into(),
            Self::SettingsPageGeneral => "General".into(),
            Self::SettingsHost => "Host".into(),
            Self::SettingsHostDescription => "Host name or IP address to ping".into(),
            Self::SettingsInterval => "Interval (seconds)".into(),
            Self::SettingsTimeout => "Timeout (milliseconds)".into(),
            Self::SettingsTimeoutDescription => "No reply within this time counts as packet loss. On macOS this is rounded up to whole seconds.".into(),
            Self::SettingsThreshold => "Losses in a row before notifying".into(),
            Self::SettingsNotifyRecovery => "Notify when the connection is restored".into(),
            Self::SettingsLossSound => "Play a sound on packet loss".into(),
            Self::SettingsLatency => "Notify on high latency".into(),
            Self::SettingsLatencyDescription => "Notify when the response time stays above the threshold".into(),
            Self::SettingsLatencyThreshold => "Latency threshold (milliseconds)".into(),
            Self::SettingsLatencyConsecutive => "Slow replies in a row before notifying".into(),
            Self::SettingsLanguage => "Language".into(),
            Self::SettingsLanguageAuto => "Follow the OS".into(),
            Self::SettingsAutoUpdate => "Install new versions automatically".into(),
            Self::SettingsSave => "Save".into(),
            Self::SettingsCancel => "Cancel".into(),
            Self::SettingsSaveFailed => "Could not save the settings".into(),
            Self::SettingsOpenFailedTitle => "Could not open the settings window".into(),

            Self::StatusPaused => "Paused".into(),
            Self::StatusUp { host } => format!("{host}: OK"),
            Self::StatusUpRtt { host, rtt } => format!("{host}: OK ({})", rtt_text(rtt)),
            Self::StatusSlow { host } => format!("{host}: slow"),
            Self::StatusSlowRtt { host, rtt } => format!("{host}: slow ({})", rtt_text(rtt)),
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
            Self::LatencyHighTitle => "High latency detected".into(),
            Self::LatencyHighBody { host, rtt } => {
                format!("Response time from {host} is high ({})", rtt_text(rtt))
            }
            Self::LatencyNormalTitle => "Latency back to normal".into(),
            Self::LatencyNormalBody { host, secs } => {
                format!("Response time from {host} is back to normal (about {secs}s)")
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
            Self::MenuSilent => "サイレントモード（通知しない）".into(),
            Self::MenuOpenConfig => "設定ファイルを開く".into(),
            Self::MenuReloadConfig => "設定を再読み込み".into(),
            Self::MenuLaunchAtLogin => "ログイン時に起動".into(),
            Self::MenuLaunchAtLoginUnavailable => "ログイン時に起動（この環境では利用不可）".into(),
            Self::MenuCheckForUpdates => "アップデートを確認".into(),
            Self::MenuDownloadUpdate { version } => format!("v{version} をダウンロード…"),
            Self::MenuVersion { version } => format!("バージョン {version}"),
            Self::MenuQuit => "終了".into(),
            Self::MenuSettings => "設定…".into(),
            Self::SettingsTitle => "Ping Notifier の設定".into(),
            Self::SettingsPageMonitoring => "監視".into(),
            Self::SettingsPageNotifications => "通知".into(),
            Self::SettingsPageGeneral => "一般".into(),
            Self::SettingsHost => "監視するホスト".into(),
            Self::SettingsHostDescription => "ping を送るホスト名または IP アドレス".into(),
            Self::SettingsInterval => "間隔（秒）".into(),
            Self::SettingsTimeout => "タイムアウト（ミリ秒）".into(),
            Self::SettingsTimeoutDescription => {
                "この時間内に応答がなければロスとみなします。macOS では秒単位に切り上げられます"
                    .into()
            }
            Self::SettingsThreshold => "何回連続でロスしたら通知するか".into(),
            Self::SettingsNotifyRecovery => "復旧時にも通知する".into(),
            Self::SettingsLossSound => "パケットロスの通知で音を鳴らす".into(),
            Self::SettingsLatency => "遅延を通知する".into(),
            Self::SettingsLatencyDescription => {
                "応答時間がしきい値を超える状態が続いたら通知します".into()
            }
            Self::SettingsLatencyThreshold => "遅延のしきい値（ミリ秒）".into(),
            Self::SettingsLatencyConsecutive => "何回連続で超えたら通知するか".into(),
            Self::SettingsLanguage => "表示言語".into(),
            Self::SettingsLanguageAuto => "OS に合わせる".into(),
            Self::SettingsAutoUpdate => "新しいバージョンを自動でインストールする".into(),
            Self::SettingsSave => "保存".into(),
            Self::SettingsCancel => "キャンセル".into(),
            Self::SettingsSaveFailed => "設定を保存できませんでした".into(),
            Self::SettingsOpenFailedTitle => "設定画面を開けませんでした".into(),

            Self::StatusPaused => "一時停止中".into(),
            Self::StatusUp { host } => format!("{host}: 正常"),
            Self::StatusUpRtt { host, rtt } => format!("{host}: 正常（{}）", rtt_text(rtt)),
            Self::StatusSlow { host } => format!("{host}: 遅延"),
            Self::StatusSlowRtt { host, rtt } => format!("{host}: 遅延（{}）", rtt_text(rtt)),
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
            Self::LatencyHighTitle => "遅延を検知".into(),
            Self::LatencyHighBody { host, rtt } => {
                format!("{host} の応答時間が長くなっています（{}）", rtt_text(rtt))
            }
            Self::LatencyNormalTitle => "遅延が解消".into(),
            Self::LatencyNormalBody { host, secs } => {
                format!("{host} の応答時間が元に戻りました（約 {secs} 秒）")
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
            Self::MenuSilent => "静默模式（不发送通知）".into(),
            Self::MenuOpenConfig => "打开设置文件".into(),
            Self::MenuReloadConfig => "重新加载设置".into(),
            Self::MenuLaunchAtLogin => "登录时启动".into(),
            Self::MenuLaunchAtLoginUnavailable => "登录时启动（当前环境不可用）".into(),
            Self::MenuCheckForUpdates => "检查更新".into(),
            Self::MenuDownloadUpdate { version } => format!("下载 v{version}…"),
            Self::MenuVersion { version } => format!("版本 {version}"),
            Self::MenuQuit => "退出".into(),
            Self::MenuSettings => "设置…".into(),
            Self::SettingsTitle => "Ping Notifier 设置".into(),
            Self::SettingsPageMonitoring => "监控".into(),
            Self::SettingsPageNotifications => "通知".into(),
            Self::SettingsPageGeneral => "常规".into(),
            Self::SettingsHost => "主机".into(),
            Self::SettingsHostDescription => "要 ping 的主机名或 IP 地址".into(),
            Self::SettingsInterval => "间隔（秒）".into(),
            Self::SettingsTimeout => "超时（毫秒）".into(),
            Self::SettingsTimeoutDescription => {
                "在此时间内无响应即视为丢包。在 macOS 上会向上取整为整秒。".into()
            }
            Self::SettingsThreshold => "连续丢包多少次后通知".into(),
            Self::SettingsNotifyRecovery => "连接恢复时也发送通知".into(),
            Self::SettingsLossSound => "丢包通知时播放声音".into(),
            Self::SettingsLatency => "高延迟时通知".into(),
            Self::SettingsLatencyDescription => "响应时间持续超过阈值时发送通知".into(),
            Self::SettingsLatencyThreshold => "延迟阈值（毫秒）".into(),
            Self::SettingsLatencyConsecutive => "连续超过多少次后通知".into(),
            Self::SettingsLanguage => "显示语言".into(),
            Self::SettingsLanguageAuto => "跟随系统".into(),
            Self::SettingsAutoUpdate => "自动安装新版本".into(),
            Self::SettingsSave => "保存".into(),
            Self::SettingsCancel => "取消".into(),
            Self::SettingsSaveFailed => "无法保存设置".into(),
            Self::SettingsOpenFailedTitle => "无法打开设置窗口".into(),

            Self::StatusPaused => "已暂停".into(),
            Self::StatusUp { host } => format!("{host}: 正常"),
            Self::StatusUpRtt { host, rtt } => format!("{host}: 正常（{}）", rtt_text(rtt)),
            Self::StatusSlow { host } => format!("{host}: 延迟高"),
            Self::StatusSlowRtt { host, rtt } => format!("{host}: 延迟高（{}）", rtt_text(rtt)),
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
            Self::LatencyHighTitle => "检测到高延迟".into(),
            Self::LatencyHighBody { host, rtt } => {
                format!("{host} 的响应时间变长（{}）", rtt_text(rtt))
            }
            Self::LatencyNormalTitle => "延迟已恢复正常".into(),
            Self::LatencyNormalBody { host, secs } => {
                format!("{host} 的响应时间已恢复正常（约 {secs} 秒）")
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
            Self::MenuSilent => "무음 모드 (알림 끄기)".into(),
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
            Self::MenuSettings => "설정…".into(),
            Self::SettingsTitle => "Ping Notifier 설정".into(),
            Self::SettingsPageMonitoring => "모니터링".into(),
            Self::SettingsPageNotifications => "알림".into(),
            Self::SettingsPageGeneral => "일반".into(),
            Self::SettingsHost => "호스트".into(),
            Self::SettingsHostDescription => "ping을 보낼 호스트 이름 또는 IP 주소".into(),
            Self::SettingsInterval => "간격(초)".into(),
            Self::SettingsTimeout => "시간 초과(밀리초)".into(),
            Self::SettingsTimeoutDescription => "이 시간 안에 응답이 없으면 패킷 손실로 간주합니다. macOS에서는 초 단위로 올림됩니다.".into(),
            Self::SettingsThreshold => "몇 번 연속으로 손실되면 알림을 보낼지".into(),
            Self::SettingsNotifyRecovery => "연결이 복구되었을 때도 알림".into(),
            Self::SettingsLossSound => "패킷 손실 알림에서 소리 재생".into(),
            Self::SettingsLatency => "지연 알림".into(),
            Self::SettingsLatencyDescription => "응답 시간이 임곗값을 넘는 상태가 계속되면 알림을 보냅니다".into(),
            Self::SettingsLatencyThreshold => "지연 임곗값(밀리초)".into(),
            Self::SettingsLatencyConsecutive => "몇 번 연속으로 넘으면 알림을 보낼지".into(),
            Self::SettingsLanguage => "표시 언어".into(),
            Self::SettingsLanguageAuto => "OS 언어를 따름".into(),
            Self::SettingsAutoUpdate => "새 버전을 자동으로 설치".into(),
            Self::SettingsSave => "저장".into(),
            Self::SettingsCancel => "취소".into(),
            Self::SettingsSaveFailed => "설정을 저장할 수 없습니다".into(),
            Self::SettingsOpenFailedTitle => "설정 창을 열 수 없습니다".into(),

            Self::StatusPaused => "일시 정지됨".into(),
            Self::StatusUp { host } => format!("{host}: 정상"),
            Self::StatusUpRtt { host, rtt } => format!("{host}: 정상 ({})", rtt_text(rtt)),
            Self::StatusSlow { host } => format!("{host}: 지연"),
            Self::StatusSlowRtt { host, rtt } => format!("{host}: 지연 ({})", rtt_text(rtt)),
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
            Self::LatencyHighTitle => "지연 감지".into(),
            Self::LatencyHighBody { host, rtt } => {
                format!("{host}의 응답 시간이 길어졌습니다. ({})", rtt_text(rtt))
            }
            Self::LatencyNormalTitle => "지연 해소됨".into(),
            Self::LatencyNormalBody { host, secs } => {
                format!("{host}의 응답 시간이 정상으로 돌아왔습니다. (약 {secs}초)")
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

    fn eo(&self) -> String {
        match *self {
            Self::MenuStarting => "Lanĉiĝas…".into(),
            Self::MenuPause => "Paŭzi".into(),
            Self::MenuSilent => "Silenta reĝimo (sen sciigoj)".into(),
            Self::MenuOpenConfig => "Malfermi agordan dosieron".into(),
            Self::MenuReloadConfig => "Reŝargi agordojn".into(),
            Self::MenuLaunchAtLogin => "Lanĉi ĉe ensaluto".into(),
            Self::MenuLaunchAtLoginUnavailable => "Lanĉi ĉe ensaluto (ne disponebla ĉi tie)".into(),
            Self::MenuCheckForUpdates => "Serĉi ĝisdatigojn".into(),
            Self::MenuDownloadUpdate { version } => format!("Elŝuti v{version}…"),
            Self::MenuVersion { version } => format!("Versio {version}"),
            Self::MenuQuit => "Eliri".into(),
            Self::MenuSettings => "Agordoj…".into(),
            Self::SettingsTitle => "Agordoj de Ping Notifier".into(),
            Self::SettingsPageMonitoring => "Kontrolado".into(),
            Self::SettingsPageNotifications => "Sciigoj".into(),
            Self::SettingsPageGeneral => "Ĝeneralaj".into(),
            Self::SettingsHost => "Gastiganto".into(),
            Self::SettingsHostDescription => "Pingota gastiga nomo aŭ IP-adreso".into(),
            Self::SettingsInterval => "Intervalo (sekundoj)".into(),
            Self::SettingsTimeout => "Tempolimo (milisekundoj)".into(),
            Self::SettingsTimeoutDescription => "Se neniu respondo venas dum tiu tempo, tio kalkuliĝas kiel perdo de pakoj. Ĉe macOS tio rondiĝas supren al tutaj sekundoj.".into(),
            Self::SettingsThreshold => "Sinsekvaj perdoj antaŭ sciigo".into(),
            Self::SettingsNotifyRecovery => "Sciigi ankaŭ kiam la konekto restariĝas".into(),
            Self::SettingsLossSound => "Ludi sonon ĉe perdo de pakoj".into(),
            Self::SettingsLatency => "Sciigi pri alta latenco".into(),
            Self::SettingsLatencyDescription => "Sciigi kiam la respondtempo daŭre superas la sojlon".into(),
            Self::SettingsLatencyThreshold => "Sojlo de latenco (milisekundoj)".into(),
            Self::SettingsLatencyConsecutive => "Sinsekvaj malrapidaj respondoj antaŭ sciigo".into(),
            Self::SettingsLanguage => "Lingvo".into(),
            Self::SettingsLanguageAuto => "Laŭ la operaciumo".into(),
            Self::SettingsAutoUpdate => "Aŭtomate instali novajn versiojn".into(),
            Self::SettingsSave => "Konservi".into(),
            Self::SettingsCancel => "Nuligi".into(),
            Self::SettingsSaveFailed => "Ne eblis konservi la agordojn".into(),
            Self::SettingsOpenFailedTitle => "Ne eblis malfermi la agordojn".into(),

            Self::StatusPaused => "Paŭzita".into(),
            Self::StatusUp { host } => format!("{host}: en ordo"),
            Self::StatusUpRtt { host, rtt } => format!("{host}: en ordo ({})", rtt_text(rtt)),
            Self::StatusSlow { host } => format!("{host}: malrapida"),
            Self::StatusSlowRtt { host, rtt } => format!("{host}: malrapida ({})", rtt_text(rtt)),
            Self::StatusDown { host, consecutive } => {
                format!("{host}: neniu respondo ({consecutive} sinsekve)")
            }
            Self::StatusPingError { host } => format!("{host}: ne eblas ruli ping"),

            Self::LostTitle => "Perdo de pakoj detektita".into(),
            Self::LostBody { host, consecutive } => {
                format!("Neniu respondo de {host} ({consecutive} sinsekve)")
            }
            Self::RecoveredTitle => "Konekto restarigita".into(),
            Self::RecoveredBody { host, lost, secs } => {
                format!("{host} denove atingeblas (perditaj: {lost}, ĉirkaŭ {secs} s)")
            }
            Self::LatencyHighTitle => "Alta latenco detektita".into(),
            Self::LatencyHighBody { host, rtt } => {
                format!("La respondtempo de {host} estas alta ({})", rtt_text(rtt))
            }
            Self::LatencyNormalTitle => "Latenco revenis al normalo".into(),
            Self::LatencyNormalBody { host, secs } => {
                format!("La respondtempo de {host} revenis al normalo (ĉirkaŭ {secs} s)")
            }

            Self::UpdateCheckFailedTitle => "Ne eblis serĉi ĝisdatigojn".into(),
            Self::UpToDateTitle => "Vi havas la plej novan version".into(),
            Self::UpToDateBody { version } => format!("v{version} estas la plej nova versio."),
            Self::UpdateAvailableTitle => "Nova versio disponeblas".into(),
            Self::UpdateAvailableBody { version } => {
                format!("v{version} disponeblas. Vi povas elŝuti ĝin per la menuo.")
            }
            Self::UpdatedTitle => "Ĝisdatigita".into(),
            Self::UpdatedBody { from, to } => format!("Ĝisdatigita de v{from} al v{to}."),
            Self::UpdateFailedTitle => "Ĝisdatigo malsukcesis".into(),
            Self::UpdateFailedHint => "Vi povas malfermi la elŝutan paĝon per la menuo.".into(),
            Self::RelaunchFailedTitle => {
                "Ne eblis restartigi. Bonvolu remalfermi la aplikaĵon.".into()
            }

            Self::AutostartSetFailedTitle => "Ne eblis ŝanĝi la lanĉon ĉe ensaluto".into(),
            Self::AutostartApprovalTitle => "Lanĉo ĉe ensaluto bezonas vian permeson".into(),
            Self::AutostartApprovalBody => {
                "Permesu Ping Notifier en Sistemaj agordoj > Ĝenerala > Ensalutaj eroj.".into()
            }

            Self::ConfigLoadFailedUsingDefaultsTitle => {
                "Ne eblis ŝargi la agordojn (uziĝas la defaŭltaj)".into()
            }
            Self::ConfigLoadFailedTitle => "Ne eblis ŝargi la agordojn".into(),
            Self::ConfigOpenFailedTitle => "Ne eblis malfermi la agordan dosieron".into(),
            Self::DownloadPageOpenFailedTitle => "Ne eblis malfermi la elŝutan paĝon".into(),
            Self::TrayCreateFailedTitle => "Ne eblis krei la pletan piktogramon".into(),
        }
    }

    fn de(&self) -> String {
        match *self {
            Self::MenuStarting => "Wird gestartet …".into(),
            Self::MenuPause => "Pausieren".into(),
            Self::MenuSilent => "Stiller Modus (keine Benachrichtigungen)".into(),
            Self::MenuOpenConfig => "Einstellungsdatei öffnen".into(),
            Self::MenuReloadConfig => "Einstellungen neu laden".into(),
            Self::MenuLaunchAtLogin => "Beim Anmelden starten".into(),
            Self::MenuLaunchAtLoginUnavailable => {
                "Beim Anmelden starten (hier nicht verfügbar)".into()
            }
            Self::MenuCheckForUpdates => "Nach Updates suchen".into(),
            Self::MenuDownloadUpdate { version } => format!("v{version} herunterladen …"),
            Self::MenuVersion { version } => format!("Version {version}"),
            Self::MenuQuit => "Beenden".into(),
            Self::MenuSettings => "Einstellungen…".into(),
            Self::SettingsTitle => "Ping Notifier – Einstellungen".into(),
            Self::SettingsPageMonitoring => "Überwachung".into(),
            Self::SettingsPageNotifications => "Benachrichtigungen".into(),
            Self::SettingsPageGeneral => "Allgemein".into(),
            Self::SettingsHost => "Host".into(),
            Self::SettingsHostDescription => "Hostname oder IP-Adresse, die angepingt wird".into(),
            Self::SettingsInterval => "Intervall (Sekunden)".into(),
            Self::SettingsTimeout => "Zeitlimit (Millisekunden)".into(),
            Self::SettingsTimeoutDescription => "Bleibt in dieser Zeit eine Antwort aus, gilt das als Paketverlust. Unter macOS wird auf ganze Sekunden aufgerundet.".into(),
            Self::SettingsThreshold => "Verluste in Folge bis zur Benachrichtigung".into(),
            Self::SettingsNotifyRecovery => "Auch benachrichtigen, wenn die Verbindung wiederhergestellt ist".into(),
            Self::SettingsLossSound => "Bei Paketverlust einen Ton abspielen".into(),
            Self::SettingsLatency => "Bei hoher Latenz benachrichtigen".into(),
            Self::SettingsLatencyDescription => "Benachrichtigen, wenn die Antwortzeit dauerhaft über dem Schwellenwert liegt".into(),
            Self::SettingsLatencyThreshold => "Latenz-Schwellenwert (Millisekunden)".into(),
            Self::SettingsLatencyConsecutive => "Langsame Antworten in Folge bis zur Benachrichtigung".into(),
            Self::SettingsLanguage => "Sprache".into(),
            Self::SettingsLanguageAuto => "Wie das Betriebssystem".into(),
            Self::SettingsAutoUpdate => "Neue Versionen automatisch installieren".into(),
            Self::SettingsSave => "Speichern".into(),
            Self::SettingsCancel => "Abbrechen".into(),
            Self::SettingsSaveFailed => "Die Einstellungen konnten nicht gespeichert werden".into(),
            Self::SettingsOpenFailedTitle => "Das Einstellungsfenster konnte nicht geöffnet werden".into(),

            Self::StatusPaused => "Pausiert".into(),
            Self::StatusUp { host } => format!("{host}: OK"),
            Self::StatusUpRtt { host, rtt } => format!("{host}: OK ({})", rtt_text(rtt)),
            Self::StatusSlow { host } => format!("{host}: langsam"),
            Self::StatusSlowRtt { host, rtt } => format!("{host}: langsam ({})", rtt_text(rtt)),
            Self::StatusDown { host, consecutive } => {
                format!("{host}: keine Antwort ({consecutive}-mal in Folge)")
            }
            Self::StatusPingError { host } => format!("{host}: ping kann nicht ausgeführt werden"),

            Self::LostTitle => "Paketverlust erkannt".into(),
            Self::LostBody { host, consecutive } => {
                format!("Keine Antwort von {host} ({consecutive}-mal in Folge)")
            }
            Self::RecoveredTitle => "Verbindung wiederhergestellt".into(),
            Self::RecoveredBody { host, lost, secs } => {
                format!("{host} ist wieder erreichbar ({lost} verloren, etwa {secs} s)")
            }
            Self::LatencyHighTitle => "Hohe Latenz erkannt".into(),
            Self::LatencyHighBody { host, rtt } => {
                format!("Die Antwortzeit von {host} ist hoch ({})", rtt_text(rtt))
            }
            Self::LatencyNormalTitle => "Latenz wieder normal".into(),
            Self::LatencyNormalBody { host, secs } => {
                format!("Die Antwortzeit von {host} ist wieder normal (etwa {secs} s)")
            }

            Self::UpdateCheckFailedTitle => "Suche nach Updates fehlgeschlagen".into(),
            Self::UpToDateTitle => "Die App ist auf dem neuesten Stand".into(),
            Self::UpToDateBody { version } => format!("v{version} ist die neueste Version."),
            Self::UpdateAvailableTitle => "Eine neue Version ist verfügbar".into(),
            Self::UpdateAvailableBody { version } => {
                format!("v{version} ist verfügbar. Du kannst sie über das Menü herunterladen.")
            }
            Self::UpdatedTitle => "Aktualisiert".into(),
            Self::UpdatedBody { from, to } => format!("Von v{from} auf v{to} aktualisiert."),
            Self::UpdateFailedTitle => "Update fehlgeschlagen".into(),
            Self::UpdateFailedHint => "Du kannst die Download-Seite über das Menü öffnen.".into(),
            Self::RelaunchFailedTitle => {
                "Neustart fehlgeschlagen. Bitte öffne die App erneut.".into()
            }

            Self::AutostartSetFailedTitle => {
                "„Beim Anmelden starten“ konnte nicht geändert werden".into()
            }
            Self::AutostartApprovalTitle => "„Beim Anmelden starten“ muss erlaubt werden".into(),
            Self::AutostartApprovalBody => {
                "Erlaube Ping Notifier unter Systemeinstellungen > Allgemein > Anmeldeobjekte."
                    .into()
            }

            Self::ConfigLoadFailedUsingDefaultsTitle => {
                "Einstellungen konnten nicht geladen werden (Standardwerte werden verwendet)".into()
            }
            Self::ConfigLoadFailedTitle => "Einstellungen konnten nicht geladen werden".into(),
            Self::ConfigOpenFailedTitle => {
                "Die Einstellungsdatei konnte nicht geöffnet werden".into()
            }
            Self::DownloadPageOpenFailedTitle => {
                "Die Download-Seite konnte nicht geöffnet werden".into()
            }
            Self::TrayCreateFailedTitle => "Das Statussymbol konnte nicht erstellt werden".into(),
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
        assert_eq!(from_locales(locales(&["fr-FR", "it-IT"])), Lang::En);
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

        let msg = Msg::StatusUpRtt {
            host: "8.8.8.8",
            rtt: Duration::from_micros(12_345),
        };
        assert_eq!(msg.in_lang(Lang::En), "8.8.8.8: OK (12 ms)");
        assert_eq!(msg.in_lang(Lang::Ja), "8.8.8.8: 正常（12 ms）");
        let msg = Msg::StatusUpRtt {
            host: "8.8.8.8",
            rtt: Duration::from_micros(450),
        };
        assert_eq!(msg.in_lang(Lang::En), "8.8.8.8: OK (<1 ms)");

        let msg = Msg::StatusSlowRtt {
            host: "8.8.8.8",
            rtt: Duration::from_millis(350),
        };
        assert_eq!(msg.in_lang(Lang::En), "8.8.8.8: slow (350 ms)");
        assert_eq!(msg.in_lang(Lang::Ja), "8.8.8.8: 遅延（350 ms）");

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

    #[test]
    fn supports_esperanto() {
        assert_eq!(from_locale("eo"), Some(Lang::Eo));
        assert_eq!(from_locale("eo-001"), Some(Lang::Eo));
        assert_eq!(LanguageSetting::Eo.resolve(), Lang::Eo);

        let msg = Msg::StatusDown {
            host: "8.8.8.8",
            consecutive: 3,
        };
        assert_eq!(
            msg.in_lang(Lang::Eo),
            "8.8.8.8: neniu respondo (3 sinsekve)"
        );
    }

    #[test]
    fn supports_german() {
        assert_eq!(from_locale("de-DE"), Some(Lang::De));
        assert_eq!(from_locale("de-AT"), Some(Lang::De));
        assert_eq!(from_locale("de_CH"), Some(Lang::De));
        assert_eq!(LanguageSetting::De.resolve(), Lang::De);

        let msg = Msg::StatusDown {
            host: "8.8.8.8",
            consecutive: 3,
        };
        assert_eq!(
            msg.in_lang(Lang::De),
            "8.8.8.8: keine Antwort (3-mal in Folge)"
        );
    }
}
