use image::{ImageFormat, RgbaImage};
use std::fs;
use std::path::Path;

use crate::config::Format;

pub fn save_image(
    img: &RgbaImage,
    path: &Path,
    format: &Format,
    quality: u8,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    match format {
        Format::Png => {
            img.save_with_format(path, ImageFormat::Png)
                .map_err(|e| format!("Failed to save PNG: {}", e))?;
        }
        Format::Jpeg => {
            // Convert RGBA to RGB for JPEG
            let img_rgb = image::DynamicImage::ImageRgba8(img.clone()).into_rgb8();
            let mut file =
                fs::File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
            encoder
                .encode_image(&img_rgb)
                .map_err(|e| format!("Failed to save JPEG: {}", e))?;
        }
    }

    Ok(())
}
