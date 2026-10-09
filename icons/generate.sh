#!/usr/bin/env bash
# icons/icon.svg からアプリアイコンを生成する。`nix develop` 環境で実行すること。
#   - icons/png/*.png : cargo-bundle が macOS の .icns を作るのに使う
#   - icons/icon.ico  : Windows の実行ファイルに埋め込む
set -euo pipefail

cd "$(dirname "$0")"
mkdir -p png

for size in 16 32 64 128 256 512; do
  rsvg-convert -w "$size" -h "$size" icon.svg -o "png/${size}x${size}.png"
done
# cargo-bundle は 1024px を「512x512@2x」という名前でないと .icns に取り込めない
rsvg-convert -w 1024 -h 1024 icon.svg -o "png/512x512@2x.png"

magick png/16x16.png png/32x32.png png/64x64.png png/128x128.png png/256x256.png icon.ico

echo "generated: $(ls png | tr '\n' ' ')icon.ico"
