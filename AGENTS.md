# AGENTS.md

このファイルは、AI コーディングエージェントがこのリポジトリで作業する際の指針を提供します。

## プロジェクト概要

ping のタイムアウト（パケットロス）を検知して OS の通知に表示する、Rust 製のメニューバー（Windows ではタスクトレイ）常駐アプリ。対象 OS は macOS と Windows。設定は TOML ファイルで、GUI の設定画面は持たない。

## アーキテクチャ

`src/` の責務分割は以下のとおり:

- `main.rs` — `tao` のイベントループと `tray-icon` のメニューバーアイコン / メニュー。監視スレッドからの `Status` を受けてアイコン色とメニュー文言を更新する
- `worker.rs` — 監視スレッド。`ping` → `Monitor` → 通知を回し、UI からの `Command`（一時停止 / 設定再読み込み）を `mpsc` で受ける。待機に `recv_timeout` を使うので指示は即座に反映される
- `monitor.rs` — ping 結果の列から「ロス発生」「復旧」を判定する状態機械。副作用を持たない
- `ping.rs` — OS 標準の `ping` コマンドを 1 回実行して疎通を判定する
- `notifier.rs` — `notify-rust` による通知送信と、macOS での送り主（Bundle ID）設定
- `updater.rs` — GitHub Releases からの自動アップデート（更新スレッド、検証、OS ごとの置き換えと再起動）
- `autostart.rs` — ログイン時の自動起動の登録 / 解除（macOS: `SMAppService`、Windows: レジストリの Run キー）
- `i18n.rs` — 表示言語の決定と、利用者に見せる文言（`Msg`）の英語 / 日本語訳
- `config.rs` — 設定ファイルの読み込み・初回生成・検証
- `icon.rs` — 状態アイコン（色付きの円）を実行時に RGBA で描画する

スレッドは UI（メイン）・監視・更新の 3 本。async ランタイムは導入していない。

## 開発ワークフロー

### 環境構築

Nix flake と direnv で完結する:

```sh
direnv allow   # 初回のみ
```

direnv を使わない場合は `nix develop` で同じ devShell に入る。devShell には Rust ツールチェーンに加え `cargo-bundle`・`librsvg`・`imagemagick` が入っている。

**新規ファイルは `git add` するまで Nix から見えない**（flake は git の追跡ファイルだけを評価する）。ファイルを追加したら `nix develop` の前に `git add` すること。

### よく使うコマンド

| コマンド | 用途 |
| --- | --- |
| `cargo run` | 開発実行（メニューバーに常駐する。通知はターミナル.app 名義） |
| `cargo test` | ユニットテスト |
| `cargo clippy --all-targets` | lint（警告ゼロを維持する） |
| `cargo bundle --release` | macOS の `.app` を `target/release/bundle/osx/` に生成 |
| `codesign --force --sign - "target/release/bundle/osx/Ping Notifier.app"` | `.app` の ad-hoc 署名（bundle のたびに必要） |
| `./icons/generate.sh` | `icons/icon.svg` から PNG / ICO を再生成 |

flake は devShell に加え、macOS 向けの `packages`（`nix/package.nix`）と `apps` を出している。詳しくは「Nix パッケージ」を参照。

### 設定ファイル

`config::path()` は `dirs::config_dir()` 配下の `ping-notifier/config.toml`（macOS: `~/Library/Application Support/`、Windows: `%APPDATA%`）。不在なら表示言語に応じて `DEFAULT_CONFIG_EN` / `DEFAULT_CONFIG_JA`（コメント付きの手書き TOML）を書き出す。`serde(deny_unknown_fields)` でキーの打ち間違いを検出する。

両テンプレートと `Config::default()` は値が一致している必要があり、テストで検証している。キーを追加するときは 3 つすべてと README（英語・日本語）の表を更新すること。

起動時に設定が壊れていてもアプリは終了せず、既定値で起動してエラーを通知する（常駐アプリなので、修正後にメニューから再読み込みしてもらう）。

## 規約とノウハウ

### 通知（macOS）

