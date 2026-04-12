mod cli;
mod commands;
mod core;
mod types;
mod update;

use clap::{CommandFactory, Parser};
use cli::{menu_items, normalize_argv, Cli, Commands};
use commands::{
    handle_compress, handle_encrypt, handle_images_to_pdf, handle_merge, handle_reorder,
    handle_split, handle_update, handle_watermark,
};
use core::page::{build_reordered_sequence, parse_page_order};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use directories::BaseDirs;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, GrayImage, RgbImage};
use lopdf::content::{Content, Operation};
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
use lopdf::{
    dictionary, Dictionary, Document, EncryptionState, EncryptionVersion, Object, ObjectId,
    Permissions, SaveOptions, Stream,
};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use types::{PdfToolError, Result};
use update::{check_version_state, VersionState, UPDATE_REPO_REF, UPDATE_REPO_URL};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff"];
const DOCX_REL_TYPE_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const DOCX_REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DOCX_HEADER_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CONTROL_BACK_TO_MENU: &str = "__CONTROL_BACK_TO_MENU__";
const MULTUS_ASCII_LOGO: &[&str] = &[
    "███╗   ███╗██╗   ██╗██╗  ████████╗██╗   ██╗███████╗",
    "████╗ ████║██║   ██║██║  ╚══██╔══╝██║   ██║██╔════╝",
    "██╔████╔██║██║   ██║██║     ██║   ██║   ██║███████╗",
    "██║╚██╔╝██║██║   ██║██║     ██║   ██║   ██║╚════██║",
    "██║ ╚═╝ ██║╚██████╔╝███████╗██║   ╚██████╔╝███████║",
    "╚═╝     ╚═╝ ╚═════╝ ╚══════╝╚═╝    ╚═════╝ ╚══════╝",
];

static INTERACTIVE_MODE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
struct CompressionStats {
    original_size: u64,
    output_size: u64,
    fallback_to_original: bool,
    images_optimized: usize,
    level: u8,
}

#[derive(Debug, Clone, Copy)]
struct CompressionProfile {
    jpeg_quality: u8,
    max_dimension: u32,
    min_pixels: u32,
}

fn strip_wrapping_quotes(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn user_home_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

fn expand_user(path_text: &str) -> PathBuf {
    if path_text == "~" {
        if let Some(home) = user_home_dir() {
            return home;
        }
    }

    if let Some(rest) = path_text.strip_prefix("~/") {
        if let Some(home) = user_home_dir() {
            return home.join(rest);
        }
    }

    if let Some(rest) = path_text.strip_prefix("~\\") {
        if let Some(home) = user_home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(path_text)
}

fn resolve_input_path(input_arg: &str) -> Result<PathBuf> {
    let cleaned = strip_wrapping_quotes(input_arg);
    let expanded = expand_user(&cleaned);
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        let cwd = env::current_dir()
            .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?;
        Ok(cwd.join(expanded))
    }
}

fn ensure_output_dir(output_arg: Option<&str>) -> Result<PathBuf> {
    let cleaned = output_arg
        .map(strip_wrapping_quotes)
        .unwrap_or_default()
        .trim()
        .to_string();

    let output_dir = if cleaned.is_empty() {
        env::current_dir()
            .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
    } else {
        let base = PathBuf::from(cleaned);
        if base.is_absolute() {
            base
        } else {
            env::current_dir()
                .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
                .join(base)
        }
    };

    fs::create_dir_all(&output_dir).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to create output directory '{}': {e}",
            output_dir.display()
        ))
    })?;
    Ok(output_dir)
}

fn has_pdf_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn has_docx_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("docx"))
        .unwrap_or(false)
}

fn has_supported_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            SUPPORTED_IMAGE_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false)
}

fn open_pdf(input_path: &Path) -> Result<(Vec<u8>, usize)> {
    if !has_pdf_extension(input_path) {
        return Err(PdfToolError::new(format!(
            "Input format is not supported for this command: '{}'",
            input_path.display()
        )));
    }

    let bytes = fs::read(input_path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            PdfToolError::new(format!("File not found: '{}'", input_path.display()))
        } else if e.kind() == io::ErrorKind::IsADirectory {
            PdfToolError::new(format!(
                "Input path is a directory: '{}'",
                input_path.display()
            ))
        } else {
            PdfToolError::new(format!(
                "Failed to open file: '{}': {e}",
                input_path.display()
            ))
        }
    })?;

    let document = Document::load_mem(&bytes).map_err(|_| {
        PdfToolError::new(format!("Failed to read document: '{}'", input_path.display()))
    })?;
    let total_pages = document.get_pages().len();
    if total_pages < 1 {
        return Err(PdfToolError::new("Input has no pages."));
    }
    Ok((bytes, total_pages))
}

