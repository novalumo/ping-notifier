# Ping Notifier

[English](README.md) | 日本語

ping のタイムアウト（パケットロス）を検知して OS の通知に表示する、メニューバー（Windows ではタスクトレイ）常駐アプリ。

- アイコンの色で状態を表示: 緑 = 正常 / 赤 = ロス中 / 灰 = 一時停止 / 橙 = ping を実行できない
- 直近の応答時間（RTT）をメニューとツールチップに表示する
- メニューの「サイレントモード」で、監視は続けたまま（アイコンの色は変わる）通知だけ止められる
- 通知はロス状態に入ったときと復旧したときにだけ送られる（ロスが続いている間は繰り返さない）
- 新しいバージョンが公開されると自動でアップデートする（後述）
- メニューの「ログイン時に起動」で自動起動を切り替えられる（macOS 13 以降 / Windows）
- 表示言語: 英語、日本語、中国語（簡体字）、韓国語、エスペラント、ドイツ語（OS の言語に合わせて切り替わり、設定で固定もできる）

## Homebrew でインストール（macOS）

```sh
brew install --cask novalumo/tap/ping-notifier
```

Cask は [novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap) にある。アプリが自身でアップデートするため、`brew upgrade` では `--greedy` を付けない限り更新対象にならない。

## Nix でインストール（macOS、Apple Silicon）

```sh
nix run github:novalumo/ping-notifier          # インストールせずに試す
nix profile add github:novalumo/ping-notifier  # インストール
```

flake はソースからビルドする。`$out/Applications/Ping Notifier.app` がアプリ本体で、`$out/bin/ping-notifier` はそれを起動する。nix-darwin や Home Manager では flake を input に加え、`inputs.ping-notifier.packages.${pkgs.system}.default` を `environment.systemPackages` や `home.packages` に入れる。

注意:

- Nix ストアは読み取り専用なので、アプリ自身による置き換えは行わない。アップデートは Nix で行う（`nix profile upgrade ping-notifier` や `nix flake update` など）。新しいバージョンの公開は引き続き通知される
- アップデートのたびにアプリのパスが変わるため、更新後に「ログイン時に起動」を入れ直す必要があることがある
- Homebrew 版やダウンロード版と同時に入れないこと（Bundle ID が同じため）

## ダウンロード