- `notify-rust` は macOS で非推奨の `NSUserNotificationCenter` を使い、送り主を指定しないと **Finder（`com.apple.Finder`）になりすまして送る**。macOS 27.0.1 ではこれが**エラーなしで破棄される**ことを確認している（`show()` は `Ok` を返し、配送確認待ちで約 2 秒ブロックするだけ）。そのため `notifier::init()` で送り主を必ず明示している
- `.app` 内から起動されたときは自身の Bundle ID、`cargo run` など素のバイナリのときは `com.apple.Terminal` を使う。判定は実行ファイルのパスに `.app/Contents/MacOS/` を含むかどうか
- `notifier::BUNDLE_ID` と `Cargo.toml` の `[package.metadata.bundle] identifier` は**必ず一致させる**（現在は `com.novalumo.ping-notifier`）
- Bundle ID を変えると macOS は別アプリとみなすため、通知の許可を取り直すことになる
- 通知の許可前は、許可ダイアログのアイコンが空白になることがある。許可後のバナーには正しく表示される

### ping

- raw ICMP ソケットは OS によって管理者権限が必要なため、OS 標準の `ping` コマンドに委譲している。OS ごとにオプションの単位が違うので `build_command` を `cfg` で分けている
- macOS の `-W` はミリ秒単位だが、`-c 1 -W` だけだと「`-W` + 約 1 秒」待ってから終了する。全体のタイムアウト `-t`（秒単位）も併用している。そのため実効タイムアウトは秒単位に切り上がる
- Linux の `-W` は秒単位
- Windows の `ping` は「宛先ホストに到達できません」でも終了コード 0 を返すため、出力に `TTL=` が含まれるかも確認する（日本語版でも同じ表記）
- Windows では `CREATE_NO_WINDOW` を付けて起動する。リリースビルドは `windows_subsystem = "windows"` の GUI アプリなので、付けないと ping のたびにコンソールが一瞬表示される
- ロスを手元で再現するには、到達不能な TEST-NET アドレス `192.0.2.1` を監視対象にする

### 通知の頻度

`Monitor` はロス状態に入ったとき（`threshold` 回連続）と復旧したときだけイベントを返す。ロスが続いている間は繰り返し通知しない。一時停止と設定の再読み込みでは `Monitor` を作り直し、ロス回数を持ち越さない。

### メニューバー / トレイ

- macOS ではイベントループの開始後に `TrayIcon` を作る必要があるため、`StartCause::Init` で生成している
- Dock に表示しないため、実行時の `ActivationPolicy::Accessory`（`cargo run` 用）と `Info.plist` の `LSUIElement`（`.app` 用）の両方を設定している
- GUI アプリでは標準出力が見えないので、利用者に伝えるべきエラーは `report_error` で通知にも出す

### パッケージングとアイコン（cargo-bundle の癖）

- `osx_info_plist_exts` のファイルは `Info.plist` の `<dict>` 内に**そのまま差し込まれる**。完全な plist ではなく、`<key>` と値だけの断片を書くこと（XML 宣言や `<dict>` を含めると壊れた plist になる）
- 1024px の画像は `1024x1024.png` という名前では `Failed to create app icon` で失敗する。`512x512@2x.png` という名前にする必要があり、`icons/generate.sh` はその名前で出力している
- 生成物（`icons/png/`、`icons/icon.ico`）もリポジトリに含めている（ビルドに ImageMagick 等を要求しないため）。アイコンを変えるときは `icons/icon.svg` を編集してスクリプトで再生成する
- Windows の `.exe` へのアイコン埋め込みは `build.rs` が `winresource` で行う。対象 OS は `CARGO_CFG_TARGET_OS` で判定する（`cfg(windows)` はビルドホストの判定になるため使わない）

### 自動アップデート

