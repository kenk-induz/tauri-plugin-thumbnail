use crate::models::*;
use home::home_dir;
use image::ImageFormat;
use log::{debug, warn};
use md5;
use serde::de::DeserializeOwned;
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use url::Url;

/// Maximum size for a thumbnail to be considered "ready" without further compression.
const MAX_THUMBNAIL_SIZE: usize = 200 * 1024; // 200KB

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Thumbnail<R>> {
    Ok(Thumbnail(app.clone()))
}

/// Access to the thumbnail APIs.
pub struct Thumbnail<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Thumbnail<R> {
    /// Generates or retrieves a thumbnail for the specified file path.
    pub fn get_thumbnail(
        &self,
        payload: GetThumbnailRequest,
    ) -> crate::Result<GetThumbnailResponse> {
        let path = Path::new(&payload.path);
        if !path.exists() {
            return Err(crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", payload.path),
            )));
        }

        let width = payload.width.unwrap_or(128);
        let height = payload.height.unwrap_or(128);

        // 1. Try OS-native thumbnail
        #[cfg(target_os = "linux")]
        {
            if let Ok(thumb_data) = self.get_linux_thumbnail(&payload.path) {
                debug!("Found native Linux thumbnail for {}", payload.path);
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(thumb_data) = self.get_macos_thumbnail(&payload.path, width, height) {
                debug!("Found native macOS thumbnail for {}", payload.path);
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(thumb_data) = self.get_windows_thumbnail(&payload.path, width, height) {
                debug!("Found native Windows thumbnail for {}", payload.path);
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        // 2. Fallback to manual generation
        debug!(
            "Falling back to manual thumbnail generation for {}",
            payload.path
        );

        // Only read enough to infer type
        let mut file = File::open(path)?;
        let mut header = [0u8; 8192];
        let read_count = file.read(&mut header)?;
        let kind = infer::get(&header[..read_count]);
        let mime = kind
            .map(|k| k.mime_type())
            .unwrap_or("application/octet-stream");

        // Handle Images
        if mime.starts_with("image/") {
            // Re-read for image crate (full load is unfortunately often required for images)
            let file_content = fs::read(path)?;
            let img = image::load_from_memory(&file_content)?;
            let thumb = img.thumbnail(width, height);

            let mut buffer = Cursor::new(Vec::new());
            thumb.write_to(&mut buffer, ImageFormat::Png)?;

            let thumb_data = optimize_thumbnail_size(buffer.into_inner())?;
            return Ok(GetThumbnailResponse {
                thumbnail: thumb_data,
                mime_type: "image/png".to_string(),
            });
        }

        // Handle Audio (Cover Art)
        if mime.starts_with("audio/") {
            if let Ok(tag) = id3::Tag::read_from_path(path) {
                if let Some(picture) = tag.pictures().next() {
                    let img = image::load_from_memory(&picture.data)?;
                    let thumb = img.thumbnail(width, height);

                    let mut buffer = Cursor::new(Vec::new());
                    thumb.write_to(&mut buffer, ImageFormat::Png)?;

                    let thumb_data = optimize_thumbnail_size(buffer.into_inner())?;
                    return Ok(GetThumbnailResponse {
                        thumbnail: thumb_data,
                        mime_type: "image/png".to_string(),
                    });
                }
            }
        }

        // Handle Video (via system ffmpeg)
        if mime.starts_with("video/") {
            if let Ok(thumb_data) = self.generate_video_thumbnail(path, width, height) {
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        // Handle PDF (via system pdftoppm)
        if mime == "application/pdf" {
            if let Ok(thumb_data) = self.generate_pdf_thumbnail(path, width, height) {
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        warn!("Could not generate thumbnail for {}", payload.path);
        Err(crate::Error::NotFound)
    }

    #[cfg(target_os = "linux")]
    fn get_linux_thumbnail(&self, file_path: &str) -> crate::Result<Vec<u8>> {
        let abs_path = fs::canonicalize(file_path)?;
        let uri = Url::from_file_path(&abs_path).map_err(|_| crate::Error::NotFound)?;
        let hash = format!("{:x}", md5::compute(uri.as_str()));

        let cache_dir = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().map(|h| h.join(".cache")).unwrap_or_default());

        if cache_dir.as_os_str().is_empty() {
            return Err(crate::Error::NotFound);
        }

        let thumb_dirs = [
            cache_dir.join("thumbnails/large"),
            cache_dir.join("thumbnails/normal"),
            cache_dir.join("thumbnails/x-large"),
            cache_dir.join("thumbnails/xx-large"),
        ];

        for dir in thumb_dirs {
            let thumb_path = dir.join(format!("{}.png", hash));
            if thumb_path.exists() {
                return Ok(fs::read(thumb_path)?);
            }
        }

        Err(crate::Error::NotFound)
    }

    #[cfg(target_os = "windows")]
    fn get_windows_thumbnail(
        &self,
        file_path: &str,
        width: u32,
        height: u32,
    ) -> crate::Result<Vec<u8>> {
        use windows::{
            core::*,
            Win32::{Foundation::*, Graphics::Gdi::*, System::Com::*, UI::Shell::*},
        };

        let height = height.max(64);
        let width = width.max(64);
        unsafe {
            // Initialize COM
            if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
                return Err(crate::Error::NotFound);
            }

            // Convert path
            let wide: Vec<u16> = file_path.encode_utf16().chain(Some(0)).collect();

            // Create IShellItem
            let shell_item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
                .map_err(|_| crate::Error::NotFound)?;

            // Get IShellItemImageFactory
            let image_factory: IShellItemImageFactory =
                shell_item.cast().map_err(|_| crate::Error::NotFound)?;

            // Request thumbnail
            let size = SIZE {
                cx: width as i32,
                cy: height as i32,
            };

            let hbitmap: HBITMAP = image_factory
                .GetImage(size, SIIGBF_BIGGERSIZEOK)
                .map_err(|_| crate::Error::NotFound)?;

            // Convert HBITMAP → PNG bytes
            let mut bmp = BITMAP::default();
            GetObjectW(
                HGDIOBJ(hbitmap.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as *mut core::ffi::c_void),
            );

            let width = bmp.bmWidth as u32;
            let height = bmp.bmHeight as u32;

            let mut buffer = vec![0u8; (width * height * 4) as usize];

            let hdc = GetDC(HWND(0));

            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width as i32;
            bmi.bmiHeader.biHeight = -(height as i32); // top-down
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;

            GetDIBits(
                hdc,
                hbitmap,
                0,
                height as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            ReleaseDC(HWND(0), hdc);
            DeleteObject(HGDIOBJ(hbitmap.0));

            // Encode to PNG
            let img =
                image::RgbaImage::from_raw(width, height, buffer).ok_or(crate::Error::NotFound)?;

            let mut png_bytes = Vec::new();
            img.write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .map_err(|_| crate::Error::NotFound)?;

            Ok(png_bytes)
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_thumbnail(
        &self,
        file_path: &str,
        width: u32,
        height: u32,
    ) -> crate::Result<Vec<u8>> {
        use std::process::Command;

        let app_cache = self
            .0
            .path()
            .app_cache_dir()
            .unwrap_or_else(|_| std::env::temp_dir());
        let plugin_cache = app_cache.join("tauri-plugin-thumbnail");
        let _ = fs::create_dir_all(&plugin_cache);

        let hash = hash_path(Path::new(file_path));
        let cache_file = plugin_cache.join(format!("{}.png", hash));

        if cache_file.exists() {
            return Ok(fs::read(cache_file)?);
        }

        // Try Quick Look (qlmanage)
        let ql_output = Command::new("qlmanage")
            .args([
                "-t",
                "-s",
                &width.to_string(),
                "-o",
                plugin_cache.to_str().unwrap(),
                file_path,
            ])
            .output();

        if let Ok(out) = ql_output {
            if out.status.success() {
                let generated_name = format!(
                    "{}.png",
                    Path::new(file_path).file_name().unwrap().to_string_lossy()
                );
                let generated_path = plugin_cache.join(generated_name);

                if generated_path.exists() {
                    let data = fs::read(&generated_path)?;
                    let _ = fs::rename(&generated_path, &cache_file);
                    return Ok(data);
                }
            }
        }

        // Fallback to sips
        let tmp_file = plugin_cache.join(format!("tmp_{}.png", hash));
        let sips_output = Command::new("sips")
            .args([
                "-s",
                "format",
                "png",
                "-Z",
                &width.max(height).to_string(),
                "-o",
                tmp_file.to_str().unwrap(),
                file_path,
            ])
            .output();

        if let Ok(out) = sips_output {
            if out.status.success() {
                if tmp_file.exists() {
                    let data = fs::read(&tmp_file)?;
                    let _ = fs::rename(&tmp_file, &cache_file);
                    return Ok(data);
                }
            }
        }

        Err(crate::Error::NotFound)
    }

    fn generate_video_thumbnail(
        &self,
        path: &Path,
        width: u32,
        height: u32,
    ) -> crate::Result<Vec<u8>> {
        let temp_dir = std::env::temp_dir();
        let thumb_path = temp_dir.join(format!("v_thumb_{}.png", hash_path(path)));

        let output = std::process::Command::new("ffmpeg")
            .args([
                "-i",
                &path.to_string_lossy(),
                "-ss",
                "00:00:01",
                "-vframes",
                "1",
                "-s",
                &format!("{}x{}", width, height),
                "-f",
                "image2",
                "-y",
                thumb_path.to_str().unwrap(),
            ])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(data) = fs::read(&thumb_path) {
                    let _ = fs::remove_file(thumb_path);
                    return optimize_thumbnail_size(data);
                }
            }
        }
        Err(crate::Error::NotFound)
    }

    fn generate_pdf_thumbnail(
        &self,
        path: &Path,
        width: u32,
        _height: u32,
    ) -> crate::Result<Vec<u8>> {
        let temp_dir = std::env::temp_dir();
        let thumb_prefix = temp_dir.join(format!("p_thumb_{}", hash_path(path)));
        let thumb_path = temp_dir.join(format!("p_thumb_{}.png", hash_path(path)));

        let output = std::process::Command::new("pdftoppm")
            .args([
                "-png",
                "-f",
                "1",
                "-l",
                "1",
                "-scale-to",
                &width.to_string(),
                "-singlefile",
                &path.to_string_lossy(),
                thumb_prefix.to_str().unwrap(),
            ])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(data) = fs::read(&thumb_path) {
                    let _ = fs::remove_file(thumb_path);
                    return optimize_thumbnail_size(data);
                }
            }
        }
        Err(crate::Error::NotFound)
    }
}

fn optimize_thumbnail_size(mut data: Vec<u8>) -> crate::Result<Vec<u8>> {
    if data.len() <= MAX_THUMBNAIL_SIZE {
        return Ok(data);
    }

    let img = image::load_from_memory(&data)?;
    let mut width = img.width();
    let mut height = img.height();

    // Resize by half until it fits or becomes too small
    while data.len() > MAX_THUMBNAIL_SIZE && width > 32 && height > 32 {
        width /= 2;
        height /= 2;
        let resized = img.resize(width, height, image::imageops::FilterType::Lanczos3);
        let mut buffer = Cursor::new(Vec::new());
        resized.write_to(&mut buffer, ImageFormat::Png)?;
        data = buffer.into_inner();
    }

    Ok(data)
}

fn hash_path(path: &Path) -> String {
    format!("{:x}", md5::compute(path.to_string_lossy().as_bytes()))
}
