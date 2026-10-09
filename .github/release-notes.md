## ダウンロード

| OS | ファイル |
| --- | --- |
| macOS（Apple Silicon / Intel） | `PingNotifier-*-macos-universal.zip` |
| Windows（x64） | `PingNotifier-*-windows-x64.zip` |

## インストール

### macOS

1. zip を展開し、`Ping Notifier.app` を「アプリケーション」フォルダに移動する
2. 初回はダブルクリックすると「開けません」と表示される。「システム設定 > プライバシーとセキュリティ」の下部にある「このまま開く」を押す
   - ターミナルで `xattr -dr com.apple.quarantine "/Applications/Ping Notifier.app"` を実行してもよい
3. 初回の通知時に表示される通知許可ダイアログで「許可」を選ぶ

Apple の公証（notarization）を受けた版では手順 2 は不要。

### Windows

1. zip を展開し、`ping-notifier.exe` を任意の場所に置いて実行する
2. 「Windows によって PC が保護されました」と表示されたら「詳細情報」→「実行」を押す

## アップデート

一度インストールすれば、新しいバージョンは自動でインストールされる（メニューの「アップデートを確認」で手動確認も可能）。

## 設定

メニューバー（Windows ではタスクトレイ）のアイコンから「設定ファイルを開く」で編集し、「設定を再読み込み」で反映する。
