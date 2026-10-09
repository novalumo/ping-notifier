//! GitHub Releases を使った自動アップデート。
//!
//! 新しいリリースを見つけたら zip をダウンロードし、同じリリースの `SHA256SUMS` と照合する。
//! macOS では展開した `.app` の署名も検証してから、実行中のアプリを置き換えて再起動する。

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::i18n::{Msg, t};
use crate::{log, notifier};

const REPO: &str = "siraken/ping-notifier";
const USER_AGENT: &str = concat!("ping-notifier/", env!("CARGO_PKG_VERSION"));
const SUMS_ASSET: &str = "SHA256SUMS";

/// 起動直後はネットワークが安定していないことがあるので少し待ってから確認する
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;

#[cfg(target_os = "macos")]
const ASSET_SUFFIX: &str = "-macos-universal.zip";
#[cfg(target_os = "windows")]
const ASSET_SUFFIX: &str = "-windows-x64.zip";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const ASSET_SUFFIX: &str = "";

/// UI スレッドから更新スレッドへの指示
#[derive(Debug)]
pub enum UpdateCommand {
    /// メニューからの手動確認。結果を必ず通知する
    CheckNow,
    /// 設定の `auto_update` が変わった
    SetAuto(bool),
}

/// 更新スレッドから UI スレッドへ知らせる出来事
#[derive(Debug)]
pub enum UpdateEvent {
    /// 置き換えが済んだ。`relaunch` を起動して自身は終了する
    Installed { version: Version, relaunch: PathBuf },
    /// 自動では置き換えられないため、ダウンロードページを案内する
    Available { version: Version, url: String },
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo.toml version must be semver")
}

pub fn spawn(auto: bool, on_event: impl Fn(UpdateEvent) + Send + 'static) -> Sender<UpdateCommand> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("updater".into())
        .spawn(move || run(auto, &rx, &on_event))
        .expect("failed to spawn the updater thread");
    tx
}

