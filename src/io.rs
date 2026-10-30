use crate::document::{validate_dimensions, BlendMode, Document, Layer, MAX_LAYERS};
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use image::{io::Reader, ImageFormat};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"OPNT\x01\r\n\0";
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 1024 * 1024;
static SAVE_ID: AtomicU64 = AtomicU64::new(0);

pub const SUPPORTED_FORMATS: &str =
    "openpaint (*.opaint), PNG, BMP, TGA, ICO, PNM; WebP and DDS import";

fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}

pub fn open(path: &Path) -> Result<Document, String> {
    if extension(path) == "pdn" {
        return Err("paint.net .pdn projects are not supported yet. export layers as PNG from paint.net, or open an .opaint project.".into());
    }
    if extension(path) == "opaint" {
        return open_project(path);
    }
    let mut reader = Reader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(MAX_BYTES as u64);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| format!("cannot open image: {e}"))?
        .to_rgba8();
    let mut doc = Document::from_rgba(
        image.width(),
        image.height(),
        image.into_raw(),
        file_name(path),
    )?;
    doc.path = Some(path.to_path_buf());
    doc.mark_saved();
    Ok(doc)
}

pub fn save(doc: &mut Document, path: &Path) -> Result<bool, String> {
    doc.validate()?;
    if extension(path) == "pdn" {
        return Err("use .opaint for layered projects; .pdn writing is not supported.".into());
    }
    if extension(path) == "opaint" {
        atomic_write(path, |file| write_project(doc, file))?;
        doc.path = Some(path.to_path_buf());
        doc.name = file_name(path);
        doc.mark_saved();
        return Ok(true);
    }
    let format = if extension(path) == "pnm" {
        ImageFormat::Pnm
    } else {
        ImageFormat::from_path(path).map_err(|_| {
            "choose a supported file extension: .opaint, .png, .bmp, .tga, .ico, or .pnm"
                .to_string()
        })?
    };
    if !matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Bmp
            | ImageFormat::Tga
            | ImageFormat::Ico
            | ImageFormat::Pnm
    ) {
        return Err("saving this format is unavailable; choose PNG or .opaint.".into());
    }
    if format == ImageFormat::Ico && (doc.width > 256 || doc.height > 256) {
        return Err("ICO images must be at most 256 × 256 pixels.".into());
    }
    let single_layer = doc.layers.len() == 1
        && doc.layers[0].visible
        && doc.layers[0].opacity == 255
        && doc.layers[0].blend_mode == BlendMode::Normal;
    let pixels = if single_layer {
        doc.layers[0].pixels.clone()
    } else {
        doc.composite()
    };
    let preserves_alpha =
        format != ImageFormat::Pnm || pixels.chunks_exact(4).all(|pixel| pixel[3] == 255);
    let image = image::RgbaImage::from_raw(doc.width, doc.height, pixels)
        .ok_or("invalid document dimensions")?;
    let image = image::DynamicImage::ImageRgba8(image);
    atomic_write(path, |file| {
        // this portable anymap export writes rgb samples.
        let encoded = if format == ImageFormat::Pnm {
            image::DynamicImage::ImageRgb8(image.to_rgb8())
        } else {
            image
        };
        encoded.write_to(file, format).map_err(|e| e.to_string())
    })?;
    if single_layer && preserves_alpha {
        doc.path = Some(path.to_path_buf());
        doc.name = file_name(path);
        doc.mark_saved();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn write_project(doc: &Document, mut file: &mut File) -> Result<(), String> {
    let layers: Vec<Value> = doc
        .layers
        .iter()
        .map(|layer| {
            json!({
                "name": layer.name, "visible": layer.visible, "opacity": layer.opacity,
                "blend": layer.blend_mode.name()
            })
        })
        .collect();
    let header = serde_json::to_vec(&json!({"width":doc.width,"height":doc.height,"active_layer":doc.active_layer,"layers":layers})).map_err(|e|e.to_string())?;
    if header.len() > MAX_HEADER_BYTES {
        return Err("project header is too large".into());
    }
    file.write_all(MAGIC)
        .and_then(|_| file.write_all(&(header.len() as u32).to_le_bytes()))
        .and_then(|_| file.write_all(&header))
        .map_err(|e| e.to_string())?;
    let mut encoder = GzEncoder::new(&mut file, Compression::default());
    for layer in &doc.layers {
        encoder
            .write_all(&layer.pixels)
            .map_err(|e| e.to_string())?;
    }
    encoder.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn open_project(path: &Path) -> Result<Document, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err("unsupported or invalid openpaint project".into());
    }
    let mut size = [0; 4];
    file.read_exact(&mut size).map_err(|e| e.to_string())?;
    let size = u32::from_le_bytes(size) as usize;
    if size > MAX_HEADER_BYTES {
        return Err("project header is too large".into());
    }
    let mut header = vec![0; size];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    let header: Value = serde_json::from_slice(&header).map_err(|e| e.to_string())?;
    let width = u32::try_from(header["width"].as_u64().ok_or("invalid width")?)
        .map_err(|_| "invalid width")?;
    let height = u32::try_from(header["height"].as_u64().ok_or("invalid height")?)
        .map_err(|_| "invalid height")?;
    let layers = header["layers"].as_array().ok_or("missing layers")?;
    if layers.is_empty() || layers.len() > MAX_LAYERS {
        return Err("invalid layer count".into());
    }
    let layer_bytes = validate_dimensions(width, height)?;
    layer_bytes
        .checked_mul(layers.len())
        .filter(|&n| n <= MAX_BYTES)
        .ok_or("project exceeds 256 MiB pixel budget")?;
    let mut decoded = Vec::new();
    for metadata in layers {
        decoded.push(Layer {
            name: metadata["name"]
                .as_str()
                .ok_or("invalid layer name")?
                .to_string(),
            visible: metadata["visible"].as_bool().ok_or("invalid visibility")?,
            opacity: u8::try_from(metadata["opacity"].as_u64().ok_or("invalid opacity")?)
                .map_err(|_| "invalid opacity")?,
            blend_mode: BlendMode::from_name(
                metadata["blend"].as_str().ok_or("invalid blend mode")?,
            )
            .ok_or("unknown blend mode")?,
            pixels: Vec::new(),
        });
    }
    let active = usize::try_from(
        header["active_layer"]
            .as_u64()
            .ok_or("missing active layer")?,
    )
    .map_err(|_| "invalid active layer")?;
    if active >= decoded.len() {
        return Err("invalid active layer".into());
    }
    let mut decoder = GzDecoder::new(BufReader::new(file));
    for layer in &mut decoded {
        layer.pixels = vec![0; layer_bytes];
        decoder
            .read_exact(&mut layer.pixels)
            .map_err(|e| format!("damaged project: {e}"))?;
    }
    let mut extra = [0; 1];
    if decoder
        .read(&mut extra)
        .map_err(|e| format!("damaged project: {e}"))?
        != 0
    {
        return Err("project pixel data has an invalid length".into());
    }
    if decoder
        .into_inner()
        .read(&mut extra)
        .map_err(|e| e.to_string())?
        != 0
    {
        return Err("unexpected data after the project payload".into());
    }
    let mut doc = Document::from_layers(width, height, decoded, active, file_name(path))?;
    doc.path = Some(path.to_path_buf());
    Ok(doc)
}