fn build_output_file_path(
    input_path: &Path,
    output_arg: Option<&str>,
    default_filename: &str,
) -> Result<PathBuf> {
    let raw_output = output_arg
        .map(strip_wrapping_quotes)
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut output_path = if raw_output.is_empty() {
        input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(default_filename)
    } else {
        let given = PathBuf::from(raw_output);
        if given.is_absolute() {
            given
        } else {
            env::current_dir()
                .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
                .join(given)
        }
    };

    if output_path.is_dir() {
        output_path = output_path.join(default_filename);
    } else if output_path.extension().is_none() {
        fs::create_dir_all(&output_path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to create output directory '{}': {e}",
                output_path.display()
            ))
        })?;
        output_path = output_path.join(default_filename);
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to create output directory '{}': {e}",
                parent.display()
            ))
        })?;
    }

    Ok(output_path)
}

fn split_pdf(
    input_path: &Path,
    input_pdf_bytes: &[u8],
    groups: &[Vec<u32>],
    output_dir: &Path,
) -> Result<usize> {
    let stem = input_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("output");

    let mut written = 0usize;
    for group in groups {
        if group.is_empty() {
            continue;
        }

        let mut doc = Document::load_mem(input_pdf_bytes).map_err(|_| {
            PdfToolError::new(format!("Failed to read document: '{}'", input_path.display()))
        })?;
        let selected: HashSet<u32> = group.iter().copied().collect();

        let existing_pages = doc.get_pages();
        let to_delete: Vec<u32> = existing_pages
            .keys()
            .copied()
            .filter(|page| !selected.contains(page))
            .collect();
        if !to_delete.is_empty() {
            doc.delete_pages(&to_delete);
        }

        doc.renumber_objects();
        doc.adjust_zero_pages();
        doc.compress();

        let label = if group.len() == 1 {
            group[0].to_string()
        } else {
            format!("{}-{}", group[0], group[group.len() - 1])
        };
        let output_path = output_dir.join(format!("{stem}_page_{label}.pdf"));
        doc.save(&output_path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to save split output '{}': {e}",
                output_path.display()
            ))
        })?;
        written += 1;
    }

    Ok(written)
}

fn compression_profile(level: u8) -> CompressionProfile {
    match level {
        1 => CompressionProfile {
            jpeg_quality: 78,
            max_dimension: 2600,
            min_pixels: 200_000,
        },
        3 => CompressionProfile {
            jpeg_quality: 42,
            max_dimension: 1450,
            min_pixels: 60_000,
        },
        _ => CompressionProfile {
            jpeg_quality: 58,
            max_dimension: 1900,
            min_pixels: 100_000,
        },
    }
}

fn stream_has_filter(stream: &Stream, filter_name: &[u8]) -> bool {
    stream
        .filters()
        .map(|filters| filters.iter().any(|name| *name == filter_name))
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
    } else {
        let needed = pixel_count.checked_mul(3)?;
        if raw.len() < needed {
            return None;
        }
        let img = RgbImage::from_raw(width, height, raw[..needed].to_vec())?;
        Some((DynamicImage::ImageRgb8(img), false))
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
    if encoded.is_empty() || encoded.len() >= stream.content.len() {
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
    let profile = compression_profile(level);
    let mut optimized = 0usize;
    for object in doc.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            if recompress_image_stream(stream, profile) {
                optimized += 1;
            }
        }
    }
    optimized
}

fn compress_pdf(input_path: &Path, output_path: &Path, level: u8) -> Result<CompressionStats> {
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

    let zlib_level = match level {
        1 => 6,
        2 => 8,
        _ => 9,
    };
    // Context7/lopdf recommendation: use object streams + xref streams + tuned compression level.
    let options = SaveOptions::builder()
        .use_object_streams(true)
        .use_xref_streams(true)
        .max_objects_per_stream(200)
        .compression_level(zlib_level)
        .build();
    let mut compressed_bytes = Vec::new();
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

fn merge_documents(mut docs: Vec<Document>) -> Result<Document> {
    if docs.is_empty() {
        return Err(PdfToolError::new("No input files to merge."));
    }

    let mut max_id = 1;
    let mut pages = BTreeMap::<ObjectId, Object>::new();
    let mut objects = BTreeMap::<ObjectId, Object>::new();

    for doc in &mut docs {
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        for (_, object_id) in doc.get_pages() {
            let object = doc.get_object(object_id).map_err(|e| {
                PdfToolError::new(format!("Failed to read page object while merging: {e}"))
            })?;
            pages.insert(object_id, object.to_owned());
        }
        objects.extend(doc.objects.clone());
    }

    let mut merged = Document::with_version("1.5");
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in &objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                catalog_object = Some((
                    catalog_object.map(|(id, _)| id).unwrap_or(*object_id),
                    object.clone(),
                ));
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref old)) = pages_object {
                        if let Ok(old_dictionary) = old.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }
                    pages_object = Some((
                        pages_object.map(|(id, _)| id).unwrap_or(*object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                merged.objects.insert(*object_id, object.clone());
            }
        }
    }

    let (catalog_id, catalog_obj) =
        catalog_object.ok_or_else(|| PdfToolError::new("Catalog root not found."))?;
    let (pages_id, pages_obj) =
        pages_object.ok_or_else(|| PdfToolError::new("Pages root not found."))?;

    for (object_id, object) in &pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            merged
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    if let Ok(dictionary) = pages_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", pages.len() as u32);
        dictionary.set(
            "Kids",
            pages
                .keys()
                .map(|id| Object::Reference(*id))
                .collect::<Vec<_>>(),
        );
        merged
            .objects
            .insert(pages_id, Object::Dictionary(dictionary));
    }

    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        merged
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }

    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged.objects.len() as u32;
    merged.renumber_objects();
    merged.adjust_zero_pages();
    merged.compress();
    Ok(merged)
}

