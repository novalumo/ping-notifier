//! ログイン時の自動起動の登録と解除。
//!
//! 状態は OS 側の登録を正とし、アプリの設定ファイルには持たない。
//! システム設定などで無効にされた場合も、メニューの表示が実態と食い違わないようにするため。
//!
//! - macOS: `SMAppService.mainApp`（macOS 13 以降）。「システム設定 > 一般 > ログイン項目」に表示される
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! - Linux: XDG Autostart（`~/.config/autostart/ping-notifier.desktop`）

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Enabled,
    Disabled,
    /// 登録はされたが、ユーザーがシステム設定で許可する必要がある（macOS）
    RequiresApproval,
}

/// この環境で自動起動を設定できるか。開発ビルドなどでは設定させない
pub fn is_supported() -> bool {
    platform::is_supported()
}

pub fn status() -> anyhow::Result<Status> {
    platform::status()
}

pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    platform::set_enabled(enabled)
}

/// 許可が必要なときに、自動起動を管理する OS の設定画面を開く
pub fn open_system_settings() {
    platform::open_system_settings();
}

#[cfg(target_os = "macos")]
mod platform {
    use anyhow::{Result, anyhow};
    use objc2::available;
    use objc2::rc::Retained;
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    use super::Status;

    pub fn is_supported() -> bool {
        // mainAppService は .app として登録するため、cargo run などの素のバイナリでは使えない
        available!(macos = 13.0) && crate::notifier::running_in_app_bundle()
    }

    fn service() -> Retained<SMAppService> {
        // SAFETY: 引数を取らないクラスメソッドで、自身の .app を表すサービスを返すだけ
        unsafe { SMAppService::mainAppService() }
    }

    pub fn status() -> Result<Status> {
        // SAFETY: 状態の問い合わせのみで副作用はない
        let status = unsafe { service().status() };
        Ok(if status == SMAppServiceStatus::Enabled {
            Status::Enabled
        } else if status == SMAppServiceStatus::RequiresApproval {
            Status::RequiresApproval
        } else {
            Status::Disabled
        })
    }

    pub fn set_enabled(enabled: bool) -> Result<()> {
        let service = service();
        // SAFETY: 自身の .app のログイン項目を登録 / 解除するだけで、引数は取らない
        let result = unsafe {
            if enabled {
                service.registerAndReturnError()
            } else {
                service.unregisterAndReturnError()
            }
        };
        result.map_err(|e| anyhow!("{}", e.localizedDescription()))
    }

