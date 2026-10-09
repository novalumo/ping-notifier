fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");

    // Windows 向けのときだけ、実行ファイルにアプリアイコンとバージョン情報を埋め込む
    // （macOS の .app のアイコンは cargo-bundle が icons/png から作る）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // バージョン番号（FileVersion / ProductVersion）は winresource が Cargo.toml から入れる。
        // 説明や会社名が空の exe はウイルス対策ソフトに疑われやすいため、残りの欄も埋める。
        // FileDescription はタスクマネージャーなどでアプリ名として表示される
        winresource::WindowsResource::new()
            .set_icon("icons/icon.ico")
            .set("ProductName", "Ping Notifier")
            .set("FileDescription", "Ping Notifier")
            .set("CompanyName", "Novalumo Japan G.K.")
            .set("LegalCopyright", "Copyright © 2026 Novalumo Japan G.K.")
            .set("OriginalFilename", "ping-notifier.exe")
            .set("InternalName", "ping-notifier")
            .compile()
            .expect("アイコンとバージョン情報を埋め込めませんでした");
    }
}