fn run(mut auto: bool, rx: &Receiver<UpdateCommand>, on_event: &dyn Fn(UpdateEvent)) {
    let mut wait = FIRST_CHECK_DELAY;
    // 自動更新に失敗した版は、手動で確認されるまで再試行しない（失敗通知の繰り返しを防ぐ）
    let mut failed: Option<Version> = None;

    loop {
        let manual = match rx.recv_timeout(wait) {
            Ok(UpdateCommand::CheckNow) => true,
            Ok(UpdateCommand::SetAuto(a)) => {
                auto = a;
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {
                wait = CHECK_INTERVAL;
                if !auto {
                    continue;
                }
                false
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        check_and_apply(manual, &mut failed, on_event);
    }
}

fn check_and_apply(manual: bool, failed: &mut Option<Version>, on_event: &dyn Fn(UpdateEvent)) {
    let current = current_version();
    let (release, latest) = match fetch_latest() {
        Ok(found) => found,
        Err(e) => {
            log(&format!("failed to check for updates: {e:#}"));
            if manual {
                notifier::notify(&t(Msg::UpdateCheckFailedTitle), &format!("{e:#}"));
            }
            return;
        }
    };

    if latest <= current {
        log(&format!(
            "up to date (current v{current} / latest v{latest})"
        ));
        if manual {
            notifier::notify(
                &t(Msg::UpToDateTitle),
                &t(Msg::UpToDateBody { version: &current }),
            );
        }
        return;
    }
    if !manual && failed.as_ref() == Some(&latest) {
        return;
    }

    let available = UpdateEvent::Available {
        version: latest.clone(),
        url: release.html_url.clone(),
    };
    if !can_self_update() {
        notifier::notify(
            &t(Msg::UpdateAvailableTitle),
            &t(Msg::UpdateAvailableBody { version: &latest }),
        );
        on_event(available);
        return;
    }

    log(&format!("installing v{latest}"));
    match install(&release) {
        Ok(relaunch) => {
            log(&format!("installed v{latest}"));
            notifier::notify(
                &t(Msg::UpdatedTitle),
                &t(Msg::UpdatedBody { version: &latest }),
            );
            on_event(UpdateEvent::Installed {
                version: latest,
                relaunch,
            });
        }
        Err(e) => {
            log(&format!("failed to install v{latest}: {e:#}"));
            notifier::notify(
                &t(Msg::UpdateFailedTitle),
                &format!("{e:#}\n{}", t(Msg::UpdateFailedHint)),
            );
            *failed = Some(latest);
            on_event(available);
        }
    }
}

/// 実行中のアプリをこの場で置き換えられるか。
/// 開発中の `cargo run` などでは置き換えず、ダウンロードページの案内にとどめる
pub fn can_self_update() -> bool {
    platform::can_self_update()
}

/// 前回のアップデートで残った一時ファイルを掃除する。起動時に呼ぶ
pub fn cleanup_previous() {
    platform::cleanup_previous();
}

/// 置き換え後のアプリを起動する。呼び出し側はこの後すぐに終了すること
pub fn relaunch(path: &Path) -> Result<()> {
    platform::relaunch(path)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build()
        .into()
}

fn fetch_latest() -> Result<(Release, Version)> {
    // /releases/latest は draft と prerelease を除いた最新のリリースを返す
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release: Release = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", USER_AGENT)
        .call()
        .context("cannot connect to GitHub")?
        .body_mut()
        .read_json()
        .context("cannot read the release information")?;
    let version = parse_tag(&release.tag_name)?;
    Ok((release, version))
}

fn download(url: &str) -> Result<Vec<u8>> {
    agent()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("cannot download {url}"))?
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .read_to_vec()
        .with_context(|| format!("cannot download {url}"))
}

/// ダウンロードと検証を行い、実行中のアプリを置き換える。戻り値は再起動に使うパス
fn install(release: &Release) -> Result<PathBuf> {
    let asset = release
        .assets
        .iter()
        .find(|a| is_platform_asset(&a.name))
        .context("the release has no file for this OS")?;
    let sums_asset = release
        .assets
        .iter()
        .find(|a| a.name == SUMS_ASSET)
        .context("the release has no SHA256SUMS to verify against")?;

    let sums = String::from_utf8(download(&sums_asset.browser_download_url)?)
        .context("SHA256SUMS is not valid UTF-8")?;
    let expected = expected_hash(&sums, &asset.name)
        .with_context(|| format!("SHA256SUMS has no entry for {}", asset.name))?;

    let archive = download(&asset.browser_download_url)?;
    let actual = sha256_hex(&archive);
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "hash mismatch for {} (expected {expected}, got {actual})",
        asset.name
    );

    platform::install(&archive)
}

fn parse_tag(tag: &str) -> Result<Version> {
    Version::parse(tag.trim_start_matches('v'))
        .with_context(|| format!("cannot parse tag {tag} as a version"))
}

fn is_platform_asset(name: &str) -> bool {
    !ASSET_SUFFIX.is_empty() && name.starts_with("PingNotifier-") && name.ends_with(ASSET_SUFFIX)
}

/// `sha256sum` 形式（`<hash>  <name>` / バイナリモードの `<hash> *<name>`）から該当行のハッシュを取り出す
fn expected_hash<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*');
        (file == name).then_some(hash)
    })
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(target_os = "macos")]
mod platform {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    use anyhow::{Context, Result, bail, ensure};

    use crate::{log, notifier};

