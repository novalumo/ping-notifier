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
use toml_edit::{DocumentMut, Item, Value};

use crate::i18n::{self, Lang, LanguageSetting};

/// 初回起動時に書き出す設定ファイル。コメントを残すため serde で生成せず手書きしている。
/// 作成時の表示言語（OS の言語）に合わせて英語版か日本語版を選ぶ
const DEFAULT_CONFIG_EN: &str = r#"# Ping Notifier settings
# After editing, choose "Reload Settings" from the menu to apply your changes.

# Display language: "auto" (follow the OS) or one of "en", "ja", "zh", "ko", "eo", "de"
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

# Play a sound with the packet loss notification (follows the OS notification sound settings)
loss_sound = true

# Notify when the response time stays above this value (milliseconds).
# Disabled by default. Remove the leading # to enable.
# latency_threshold_ms = 200

# Number of consecutive slow replies before notifying (the same count is used to decide it is back to normal)
latency_consecutive = 5

# Install new versions automatically
# Even when false, you can update manually with "Check for Updates" in the menu.
auto_update = true
"#;

const DEFAULT_CONFIG_JA: &str = r#"# Ping Notifier の設定
# 変更後はメニューの「設定を再読み込み」で反映されます

# 表示言語: "auto"（OS の言語に合わせる）または "en"、"ja"、"zh"、"ko"、"eo"、"de"
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

# パケットロスの通知で音を鳴らすか（OS の通知音の設定に従います）
loss_sound = true

# 応答時間がこの値（ミリ秒）を超える状態が続いたら通知する
# 既定では無効。行頭の # を外すと有効になります
# latency_threshold_ms = 200

# 何回連続でしきい値を超えたら通知するか（元に戻ったと判定するときも同じ回数）
latency_consecutive = 5

# 新しいバージョンを自動でインストールするか
# false でもメニューの「アップデートを確認」から手動で更新できます
auto_update = true
"#;

const DEFAULT_CONFIG_ZH: &str = r#"# Ping Notifier 设置
# 修改后，请在菜单中选择“重新加载设置”使更改生效。

# 显示语言："auto"（跟随系统）或以下之一："en"、"ja"、"zh"、"ko"、"eo"、"de"
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

# 丢包通知时是否播放声音（遵循系统的通知声音设置）
loss_sound = true

# 响应时间持续超过此值（毫秒）时发送通知
# 默认禁用。删除行首的 # 即可启用。
# latency_threshold_ms = 200

# 连续多少次超过阈值后发送通知（判断恢复正常时也使用相同次数）
latency_consecutive = 5

# 是否自动安装新版本
# 即使设为 false，也可以通过菜单中的“检查更新”手动更新。
auto_update = true
"#;

const DEFAULT_CONFIG_KO: &str = r#"# Ping Notifier 설정
# 수정한 후 메뉴에서 "설정 다시 불러오기"를 선택하면 반영됩니다.

# 표시 언어: "auto"(OS 언어를 따름) 또는 다음 중 하나: "en", "ja", "zh", "ko", "eo", "de"
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

# 패킷 손실 알림에서 소리를 낼지 (OS의 알림 소리 설정을 따릅니다)
loss_sound = true

# 응답 시간이 이 값(밀리초)을 넘는 상태가 계속되면 알림을 보낼지
# 기본값은 비활성화입니다. 줄 앞의 #을 지우면 활성화됩니다.
# latency_threshold_ms = 200

# 몇 번 연속으로 임곗값을 넘으면 알림을 보낼지 (정상으로 돌아왔다고 판정할 때도 같은 횟수)
latency_consecutive = 5

# 새 버전을 자동으로 설치할지
# false로 설정해도 메뉴의 "업데이트 확인"으로 수동 업데이트할 수 있습니다.
auto_update = true
"#;

const DEFAULT_CONFIG_EO: &str = r#"# Agordoj de Ping Notifier
# Post redaktado, elektu "Reŝargi agordojn" en la menuo por apliki la ŝanĝojn.

# Lingvo de la interfaco: "auto" (laŭ la operaciumo) aŭ unu el "en", "ja", "zh", "ko", "eo", "de"
language = "auto"

