//! 設定ファイル（TOML）の読み書き。
//!
//! 保存場所は OS ごとの設定ディレクトリ配下:
//! - macOS: `~/Library/Application Support/ping-notifier/config.toml`
//! - Windows: `%APPDATA%\ping-notifier\config.toml`

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::i18n::{self, Lang, LanguageSetting};

/// 初回起動時に書き出す設定ファイル。コメントを残すため serde で生成せず手書きしている。
/// 作成時の表示言語（OS の言語）に合わせて英語版か日本語版を選ぶ
const DEFAULT_CONFIG_EN: &str = r#"# Ping Notifier settings
# After editing, choose "Reload Settings" from the menu to apply your changes.

# Display language: "auto" (follow the OS) or one of "en", "ja", "zh", "ko"
language = "auto"

# Host name or IP address to monitor
host = "8.8.8.8"

# Interval between pings (seconds)
interval_secs = 1.0

# How long to wait for a reply (milliseconds). No reply within this time counts as packet loss.
# On macOS this is rounded up to whole seconds.
timeout_ms = 1000

# Number of consecutive losses before notifying
threshold = 1

# Also notify when the connection is restored
notify_recovery = true

# Install new versions automatically
# Even when false, you can update manually with "Check for Updates" in the menu.
auto_update = true
"#;

const DEFAULT_CONFIG_JA: &str = r#"# Ping Notifier の設定
# 変更後はメニューの「設定を再読み込み」で反映されます

# 表示言語: "auto"（OS の言語に合わせる）または "en"、"ja"、"zh"、"ko"
language = "auto"

# 監視対象のホスト名または IP アドレス
host = "8.8.8.8"

# ping を送る間隔（秒）
interval_secs = 1.0

# 応答を待つ時間（ミリ秒）。これを超えるとロスとみなす
# macOS では秒単位に切り上げられます
timeout_ms = 1000

# 何回連続でロスしたら通知するか
threshold = 1

# 復旧時にも通知するか
notify_recovery = true

# 新しいバージョンを自動でインストールするか
# false でもメニューの「アップデートを確認」から手動で更新できます
auto_update = true
"#;

const DEFAULT_CONFIG_ZH: &str = r#"# Ping Notifier 设置
# 修改后，请在菜单中选择“重新加载设置”使更改生效。

# 显示语言："auto"（跟随系统）或以下之一："en"、"ja"、"zh"、"ko"
language = "auto"

# 要监控的主机名或 IP 地址
host = "8.8.8.8"

# 发送 ping 的间隔（秒）
interval_secs = 1.0

# 等待响应的时间（毫秒）。超过此时间仍无响应即视为丢包。
# 在 macOS 上会向上取整为整秒。
timeout_ms = 1000

# 连续丢包多少次后发送通知
threshold = 1

# 连接恢复时是否也发送通知
notify_recovery = true

# 是否自动安装新版本
# 即使设为 false，也可以通过菜单中的“检查更新”手动更新。
auto_update = true
"#;

const DEFAULT_CONFIG_KO: &str = r#"# Ping Notifier 설정
# 수정한 후 메뉴에서 "설정 다시 불러오기"를 선택하면 반영됩니다.

# 표시 언어: "auto"(OS 언어를 따름) 또는 다음 중 하나: "en", "ja", "zh", "ko"
language = "auto"

# 모니터링할 호스트 이름 또는 IP 주소
host = "8.8.8.8"

# ping을 보내는 간격(초)
interval_secs = 1.0

# 응답을 기다리는 시간(밀리초). 이 시간 안에 응답이 없으면 패킷 손실로 간주합니다.
# macOS에서는 초 단위로 올림됩니다.
timeout_ms = 1000

# 몇 번 연속으로 손실되면 알림을 보낼지
threshold = 1

# 연결이 복구되었을 때도 알림을 보낼지
notify_recovery = true

# 새 버전을 자동으로 설치할지
# false로 설정해도 메뉴의 "업데이트 확인"으로 수동 업데이트할 수 있습니다.
auto_update = true
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub host: String,
    pub interval_secs: f64,
    pub timeout_ms: u64,
    pub threshold: u32,
    pub notify_recovery: bool,
    pub auto_update: bool,
    pub language: LanguageSetting,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "8.8.8.8".into(),
            interval_secs: 1.0,
            timeout_ms: 1000,
            threshold: 1,
            notify_recovery: true,
            auto_update: true,
            language: LanguageSetting::Auto,
        }
    }
}

impl Config {
    pub fn interval(&self) -> Duration {
        Duration::from_secs_f64(self.interval_secs)
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    fn validate(&self) -> Result<()> {
        ensure!(!self.host.trim().is_empty(), "host must not be empty");
        ensure!(
            self.interval_secs.is_finite() && self.interval_secs > 0.0,
            "interval_secs must be a positive number"
        );
        ensure!(self.timeout_ms > 0, "timeout_ms must be a positive number");
        ensure!(self.threshold > 0, "threshold must be at least 1");
        Ok(())
    }
}

pub fn path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("config directory not found")?;
    Ok(dir.join("ping-notifier").join("config.toml"))
}

fn default_config(lang: Lang) -> &'static str {
    match lang {
        Lang::En => DEFAULT_CONFIG_EN,
        Lang::Ja => DEFAULT_CONFIG_JA,
        Lang::Zh => DEFAULT_CONFIG_ZH,
        Lang::Ko => DEFAULT_CONFIG_KO,
    }
}

/// 設定ファイルを読み込む。存在しなければ現在の表示言語のテンプレートで作成する
pub fn load_or_create(path: &Path) -> Result<Config> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        fs::write(path, default_config(i18n::current()))
            .with_context(|| format!("cannot create {}", path.display()))?;
    }

    let text =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let config: Config =
        toml::from_str(&text).with_context(|| format!("invalid format in {}", path.display()))?;
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_files_match_default_values() {
        let default = Config::default();
        for lang in Lang::ALL {
            let parsed: Config = toml::from_str(default_config(lang)).unwrap();
            assert_eq!(parsed.host, default.host, "{lang:?}");
            assert_eq!(parsed.interval_secs, default.interval_secs, "{lang:?}");
            assert_eq!(parsed.timeout_ms, default.timeout_ms, "{lang:?}");
            assert_eq!(parsed.threshold, default.threshold, "{lang:?}");
            assert_eq!(parsed.notify_recovery, default.notify_recovery, "{lang:?}");
            assert_eq!(parsed.auto_update, default.auto_update, "{lang:?}");
            assert_eq!(parsed.language, default.language, "{lang:?}");
        }
    }

    #[test]
    fn parses_language_setting() {
        let config: Config = toml::from_str(r#"language = "ja""#).unwrap();
        assert_eq!(config.language, LanguageSetting::Ja);
        assert!(toml::from_str::<Config>(r#"language = "fr""#).is_err());
    }

    #[test]
    fn rejects_invalid_values() {
        let config: Config = toml::from_str("interval_secs = 0").unwrap();
        assert!(config.validate().is_err());
        assert!(toml::from_str::<Config>("unknown = 1").is_err());
    }
}