[Releases](https://github.com/novalumo/ping-notifier/releases) から OS に合ったファイルをダウンロードする。

| OS | ファイル |
| --- | --- |
| macOS（Apple Silicon） | `PingNotifier-<version>-macos-arm64.zip` |
| Windows（x64） | `PingNotifier-<version>-windows-x64.zip` |

署名・公証が済んでいない版では、初回起動時に OS の警告が出る。回避手順は各リリースのノート（`.github/release-notes.md`）を参照。一度インストールすれば、以降は自動でアップデートされる。

## リリース手順

1. `Cargo.toml` の `version` を更新し、[CHANGELOG.md](CHANGELOG.md) の `Unreleased` の内容を新しいバージョンの見出しへ移してコミットする
2. 同じバージョンのタグを push する（例: `git tag v0.2.0 && git push origin v0.2.0`）
3. `.github/workflows/release.yml` が macOS / Windows 向けにビルドして GitHub Release を作成し、[novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap) の `Casks/ping-notifier.rb` を更新する（後述）

タグと `Cargo.toml` の `version` が一致しないとワークフローは失敗する。Actions 画面から手動実行（workflow_dispatch）すると、Release を作らずにビルドだけを試せる（成果物は実行結果の Artifacts から取得できる）。

## 自動アップデート

起動 30 秒後と、以降 6 時間ごとに [Releases](https://github.com/novalumo/ping-notifier/releases) の最新版を確認する。新しいバージョンがあれば次の検証をしてから置き換え、再起動する。

1. リリースに添付された `SHA256SUMS` とダウンロードしたファイルのハッシュが一致すること
2. macOS のみ: 展開した `.app` の Bundle ID が一致し、署名の検証が通ること。実行中のアプリが Developer ID で署名されている場合は、同じ Team ID の署名であることも求める

メニューの「アップデートを確認」で手動でも確認できる。設定の `auto_update = false` で自動確認を止められる。置き換えに失敗した場合や開発ビルドでは、メニューからダウンロードページを開くよう案内する。

GitHub API に認証なしでアクセスするため、リポジトリが公開されている必要がある。

## macOS の署名と公証（リリース用）

以下の GitHub Secrets が登録されていると、リリース時に Developer ID で署名し、Apple の公証を受けてから配布する。未登録の場合は ad-hoc 署名のまま配布する（初回起動時に Gatekeeper の警告が出る）。

| Secret | 内容 |
| --- | --- |
| `MACOS_CERTIFICATE_P12` | 「Developer ID Application」証明書と秘密鍵を書き出した `.p12` を base64 にしたもの |
| `MACOS_CERTIFICATE_PASSWORD` | `.p12` 書き出し時に設定したパスワード |
| `APPLE_API_KEY_P8` | App Store Connect API キー（`.p8`）の中身 |
| `APPLE_API_KEY_ID` | API キーの Key ID |
| `APPLE_API_ISSUER_ID` | API キーの Issuer ID |

準備の手順:

1. [Apple Developer](https://developer.apple.com/account/resources/certificates/list) で「Developer ID Application」証明書を作成し、キーチェーンに取り込む（作成にはアカウント保有者の権限が必要）
2. キーチェーンアクセスで証明書を秘密鍵ごと `.p12` に書き出す
3. [App Store Connect](https://appstoreconnect.apple.com/access/integrations/api) の「チームキー」で API キーを作成し（アクセス: Developer）、`.p8` をダウンロードする
4. GitHub に登録する:

```bash
base64 -i DeveloperIDApplication.p12 | gh secret set MACOS_CERTIFICATE_P12
gh secret set MACOS_CERTIFICATE_PASSWORD          # 対話的に入力
gh secret set APPLE_API_KEY_P8 < AuthKey_XXXXXXXXXX.p8
gh secret set APPLE_API_KEY_ID --body XXXXXXXXXX
gh secret set APPLE_API_ISSUER_ID --body xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

ad-hoc 署名の版から Developer ID 署名の版へも自動アップデートできる。一度 Developer ID 署名の版になると、以降は同じ Team ID で署名された版しか受け付けない。

## Homebrew の Cask の更新（リリース用）

Release の作成後、ワークフローが [novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap) の `Casks/ping-notifier.rb` の `version`・`sha256`・`url` を書き換えて push する。tap への書き込みには GitHub App のトークンを使う。以下のシークレットが未登録ならこの手順は警告を出してスキップされるので、Cask を手で更新する。

| シークレット | 内容 |
| --- | --- |
| `HOMEBREW_TAP_APP_CLIENT_ID` | GitHub App の Client ID |
| `HOMEBREW_TAP_APP_PRIVATE_KEY` | GitHub App の秘密鍵（`.pem`） |

準備:

1. novalumo の Organization 設定（Developer settings → GitHub Apps）で GitHub App を作る。Webhook はオフにし、権限は Repository permissions → Contents: Read and write だけにする
2. 秘密鍵を生成して `.pem` をダウンロードする
3. App を novalumo にインストールし、対象リポジトリを `homebrew-tap` だけにする
4. GitHub に登録する:

```bash
gh secret set HOMEBREW_TAP_APP_CLIENT_ID --body Iv23xxxxxxxxxxxxxxxx
gh secret set HOMEBREW_TAP_APP_PRIVATE_KEY < app-name.2026-10-09.private-key.pem
```

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
| `auto_update` | `true` | 新しいバージョンを自動でインストールするか |
| `language` | `"auto"` | 表示言語。`"auto"`（OS の言語に合わせる）または言語コード（[表示言語](#表示言語)を参照） |

## 表示言語

メニュー・通知の言語は下表の言語に対応している。`language = "auto"`（既定）では OS の優先言語を上から順に見て、最初に対応している言語を使う（どれも対応していなければ英語）。設定で下表のコードに固定することもでき、「設定を再読み込み」ですぐに切り替わる。

| 言語 | `language` |
| --- | --- |
| 英語 | `"en"` |
| 日本語 | `"ja"` |
| 中国語（簡体字） | `"zh"` |
| 韓国語 | `"ko"` |
| エスペラント | `"eo"` |
| ドイツ語 | `"de"` |

中国語は簡体字のみ。`"auto"` では繁体字のロケール（`zh-Hant`・`zh-TW`・`zh-HK`・`zh-MO`）は対象外とし、次の優先言語に進む。

初回起動時に作られる設定ファイルのコメントも、そのときの表示言語で書かれる。ログとエラーの詳細は英語で出力する。

## ログイン時の起動

メニューの「ログイン時に起動」で切り替える。状態は OS 側の登録をそのまま表示する。

- macOS: 「システム設定 > 一般 > ログイン項目」に Ping Notifier として登録される（`SMAppService`、macOS 13 以降）。以前ここで無効にしていた場合は許可を求められるので、開いた設定画面で有効にする
- Windows: `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` に登録される。exe を移動した場合は、もう一度オンにすると新しい場所で登録し直す

`cargo run` などの開発ビルドでは設定できない（メニュー項目が無効になる）。

## 仕組み

- 疎通確認は OS 標準の `ping` コマンドを 1 回ずつ実行して行う（raw ICMP ソケットは OS によって管理者権限が必要になるため）
- 通知は [notify-rust](https://crates.io/crates/notify-rust)、メニューバー / トレイは [tray-icon](https://crates.io/crates/tray-icon) + [tao](https://crates.io/crates/tao)
- macOS では notify-rust が既定で Finder 名義の通知を送ろうとして破棄されるため、起動時に自身の Bundle ID（`com.novalumo.ping-notifier`）を送り主に設定している（`src/notifier.rs`）

## 注意

- macOS の `ping` はタイムアウトを秒単位（`-t`）でも制限しているため、`timeout_ms` は実質的に秒単位に切り上げられる
- Windows 版はビルドとテストのみ CI で確認しており、実機での動作は未検証。通知は notify-rust の既定（PowerShell 名義）で送られる