# Kontrolota gastiga nomo aŭ IP-adreso
host = "8.8.8.8"

# Intervalo inter pingoj (sekundoj)
interval_secs = 1.0

# Kiom longe atendi respondon (milisekundoj). Se neniu respondo venas dum tiu tempo, tio kalkuliĝas kiel perdo de pakoj.
# Ĉe macOS tio rondiĝas supren al tutaj sekundoj.
timeout_ms = 1000

# Nombro de sinsekvaj perdoj antaŭ sciigo
threshold = 1

# Sciigi ankaŭ kiam la konekto restariĝas
notify_recovery = true

# Ludi sonon kun la sciigo pri perdo de pakoj (laŭ la sonaj agordoj de sciigoj en la operaciumo)
loss_sound = true

# Sciigi kiam la respondtempo daŭre superas ĉi tiun valoron (milisekundoj).
# Malŝaltita defaŭlte. Forigu la komencan # por ŝalti ĝin.
# latency_threshold_ms = 200

# Nombro de sinsekvaj malrapidaj respondoj antaŭ sciigo (la sama nombro validas por decidi, ke ĝi revenis al normalo)
latency_consecutive = 5

# Aŭtomate instali novajn versiojn
# Eĉ se false, vi povas ĝisdatigi permane per "Serĉi ĝisdatigojn" en la menuo.
auto_update = true
"#;

const DEFAULT_CONFIG_DE: &str = r#"# Einstellungen für Ping Notifier
# Wähle nach dem Bearbeiten „Einstellungen neu laden“ im Menü, um die Änderungen zu übernehmen.

# Anzeigesprache: "auto" (wie das Betriebssystem) oder einer der Werte "en", "ja", "zh", "ko", "eo", "de"
language = "auto"

# Zu überwachender Host (Hostname oder IP-Adresse)
host = "8.8.8.8"

# Abstand zwischen den Pings (Sekunden)
interval_secs = 1.0

# Wie lange auf eine Antwort gewartet wird (Millisekunden). Bleibt sie in dieser Zeit aus, gilt das als Paketverlust.
# Unter macOS wird auf ganze Sekunden aufgerundet.
timeout_ms = 1000

# Anzahl aufeinanderfolgender Verluste, ab der benachrichtigt wird
threshold = 1

# Auch benachrichtigen, wenn die Verbindung wiederhergestellt ist
notify_recovery = true

# Bei der Benachrichtigung über Paketverlust einen Ton abspielen (richtet sich nach den Benachrichtigungstönen des Betriebssystems)
loss_sound = true

# Benachrichtigen, wenn die Antwortzeit dauerhaft über diesem Wert liegt (Millisekunden).
# Standardmäßig deaktiviert. Entferne das # am Zeilenanfang, um es zu aktivieren.
# latency_threshold_ms = 200

# Anzahl aufeinanderfolgender langsamer Antworten, ab der benachrichtigt wird (gilt auch für die Rückkehr zum Normalzustand)
latency_consecutive = 5

# Neue Versionen automatisch installieren
# Auch bei false kannst du über „Nach Updates suchen“ im Menü manuell aktualisieren.
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
    /// パケットロスの通知で音を鳴らすか
    pub loss_sound: bool,
    /// 応答時間のしきい値（ミリ秒）。`None` なら遅延の通知は無効
    pub latency_threshold_ms: Option<u64>,
    pub latency_consecutive: u32,
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
            loss_sound: true,
            latency_threshold_ms: None,
            latency_consecutive: 5,
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

    pub fn latency_threshold(&self) -> Option<Duration> {
        self.latency_threshold_ms.map(Duration::from_millis)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.host.trim().is_empty(), "host must not be empty");
        ensure!(
            self.interval_secs.is_finite() && self.interval_secs > 0.0,
            "interval_secs must be a positive number"
        );
        ensure!(self.timeout_ms > 0, "timeout_ms must be a positive number");
        ensure!(self.threshold > 0, "threshold must be at least 1");
        ensure!(
            self.latency_threshold_ms != Some(0),
            "latency_threshold_ms must be a positive number"
        );
        ensure!(
            self.latency_consecutive > 0,
            "latency_consecutive must be at least 1"
        );
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
        Lang::Eo => DEFAULT_CONFIG_EO,
        Lang::De => DEFAULT_CONFIG_DE,
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