- `updater.rs` は `api.github.com/repos/siraken/ping-notifier/releases/latest` を認証なしで読む。リポジトリが非公開だと 404 になり更新できない
- 配布ファイル名（`PingNotifier-<ver>-macos-arm64.zip` / `-windows-x64.zip`）と `SHA256SUMS` はアップデータとリリースワークフローの間の契約。どちらかを変えるときは両方を合わせること
- 検証: `SHA256SUMS` のハッシュ照合に加え、macOS では Bundle ID・`codesign --verify --deep --strict`・（実行中のアプリが Developer ID 署名なら）同一 Team ID の要件を確認する。Windows は署名がないのでハッシュ照合のみ
- macOS の置き換えは `.app` と同じディレクトリに `.ping-notifier-update/` を作り、`ditto` で展開して `rename` で入れ替える（同一ボリューム内で原子的に入れ替えるため）。署名済み `.app` の展開に `unzip` や `zip` クレートを使うと拡張属性やシンボリックリンクが崩れて署名が壊れるので `ditto` を使う
- macOS の再起動は、**自身が動いているうちに** `open -n` で新しい版を起動し、成功を確認してから終了する。以前は子プロセスの `/bin/sh` で自身の終了を待ってから `open` していたが、置き換え直後の .app に対する Gatekeeper の初回起動処理（CoreServicesUIAgent の quarantine-resolver）で要求元の自身が既に存在せず `-600 procNotFound` で失敗し、アプリが消えたまま起動しないことがあった（v0.3.0 で発生）。`-n` がないと同じ Bundle ID の自身が前面に出るだけになる。新旧が一瞬同時に動くのは許容している
- Windows は実行中の exe を上書きできないが名前は変えられるため、`ping-notifier.exe.old` に退避して差し替え、新しい exe を起動してから終了する。`.old` は次回起動時に `cleanup_previous` が消す
- 開発ビルド（macOS で `.app` 外、Windows の debug ビルド）では置き換えず、ダウンロードページの案内にとどめる
- 自動更新に失敗した版は、手動確認されるまで自動では再試行しない（失敗通知の繰り返しを防ぐ）
- `cfg(target_os = "macos")` 内だけで使う関数を共通部分に置くと、Windows の CI で dead code として `-D warnings` に落ちる。OS 固有の補助関数は `platform` モジュール内に置くこと

### Nix パッケージ

- 対象は `aarch64-darwin` のみ（配布物と同じく Intel Mac には対応しない）
- `nix/package.nix` は `buildRustPackage` でソースからビルドし、`cargo bundle` で `.app` を作って `$out/Applications` に置く。バージョンは `Cargo.toml` から読む。`cargoLock.lockFile` を使うので、依存を変えても Nix 側のハッシュ更新は不要
- `$out/bin/ping-notifier` は `.app` 内の実行ファイルを `exec` するシェルスクリプト。バイナリを直接 `bin` に置くと `.app` 外の起動と判定され、通知がターミナル.app 名義になり、ログイン時の起動も使えなくなる
- ビルド時に `PING_NOTIFIER_DISABLE_SELF_UPDATE` を設定し、`updater::can_self_update` が `false` を返すようにしている（`option_env!` でコンパイル時に埋め込む）。`/nix/store` は読み取り専用で、更新は Nix が担うため。新版の通知とダウンロードページの案内は残る
- cargo-bundle は `CFBundleVersion` にビルド時刻を入れるので、再現性のため `Info.plist` をバージョンで書き換えている
- fixupPhase の strip でバイナリが変わるため、`postFixup` で `rcodesign` を使って `.app` 全体に ad-hoc 署名し直している（サンドボックス内では `/usr/bin/codesign` を使えない）
- ソースは `lib.fileset` で必要なファイルだけに絞っている。ビルドに使うファイルを追加したら `fileset` にも加えること
- Nix 版のバイナリは `/nix/store` の libiconv にリンクする。ハードンドランタイムを付けていない ad-hoc 署名なので問題なく起動する（配布物を CI でビルドする理由とは別の話）

### ログイン時の起動