pub fn atomic_write(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or("missing file name")?
        .to_string_lossy();
    let temp = parent.join(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        SAVE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        write(&mut file)?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    root.map(|p| p.join("openpaint/settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_path(extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openpaint-{}-{}.{}",
            std::process::id(),
            SAVE_ID.fetch_add(1, Ordering::Relaxed),
            extension
        ))
    }
    #[test]
    fn layered_round_trip() {
        let path = temp_path("opaint");
        let mut doc = Document::new(3, 2).unwrap();
        doc.add_layer().unwrap();
        doc.layers[1].name = "ink".into();
        doc.layers[1].opacity = 172;
        doc.layers[1].blend_mode = BlendMode::Multiply;
        doc.layers[1].pixels[0..4].copy_from_slice(&[20, 80, 120, 200]);
        save(&mut doc, &path).unwrap();
        let loaded = open(&path).unwrap();
        assert_eq!(loaded.layers.len(), 2);
        assert_eq!(loaded.layers[1].pixels, doc.layers[1].pixels);
        assert_eq!(loaded.layers[1].name, "ink");
        assert_eq!(loaded.composite(), doc.composite());
        assert!(!loaded.is_dirty());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_save_preserves_original() {
        let path = temp_path("png");
        fs::write(&path, b"original").unwrap();
        let result = atomic_write(&path, |file| {
            file.write_all(b"partial").unwrap();
            Err("disk error".into())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn png_round_trip() {
        let path = temp_path("png");
        let mut doc = Document::from_rgba(1, 1, vec![3, 6, 9, 90], "sample".into()).unwrap();
        save(&mut doc, &path).unwrap();
        assert_eq!(open(&path).unwrap().layers[0].pixels, doc.layers[0].pixels);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn rejects_pdn_without_overwriting() {
        let path = temp_path("pdn");
        let mut doc = Document::new(1, 1).unwrap();
        assert!(save(&mut doc, &path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn lossless_exports_preserve_alpha_and_hidden_rgb() {
        let pixels = vec![
            255, 7, 30, 0, 15, 30, 200, 1, 10, 20, 30, 128, 40, 50, 60, 255,
        ];
        for extension in ["png", "bmp", "tga", "ico"] {
            let path = temp_path(extension);
            let mut doc = Document::from_rgba(2, 2, pixels.clone(), "sample".into()).unwrap();
            doc.mark_unsaved();
            assert!(save(&mut doc, &path).unwrap(), "{extension}");
            assert_eq!(open(&path).unwrap().layers[0].pixels, pixels, "{extension}");
            assert!(!doc.is_dirty());
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn exporting_without_alpha_does_not_mark_project_saved() {
        let path = temp_path("pnm");
        let mut doc = Document::from_rgba(1, 1, vec![30, 60, 90, 128], "sample".into()).unwrap();
        doc.mark_unsaved();
        assert!(!save(&mut doc, &path).unwrap());
        assert!(doc.is_dirty());
        assert!(doc.path.is_none());
        assert_eq!(open(&path).unwrap().layers[0].pixels, [30, 60, 90, 255]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn first_undo_after_loading_restores_loaded_layers() {
        let path = temp_path("opaint");
        let mut doc = Document::from_rgba(1, 1, vec![40, 70, 90, 120], "sample".into()).unwrap();
        doc.add_layer().unwrap();
        doc.active_pixels_mut().copy_from_slice(&[170, 10, 80, 190]);
        save(&mut doc, &path).unwrap();
        let mut loaded = open(&path).unwrap();
        assert!(!loaded.can_undo());
        loaded.begin_edit("paint");
        loaded.active_pixels_mut()[0] = 0;
        loaded.finish_edit();
        assert!(loaded.undo());
        assert_eq!(loaded.layers, doc.layers);
        assert!(!loaded.is_dirty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_documents_and_oversized_metadata_preserve_existing_files() {
        let path = temp_path("opaint");
        fs::write(&path, b"original").unwrap();
        let mut doc = Document::new(1, 1).unwrap();
        doc.layers[0].pixels.pop();
        assert!(save(&mut doc, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        doc.layers[0].pixels.push(255);
        doc.layers[0].name = "x".repeat(MAX_HEADER_BYTES);
        assert!(save(&mut doc, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        doc.layers[0].name = "layer".into();
        doc.layers = vec![doc.layers[0].clone(); MAX_LAYERS + 1];
        assert!(save(&mut doc, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_damaged_or_extended_project_payloads() {
        let path = temp_path("opaint");
        let mut doc = Document::new(2, 2).unwrap();
        save(&mut doc, &path).unwrap();
        let original = fs::read(&path).unwrap();
        let mut wrong_magic = original.clone();
        wrong_magic[0] = 0;
        let mut wrong_version = original.clone();
        wrong_version[4] = 2;
        let mut damaged_checksum = original.clone();
        let checksum_index = damaged_checksum.len() - 8;
        damaged_checksum[checksum_index] ^= 1;
        let truncated = original[..original.len() - 2].to_vec();
        let mut trailing_data = original.clone();
        trailing_data.extend_from_slice(b"unexpected");
        for bytes in [
            wrong_magic,
            wrong_version,
            damaged_checksum,
            truncated,
            trailing_data,
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(open(&path).is_err());
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_oversized_project_headers_before_allocating() {
        let path = temp_path("opaint");
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&(MAX_HEADER_BYTES as u32 + 1).to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(open(&path).unwrap_err().contains("header is too large"));
        fs::remove_file(path).unwrap();
    }
}
