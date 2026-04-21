use crate::models::*;
use image::ImageFormat;
use image::imageops::FilterType;
use md5;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Thumbnail<R>> {
    Ok(Thumbnail(app.clone()))
}

/// Access to the thumbnail APIs.
pub struct Thumbnail<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Thumbnail<R> {
    pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
        Ok(PingResponse {
            value: payload.value,
        })
    }

    pub fn get_thumbnail(
        &self,
        payload: GetThumbnailRequest,
    ) -> crate::Result<GetThumbnailResponse> {
        let path = Path::new(&payload.path);
        if !path.exists() {
            return Err(crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            )));
        }

        // 1. Try OS-native thumbnail (Linux Freedesktop spec)
        #[cfg(target_os = "linux")]
        {
            if let Ok(thumb_data) = self.get_linux_thumbnail(&payload.path) {
                let thumb_data = ensure_thumbnail_size_under_100kb(thumb_data)?;
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(), // Freedesktop thumbnails are always PNG
                });
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(thumb_data) = self.get_windows_thumbnail(&payload.path) {
                let thumb_data = ensure_thumbnail_size_under_100kb(thumb_data)?;
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(thumb_data) = self.get_macos_thumbnail(&payload.path) {
                let thumb_data = ensure_thumbnail_size_under_100kb(thumb_data)?;
                return Ok(GetThumbnailResponse {
                    thumbnail: thumb_data,
                    mime_type: "image/png".to_string(),
                });
            }
        }

        // 2. Fallback to manual generation
        let file_content = fs::read(path)?;
        let kind = infer::get(&file_content);
        let mime = kind
            .map(|k| k.mime_type())
            .unwrap_or("application/octet-stream");

        let (width, height) = if payload.width.is_some() && payload.height.is_some() {
            (payload.width.unwrap(), payload.height.unwrap())
        } else {
            (64, 64)
        };

        println!("Getting thumbnail from fallback");
        // Handle Images
        if mime.starts_with("image/") {
            let img = image::load_from_memory(&file_content)?;
            let thumb = img.thumbnail(width, height);

            let mut buffer = std::io::Cursor::new(Vec::new());
            thumb.write_to(&mut buffer, ImageFormat::Png)?;

            let thumb_data = ensure_thumbnail_size_under_100kb(buffer.into_inner())?;
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

                    let mut buffer = std::io::Cursor::new(Vec::new());
                    thumb.write_to(&mut buffer, ImageFormat::Png)?;

                    let thumb_data = ensure_thumbnail_size_under_100kb(buffer.into_inner())?;
                    return Ok(GetThumbnailResponse {
                        thumbnail: thumb_data,
                        mime_type: "image/png".to_string(),
                    });
                }
            }
        }

        // Handle Video (Best effort via system ffmpeg)
        if mime.starts_with("video/") {
            let temp_dir = std::env::temp_dir();
            let thumb_path = temp_dir.join(format!("thumb_{}.png", hash_path(path)));

            let output = std::process::Command::new("ffmpeg")
                .args([
                    "-i",
                    &payload.path,
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
                        let thumb_data = ensure_thumbnail_size_under_100kb(data)?;
                        return Ok(GetThumbnailResponse {
                            thumbnail: thumb_data,
                            mime_type: "image/png".to_string(),
                        });
                    }
                }
            }
        }

        // Handle PDF (Best effort via system pdftoppm)
        if mime == "application/pdf" {
            let temp_dir = std::env::temp_dir();
            let thumb_prefix = temp_dir.join(format!("thumb_pdf_{}", hash_path(path)));
            let thumb_path = temp_dir.join(format!("thumb_pdf_{}.png", hash_path(path)));

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
                    &payload.path,
                    thumb_prefix.to_str().unwrap(),
                ])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    if let Ok(data) = fs::read(&thumb_path) {
                        let _ = fs::remove_file(thumb_path);
                        let thumb_data = ensure_thumbnail_size_under_100kb(data)?;
                        return Ok(GetThumbnailResponse {
                            thumbnail: thumb_data,
                            mime_type: "image/png".to_string(),
                        });
                    }
                }
            }
        }

        Err(crate::Error::NotFound)
    }

    #[cfg(target_os = "linux")]
    fn get_linux_thumbnail(&self, file_path: &str) -> crate::Result<Vec<u8>> {
        println!("get_linux_thumbnail: {}", file_path);
        let abs_path = fs::canonicalize(file_path)?;
        let uri = Url::from_file_path(&abs_path).map_err(|_| crate::Error::NotFound)?;
        let hash = format!("{:x}", md5::compute(uri.as_str()));

        let cache_dir = std::env::var("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| home_dir().map(|h| h.join(".cache")).unwrap_or_default());

        println!("Cache dir: {:?}", cache_dir);
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
                println!("Thumbnail found at {:?}", thumb_path);
                return Ok(fs::read(thumb_path)?);
            }
        }

        Err(crate::Error::NotFound)
    }

    #[cfg(target_os = "windows")]
    fn get_windows_thumbnail(&self, file_path: &str) -> crate::Result<Vec<u8>> {
        use windows::Win32::UI::Shell::SHCreateItemFromParsingName;

        // Create IFileExtractIcon
        unsafe {
            let item = SHCreateItemFromParsingName(
                file_path,
                None,
                &windows::Win32::UI::Shell::IFileExtractIcon::IID,
            )
            .map_err(|_| crate::Error::NotFound)?;
            let extract_icon: windows::Win32::UI::Shell::IFileExtractIcon =
                std::mem::transmute_copy(&item);

            // Create HICON (We need to implement the trait properly for this)
            // For now, let's return an error or use a fallback if we can't easily implement the trait here
        }

        Err(crate::Error::UnsupportedType(
            "Windows thumbnail extraction requires COM implementation".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    fn get_macos_thumbnail(&self, file_path: &str) -> crate::Result<Vec<u8>> {
        use std::{fs, path::Path, process::Command};

        let cache_file = format!(
            "/tmp/myapp_thumbs/{}.png",
            hash_path(&std::path::Path::new(file_path))
        );

        // ✅ 1. Check YOUR cache
        if let Ok(data) = fs::read(&cache_file) {
            return Ok(data);
        }

        // Ensure cache dir exists
        let _ = fs::create_dir_all("/tmp/myapp_thumbs");

        // ✅ 2. Try Quick Look (better than sips)
        let ql_output = Command::new("qlmanage")
            .args([
                "-t", // thumbnail mode
                "-s", "128", // size
                "-o", "/tmp", // output dir
                file_path,
            ])
            .output();

        if let Ok(out) = ql_output {
            if out.status.success() {
                let generated = format!(
                    "/tmp/{}.png",
                    Path::new(file_path).file_name().unwrap().to_string_lossy()
                );

                if let Ok(data) = fs::read(&generated) {
                    let _ = fs::rename(&generated, &cache_file);
                    return Ok(data);
                }
            }
        }

        // ✅ 3. Fallback to sips (images only)
        let tmp_file = format!("/tmp/thumb_{}.png", hash_path(Path::new(file_path)));

        let sips_output = Command::new("sips")
            .args([
                "-s", "format", "png", "-Z", "128", "-o", &tmp_file, file_path,
            ])
            .output();

        if let Ok(out) = sips_output {
            if out.status.success() {
                if let Ok(data) = fs::read(&tmp_file) {
                    let _ = fs::rename(&tmp_file, &cache_file);
                    return Ok(data);
                }
            }
        }

        Err(crate::Error::NotFound)
    }
}

fn ensure_thumbnail_size_under_100kb(mut data: Vec<u8>) -> crate::Result<Vec<u8>> {
    while data.len() > 100 * 1024 {
        let img = image::load_from_memory(&data)?;
        let width = img.width() / 2;
        let height = img.height() / 2;
        if width < 16 || height < 16 {
            break;
        }
        let resized = img.resize(width, height, image::imageops::FilterType::Lanczos3);
        let mut buffer = std::io::Cursor::new(Vec::new());
        resized.write_to(&mut buffer, ImageFormat::Png)?;
        data = buffer.into_inner();
    }
    Ok(data)
}

fn hash_path(path: &Path) -> String {
    format!("{:x}", md5::compute(path.to_string_lossy().as_bytes()))
}
