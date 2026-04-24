use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, GrayImage, RgbImage};
use lopdf::{Document, Object, SaveOptions, Stream};
use std::fs;
use std::path::Path;

use crate::types::{PdfToolError, Result};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompressionStats {
    pub(crate) original_size: u64,
    pub(crate) output_size: u64,
    pub(crate) fallback_to_original: bool,
    pub(crate) images_optimized: usize,
    pub(crate) level: u8,
}

#[derive(Debug, Clone, Copy)]
struct CompressionProfile {
    jpeg_quality: u8,
    max_dimension: u32,
    min_pixels: u32,
    min_savings_bytes: usize,
    zlib_level: u8,
    max_objects_per_stream: usize,
}

fn compression_profile(level: u8) -> CompressionProfile {
    match level {
        1 => CompressionProfile {
            jpeg_quality: 82,
            max_dimension: 2800,
            min_pixels: 350_000,
            min_savings_bytes: 4096,
            zlib_level: 6,
            max_objects_per_stream: 100,
        },
        3 => CompressionProfile {
            jpeg_quality: 38,
            max_dimension: 1400,
            min_pixels: 50_000,
            min_savings_bytes: 1024,
            zlib_level: 9,
            max_objects_per_stream: 250,
        },
        _ => CompressionProfile {
            jpeg_quality: 62,
            max_dimension: 2000,
            min_pixels: 120_000,
            min_savings_bytes: 2048,
            zlib_level: 7,
            max_objects_per_stream: 180,
        },
    }
}

fn normalized_level(level: u8) -> u8 {
    level.clamp(1, 3)
}

fn stream_has_filter(stream: &Stream, filter_name: &[u8]) -> bool {
    stream
        .filters()
        .map(|filters| filters.contains(&filter_name))
        .unwrap_or(false)
}

fn is_image_xobject(stream: &Stream) -> bool {
    stream
        .dict
        .get(b"Subtype")
        .and_then(Object::as_name)
        .map(|name| name == b"Image")
        .unwrap_or(false)
}