fn merge_pdfs(input_paths: &[PathBuf], output_path: &Path) -> Result<()> {
    let mut docs = Vec::new();
    for path in input_paths {
        let doc = Document::load(path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to read file for merging: '{}'. {e}",
                path.display()
            ))
        })?;
        docs.push(doc);
    }

    let mut merged = merge_documents(docs)?;
    merged
        .save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save merged output: {e}")))?;
    Ok(())
}

fn encrypt_pdf(
    input_path: &Path,
    output_path: &Path,
    user_password: &str,
    owner_password: Option<&str>,
) -> Result<()> {
    let mut doc = Document::load(input_path)
        .map_err(|e| PdfToolError::new(format!("Failed to read document: {e}")))?;
    if doc.is_encrypted() {
        return Err(PdfToolError::new(
            "Input file is already encrypted. Decrypt it first before re-encrypting.",
        ));
    }

    if doc.trailer.get(b"ID").is_err() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        let doc_id = format!("multus-{}-{nonce}", std::process::id());
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal(doc_id.clone().into_bytes()),
                Object::string_literal(doc_id.into_bytes()),
            ]),
        );
    }

    let owner_password_owned = owner_password.unwrap_or(user_password).to_string();
    let permissions = Permissions::PRINTABLE
        | Permissions::COPYABLE
        | Permissions::COPYABLE_FOR_ACCESSIBILITY
        | Permissions::PRINTABLE_IN_HIGH_QUALITY;

    let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
    let version = EncryptionVersion::V4 {
        document: &doc,
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: &owner_password_owned,
        user_password,
        permissions,
    };

    let state = EncryptionState::try_from(version)
        .map_err(|e| PdfToolError::new(format!("Failed to prepare encryption: {e}")))?;
    doc.encrypt(&state)
        .map_err(|e| PdfToolError::new(format!("Failed to encrypt file: {e}")))?;
    doc.save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save encrypted output: {e}")))?;
    Ok(())
}

fn images_to_pdf(image_paths: &[PathBuf], output_path: &Path) -> Result<()> {
    if image_paths.is_empty() {
        return Err(PdfToolError::new("No image files provided."));
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_kids: Vec<Object> = Vec::new();

    for image_path in image_paths {
        let image_stream = lopdf::xobject::image(image_path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to read image '{}': {e}",
                image_path.display()
            ))
        })?;

        let width = image_stream
            .dict
            .get(b"Width")
            .and_then(Object::as_i64)
            .map_err(|e| {
                PdfToolError::new(format!(
                    "Failed to detect width for '{}': {e}",
                    image_path.display()
                ))
            })?
            .max(1);
        let height = image_stream
            .dict
            .get(b"Height")
            .and_then(Object::as_i64)
            .map_err(|e| {
                PdfToolError::new(format!(
                    "Failed to detect height for '{}': {e}",
                    image_path.display()
                ))
            })?
            .max(1);

        let content_id = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
        });
        doc.insert_image(
            page_id,
            image_stream,
            (0.0, 0.0),
            (width as f32, height as f32),
        )
        .map_err(|e| {
            PdfToolError::new(format!(
                "Failed to place image on page for '{}': {e}",
                image_path.display()
            ))
        })?;

        page_kids.push(page_id.into());
    }

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => page_kids,
        "Count" => image_paths.len() as i64,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    doc.renumber_objects();
    doc.compress();
    doc.save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save output file: {e}")))?;
    Ok(())
}

