use image::{DynamicImage, ImageFormat, RgbaImage};
use std::io::{Cursor, Write};
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::Stdio;

#[derive(Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub fn write(data: &ImageData) -> Result<(), String> {
    let image = RgbaImage::from_raw(data.width, data.height, data.pixels.clone())
        .ok_or("invalid clipboard image")?;
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    write_png(&encoded.into_inner())
}

pub fn read() -> Result<ImageData, String> {
    let png = read_png()?;
    let mut reader = image::io::Reader::with_format(Cursor::new(png), ImageFormat::Png);
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
    Ok(ImageData {
        width: image.width(),
        height: image.height(),
        pixels: image.into_raw(),
    })
}

#[cfg(target_os = "linux")]
fn write_png(png: &[u8]) -> Result<(), String> {
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png", "-i"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("cannot write clipboard")?
        .write_all(png)
        .map_err(|e| e.to_string())?;
    if child.wait().map_err(|e| e.to_string())?.success() {
        Ok(())
    } else {
        Err("system clipboard unavailable".into())
    }
}

#[cfg(target_os = "linux")]
fn read_png() -> Result<Vec<u8>, String> {
    let output = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png", "-o"])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err("no PNG image on system clipboard".into())
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct TempImage(std::path::PathBuf);

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl TempImage {
    fn new(png: &[u8]) -> Result<Self, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "openpaint-clipboard-{}-{}.png",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(png).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Drop for TempImage {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn write_png(png: &[u8]) -> Result<(), String> {
    let file = TempImage::new(png)?;
    let output=Command::new("osascript").args(["-e","on run argv\nset the clipboard to (read (POSIX file (item 1 of argv)) as «class PNGf»)\nend run","--"]).arg(&file.0).output().map_err(|e|e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err("system clipboard unavailable".into())
    }
}
#[cfg(target_os = "macos")]
fn read_png() -> Result<Vec<u8>, String> {
    let file = TempImage::new(&[])?;
    let script="on run argv\nset f to open for access (POSIX file (item 1 of argv)) with write permission\ntry\nset eof f to 0\nwrite (the clipboard as «class PNGf») to f\nclose access f\non error messageText\nclose access f\nerror messageText\nend try\nend run";
    let output = Command::new("osascript")
        .args(["-e", script, "--"])
        .arg(&file.0)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        std::fs::read(&file.0).map_err(|e| e.to_string())
    } else {
        Err("no PNG image on system clipboard".into())
    }
}
#[cfg(target_os = "windows")]
fn write_png(png: &[u8]) -> Result<(), String> {
    let file = TempImage::new(png)?;
    let script="Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $i = [Drawing.Image]::FromFile($env:OPENPAINT_CLIPBOARD); try { [Windows.Forms.Clipboard]::SetImage($i) } finally { $i.Dispose() }";
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-Command", script])
        .env("OPENPAINT_CLIPBOARD", &file.0)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err("system clipboard unavailable".into())
    }
}
#[cfg(target_os = "windows")]
fn read_png() -> Result<Vec<u8>, String> {
    let file = TempImage::new(&[])?;
    let script="Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $i = [Windows.Forms.Clipboard]::GetImage(); if ($null -eq $i) { exit 1 }; try { $i.Save($env:OPENPAINT_CLIPBOARD, [Drawing.Imaging.ImageFormat]::Png) } finally { $i.Dispose() }";
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-Command", script])
        .env("OPENPAINT_CLIPBOARD", &file.0)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        std::fs::read(&file.0).map_err(|e| e.to_string())
    } else {
        Err("no image on system clipboard".into())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn write_png(_png: &[u8]) -> Result<(), String> {
    Err("system clipboard unavailable".into())
}
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn read_png() -> Result<Vec<u8>, String> {
    Err("system clipboard unavailable".into())
}