- 状態は OS 側の登録を正とし、設定ファイルには持たない（システム設定などで無効にされても表示が食い違わないように）。メニュー操作後は OS の実際の状態でチェック表示を上書きする
- macOS は `SMAppService.mainAppService`（macOS 13 以降）。LaunchAgent の plist を自前で置く方式はログイン項目に出どころ不明の項目として表示され、AppleScript 方式は自動化の許可ダイアログが出るため採用していない
- `mainAppService` は実行中の .app 自身を登録するため、.app 外（`cargo run`）では使えない。判定は `notifier::running_in_app_bundle()` を共用
- 以前ユーザーがシステム設定で無効にしていると、登録しても `RequiresApproval` になる。その場合は `openSystemSettingsLoginItems` で設定画面を開いて許可を促す
- 自動アップデートは .app を同じパスで置き換えるので、登録は引き継がれる想定（Bundle ID と Team ID が変わらないため）
- macOS のログイン項目（BTM）は、登録された .app が元の場所からなくなると、LaunchServices が知っている**同じ Bundle ID の別のコピー**に登録先を付け替えることがある。v0.5.0 への入れ替え時、`/Applications` の .app をゴミ箱に移した直後に登録先が `/nix/store/...`（`nix run` で起動した Nix 版）に変わり、新しい .app を `/Applications` に置いて起動すると戻ったのを `sfltool dumpbtm` で確認した。Homebrew 版・ダウンロード版と Nix 版を同時に入れると、ログイン時に意図しない方が起動しうる（README で同時インストールを避けるよう案内している理由）
- Windows は `HKCU\...\Run` に引用符付きの exe パスを登録する。登録値が現在の exe パスと一致しなければ無効とみなす。タスクマネージャーの「スタートアップ アプリ」で無効化された状態（`StartupApproved`）までは見ていないため、その場合はメニュー上オンのままになる

### 多言語対応（i18n）

