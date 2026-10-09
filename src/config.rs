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

/// 初回起動時に書き出す設定ファイル。コメントを残すため serde で生成せず手書きしている
const DEFAULT_CONFIG: &str = r#"# ping-notifier の設定
# 変更後はメニューの「設定を再読み込み」で反映されます

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
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub host: String,
    pub interval_secs: f64,
    pub timeout_ms: u64,
    pub threshold: u32,
    pub notify_recovery: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "8.8.8.8".into(),
            interval_secs: 1.0,
            timeout_ms: 1000,
            threshold: 1,
            notify_recovery: true,
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
        ensure!(!self.host.trim().is_empty(), "host が空です");
        ensure!(
            self.interval_secs.is_finite() && self.interval_secs > 0.0,
            "interval_secs には正の数を指定してください"
        );
        ensure!(
            self.timeout_ms > 0,
            "timeout_ms には正の数を指定してください"
        );
        ensure!(
            self.threshold > 0,
            "threshold には 1 以上を指定してください"
        );
        Ok(())
    }
}

pub fn path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("設定ディレクトリが見つかりません")?;
    Ok(dir.join("ping-notifier").join("config.toml"))
}

/// 設定ファイルを読み込む。存在しなければ既定値で作成する
pub fn load_or_create(path: &Path) -> Result<Config> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("{} を作成できません", dir.display()))?;
        }
        fs::write(path, DEFAULT_CONFIG)
            .with_context(|| format!("{} を作成できません", path.display()))?;
    }

    let text =
        fs::read_to_string(path).with_context(|| format!("{} を読めません", path.display()))?;
    let config: Config = toml::from_str(&text)
        .with_context(|| format!("{} の形式が正しくありません", path.display()))?;
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_file_matches_default_values() {
        let parsed: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let default = Config::default();
        assert_eq!(parsed.host, default.host);
        assert_eq!(parsed.interval_secs, default.interval_secs);
        assert_eq!(parsed.timeout_ms, default.timeout_ms);
        assert_eq!(parsed.threshold, default.threshold);
        assert_eq!(parsed.notify_recovery, default.notify_recovery);
    }

    #[test]
    fn rejects_invalid_values() {
        let config: Config = toml::from_str("interval_secs = 0").unwrap();
        assert!(config.validate().is_err());
        assert!(toml::from_str::<Config>("unknown = 1").is_err());
    }
}
