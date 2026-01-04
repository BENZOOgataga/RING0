use std::env;
use std::fs;
use std::path::PathBuf;

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};

fn main() {
    if cfg!(target_os = "windows") {
        if let Err(err) = build_windows_icon() {
            println!("cargo:warning=icon build failed: {err}");
        }
    }
}

fn build_windows_icon() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let icon_path = out_dir.join("ring0.ico");
    let rc_path = out_dir.join("ring0.rc");

    let sizes = [16u32, 20, 24, 32, 48, 64, 128, 256];
    let mut icon_dir = IconDir::new(ResourceType::Icon);
    for size in sizes {
        let rgba = make_terminal_icon_rgba(size, size);
        let image = IconImage::from_rgba_data(size, size, rgba);
        icon_dir.add_entry(IconDirEntry::encode(&image)?);
    }

    let mut file = fs::File::create(&icon_path)?;
    icon_dir.write(&mut file)?;

    let icon_path = icon_path.display().to_string().replace('\\', "\\\\");
    let rc_contents = format!("1 ICON \"{}\"\n", icon_path);
    fs::write(&rc_path, rc_contents)?;

    embed_resource::compile(rc_path, std::iter::empty::<&str>());
    Ok(())
}

fn make_terminal_icon_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut buffer = vec![0u8; (width * height * 4) as usize];
    let bg = [12u8, 16u8, 22u8, 255u8];
    fill_rect(&mut buffer, width, 0, 0, width, height, bg);

    let bar = [20u8, 26u8, 34u8, 255u8];
    fill_rect(&mut buffer, width, 0, 0, width, 36, bar);

    let accent = [80u8, 160u8, 255u8, 255u8];
    fill_rect(&mut buffer, width, 28, 14, 20, 12, accent);
    fill_rect(&mut buffer, width, 54, 14, 20, 12, [60u8, 210u8, 120u8, 255u8]);
    fill_rect(&mut buffer, width, 80, 14, 20, 12, [255u8, 208u8, 90u8, 255u8]);

    let prompt = [230u8, 240u8, 250u8, 255u8];
    draw_glyph(
        &mut buffer,
        width,
        64,
        92,
        &GLYPH_GT,
        14,
        prompt,
    );
    draw_glyph(
        &mut buffer,
        width,
        140,
        110,
        &GLYPH_UNDERSCORE,
        14,
        prompt,
    );

    buffer
}

fn fill_rect(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_w: u32,
    rect_h: u32,
    color: [u8; 4],
) {
    let max_x = (x + rect_w).min(width);
    for py in y..(y + rect_h) {
        for px in x..max_x {
            set_pixel(buffer, width, px, py, color);
        }
    }
}

fn draw_glyph(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    glyph: &[&str; 7],
    scale: u32,
    color: [u8; 4],
) {
    for (row, line) in glyph.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != '1' {
                continue;
            }
            let px = x + col as u32 * scale;
            let py = y + row as u32 * scale;
            fill_rect(buffer, width, px, py, scale, scale, color);
        }
    }
}

fn set_pixel(buffer: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let idx = (y * width + x) as usize * 4;
    if idx + 4 <= buffer.len() {
        buffer[idx..idx + 4].copy_from_slice(&color);
    }
}

const GLYPH_GT: [&str; 7] = [
    "10000", "01000", "00100", "01000", "10000", "00000", "00000",
];
const GLYPH_UNDERSCORE: [&str; 7] = [
    "00000", "00000", "00000", "00000", "00000", "11111", "00000",
];