- 対応言語は英語と日本語。既定の `language = "auto"` では OS の優先言語（`sys-locale`）を上から見て最初に対応している言語を使い、なければ英語
- 文言は `i18n::Msg` のバリアントで、`en()` / `ja()` が網羅的に `match` する。文言を足したら両方に訳を書く（書かないとコンパイルエラー）。言語を足すときは `Lang` にバリアントを足し、`Msg::in_lang` と同様の関数を足す
- **翻訳するのはメニュー・ツールチップ・通知の見出しと案内文だけ**。ログと anyhow のエラーメッセージ（通知の本文に埋め込まれる詳細を含む）は英語で書く。ログに `Msg` を出すときは `msg.in_lang(Lang::En)` を使う（`report_error` 参照）
- 表示言語はグローバル（`i18n::set` / `current`）。起動直後は OS の言語、設定を読んだ後は `config.language` で上書きし、「設定を再読み込み」で変わったら `MenuItems::relabel` で全項目を付け直す
- 初回の設定ファイルは表示言語に合わせて `DEFAULT_CONFIG_EN` / `DEFAULT_CONFIG_JA` を書き出す。キーを追加するときは両方のテンプレートと `Config::default()`、README（英語・日本語）の表を更新する。テストで両テンプレートと既定値の一致を確認している
- `CHANGELOG.md` は [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 形式で英語のみ。利用者に見える変更（機能・修正・配布物・対応 OS）は `## [Unreleased]` に追記し、リリース時にバージョンと日付の見出しへ移して末尾の比較リンクを足す。内部的な変更（CI・リファクタリング・AGENTS.md）は載せない
- README は英語版（`README.md`）が正、日本語版は `README.ja.md`。内容を変えるときは両方を更新する。リリースノートのテンプレート（`.github/release-notes.md`）も英語を先、日本語を `<details>` 内に置いた二言語構成

## CI とリリース

| ワークフロー | 契機 | 内容 |
| --- | --- | --- |
| `.github/workflows/ci.yml` | main への push、PR | macOS / Windows で `fmt --check`・`clippy -D warnings`・`test` |
| `.github/workflows/release.yml` | `v*` タグの push、手動実行 | macOS（Apple Silicon）の `.app` と Windows x64 `.exe` を zip 化。タグ時のみ GitHub Release を作成し、Homebrew の Cask を更新 |

- CI は Nix を使わず `dtolnay/rust-toolchain@stable` を使う（Windows ランナーで Nix が使えないため）。`cargo-bundle` は `cargo install` で入れる
- macOS は Apple Silicon（`aarch64-apple-darwin`）のみ。Intel Mac は今後廃止されるため対応しない（v0.4.0 までは `lipo` で結合した universal を `-macos-universal.zip` として配布していた）。`cargo bundle` は `--locked` を受け付けないので、先に `cargo build --locked` しておく
- リリースはタグと `Cargo.toml` の `version` の一致を検証する。バージョンを上げるときは `Cargo.toml` を更新してからタグを打つ
- Release 本文は `.github/release-notes.md`（インストール手順）に、GitHub の自動生成ノートを連結したもの
- macOS は Secrets（`MACOS_CERTIFICATE_P12` ほか。README 参照）が登録されていれば Developer ID 署名（ハードンドランタイム + タイムスタンプ）→ `notarytool` で公証 → `stapler` で添付する。未登録なら ad-hoc 署名で配布し、ワークフローに警告を出す
- **配布物は必ず CI でビルドする**。Nix の devShell でビルドしたバイナリは `/nix/store` の dylib（libiconv など）にリンクするため、ハードンドランタイムで署名すると「異なる Team ID のライブラリ」として dyld に読み込みを拒否され起動しない（CI のバイナリは `/usr/lib` と `/System/Library` にしかリンクしない）
- `codesign --verify -R` に要件文字列を渡すときは、**別の引数として `=` 始まりで**渡す（`-R "=anchor apple generic ..."`）。`=` がないとファイルパスとして扱われ、`-R='=...'` のように連結すると構文エラーになる。`updater.rs` は前者の形
- ワークフローの `if:` では `secrets` コンテキストを直接参照できないため、ジョブの `env` に移してから `env.X != ''` で判定している
- Windows のコード署名はしていない（SmartScreen の警告が出る）
- リリースジョブは配布ファイルから `SHA256SUMS` を生成して添付する。自動アップデートの検証に使う
- Homebrew の Cask は別リポジトリ [novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap) の `Casks/ping-notifier.rb`。リリースジョブの後に `homebrew` ジョブが `version`・`sha256`・`url` を `sed` で書き換えて push する。`url` はジョブ内に直書きしているので、配布ファイル名を変えるときはここも合わせること。Cask の書式（2 スペースのインデントで `version "..."` など 1 行）を変えると置換が効かなくなるが、`grep -qxF` で検出してジョブを失敗させる
- tap への書き込みは GitHub App（`actions/create-github-app-token`、`client-id` と秘密鍵をシークレットに登録）のトークンで行う。個人の PAT に依存せず、権限を `homebrew-tap` の Contents に絞れるため。シークレットが未登録なら警告を出してスキップする
- actionlint の同梱定義が古く、`create-github-app-token@v3` の `client-id` を未定義と誤検出する。`-ignore 'create-github-app-token'` で抑制してよい
- `--locked` を付けているので、依存を変えたら `Cargo.lock` もコミットすること

## テスト方針

| 場所 | 対象 |
| --- | --- |
| `src/monitor.rs::tests` | しきい値、1 回の障害につき通知 1 回、復旧時のロス回数と停止時間 |
| `src/config.rs::tests` | 英語・日本語の初期テンプレートと `Config::default()` の一致、`language` の解釈、不正値・未知キーの拒否 |
| `src/i18n.rs::tests` | ロケール文字列からの言語判定、優先順位とフォールバック、引数付き文言の整形 |
| `src/autostart.rs` の Windows 用 tests | Run キーに登録するコマンドの引用符（Windows の CI でのみ実行） |
| `src/updater.rs::tests` | タグのバージョン解釈と比較、`SHA256SUMS` の解析、SHA-256、OS ごとの配布ファイルの選択 |

`ping.rs`・`notifier.rs`・UI・アップデートの置き換え処理は OS やネットワークに依存するため自動テストはない。変更したら実機で確認すること。macOS では `.app` を作って起動し、`192.0.2.1` を監視する設定で通知が出るかを見る。

## 未検証・既知の制約

- Nix 版は aarch64-darwin で、通知の表示と、新版を見つけても置き換えずに案内だけ出すことを確認した。ログイン時の起動（`/nix/store` のパスでの `SMAppService` 登録）は確認していない
- Windows 版はビルド・clippy・テストを CI で確認しているが、実機での動作は確認していない。通知は `notify-rust` の既定（PowerShell の AppUserModelID）名義で送られる。自前の名義にするには、インストーラーで AppUserModelID を登録する必要がある
- IPv6 は未対応（macOS では IPv6 に `ping6` が別途必要）
- 復旧通知は判定ロジックのテストのみで、実際の回線断からの復旧では確認していない