fn prepend_page_contents(doc: &mut Document, page_id: ObjectId, content: Vec<u8>) -> Result<()> {
    let page = doc
        .get_dictionary(page_id)
        .map_err(|e| PdfToolError::new(format!("Failed to read page dictionary: {e}")))?;
    let mut current_content_list: Vec<Object> = match page.get(b"Contents") {
        Ok(Object::Reference(id)) => vec![Object::Reference(*id)],
        Ok(Object::Array(arr)) => arr.clone(),
        _ => vec![],
    };

    let content_object_id = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content)));
    current_content_list.insert(0, Object::Reference(content_object_id));

    let page_mut = doc
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|e| PdfToolError::new(format!("Failed to update page contents: {e}")))?;
    page_mut.set("Contents", current_content_list);
    Ok(())
}

fn ensure_font_in_page_resources(
    doc: &mut Document,
    page_id: ObjectId,
    font_name: &str,
    font_id: ObjectId,
) -> Result<()> {
    let font_ref_id = {
        let resources_obj = doc
            .get_or_create_resources(page_id)
            .map_err(|e| PdfToolError::new(format!("Failed to get page resources: {e}")))?;
        let resources = resources_obj
            .as_dict_mut()
            .map_err(|e| PdfToolError::new(format!("Invalid resources dictionary: {e}")))?;

        if !resources.has(b"Font") {
            resources.set("Font", Dictionary::new());
        }

        let fonts_obj = resources
            .get_mut(b"Font")
            .map_err(|e| PdfToolError::new(format!("Failed to get font resources: {e}")))?;
        if let Object::Reference(id) = fonts_obj {
            Some(*id)
        } else {
            None
        }
    };

    if let Some(font_dict_id) = font_ref_id {
        let fonts = doc
            .get_object_mut(font_dict_id)
            .and_then(Object::as_dict_mut)
            .map_err(|e| PdfToolError::new(format!("Invalid font dictionary reference: {e}")))?;
        fonts.set(font_name, Object::Reference(font_id));
    } else {
        let resources_obj = doc
            .get_or_create_resources(page_id)
            .map_err(|e| PdfToolError::new(format!("Failed to get page resources: {e}")))?;
        let resources = resources_obj
            .as_dict_mut()
            .map_err(|e| PdfToolError::new(format!("Invalid resources dictionary: {e}")))?;
        let fonts = resources
            .get_mut(b"Font")
            .and_then(Object::as_dict_mut)
            .map_err(|e| PdfToolError::new(format!("Invalid font dictionary: {e}")))?;
        fonts.set(font_name, Object::Reference(font_id));
    }

    Ok(())
}

fn page_dimensions(doc: &Document, page_id: ObjectId) -> (f32, f32) {
    let default_size = (595.0f32, 842.0f32);
    let Ok(page) = doc.get_dictionary(page_id) else {
        return default_size;
    };
    let Ok(media_box) = page.get(b"MediaBox").and_then(Object::as_array) else {
        return default_size;
    };
    if media_box.len() != 4 {
        return default_size;
    }

    let x0 = media_box[0].as_float().unwrap_or(0.0);
    let y0 = media_box[1].as_float().unwrap_or(0.0);
    let x1 = media_box[2].as_float().unwrap_or(default_size.0);
    let y1 = media_box[3].as_float().unwrap_or(default_size.1);

    let width = (x1 - x0).abs();
    let height = (y1 - y0).abs();
    if width > 1.0 && height > 1.0 {
        (width, height)
    } else {
        default_size
    }
}

fn apply_pdf_watermark(input_path: &Path, output_path: &Path, watermark_text: &str) -> Result<()> {
    let mut doc = Document::load(input_path)
        .map_err(|e| PdfToolError::new(format!("Failed to read document: {e}")))?;
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let pages = doc.get_pages();
    for (_, page_id) in pages {
        ensure_font_in_page_resources(&mut doc, page_id, "FWM", font_id)?;

        let (width, height) = page_dimensions(&doc, page_id);
        let angle = 35.0f32.to_radians();
        let cos = angle.cos();
        let sin = angle.sin();

        let font_size = ((width.min(height) * 0.12).max(28.0)).min(96.0);
        let x = width * 0.18;
        let y = height * 0.45;

        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new("g", vec![0.85.into()]),
                Operation::new(
                    "cm",
                    vec![
                        cos.into(),
                        sin.into(),
                        (-sin).into(),
                        cos.into(),
                        x.into(),
                        y.into(),
                    ],
                ),
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["FWM".into(), font_size.into()]),
                Operation::new("Td", vec![0.into(), 0.into()]),
                Operation::new("Tj", vec![Object::string_literal(watermark_text)]),
                Operation::new("ET", vec![]),
                Operation::new("Q", vec![]),
            ],
        };

        let encoded = content
            .encode()
            .map_err(|e| PdfToolError::new(format!("Failed to encode watermark content: {e}")))?;
        prepend_page_contents(&mut doc, page_id, encoded)?;
    }

    doc.compress();
    doc.save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save watermarked output: {e}")))?;
    Ok(())
}

