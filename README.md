# Ping Notifier

ping のタイムアウト（パケットロス）を検知して OS の通知に表示する、メニューバー（Windows ではタスクトレイ）常駐アプリ。

- アイコンの色で状態を表示: 緑 = 正常 / 赤 = ロス中 / 灰 = 一時停止 / 橙 = ping を実行できない
- 通知はロス状態に入ったときと復旧したときにだけ送られる（ロスが続いている間は繰り返さない）

## ビルド

```bash
nix develop   # または direnv allow

# 開発中（通知はターミナル.app 名義で送られる）
cargo run

# macOS: .app を作成
cargo bundle --release
codesign --force --sign - "target/release/bundle/osx/Ping Notifier.app"
cp -R "target/release/bundle/osx/Ping Notifier.app" /Applications/
```

初回の通知時に macOS の通知許可ダイアログが表示されるので「許可」を選ぶ。

### アプリアイコン

元データは `icons/icon.svg`。編集したら `./icons/generate.sh` で `icons/png/`（macOS 用）と `icons/icon.ico`（Windows 用）を再生成する。

## 設定

初回起動時に以下へ設定ファイルが作成される。メニューの「設定ファイルを開く」で編集し、「設定を再読み込み」で反映する。

- macOS: `~/Library/Application Support/ping-notifier/config.toml`
- Windows: `%APPDATA%\ping-notifier\config.toml`

| キー | 既定値 | 説明 |
| --- | --- | --- |
| `host` | `"8.8.8.8"` | 監視対象のホスト名 / IP アドレス |
| `interval_secs` | `1.0` | ping を送る間隔（秒） |
| `timeout_ms` | `1000` | 応答待ち時間（ミリ秒）。超えたらロスとみなす |
| `threshold` | `1` | 何回連続でロスしたら通知するか |
| `notify_recovery` | `true` | 復旧時にも通知するか |

## 仕組み

- 疎通確認は OS 標準の `ping` コマンドを 1 回ずつ実行して行う（raw ICMP ソケットは OS によって管理者権限が必要になるため）
- 通知は [notify-rust](https://crates.io/crates/notify-rust)、メニューバー / トレイは [tray-icon](https://crates.io/crates/tray-icon) + [tao](https://crates.io/crates/tao)
- macOS では notify-rust が既定で Finder 名義の通知を送ろうとして破棄されるため、起動時に自身の Bundle ID（`com.novalumo.ping-notifier`）を送り主に設定している（`src/notifier.rs`）

## 注意

- macOS の `ping` はタイムアウトを秒単位（`-t`）でも制限しているため、`timeout_ms` は実質的に秒単位に切り上げられる
- Windows 版は未検証。通知は notify-rust の既定（PowerShell 名義）で送られる