/// 設定画面で編集した値を設定ファイルに書く。
///
/// 手書きのテンプレートのコメントや並び、利用者が書いたコメントを残すため、
/// ファイルを `toml_edit` で読み、値が変わったキーだけを書き換える。
/// ファイルがなければ、現在の表示言語のテンプレートを元にする
pub fn save(path: &Path, config: &Config) -> Result<()> {
    config.validate()?;
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            default_config(i18n::current()).to_owned()
        }
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    let updated = apply_to_text(&text, config)
        .with_context(|| format!("invalid format in {}", path.display()))?;
    if updated == text && path.exists() {
        return Ok(());
    }

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    // 書きかけのファイルを読まれないよう、一時ファイルに書いてから置き換える
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &updated).with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// 設定ファイルの文字列に `config` の値を反映した文字列を返す
fn apply_to_text(text: &str, config: &Config) -> Result<String> {
    let mut doc: DocumentMut = text.parse()?;
    set(&mut doc, "language", config.language.as_str());
    set(&mut doc, "host", config.host.as_str());
    set(&mut doc, "interval_secs", config.interval_secs);
    set(&mut doc, "timeout_ms", config.timeout_ms as i64);
    set(&mut doc, "threshold", i64::from(config.threshold));
    set(&mut doc, "notify_recovery", config.notify_recovery);
    set(&mut doc, "loss_sound", config.loss_sound);
    set_optional(
        &mut doc,
        "latency_threshold_ms",
        config.latency_threshold_ms.map(|ms| ms as i64),
    );
    set(
        &mut doc,
        "latency_consecutive",
        i64::from(config.latency_consecutive),
    );
    set(&mut doc, "auto_update", config.auto_update);
    Ok(doc.to_string())
}

/// 値が変わっていれば、行末のコメントなどの装飾を残したまま書き換える。キーがなければ末尾に足す
fn set(doc: &mut DocumentMut, key: &str, value: impl Into<Value>) {
    let mut value = value.into();
    match doc.get_mut(key).and_then(Item::as_value_mut) {
        Some(current) if same_value(current, &value) => {}
        Some(current) => {
            *value.decor_mut() = current.decor().clone();
            *current = value;
        }
        None => {
            doc.insert(key, Item::Value(value));
        }
    }
}

/// `1` と `1.0` のように書き方が違うだけの値は書き換えない
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a.value() == b.value(),
        (Value::Integer(a), Value::Integer(b)) => a.value() == b.value(),
        (Value::Boolean(a), Value::Boolean(b)) => a.value() == b.value(),
        (Value::Float(a), Value::Float(b)) => a.value() == b.value(),
        (Value::Integer(a), Value::Float(b)) | (Value::Float(b), Value::Integer(a)) => {
            *a.value() as f64 == *b.value()
        }
        _ => false,
    }
}

/// 省略できるキー（`latency_threshold_ms`）を書く。
///
/// テンプレートでは `# latency_threshold_ms = 200` のようにコメントアウトして載せているので、
/// 有効にするときはその行を置き換え、無効にするときはその場でコメントアウトに戻す
fn set_optional(doc: &mut DocumentMut, key: &str, value: Option<i64>) {
    match value {
        Some(value) if doc.contains_key(key) => set(doc, key, value),
        Some(value) => uncomment(doc, key, value),
        None if doc.contains_key(key) => comment_out(doc, key),
        None => {}
    }
}

fn is_commented_key(line: &str, key: &str) -> bool {
    line.trim_start()
        .strip_prefix('#')
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix(key))
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

fn keys(doc: &DocumentMut) -> Vec<String> {
    doc.iter().map(|(k, _)| k.to_owned()).collect()
}