fn read_zip_entries(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let file = fs::File::open(path)
        .map_err(|e| PdfToolError::new(format!("Failed to open DOCX '{}': {e}", path.display())))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| PdfToolError::new(format!("Invalid DOCX archive: {e}")))?;

    let mut entries = BTreeMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PdfToolError::new(format!("Failed to read DOCX entry: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| {
            PdfToolError::new(format!("Failed to read DOCX entry '{}': {e}", entry.name()))
        })?;
        entries.insert(entry.name().to_string(), bytes);
    }

    Ok(entries)
}

fn write_zip_entries(path: &Path, entries: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let file = fs::File::create(path).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to create output DOCX '{}': {e}",
            path.display()
        ))
    })?;
    let mut writer = ZipWriter::new(file);
    let options =
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (name, data) in entries {
        writer
            .start_file(name, options)
            .map_err(|e| PdfToolError::new(format!("Failed to write DOCX entry '{name}': {e}")))?;
        writer.write_all(data).map_err(|e| {
            PdfToolError::new(format!("Failed to write DOCX content for '{name}': {e}"))
        })?;
    }

    writer
        .finish()
        .map_err(|e| PdfToolError::new(format!("Failed to finalize DOCX: {e}")))?;
    Ok(())
}

fn read_xml_entry(entries: &BTreeMap<String, Vec<u8>>, path: &str) -> Result<String> {
    let bytes = entries
        .get(path)
        .ok_or_else(|| PdfToolError::new(format!("Missing DOCX entry: '{path}'")))?;
    String::from_utf8(bytes.clone())
        .map_err(|_| PdfToolError::new(format!("DOCX entry '{path}' is not valid UTF-8 XML")))
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_docx_watermark_header_xml(text: &str) -> String {
    let escaped = escape_xml_text(text);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p>
    <w:pPr>
      <w:jc w:val="center"/>
    </w:pPr>
    <w:r>
      <w:rPr>
        <w:color w:val="CFCFCF"/>
        <w:sz w:val="96"/>
        <w:szCs w:val="96"/>
        <w:b/>
      </w:rPr>
      <w:t>{escaped}</w:t>
    </w:r>
  </w:p>
</w:hdr>"#
    )
}