    pub fn open_system_settings() {
        // SAFETY: システム設定のログイン項目画面を開くだけ
        unsafe { SMAppService::openSystemSettingsLoginItems() };
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::path::Path;

    use anyhow::{Context, Result};
    use windows_registry::CURRENT_USER;

    use super::Status;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "PingNotifier";

    pub fn is_supported() -> bool {
        // target\debug の exe を登録しても意味がないため、リリースビルドに限る
        !cfg!(debug_assertions)
    }

    fn command() -> Result<String> {
        let exe = std::env::current_exe().context("cannot determine the executable path")?;
        Ok(run_command(&exe))
    }

    pub fn status() -> Result<Status> {
        let Ok(registered) = CURRENT_USER
            .open(RUN_KEY)
            .and_then(|key| key.get_string(VALUE_NAME))
        else {
            return Ok(Status::Disabled);
        };
        // exe を移動した後の古い登録は無効とみなし、有効化し直すと現在のパスで上書きされる
        Ok(if registered.eq_ignore_ascii_case(&command()?) {
            Status::Enabled
        } else {
            Status::Disabled
        })
    }

    pub fn set_enabled(enabled: bool) -> Result<()> {
        let key = CURRENT_USER
            .create(RUN_KEY)
            .context("cannot open the Run registry key")?;
        if enabled {
            key.set_string(VALUE_NAME, command()?)
                .context("cannot register the startup entry")?;
        } else if key.get_string(VALUE_NAME).is_ok() {
            key.remove_value(VALUE_NAME)
                .context("cannot remove the startup entry")?;
        }
        Ok(())
    }

    pub fn open_system_settings() {}

    /// Run キーに登録するコマンド。パスに空白を含んでも動くよう引用符で囲む
    pub(super) fn run_command(exe: &Path) -> String {
        format!("\"{}\"", exe.display())
    }

    #[cfg(test)]
    mod tests {
        use std::path::Path;

        #[test]
        fn quotes_path_with_spaces() {
            assert_eq!(
                super::run_command(Path::new(
                    r"C:\Program Files\Ping Notifier\ping-notifier.exe"
                )),
                r#""C:\Program Files\Ping Notifier\ping-notifier.exe""#
            );
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::fs;
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    use super::Status;

    const FILE_NAME: &str = "ping-notifier.desktop";

    pub fn is_supported() -> bool {
        // target/debug のバイナリを登録しても意味がないため、リリースビルドに限る
        !cfg!(debug_assertions)
    }

    /// XDG Autostart のエントリ。`$XDG_CONFIG_HOME/autostart`（通常は `~/.config/autostart`）に置く
    fn entry_path() -> Result<PathBuf> {
        let dir = dirs::config_dir().context("config directory not found")?;
        Ok(dir.join("autostart").join(FILE_NAME))
    }

    fn exe() -> Result<PathBuf> {
        std::env::current_exe().context("cannot determine the executable path")
    }

    pub fn status() -> Result<Status> {
        let Ok(text) = fs::read_to_string(entry_path()?) else {
            return Ok(Status::Disabled);
        };
        // バイナリを移動した後の古い登録は無効とみなし、有効化し直すと現在のパスで上書きされる
        Ok(if is_enabled_entry(&text, &exe()?) {
            Status::Enabled
        } else {
            Status::Disabled
        })
    }

    pub fn set_enabled(enabled: bool) -> Result<()> {
        let path = entry_path()?;
        if enabled {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
            }
            fs::write(&path, desktop_entry(&exe()?))
                .with_context(|| format!("cannot write {}", path.display()))?;
        } else if path.exists() {
            fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
        }
        Ok(())
    }

    pub fn open_system_settings() {}

    pub(super) fn desktop_entry(exe: &Path) -> String {
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Ping Notifier\n\
             Comment=Notifies you of ping timeouts\n\
             Exec={}\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exec_value(exe)
        )
    }

    /// `Exec=` の値。Desktop Entry 仕様に従い、パスを引用符で囲んで `"` `` ` `` `$` `\\` をエスケープし、
    /// `%`（フィールドコード）を `%%` にする
    pub(super) fn exec_value(exe: &Path) -> String {
        let mut quoted = String::from("\"");
        for c in exe.to_string_lossy().chars() {
            match c {
                '"' | '`' | '$' | '\\' => {
                    quoted.push('\\');
                    quoted.push(c);
                }
                '%' => quoted.push_str("%%"),
                _ => quoted.push(c),
            }
        }
        quoted.push('"');
        quoted
    }

    /// 自分が書いたエントリで、現在のバイナリを指し、無効化されていなければ有効とみなす。
    /// デスクトップ環境の設定画面で無効にすると `Hidden=true` などが書き込まれる
    pub(super) fn is_enabled_entry(text: &str, exe: &Path) -> bool {
        let expected = exec_value(exe);
        let mut matches = false;
        for line in text.lines().map(str::trim) {
            match line.split_once('=') {
                Some(("Exec", value)) => matches = value == expected,
                Some(("Hidden", "true")) | Some(("X-GNOME-Autostart-enabled", "false")) => {
                    return false;
                }
                _ => {}
            }
        }
        matches
    }

    #[cfg(test)]
    mod tests {
        use std::path::Path;

        use super::*;

        #[test]
        fn quotes_and_escapes_exec_path() {
            assert_eq!(
                exec_value(Path::new("/opt/Ping Notifier/ping-notifier")),
                r#""/opt/Ping Notifier/ping-notifier""#
            );
            assert_eq!(
                exec_value(Path::new("/home/a$b/100%/ping-notifier")),
                r#""/home/a\$b/100%%/ping-notifier""#
            );
        }

        #[test]
        fn recognizes_own_enabled_entry() {
            let exe = Path::new("/opt/ping-notifier");
            let entry = desktop_entry(exe);
            assert!(is_enabled_entry(&entry, exe));
            // 別の場所のバイナリを指す登録は無効とみなす
            assert!(!is_enabled_entry(
                &entry,
                Path::new("/usr/bin/ping-notifier")
            ));
            // デスクトップ環境で無効にされた登録
            assert!(!is_enabled_entry(&format!("{entry}Hidden=true\n"), exe));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    use anyhow::{Result, bail};

    use super::Status;

    pub fn is_supported() -> bool {
        false
    }

    pub fn status() -> Result<Status> {
        Ok(Status::Disabled)
    }

    pub fn set_enabled(_enabled: bool) -> Result<()> {
        bail!("launch at login is not supported on this OS")
    }

    pub fn open_system_settings() {}
}
