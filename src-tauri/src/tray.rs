use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use rusttype::{Font, Scale};
use tauri::image::Image;

/// Applies rounded corners to an RGBA image in-place.
fn round_corners(img: &mut RgbaImage, radius: u32) {
    let (w, h) = img.dimensions();
    let r = radius.min(w / 2).min(h / 2) as i64;
    let corners: [(i64, i64); 4] = [
        (r, r),
        (w as i64 - 1 - r, r),
        (r, h as i64 - 1 - r),
        (w as i64 - 1 - r, h as i64 - 1 - r),
    ];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            // Only check pixels inside the corner squares
            let in_corner = (x < r && y < r)           // top-left
                || (x >= w as i64 - r && y < r)         // top-right
                || (x < r && y >= h as i64 - r)         // bottom-left
                || (x >= w as i64 - r && y >= h as i64 - r); // bottom-right
            if !in_corner {
                continue;
            }
            // Find the nearest corner center
            let dist = corners
                .iter()
                .map(|(cx, cy)| ((x - cx).pow(2) + (y - cy).pow(2)) as f64)
                .fold(f64::MAX, f64::min)
                .sqrt();
            if dist > r as f64 {
                img.put_pixel(x as u32, y as u32, Rgba([0, 0, 0, 0]));
            }
        }
    }
}

fn load_font(data: &[u8]) -> Option<Font<'_>> {
    Font::try_from_bytes(data)
        .or_else(|| Font::try_from_bytes_and_index(data, 0))
}

fn load_font_data() -> Vec<u8> {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ]
    } else if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ]
    } else {
        &[]
    };

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            return data;
        }
    }

    include_bytes!("../icons/32x32.png").to_vec()
}

pub fn generate_icon(version: &str) -> Image<'static> {
    let major = version
        .strip_prefix('v')
        .unwrap_or(version)
        .split('.')
        .next()
        .unwrap_or("?");

    let mut img = RgbaImage::new(32, 32);

    for pixel in img.pixels_mut() {
        *pixel = Rgba([0x33, 0x99, 0x33, 0xff]);
    }

    let font_data = load_font_data();
    if let Some(font) = load_font(&font_data) {
        // Dynamically choose font size so text fits inside 32×32
        let font_size: f32 = match major.len() {
            1 => 20.0,
            2 => 18.0,
            3 => 14.0,
            _ => 10.0,
        };
        let scale = Scale {
            x: font_size,
            y: font_size,
        };

        // Measure actual rendered width via glyph advances
        let text_width: f32 = major
            .chars()
            .map(|c| font.glyph(c).scaled(scale).h_metrics().advance_width)
            .sum();

        let x = ((32.0 - text_width) / 2.0).max(1.0) as i32;
        let y = ((32.0 - font_size) / 2.0) as i32 + (font_size * 0.35) as i32;

        draw_text_mut(&mut img, Rgba([0xff, 0xff, 0xff, 0xff]), x, y, scale, &font, major);
    }

    round_corners(&mut img, 6);

    let (width, height) = img.dimensions();
    Image::new_owned(img.into_raw(), width, height)
}
