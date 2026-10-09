//! メニューバー / タスクトレイ用の状態アイコンを実行時に描画する。
//! 画像ファイルを同梱せずに済むよう、単色の円を RGBA で生成している。

use tray_icon::Icon;

const SIZE: u32 = 32;

pub const GREEN: [u8; 3] = [0x34, 0xc7, 0x59];
pub const RED: [u8; 3] = [0xff, 0x3b, 0x30];
pub const GRAY: [u8; 3] = [0x8e, 0x8e, 0x93];
pub const ORANGE: [u8; 3] = [0xff, 0x95, 0x00];

/// 縁をアンチエイリアスした円のアイコンを作る
pub fn circle([r, g, b]: [u8; 3]) -> Icon {
    let center = SIZE as f32 / 2.0;
    let radius = center - 4.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = (radius + 0.5 - dist).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[r, g, b, (alpha * 255.0) as u8]);
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("アイコンのサイズが不正です")
}
