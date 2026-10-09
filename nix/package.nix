{
  lib,
  stdenv,
  rustPlatform,
  cargo-bundle,
  rcodesign,
}:

let
  cargoToml = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../build.rs
      ../src
      ../icons
      ../macos
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    cargo-bundle
    rcodesign
  ];

  # /nix/store は読み取り専用で、更新は Nix 側で行うため、アプリ自身による置き換えを止める
  env.PING_NOTIFIER_DISABLE_SELF_UPDATE = "1";

  # 通知の送り主やログイン時の起動は .app として起動されている必要があるため、.app を作る
  postBuild = ''
    cargo bundle --release --target ${stdenv.hostPlatform.rust.rustcTarget}
  '';

  postInstall = ''
    mkdir -p $out/Applications
    cp -R "target/${stdenv.hostPlatform.rust.rustcTarget}/release/bundle/osx/Ping Notifier.app" $out/Applications/
    # cargo-bundle は CFBundleVersion にビルド時刻を入れるため、再現性のためにバージョンで置き換える
    sed -i '/<key>CFBundleVersion<\/key>/{n;s|<string>.*</string>|<string>${cargoToml.package.version}</string>|;}' \
      "$out/Applications/Ping Notifier.app/Contents/Info.plist"
    # bin には .app 内の実行ファイルを起動するラッパーを置く（.app 内から起動したと判定させるため）
    rm $out/bin/ping-notifier
    cat > $out/bin/ping-notifier <<EOF
    #!/bin/sh
    exec "$out/Applications/Ping Notifier.app/Contents/MacOS/ping-notifier" "\$@"
    EOF
    chmod +x $out/bin/ping-notifier
  '';

  # .app 全体に ad-hoc 署名する。fixupPhase の strip で中身が変わるので、その後に行う。
  # 署名がないと SMAppService（ログイン時の起動）などが .app を正しく扱えない
  postFixup = ''
    rcodesign sign "$out/Applications/Ping Notifier.app"
  '';

  meta = {
    description = "Menu bar app that notifies you of ping timeouts";
    homepage = "https://github.com/novalumo/ping-notifier";
    mainProgram = "ping-notifier";
    # Intel Mac は今後廃止されるため対応しない
    platforms = [ "aarch64-darwin" ];
  };
}