fn prefix_of(doc: &DocumentMut, key: &str) -> String {
    doc.as_table()
        .key(key)
        .and_then(|k| k.leaf_decor().prefix())
        .and_then(|p| p.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn set_prefix(doc: &mut DocumentMut, key: &str, prefix: String) {
    if let Some(mut k) = doc.as_table_mut().key_mut(key) {
        k.leaf_decor_mut().set_prefix(prefix);
    }
}

fn uncomment(doc: &mut DocumentMut, key: &str, value: i64) {
    // コメントアウトされた行は、次のキーの前置き（prefix）か、文書の末尾に入っている
    let keys = keys(doc);
    let found = keys.iter().find_map(|next| {
        let prefix = prefix_of(doc, next);
        let start = line_start_of_commented_key(&prefix, key)?;
        Some((Some(next.clone()), prefix, start))
    });
    let found = found.or_else(|| {
        let trailing = doc.trailing().as_str().unwrap_or_default().to_owned();
        let start = line_start_of_commented_key(&trailing, key)?;
        Some((None, trailing, start))
    });

    doc.insert(key, Item::Value(value.into()));
    let Some((next, text, start)) = found else {
        return;
    };
    // コメントアウトされた行の手前（説明のコメント）を新しいキーの前置きに、行の後ろを元の場所に残す
    let end = text[start..]
        .find('\n')
        .map_or(text.len(), |i| start + i + 1);
    let (before, after) = (text[..start].to_owned(), text[end..].to_owned());
    set_prefix(doc, key, before);
    match next {
        Some(next) => {
            set_prefix(doc, &next, after);
            // 末尾に足されたキーを、元の行の位置（次のキーの直前）へ移す
            let mut order: Vec<String> = keys.clone();
            let at = order.iter().position(|k| *k == next).unwrap_or(order.len());
            order.insert(at, key.to_owned());
            doc.as_table_mut().sort_values_by(|a, _, b, _| {
                let pos = |k: &str| order.iter().position(|o| o == k);
                pos(a.get()).cmp(&pos(b.get()))
            });
        }
        None => doc.set_trailing(after),
    }
}

fn line_start_of_commented_key(text: &str, key: &str) -> Option<usize> {
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        if is_commented_key(line, key) {
            return Some(start);
        }
        start += line.len();
    }
    None
}

fn comment_out(doc: &mut DocumentMut, key: &str) {
    let keys = keys(doc);
    let Some(value) = doc.get(key).and_then(Item::as_value).map(|v| v.to_string()) else {
        return;
    };
    let line = format!(
        "{prefix}# {key} = {}\n",
        value.trim(),
        prefix = prefix_of(doc, key)
    );
    let next = keys
        .iter()
        .skip_while(|k| k.as_str() != key)
        .nth(1)
        .cloned();
    doc.remove(key);
    match next {
        Some(next) => {
            let prefix = prefix_of(doc, &next);
            set_prefix(doc, &next, format!("{line}{prefix}"));
        }
        None => {
            let trailing = doc.trailing().as_str().unwrap_or_default().to_owned();
            doc.set_trailing(format!("{line}{trailing}"));
        }
    }
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
            assert_eq!(parsed.loss_sound, default.loss_sound, "{lang:?}");
            assert_eq!(
                parsed.latency_threshold_ms, default.latency_threshold_ms,
                "{lang:?}"
            );
            assert_eq!(
                parsed.latency_consecutive, default.latency_consecutive,
                "{lang:?}"
            );
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
        let config: Config = toml::from_str("latency_threshold_ms = 0").unwrap();
        assert!(config.validate().is_err());
        let config: Config = toml::from_str("latency_consecutive = 0").unwrap();
        assert!(config.validate().is_err());
        assert!(toml::from_str::<Config>("unknown = 1").is_err());
    }

    #[test]
    fn latency_notification_is_disabled_unless_configured() {
        assert_eq!(Config::default().latency_threshold(), None);
        // テンプレートのコメントを外したときの値が読めること
        for lang in Lang::ALL {
            let enabled = default_config(lang)
                .replace("# latency_threshold_ms = 200", "latency_threshold_ms = 200");
            let parsed: Config = toml::from_str(&enabled).unwrap();
            assert_eq!(
                parsed.latency_threshold(),
                Some(Duration::from_millis(200)),
                "{lang:?}"
            );
        }
    }

    fn with_latency(config: &Config, ms: Option<u64>) -> Config {
        Config {
            latency_threshold_ms: ms,
            ..config.clone()
        }
    }

    #[test]
    fn saving_unchanged_values_keeps_every_template_as_is() {
        for lang in Lang::ALL {
            let text = default_config(lang);
            assert_eq!(
                apply_to_text(text, &Config::default()).unwrap(),
                text,
                "{lang:?}"
            );
        }
    }

    #[test]
    fn saving_changes_values_and_keeps_comments() {
        let text = default_config(Lang::Ja);
        let config = Config {
            host: "192.168.1.1".into(),
            interval_secs: 2.5,
            timeout_ms: 800,
            threshold: 3,
            notify_recovery: false,
            loss_sound: false,
            latency_consecutive: 4,
            auto_update: false,
            language: LanguageSetting::En,
            ..Config::default()
        };
        let saved = apply_to_text(text, &config).unwrap();
        let parsed: Config = toml::from_str(&saved).unwrap();
        assert_eq!(parsed.host, "192.168.1.1");
        assert_eq!(parsed.interval_secs, 2.5);
        assert_eq!(parsed.timeout_ms, 800);
        assert_eq!(parsed.threshold, 3);
        assert!(!parsed.notify_recovery);
        assert!(!parsed.loss_sound);
        assert_eq!(parsed.latency_consecutive, 4);
        assert!(!parsed.auto_update);
        assert_eq!(parsed.language, LanguageSetting::En);
        // コメント行はすべて残る
        let comments = |t: &str| {
            t.lines()
                .filter(|l| l.starts_with('#'))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert_eq!(comments(&saved), comments(text));
        // 書き換えた行だけが変わる
        assert!(saved.contains("host = \"192.168.1.1\"\n"));
        assert_eq!(saved.lines().count(), text.lines().count());
    }

    #[test]
    fn keeps_inline_comments_and_integer_style() {
        let text = "host = \"a\" # router\ninterval_secs = 1\n";
        let config = Config {
            host: "b".into(),
            ..Config::default()
        };
        let saved = apply_to_text(text, &config).unwrap();
        assert!(saved.starts_with("host = \"b\" # router\ninterval_secs = 1\n"));
    }

    #[test]
    fn latency_threshold_is_uncommented_in_place_and_commented_back() {
        for lang in Lang::ALL {
            let text = default_config(lang);
            let enabled =
                apply_to_text(text, &with_latency(&Config::default(), Some(300))).unwrap();
            // コメントアウトされていた行が、その場で有効な行になる
            assert_eq!(
                enabled,
                text.replace("# latency_threshold_ms = 200", "latency_threshold_ms = 300"),
                "{lang:?}"
            );
            let parsed: Config = toml::from_str(&enabled).unwrap();
            assert_eq!(parsed.latency_threshold_ms, Some(300));

            // 無効に戻すと、その場でコメントアウトされる
            let disabled = apply_to_text(&enabled, &Config::default()).unwrap();
            assert_eq!(
                disabled,
                text.replace(
                    "# latency_threshold_ms = 200",
                    "# latency_threshold_ms = 300"
                ),
                "{lang:?}"
            );
        }
    }

    #[test]
    fn latency_threshold_is_appended_when_there_is_no_commented_line() {
        let saved = apply_to_text(
            "host = \"a\"\n",
            &with_latency(&Config::default(), Some(150)),
        )
        .unwrap();
        let parsed: Config = toml::from_str(&saved).unwrap();
        assert_eq!(parsed.latency_threshold_ms, Some(150));
    }

    #[test]
    fn save_writes_file_and_rejects_invalid_values() {
        let dir = std::env::temp_dir().join(format!("ping-notifier-save-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);

        // ファイルがなければテンプレートを元に作る
        save(&path, &with_latency(&Config::default(), Some(250))).unwrap();
        let loaded = load_or_create(&path).unwrap();
        assert_eq!(loaded.latency_threshold_ms, Some(250));

        // 不正な値は書かない
        let before = fs::read_to_string(&path).unwrap();
        let invalid = Config {
            host: " ".into(),
            ..Config::default()
        };
        assert!(save(&path, &invalid).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn language_setting_round_trips() {
        for setting in LanguageSetting::ALL {
            let config: Config =
                toml::from_str(&format!("language = \"{}\"", setting.as_str())).unwrap();
            assert_eq!(config.language, setting);
        }
    }
}
