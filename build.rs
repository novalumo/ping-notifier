fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");

    // Windows 向けのときだけ、実行ファイルにアプリアイコンを埋め込む
    // （macOS の .app のアイコンは cargo-bundle が icons/png から作る）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("icons/icon.ico")
            .compile()
            .expect("アイコンを埋め込めませんでした");
    }
}
