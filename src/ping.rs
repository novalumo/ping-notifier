//! OS 標準の `ping` コマンドを 1 回だけ実行して疎通を確認する。
//!
//! raw ICMP ソケットは OS によって管理者権限が必要になるため、
//! 権限不要で全 OS に存在する `ping` コマンドに委譲している。

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// `ping` コマンド自体がハングした場合（名前解決の遅延など）に備えた猶予時間
const PROCESS_GRACE: Duration = Duration::from_secs(5);

/// Echo Reply を受け取ったときの結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply {
    /// 応答時間。`ping` の出力から読み取れなかった場合は `None`
    pub rtt: Option<Duration>,
}

/// `host` に ICMP Echo を 1 回送り、`timeout` 以内に応答があれば `Ok(Some(_))` を返す。
///
/// `ping` コマンドを起動できなかった場合のみ `Err` を返す。
pub fn ping_once(host: &str, timeout: Duration) -> std::io::Result<Option<Reply>> {
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
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(20));
    };

    // 1 回分の出力はパイプのバッファに収まるので、終了を待ってから読んでよい。
    // Windows の出力は OEM コードページ（日本語版なら CP932）だが、読み取るのは ASCII の部分だけ
    let mut buf = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut buf);
    }
    let output = String::from_utf8_lossy(&buf);

    if !(success && is_reply(&output)) {
        return Ok(None);
    }
    Ok(Some(Reply {
        rtt: parse_rtt(&output),
    }))
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
fn is_reply(output: &str) -> bool {
    output.contains("TTL=")
}

/// Unix 系は終了コードで判定できる
#[cfg(not(target_os = "windows"))]
fn is_reply(_output: &str) -> bool {
    true
}

/// `ping` の出力から応答時間を読み取る。
///
/// 見出しの語（`time` / `時間` / `Zeit` など）は OS の言語で変わるため見ずに、
/// `=` か `<` の直後にある「数値 + `ms`」を探す。最初に見つかったもの（応答の行）を使う。
///
/// - macOS / Linux: `time=12.345 ms`
/// - Windows: `time=12ms`、日本語版は `時間 =12ms`、1 ms 未満は `time<1ms`
///
/// `<` の場合（1 ms 未満）は `Duration::ZERO` を返す。
fn parse_rtt(output: &str) -> Option<Duration> {
    output.match_indices(['=', '<']).find_map(|(i, sep)| {
        let rest = output[i + sep.len()..].trim_start();
        let len = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let (number, unit) = rest.split_at(len);
        if !unit.trim_start().starts_with("ms") {
            return None;
        }
        let millis: f64 = number.parse().ok()?;
        Some(if sep == "<" {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(millis / 1000.0)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: f64) -> Option<Duration> {
        Some(Duration::from_secs_f64(millis / 1000.0))
    }

    #[test]
    fn parses_macos_output() {
        let output = "\
PING 8.8.8.8 (8.8.8.8): 56 data bytes
64 bytes from 8.8.8.8: icmp_seq=0 ttl=117 time=12.345 ms

--- 8.8.8.8 ping statistics ---
1 packets transmitted, 1 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 12.345/12.345/12.345/0.000 ms
";
        assert_eq!(parse_rtt(output), ms(12.345));
    }

    #[test]
    fn parses_linux_output() {
        let output = "\
PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.
64 bytes from 8.8.8.8: icmp_seq=1 ttl=117 time=9.87 ms

--- 8.8.8.8 ping statistics ---
1 packets transmitted, 1 received, 0% packet loss, time 0ms
rtt min/avg/max/mdev = 9.870/9.870/9.870/0.000 ms
";
        assert_eq!(parse_rtt(output), ms(9.87));
    }

    #[test]
    fn parses_windows_english_output() {
        let output = "\
Pinging 8.8.8.8 with 32 bytes of data:
Reply from 8.8.8.8: bytes=32 time=12ms TTL=117

Ping statistics for 8.8.8.8:
    Packets: Sent = 1, Received = 1, Lost = 0 (0% loss),
Approximate round trip times in milli-seconds:
    Minimum = 12ms, Maximum = 12ms, Average = 12ms
";
        assert_eq!(parse_rtt(output), ms(12.0));
    }

    #[test]
    fn parses_windows_japanese_output() {
        let output = "\
8.8.8.8 に ping を送信しています 32 バイトのデータ:
8.8.8.8 からの応答: バイト数 =32 時間 =6ms TTL=117

8.8.8.8 の ping 統計:
    パケット数: 送信 = 1、受信 = 1、損失 = 0 (0% の損失)、
ラウンド トリップの概算時間 (ミリ秒):
    最小 = 6ms、最大 = 6ms、平均 = 6ms
";
        assert_eq!(parse_rtt(output), ms(6.0));
    }

    #[test]
    fn parses_windows_other_languages() {
        // ドイツ語・中国語・韓国語・フランス語版（フランス語は数値と ms の間に空白が入る）
        let lines = [
            "Antwort von 8.8.8.8: Bytes=32 Zeit=15ms TTL=117",
            "来自 8.8.8.8 的回复: 字节=32 时间=15ms TTL=117",
            "8.8.8.8의 응답: 바이트=32 시간=15ms TTL=117",
            "Réponse de 8.8.8.8 : octets=32 temps=15 ms TTL=117",
        ];
        for line in lines {
            assert_eq!(parse_rtt(line), ms(15.0), "{line}");
        }
    }

    #[test]
    fn parses_windows_sub_millisecond_output() {
        assert_eq!(
            parse_rtt("Reply from 127.0.0.1: bytes=32 time<1ms TTL=128"),
            Some(Duration::ZERO)
        );
        assert_eq!(
            parse_rtt("127.0.0.1 からの応答: バイト数 =32 時間 <1ms TTL=128"),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn returns_none_without_reply_time() {
        let output = "\
Pinging 192.0.2.1 with 32 bytes of data:
Request timed out.

Ping statistics for 192.0.2.1:
    Packets: Sent = 1, Received = 0, Lost = 1 (100% loss),
";
        assert_eq!(parse_rtt(output), None);
        assert_eq!(parse_rtt(""), None);
    }
}