    /// コマンドを実行し、失敗したら標準エラー出力を含めてエラーにする
    fn run_checked(cmd: &mut Command) -> Result<Output> {
        let output = cmd
            .output()
            .with_context(|| format!("cannot run {cmd:?}"))?;
        if !output.status.success() {
            bail!(
                "{cmd:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output)
    }

    /// 作業用ディレクトリ。`rename` で置き換えられるよう、.app と同じディレクトリに作る
    const WORK_DIR: &str = ".ping-notifier-update";

    /// 実行中の .app のパス（`cargo run` などで .app 外から起動されていれば `None`）
    fn current_app() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let app = exe.parent()?.parent()?.parent()?;
        (app.extension()? == "app").then(|| app.to_path_buf())
    }

    pub fn can_self_update() -> bool {
        current_app().is_some()
    }

    pub fn cleanup_previous() {
        if let Some(app) = current_app() {
            let _ = fs::remove_dir_all(app.with_file_name(WORK_DIR));
        }
    }

    pub fn install(archive: &[u8]) -> Result<PathBuf> {
        let app = current_app().context("not running from an .app bundle")?;
        let work = app.with_file_name(WORK_DIR);
        let _ = fs::remove_dir_all(&work);
        fs::create_dir(&work).with_context(|| {
            format!(
                "cannot write to {}",
                work.parent().unwrap_or(&work).display()
            )
        })?;

        let result = replace(&app, &work, archive);
        // 置き換え済みの旧 .app もここで消える（実行中のバイナリは削除しても動き続ける）
        let _ = fs::remove_dir_all(&work);
        result.map(|()| app)
    }

    fn replace(app: &Path, work: &Path, archive: &[u8]) -> Result<()> {
        let zip = work.join("update.zip");
        fs::write(&zip, archive).context("cannot save the downloaded file")?;

        // 署名済みの .app を壊さないよう、拡張属性やシンボリックリンクを保つ ditto で展開する
        let extracted = work.join("extracted");
        run_checked(
            Command::new("ditto")
                .args(["-x", "-k"])
                .arg(&zip)
                .arg(&extracted),
        )?;
        let new_app = fs::read_dir(&extracted)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|ext| ext == "app"))
            .context("the downloaded archive contains no .app")?;

        verify(app, &new_app)?;