fn decode_raw_image_stream(stream: &Stream) -> Option<(DynamicImage, bool)> {
    let width = stream.dict.get(b"Width").and_then(Object::as_i64).ok()? as u32;
    let height = stream.dict.get(b"Height").and_then(Object::as_i64).ok()? as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let bits = stream
        .dict
        .get(b"BitsPerComponent")
        .and_then(Object::as_i64)
        .unwrap_or(8);
    if bits != 8 {
        return None;
    }

    if stream.dict.get(b"Decode").is_ok() {
        return None;
    }

    let color_space_obj = stream.dict.get(b"ColorSpace").ok();
    let color_name = match color_space_obj {
        Some(object) => {
            if let Ok(name) = object.as_name() {
                name.to_vec()
            } else if let Ok(array) = object.as_array() {
                array.first()?.as_name().ok()?.to_vec()
            } else {
                return None;
            }
        }
        None => b"DeviceRGB".to_vec(),
    };

    let raw = if stream_has_filter(stream, b"FlateDecode")
        || stream_has_filter(stream, b"LZWDecode")
        || stream_has_filter(stream, b"ASCII85Decode")
    {
        stream.decompressed_content().ok()?
    } else if stream.dict.get(b"Filter").is_err() {
        stream.content.clone()
    } else {
        return None;
    };

    let pixel_count = (width as usize).checked_mul(height as usize)?;
    if color_name.as_slice() == b"DeviceGray" {
        let needed = pixel_count;
        if raw.len() < needed {
            return None;
        }
        let img = GrayImage::from_raw(width, height, raw[..needed].to_vec())?;
        Some((DynamicImage::ImageLuma8(img), true))
    } else if color_name.as_slice() == b"DeviceCMYK" {
        let needed = pixel_count.checked_mul(4)?;
        if raw.len() < needed {
            return None;
        }
        let mut rgb = Vec::with_capacity(pixel_count * 3);
        for chunk in raw[..needed].chunks_exact(4) {
            let c = chunk[0] as f32 / 255.0;
            let m = chunk[1] as f32 / 255.0;
            let y = chunk[2] as f32 / 255.0;
            let k = chunk[3] as f32 / 255.0;
            let r = (255.0 * (1.0 - c) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
            let g = (255.0 * (1.0 - m) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
            let b = (255.0 * (1.0 - y) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
            rgb.extend_from_slice(&[r, g, b]);
        }
        let img = RgbImage::from_raw(width, height, rgb)?;
        Some((DynamicImage::ImageRgb8(img), false))
    } else if color_name.as_slice() == b"DeviceRGB" {
        let needed = pixel_count.checked_mul(3)?;
        if raw.len() < needed {
            return None;
        }
        let img = RgbImage::from_raw(width, height, raw[..needed].to_vec())?;
        Some((DynamicImage::ImageRgb8(img), false))
    } else {
        None
    }
}

fn decode_image_stream(stream: &Stream) -> Option<(DynamicImage, bool)> {
    if stream.dict.has(b"SMask") || stream.dict.has(b"Mask") {
        return None;
    }

    if stream_has_filter(stream, b"DCTDecode") {
        let image = image::load_from_memory(&stream.content).ok()?;
        let is_gray = matches!(
            image.color(),
            image::ColorType::L8
                | image::ColorType::La8
                | image::ColorType::L16
                | image::ColorType::La16
        );
        return Some((image, is_gray));
    }

    decode_raw_image_stream(stream)
}

fn recompress_image_stream(stream: &mut Stream, profile: CompressionProfile) -> bool {
    if !is_image_xobject(stream) {
        return false;
    }

    let Some((mut image, is_gray)) = decode_image_stream(stream) else {
        return false;
    };
    let (orig_w, orig_h) = image.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return false;
    }

    let pixels = orig_w.saturating_mul(orig_h);
    if pixels < profile.min_pixels {
        return false;
    }

    if orig_w.max(orig_h) > profile.max_dimension {
        let ratio = profile.max_dimension as f32 / orig_w.max(orig_h) as f32;
        let new_w = ((orig_w as f32 * ratio).round() as u32).max(1);
        let new_h = ((orig_h as f32 * ratio).round() as u32).max(1);
        image = image.resize(new_w, new_h, FilterType::Triangle);
    }

    let normalized = if is_gray {
        DynamicImage::ImageLuma8(image.to_luma8())
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
    };

    let mut encoded = Vec::new();
    if JpegEncoder::new_with_quality(&mut encoded, profile.jpeg_quality)
        .encode_image(&normalized)
        .is_err()
    {
        return false;
    }
    if encoded.is_empty() {
        return false;
    }

    let old_len = stream.content.len();
    if encoded.len().saturating_add(profile.min_savings_bytes) >= old_len {
        return false;
    }

    stream.dict.set("Filter", "DCTDecode");
    stream.dict.remove(b"DecodeParms");
    stream.dict.set("BitsPerComponent", 8);
    stream.dict.set("Width", normalized.width() as i64);
    stream.dict.set("Height", normalized.height() as i64);
    if is_gray {
        stream.dict.set("ColorSpace", "DeviceGray");
    } else {
        stream.dict.set("ColorSpace", "DeviceRGB");
    }
    stream.set_content(encoded);
    true
}

fn optimize_images_for_compression(doc: &mut Document, level: u8) -> usize {
    let profile = compression_profile(normalized_level(level));
    let mut optimized = 0usize;
    for object in doc.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut()
            && recompress_image_stream(stream, profile)
        {
            optimized += 1;
        }
    }
    optimized
}

pub(crate) fn compress_pdf(
    input_path: &Path,
    output_path: &Path,
    level: u8,
) -> Result<CompressionStats> {
    let level = normalized_level(level);
    let original_bytes = fs::read(input_path).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to read input file '{}': {e}",
            input_path.display()
        ))
    })?;
    let original_size = original_bytes.len() as u64;

    let mut doc = Document::load_mem(&original_bytes)
        .map_err(|e| PdfToolError::new(format!("Failed to compress file: {e}")))?;
    let images_optimized = optimize_images_for_compression(&mut doc, level);
    doc.compress();

    let mut compressed_bytes = Vec::new();
    let profile = compression_profile(level);
    let options = SaveOptions::builder()
        .use_object_streams(true)
        .use_xref_streams(true)
        .max_objects_per_stream(profile.max_objects_per_stream)
        .compression_level(u32::from(profile.zlib_level))
        .build();
    doc.save_with_options(&mut compressed_bytes, options)
        .map_err(|e| PdfToolError::new(format!("Failed to compress file: {e}")))?;

    let compressed_size = compressed_bytes.len() as u64;
    if compressed_size >= original_size {
        fs::write(output_path, &original_bytes).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to save output file '{}': {e}",
                output_path.display()
            ))
        })?;
        return Ok(CompressionStats {
            original_size,
            output_size: original_size,
            fallback_to_original: true,
            images_optimized,
            level,
        });
    }

    fs::write(output_path, &compressed_bytes).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to save compressed output '{}': {e}",
            output_path.display()
        ))
    })?;
    Ok(CompressionStats {
        original_size,
        output_size: compressed_size,
        fallback_to_original: false,
        images_optimized,
        level,
    })
}

#[cfg(test)]
mod tests {
    use super::{compression_profile, normalized_level};

    #[test]
    fn compression_level_is_clamped_to_supported_profiles() {
        assert_eq!(normalized_level(0), 1);
        assert_eq!(normalized_level(1), 1);
        assert_eq!(normalized_level(2), 2);
        assert_eq!(normalized_level(3), 3);
        assert_eq!(normalized_level(9), 3);
    }

    #[test]
    fn aggressive_profile_trades_quality_for_smaller_images() {
        let light = compression_profile(1);
        let balanced = compression_profile(2);
        let aggressive = compression_profile(3);

        assert!(light.jpeg_quality > balanced.jpeg_quality);
        assert!(balanced.jpeg_quality > aggressive.jpeg_quality);
        assert!(light.max_dimension > balanced.max_dimension);
        assert!(balanced.max_dimension > aggressive.max_dimension);
        assert!(light.min_pixels > balanced.min_pixels);
        assert!(balanced.min_pixels > aggressive.min_pixels);
    }
}