fn ensure_docx_content_type_override(
    content_types_xml: &mut String,
    part_name: &str,
) -> Result<()> {
    if content_types_xml.contains(part_name) {
        return Ok(());
    }

    let override_entry =
        format!(r#"<Override PartName="{part_name}" ContentType="{DOCX_HEADER_CONTENT_TYPE}"/>"#);
    if let Some(pos) = content_types_xml.find("</Types>") {
        content_types_xml.insert_str(pos, &override_entry);
        Ok(())
    } else {
        Err(PdfToolError::new(
            "Invalid [Content_Types].xml: missing </Types>.",
        ))
    }
}

fn extract_xml_attr(tag: &str, attr_name: &str) -> Option<String> {
    let needle = format!(r#"{attr_name}=""#);
    let start = tag.find(&needle)?;
    let rest = &tag[start + needle.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn ensure_docx_header_relationship(rels_xml: &mut String, target: &str) -> Result<String> {
    let mut cursor = 0usize;
    while let Some(rel_start_rel) = rels_xml[cursor..].find("<Relationship") {
        let rel_start = cursor + rel_start_rel;
        let Some(rel_end_rel) = rels_xml[rel_start..].find('>') else {
            break;
        };
        let rel_end = rel_start + rel_end_rel + 1;
        let tag = &rels_xml[rel_start..rel_end];
        if tag.contains(&format!(r#"Target="{target}""#)) && tag.contains(DOCX_REL_TYPE_HEADER) {
            if let Some(existing_id) = extract_xml_attr(tag, "Id") {
                return Ok(existing_id);
            }
        }
        cursor = rel_end;
    }

    let mut max_id = 0u32;
    let mut search = 0usize;
    while let Some(idx_rel) = rels_xml[search..].find(r#"Id="rId"#) {
        let start = search + idx_rel + r#"Id="rId"#.len();
        let digits = rels_xml[start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        if let Ok(num) = digits.parse::<u32>() {
            if num > max_id {
                max_id = num;
            }
        }
        search = start;
    }
    let new_id = format!("rId{}", max_id + 1);
    let relationship =
        format!(r#"<Relationship Id="{new_id}" Type="{DOCX_REL_TYPE_HEADER}" Target="{target}"/>"#);

    if let Some(pos) = rels_xml.find("</Relationships>") {
        rels_xml.insert_str(pos, &relationship);
        Ok(new_id)
    } else {
        Err(PdfToolError::new(
            "Invalid document.xml.rels: missing </Relationships>.",
        ))
    }
}

fn ensure_docx_relationship_namespace(document_xml: &mut String) -> Result<()> {
    if document_xml.contains(r#"xmlns:r=""#) {
        return Ok(());
    }

    let Some(start) = document_xml.find("<w:document") else {
        return Err(PdfToolError::new(
            "Invalid word/document.xml: missing <w:document> root.",
        ));
    };
    let Some(end_rel) = document_xml[start..].find('>') else {
        return Err(PdfToolError::new(
            "Invalid word/document.xml: malformed <w:document> root.",
        ));
    };
    let insert_at = start + end_rel;
    let xmlns = format!(r#" xmlns:r="{DOCX_REL_NS}""#);
    document_xml.insert_str(insert_at, &xmlns);
    Ok(())
}

fn upsert_docx_default_header_reference(document_xml: &mut String, rel_id: &str) -> Result<()> {
    let replacement = format!(r#"<w:headerReference w:type="default" r:id="{rel_id}"/>"#);

    let mut replaced_any = false;
    let mut replaced_xml = String::with_capacity(document_xml.len() + 64);
    let mut cursor = 0usize;
    while let Some(start_rel) = document_xml[cursor..].find("<w:headerReference") {
        let start = cursor + start_rel;
        replaced_xml.push_str(&document_xml[cursor..start]);

        let Some(end_rel) = document_xml[start..].find("/>") else {
            break;
        };
        let end = start + end_rel + 2;
        let tag = &document_xml[start..end];
        if tag.contains(r#"w:type="default""#) {
            replaced_xml.push_str(&replacement);
            replaced_any = true;
        } else {
            replaced_xml.push_str(tag);
        }
        cursor = end;
    }
    replaced_xml.push_str(&document_xml[cursor..]);

    if replaced_any {
        *document_xml = replaced_xml;
        return Ok(());
    }

    let mut inserted_any = false;
    let mut inserted_xml = String::with_capacity(document_xml.len() + 128);
    let mut pos = 0usize;
    while let Some(sect_rel) = document_xml[pos..].find("<w:sectPr") {
        let start = pos + sect_rel;
        inserted_xml.push_str(&document_xml[pos..start]);
        let Some(close_rel) = document_xml[start..].find('>') else {
            break;
        };
        let end = start + close_rel + 1;
        inserted_xml.push_str(&document_xml[start..end]);
        inserted_xml.push_str(&replacement);
        inserted_any = true;
        pos = end;
    }
    inserted_xml.push_str(&document_xml[pos..]);

    if inserted_any {
        *document_xml = inserted_xml;
        return Ok(());
    }

    let fallback = format!("<w:sectPr>{replacement}</w:sectPr>");
    if let Some(body_end) = document_xml.find("</w:body>") {
        document_xml.insert_str(body_end, &fallback);
        return Ok(());
    }

    Err(PdfToolError::new(
        "Invalid word/document.xml: failed to locate section properties.",
    ))
}

fn apply_docx_watermark(input_path: &Path, output_path: &Path, watermark_text: &str) -> Result<()> {
    let mut entries = read_zip_entries(input_path)?;

    let mut content_types_xml = read_xml_entry(&entries, "[Content_Types].xml")?;
    let mut rels_xml = read_xml_entry(&entries, "word/_rels/document.xml.rels")?;
    let mut document_xml = read_xml_entry(&entries, "word/document.xml")?;

    let header_target = "header_watermark.xml";
    let header_part_name = "/word/header_watermark.xml";
    let rel_id = ensure_docx_header_relationship(&mut rels_xml, header_target)?;
    ensure_docx_content_type_override(&mut content_types_xml, header_part_name)?;
    ensure_docx_relationship_namespace(&mut document_xml)?;
    upsert_docx_default_header_reference(&mut document_xml, &rel_id)?;

    let header_xml = build_docx_watermark_header_xml(watermark_text);

    entries.insert(
        "[Content_Types].xml".to_string(),
        content_types_xml.into_bytes(),
    );
    entries.insert(
        "word/_rels/document.xml.rels".to_string(),
        rels_xml.into_bytes(),
    );
    entries.insert("word/document.xml".to_string(), document_xml.into_bytes());
    entries.insert(
        "word/header_watermark.xml".to_string(),
        header_xml.into_bytes(),
    );

    write_zip_entries(output_path, &entries)?;
    Ok(())
}

fn apply_watermark(input_path: &Path, output_path: &Path, watermark_text: &str) -> Result<()> {
    if has_pdf_extension(input_path) {
        apply_pdf_watermark(input_path, output_path, watermark_text)
    } else if has_docx_extension(input_path) {
        apply_docx_watermark(input_path, output_path, watermark_text)
    } else {
        Err(PdfToolError::new(
            "Watermark currently supports only file types handled by this command.",
        ))
    }
}

fn reorder_pdf(input_path: &Path, output_path: &Path, page_order_expr: &str) -> Result<()> {
    let (pdf_bytes, total_pages) = open_pdf(input_path)?;
    let requested_order = parse_page_order(page_order_expr)?;
    let final_order = build_reordered_sequence(&requested_order, total_pages)?;

    let mut docs = Vec::new();
    for page in final_order {
        let mut page_doc = Document::load_mem(&pdf_bytes).map_err(|_| {
            PdfToolError::new(format!("Failed to read document: '{}'", input_path.display()))
        })?;

        let existing_pages = page_doc.get_pages();
        let to_delete: Vec<u32> = existing_pages
            .keys()
            .copied()
            .filter(|candidate| *candidate != page)
            .collect();
        if !to_delete.is_empty() {
            page_doc.delete_pages(&to_delete);
        }

        page_doc.renumber_objects();
        page_doc.adjust_zero_pages();
        docs.push(page_doc);
    }

    let mut merged = merge_documents(docs)?;
    merged.compress();
    merged
        .save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save reordered output: {e}")))?;
    Ok(())
}

fn prompt_non_empty(prompt: &str) -> Result<String> {
    loop {
        let value = prompt_optional(prompt)?;
        if !value.trim().is_empty() {
            return Ok(value.trim().to_string());
        }
        println!("Input cannot be empty.");
    }
}

fn prompt_optional(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|e| PdfToolError::new(format!("Failed to flush stdout: {e}")))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| PdfToolError::new(format!("Failed to read input: {e}")))?;
    let value = input.trim().to_string();
    if INTERACTIVE_MODE.load(Ordering::Relaxed) && value.eq_ignore_ascii_case("qq") {
        return Err(PdfToolError::new(CONTROL_BACK_TO_MENU));
    }
    Ok(value)
}

fn multus_orange() -> Color {
    Color::Rgb {
        r: 255,
        g: 145,
        b: 0,
    }
}

fn queue_multus_logo<W: Write>(stdout: &mut W) -> io::Result<()> {
    queue!(
        stdout,
        SetForegroundColor(multus_orange()),
        SetAttribute(Attribute::Bold)
    )?;

    for line in MULTUS_ASCII_LOGO {
        queue!(stdout, Print(*line), Print("\n"))?;
    }

    queue!(
        stdout,
        ResetColor,
        SetAttribute(Attribute::Reset),
        Print("\n")
    )?;
    Ok(())
}

fn print_banner() {
    let mut stdout = io::stdout();
    let _ = queue_multus_logo(&mut stdout);
    let _ = stdout.flush();
}

fn print_step(title: &str) {
    println!("\n[{title}]");
}

fn interactive_version_line() -> String {
    match check_version_state(UPDATE_REPO_URL, UPDATE_REPO_REF) {
        VersionState::UpdateAvailable { current, latest } => {
            format!("Update tersedia: v{current} -> v{latest} (pilih menu Update)")
        }
        VersionState::UpToDate { current } => format!("Version current: v{current}"),
        VersionState::Unknown { current } => format!("Version current: v{current}"),
    }
}

fn is_back_to_menu_error(err: &PdfToolError) -> bool {
    err.0 == CONTROL_BACK_TO_MENU
}

fn render_arrow_menu(menu_items: &[(&str, &str)], selected_index: usize, version_line: &str) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue_multus_logo(&mut stdout)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue!(
        stdout,
        Print("Use ↑/↓ to move, Enter to select, Esc for default Split.\nType QQ in any prompt to return here.\n")
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue!(stdout, Print("\n"))
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    let orange = multus_orange();
    for (index, (label, _command)) in menu_items.iter().enumerate() {
        let numbered = format!("{}. {label}", index + 1);
        if index == selected_index {
            queue!(
                stdout,
                SetForegroundColor(orange),
                SetAttribute(Attribute::Bold),
                Print(format!("❯ {numbered}\n")),
                SetAttribute(Attribute::Reset),
                ResetColor
            )
            .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
        } else {
            queue!(stdout, Print(format!("  {numbered}\n")))
                .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
        }
    }

    queue!(
        stdout,
        Print("\n"),
        Print(format!("{version_line}\n")),
        Print("Quit: tekan tombol Q dua kali.\n")
    )
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    stdout
        .flush()
        .map_err(|e| PdfToolError::new(format!("Failed to flush menu: {e}")))?;
    Ok(())
}

fn choose_command_with_arrows(menu_items: &[(&str, &str)], version_line: &str) -> Result<Option<String>> {
    if menu_items.is_empty() {
        return Err(PdfToolError::new("Menu options are empty."));
    }

    terminal::enable_raw_mode()
        .map_err(|e| PdfToolError::new(format!("Failed to enable raw mode: {e}")))?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        MoveTo(0, 0),
        Clear(ClearType::All),
        Hide
    )
    .map_err(|e| PdfToolError::new(format!("Failed to initialize interactive menu: {e}")))?;

    let mut selected = 0usize;
    let mut q_press_count = 0u8;
    let menu_result = (|| -> Result<Option<String>> {
        loop {
            render_arrow_menu(menu_items, selected, version_line)?;

            let evt = event::read()
                .map_err(|e| PdfToolError::new(format!("Failed to read keyboard input: {e}")))?;
            if let Event::Key(key_event) = evt {
                if key_event.kind == KeyEventKind::Release {
                    continue;
                }

                match key_event.code {
                    KeyCode::Up => {
                        q_press_count = 0;
                        if selected == 0 {
                            selected = menu_items.len() - 1;
                        } else {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        q_press_count = 0;
                        selected = (selected + 1) % menu_items.len();
                    }
                    KeyCode::Enter => return Ok(Some(menu_items[selected].1.to_string())),
                    KeyCode::Esc => return Ok(Some(menu_items[0].1.to_string())),
                    KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'q') => {
                        q_press_count += 1;
                        if q_press_count >= 2 {
                            return Ok(None);
                        }
                    }
                    _ => {
                        q_press_count = 0;
                    }
                }
            }
        }
    })();

    let _ = execute!(
        stdout,
        Show,
        ResetColor,
        SetAttribute(Attribute::Reset),
        LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
    menu_result
}

fn run_with_spinner<T, F>(message: &str, func: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let message_text = message.to_string();

    let spinner = thread::spawn(move || {
        let frames = ['|', '/', '-', '\\'];
        let mut idx = 0usize;
        while !stop_flag.load(Ordering::Relaxed) {
            print!("\r{} {}", message_text, frames[idx % frames.len()]);
            let _ = io::stdout().flush();
            idx += 1;
            thread::sleep(Duration::from_millis(80));
        }
        print!("\r{}\r", " ".repeat(message_text.len() + 2));
        let _ = io::stdout().flush();
    });

    let result = func();
    stop.store(true, Ordering::Relaxed);
    let _ = spinner.join();
    result
}

fn parse_cli(normalized_args: Vec<String>) -> std::result::Result<Cli, i32> {
    let mut parse_input = vec!["multus".to_string()];
    parse_input.extend(normalized_args);

    match Cli::try_parse_from(parse_input) {
        Ok(value) => Ok(value),
        Err(err) => {
            let code = err.exit_code();
            let _ = err.print();
            Err(code)
        }
    }
}

fn execute_command(cli: Cli) -> Result<i32> {
    match cli.command {
        Some(Commands::Split(args)) => handle_split(args),
        Some(Commands::Compress(args)) => handle_compress(args),
        Some(Commands::Merge(args)) => handle_merge(args),
        Some(Commands::Encrypt(args)) => handle_encrypt(args),
        Some(Commands::ImagesToPdf(args)) => handle_images_to_pdf(args),
        Some(Commands::Watermark(args)) => handle_watermark(args),
        Some(Commands::Reorder(args)) => handle_reorder(args),
        Some(Commands::Update(args)) => handle_update(args),
        None => {
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!();
            Ok(1)
        }
    }
}

fn run_interactive() -> i32 {
    INTERACTIVE_MODE.store(true, Ordering::Relaxed);
    let items = menu_items();
    let mut version_line = interactive_version_line();

    loop {
        let selected_command = match choose_command_with_arrows(&items, &version_line) {
            Ok(Some(command)) => command,
            Ok(None) => {
                INTERACTIVE_MODE.store(false, Ordering::Relaxed);
                println!("Goodbye.");
                return 0;
            }
            Err(err) => {
                INTERACTIVE_MODE.store(false, Ordering::Relaxed);
                eprintln!("Error: {err}");
                return 2;
            }
        };
        print_banner();

        let cli = match parse_cli(vec![selected_command]) {
            Ok(value) => value,
            Err(code) => {
                if code != 0 {
                    eprintln!("Error: Failed to parse selected command.");
                }
                continue;
            }
        };

        match execute_command(cli) {
            Ok(_) => {}
            Err(err) if is_back_to_menu_error(&err) => {}
            Err(err) => eprintln!("Error: {err}"),
        }

        version_line = interactive_version_line();
    }
}

fn run(argv: Option<Vec<String>>) -> i32 {
    let args = match argv {
        Some(values) => values,
        None => env::args().skip(1).collect::<Vec<_>>(),
    };

    if args.is_empty() {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            return run_interactive();
        }

        print_banner();
        println!("Non-interactive terminal detected. Running default command: split.");
        let cli = match parse_cli(vec!["split".to_string()]) {
            Ok(value) => value,
            Err(code) => return code,
        };

        return match execute_command(cli) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("Error: {err}");
                2
            }
        };
    }

    let normalized = normalize_argv(args);
    let cli = match parse_cli(normalized) {
        Ok(value) => value,
        Err(code) => return code,
    };

    match execute_command(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error: {err}");
            2
        }
    }
}

fn main() {
    std::process::exit(run(None));
}