        let old = work.join("old.app");
        fs::rename(app, &old).context("cannot move the current app aside")?;
        if let Err(e) = fs::rename(&new_app, app) {
            let _ = fs::rename(&old, app);
            return Err(e).context("cannot put the new app in place");
        }
        Ok(())
    }

    /// 新しい .app が本物かを検証する
    fn verify(current: &Path, new_app: &Path) -> Result<()> {
        let output = run_checked(
            Command::new("plutil")
                .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
                .arg(new_app.join("Contents/Info.plist")),
        )?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        ensure!(id == notifier::BUNDLE_ID, "bundle ID mismatch ({id})");

        run_checked(
            Command::new("codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(new_app),
        )
        .context("code signature verification failed")?;

        // 実行中のアプリが Developer ID で署名されていれば、同じ Team ID の署名であることを求める
        match team_id(current)? {
            Some(team) => {
                let requirement =
                    format!("=anchor apple generic and certificate leaf[subject.OU] = \"{team}\"");
                run_checked(
                    Command::new("codesign")
                        .args(["--verify", "--deep", "--strict", "-R"])
                        .arg(requirement)
                        .arg(new_app),
                )
                .context("signer (Team ID) mismatch")?;
            }
            None => log(
                "skipping the signer check because the running app has no Developer ID signature",
            ),
        }
        Ok(())
    }

    fn team_id(app: &Path) -> Result<Option<String>> {
        // codesign -d の詳細は標準エラー出力に出る
        let output = run_checked(
            Command::new("codesign")
                .args(["-dv", "--verbose=2"])
                .arg(app),
        )?;
        let info = String::from_utf8_lossy(&output.stderr);
        Ok(info
            .lines()
            .find_map(|l| l.strip_prefix("TeamIdentifier="))
            .filter(|t| *t != "not set")
            .map(str::to_string))
    }

    pub fn relaunch(app: &Path) -> Result<()> {
        // 自身が動いているうちに新しいインスタンスを起動し、成功を確認してから終了する。
        // 終了後に子プロセスから open すると、置き換え直後の .app に対する Gatekeeper の
        // 初回起動処理で要求元（終了済みの自身）が見つからず -600 (procNotFound) で失敗し、
        // アプリが起動しないまま消えることがあった。
        // -n を付けないと、同じ Bundle ID の自身が前面に出るだけで新しい版が起動しない
        run_checked(Command::new("open").arg("-n").arg(app))
            .context("cannot launch the new version")?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::fs::{self, File};
    use std::io::{self, Cursor};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use anyhow::{Context, Result};

    const EXE_NAME: &str = "ping-notifier.exe";

    fn old_path(exe: &Path) -> PathBuf {
        exe.with_extension("exe.old")
    }

    pub fn can_self_update() -> bool {
        // 開発中の target/debug などは置き換えない
        !cfg!(debug_assertions)
    }

    pub fn cleanup_previous() {
        if let Ok(exe) = std::env::current_exe() {
            let _ = fs::remove_file(old_path(&exe));
        }
    }

    pub fn install(archive: &[u8]) -> Result<PathBuf> {
        let exe = std::env::current_exe().context("cannot determine the executable path")?;

        let mut zip = zip::ZipArchive::new(Cursor::new(archive))
            .context("cannot open the downloaded archive")?;
        let mut entry = zip
            .by_name(EXE_NAME)
            .with_context(|| format!("the downloaded archive contains no {EXE_NAME}"))?;
        let new = exe.with_extension("exe.new");
        {
            let mut file =
                File::create(&new).with_context(|| format!("cannot write to {}", new.display()))?;
            io::copy(&mut entry, &mut file).context("cannot write the new executable")?;
        }

        // 実行中の exe は上書きできないが名前は変えられるので、退避してから差し替える
        let old = old_path(&exe);
        let _ = fs::remove_file(&old);
        fs::rename(&exe, &old).context("cannot move the current executable aside")?;
        if let Err(e) = fs::rename(&new, &exe) {
            let _ = fs::rename(&old, &exe);
            let _ = fs::remove_file(&new);
            return Err(e).context("cannot put the new executable in place");
        }
        Ok(exe)
    }

    pub fn relaunch(exe: &Path) -> Result<()> {
        Command::new(exe)
            .spawn()
            .context("cannot launch the new version")?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use std::path::{Path, PathBuf};

    use anyhow::{Result, bail};

    pub fn can_self_update() -> bool {
        false
    }

    pub fn cleanup_previous() {}

    pub fn install(_archive: &[u8]) -> Result<PathBuf> {
        bail!("automatic updates are not supported on this OS")
    }

    pub fn relaunch(_path: &Path) -> Result<()> {
        bail!("automatic updates are not supported on this OS")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_with_or_without_prefix() {
        assert_eq!(parse_tag("v1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(parse_tag("0.10.0").unwrap(), Version::new(0, 10, 0));
        assert!(parse_tag("latest").is_err());
    }

    #[test]
    fn compares_versions_semantically() {
        assert!(parse_tag("v0.10.0").unwrap() > parse_tag("v0.9.0").unwrap());
        assert!(parse_tag("v1.0.0").unwrap() > parse_tag("v1.0.0-rc.1").unwrap());
    }

    #[test]
    fn finds_hash_in_sha256sums() {
        let sums = "\
aaa111  PingNotifier-0.2.0-macos-universal.zip
bbb222 *PingNotifier-0.2.0-windows-x64.zip
";
        assert_eq!(
            expected_hash(sums, "PingNotifier-0.2.0-macos-universal.zip"),
            Some("aaa111")
        );
        assert_eq!(
            expected_hash(sums, "PingNotifier-0.2.0-windows-x64.zip"),
            Some("bbb222")
        );
        assert_eq!(expected_hash(sums, "PingNotifier-0.2.0"), None);
    }

    #[test]
    fn sha256_matches_known_value() {
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn selects_only_this_platforms_asset() {
        let name = format!("PingNotifier-0.2.0{ASSET_SUFFIX}");
        assert!(is_platform_asset(&name));
        assert!(!is_platform_asset(SUMS_ASSET));
        assert!(!is_platform_asset("PingNotifier-0.2.0-linux-x64.zip"));
    }
}
