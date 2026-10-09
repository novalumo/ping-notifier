//! OS 標準の `ping` コマンドを 1 回だけ実行して疎通を確認する。
//!
//! raw ICMP ソケットは OS によって管理者権限が必要になるため、
//! 権限不要で全 OS に存在する `ping` コマンドに委譲している。

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// `ping` コマンド自体がハングした場合（名前解決の遅延など）に備えた猶予時間
const PROCESS_GRACE: Duration = Duration::from_secs(5);

/// `host` に ICMP Echo を 1 回送り、`timeout` 以内に応答があれば `Ok(true)` を返す。
///
/// `ping` コマンドを起動できなかった場合のみ `Err` を返す。
pub fn ping_once(host: &str, timeout: Duration) -> std::io::Result<bool> {
    let mut child = build_command(host, timeout)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let deadline = Instant::now() + timeout + PROCESS_GRACE;
    let success = loop {
        if let Some(status) = child.try_wait()? {
            break status.success();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(20));
    };

    Ok(success && is_reply(&mut child))
}

#[cfg(target_os = "windows")]
fn build_command(host: &str, timeout: Duration) -> Command {
    use std::os::windows::process::CommandExt;

    /// GUI アプリから起動したときに ping のたびコンソールが一瞬表示されるのを防ぐ
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut cmd = Command::new("ping");
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.args([
        "-n",
        "1",
        "-w",
        &timeout.as_millis().max(1).to_string(),
        host,
    ]);
    cmd
}

#[cfg(target_os = "macos")]
fn build_command(host: &str, timeout: Duration) -> Command {
    // macOS の -W はミリ秒単位。ただし -W だけだと「-W + 約 1 秒」待ってから終了するため、
    // プロセス全体のタイムアウト -t（秒単位）も併せて指定する
    let millis = timeout.as_millis().max(1);
    let mut cmd = Command::new("ping");
    cmd.args([
        "-n",
        "-c",
        "1",
        "-W",
        &millis.to_string(),
        "-t",
        &millis.div_ceil(1000).to_string(),
        host,
    ]);
    cmd
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn build_command(host: &str, timeout: Duration) -> Command {
    // Linux (iputils) の -W は秒単位なので切り上げる
    let secs = timeout.as_millis().div_ceil(1000).max(1);
    let mut cmd = Command::new("ping");
    cmd.args(["-n", "-c", "1", "-W", &secs.to_string(), host]);
    cmd
}

/// Windows の ping は「宛先ホストに到達できません」という応答でも終了コード 0 を返すため、
/// 実際に Echo Reply を受け取ったか（出力に `TTL=` が含まれるか）を確認する。
/// この表記は日本語版 Windows でも共通。
#[cfg(target_os = "windows")]
fn is_reply(child: &mut Child) -> bool {
    let mut buf = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).contains("TTL=")
}

#[cfg(not(target_os = "windows"))]
fn is_reply(child: &mut Child) -> bool {
    // パイプに残った出力は読み捨てる（Unix 系は終了コードで判定できる）
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut Vec::new());
    }
    true
}
