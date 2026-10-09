//! ログイン時の自動起動の登録と解除。
//!
//! 状態は OS 側の登録を正とし、アプリの設定ファイルには持たない。
//! システム設定などで無効にされた場合も、メニューの表示が実態と食い違わないようにするため。
//!
//! - macOS: `SMAppService.mainApp`（macOS 13 以降）。「システム設定 > 一般 > ログイン項目」に表示される
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

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
        let exe = std::env::current_exe().context("実行ファイルの場所が分かりません")?;
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
            .context("自動起動の設定を開けません")?;
        if enabled {
            key.set_string(VALUE_NAME, command()?)
                .context("自動起動を登録できません")?;
        } else if key.get_string(VALUE_NAME).is_ok() {
            key.remove_value(VALUE_NAME)
                .context("自動起動を解除できません")?;
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

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
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
        bail!("この OS は自動起動の設定に対応していません")
    }

    pub fn open_system_settings() {}
}
