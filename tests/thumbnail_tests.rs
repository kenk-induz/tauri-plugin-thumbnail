use image::{ImageBuffer, Rgb};
use std::path::PathBuf;
use tauri_plugin_thumbnail::desktop_test_helpers::*;
use tauri_plugin_thumbnail::*;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[allow(dead_code)]
fn sample_mp3_path() -> PathBuf {
    fixtures_dir().join("sample.mp3")
}

#[allow(dead_code)]
fn sample_pdf_path() -> PathBuf {
    fixtures_dir().join("sample.pdf")
}

#[allow(dead_code)]
fn sample_video_path() -> PathBuf {
    fixtures_dir().join("sample.mp4")
}

// =========================================================
// Error handling tests
// =========================================================

mod error_cases {
    use super::*;

    #[test]
    fn test_missing_file_returns_not_found() {
        let req = GetThumbnailRequest {
            path: "/nonexistent/path/does_not_exist_thumbnail_test.png".to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req);
        assert!(result.is_err(), "Missing file should return error");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("not found")
                || err_str.contains("No such file")
                || err_str.contains("NotFound"),
            "Error should mention missing file, got: {}",
            err_str
        );
    }

    #[test]
    fn test_empty_path_returns_error() {
        let req = GetThumbnailRequest {
            path: "".to_string(),
            width: None,
            height: None,
        };
        let result = get_thumbnail_core(req);
        assert!(result.is_err(), "Empty path should return error");
    }

    #[test]
    fn test_unsupported_file_type_returns_error() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"\x00\x01\x02\x03unknown_format_header").unwrap();
        let req = GetThumbnailRequest {
            path: tmp.path().to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req);
        assert!(
            result.is_err(),
            "Unknown file type should return NotFound error"
        );
    }
}

// =========================================================
// Image thumbnail tests
// =========================================================

mod image_thumbnails {
    use super::*;

    fn create_test_png(path: &std::path::Path, width: u32, height: u32) {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, _| {
            Rgb([255u8, (x % 256) as u8, 0u8])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn test_png_thumbnail_generation() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("test.png");
        create_test_png(&png_path, 400, 300);

        let req = GetThumbnailRequest {
            path: png_path.to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req).expect("PNG thumbnail should succeed");
        assert!(
            !result.thumbnail.is_empty(),
            "Thumbnail data should not be empty"
        );
        assert_eq!(result.mime_type, "image/png");

        let decoded = image::load_from_memory(&result.thumbnail).expect("Should be valid PNG");
        assert!(decoded.width() <= 128, "Width should be <= requested size");
        assert!(decoded.height() <= 128, "Height should be <= requested size");
    }

    #[test]
    fn test_jpeg_thumbnail_generation() {
        let dir = tempfile::tempdir().unwrap();
        let jpg_path = dir.path().join("test.jpg");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(300, 300, |_, y| {
            Rgb([0u8, 0u8, (y % 256) as u8])
        });
        img.save(&jpg_path).unwrap();

        let req = GetThumbnailRequest {
            path: jpg_path.to_string_lossy().to_string(),
            width: Some(64),
            height: Some(64),
        };
        let result = get_thumbnail_core(req).expect("JPEG thumbnail should succeed");
        assert!(!result.thumbnail.is_empty());
        assert_eq!(result.mime_type, "image/png");
    }

    #[test]
    fn test_default_size_applied_when_not_specified() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("test.png");
        create_test_png(&png_path, 400, 400);

        let req = GetThumbnailRequest {
            path: png_path.to_string_lossy().to_string(),
            width: None,
            height: None,
        };
        let result = get_thumbnail_core(req).expect("Should use default size");
        let decoded = image::load_from_memory(&result.thumbnail).unwrap();
        assert!(decoded.width() <= 128);
        assert!(decoded.height() <= 128);
    }

    #[test]
    fn test_thumbnail_respects_aspect_ratio() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("wide.png");
        create_test_png(&png_path, 800, 400);

        let req = GetThumbnailRequest {
            path: png_path.to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req).expect("Wide image thumbnail should succeed");
        let decoded = image::load_from_memory(&result.thumbnail).unwrap();

        assert!(decoded.width() <= 128);
        assert!(decoded.height() <= 128);
        let ratio = decoded.width() as f32 / decoded.height() as f32;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "Aspect ratio should be ~2:1, got {:.2}",
            ratio
        );
    }

    #[test]
    fn test_large_image_optimized_under_200kb() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("large.png");
        let mut img = image::RgbaImage::new(1200, 1200);
        for (i, pixel) in img.pixels_mut().enumerate() {
            *pixel = image::Rgba([
                (i * 37 % 256) as u8,
                (i * 73 % 256) as u8,
                (i * 109 % 256) as u8,
                255,
            ]);
        }
        img.save(&png_path).unwrap();

        let req = GetThumbnailRequest {
            path: png_path.to_string_lossy().to_string(),
            width: Some(800),
            height: Some(800),
        };
        let result = get_thumbnail_core(req).expect("Large image thumbnail should succeed");
        assert!(
            result.thumbnail.len() <= 200 * 1024,
            "Thumbnail must be <= 200KB, got {} bytes",
            result.thumbnail.len()
        );
    }
}

// =========================================================
// Audio thumbnail tests (cover art via ID3)
// =========================================================

mod audio_thumbnails {
    use super::*;

    #[test]
    #[ignore = "Requires tests/fixtures/sample.mp3 with embedded cover art"]
    fn test_mp3_cover_art_extraction() {
        let mp3_path = super::sample_mp3_path();
        assert!(mp3_path.exists(), "sample.mp3 fixture must exist");

        let req = GetThumbnailRequest {
            path: mp3_path.to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req).expect("MP3 cover art extraction should succeed");
        assert!(!result.thumbnail.is_empty());
        assert_eq!(result.mime_type, "image/png");
        image::load_from_memory(&result.thumbnail).expect("Should decode to a valid image");
    }

    #[test]
    fn test_mp3_without_cover_art_returns_error() {
        let tmp = tempfile::Builder::new().suffix(".mp3").tempfile().unwrap();
        std::fs::write(tmp.path(), b"ID3\x03\x00\x00\x00\x00\x00\x00").unwrap();

        let req = GetThumbnailRequest {
            path: tmp.path().to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req);
        assert!(result.is_err(), "MP3 without cover art should return error");
    }
}

// =========================================================
// Video thumbnail tests (requires ffmpeg)
// =========================================================

mod video_thumbnails {
    use super::*;

    fn ffmpeg_available() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_video_thumbnail_skipped_if_no_ffmpeg() {
        if ffmpeg_available() {
            println!("ffmpeg is available — skipping 'no ffmpeg' test");
            return;
        }
        let tmp = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
        std::fs::write(tmp.path(), b"\x00\x00\x00\x18ftypmp42").unwrap();
        let req = GetThumbnailRequest {
            path: tmp.path().to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req);
        assert!(result.is_err(), "Without ffmpeg, video should fail gracefully");
    }

    #[test]
    #[ignore = "Requires ffmpeg and tests/fixtures/sample.mp4"]
    fn test_video_thumbnail_with_ffmpeg() {
        if !ffmpeg_available() {
            println!("Skipping: ffmpeg not found");
            return;
        }
        let video_path = super::sample_video_path();
        assert!(video_path.exists(), "sample.mp4 fixture must exist");

        let req = GetThumbnailRequest {
            path: video_path.to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req).expect("Video thumbnail should succeed with ffmpeg");
        assert!(!result.thumbnail.is_empty());
        assert_eq!(result.mime_type, "image/png");
        image::load_from_memory(&result.thumbnail).expect("Should be valid PNG");
    }
}

// =========================================================
// PDF thumbnail tests (requires pdftoppm)
// =========================================================

mod pdf_thumbnails {
    use super::*;

    fn pdftoppm_available() -> bool {
        std::process::Command::new("pdftoppm")
            .arg("-v")
            .output()
            .map(|o| o.status.success() || !o.stderr.is_empty())
            .unwrap_or(false)
    }

    #[test]
    #[ignore = "Requires pdftoppm and tests/fixtures/sample.pdf"]
    fn test_pdf_thumbnail_generation() {
        if !pdftoppm_available() {
            println!("Skipping: pdftoppm not found");
            return;
        }
        let pdf_path = super::sample_pdf_path();
        assert!(pdf_path.exists(), "sample.pdf fixture must exist");

        let req = GetThumbnailRequest {
            path: pdf_path.to_string_lossy().to_string(),
            width: Some(128),
            height: Some(128),
        };
        let result = get_thumbnail_core(req).expect("PDF thumbnail should succeed with pdftoppm");
        assert!(!result.thumbnail.is_empty());
        assert_eq!(result.mime_type, "image/png");
        image::load_from_memory(&result.thumbnail).expect("Should be valid PNG");
    }
}

// =========================================================
// Linux XDG native thumbnail tests
// =========================================================

#[cfg(target_os = "linux")]
mod linux_xdg_thumbnails {
    use super::*;

    #[test]
    #[ignore = "Requires a pre-existing XDG thumbnail in ~/.cache/thumbnails/"]
    fn test_xdg_thumbnail_hit() {
        let file_path = std::env::var("TEST_XDG_FILE")
            .expect("Set TEST_XDG_FILE to a file with a cached XDG thumbnail");

        let result = get_linux_thumbnail_core(&file_path)
            .expect("XDG thumbnail should be found in cache");
        assert!(!result.is_empty(), "XDG thumbnail bytes should not be empty");
        image::load_from_memory(&result).expect("XDG thumbnail should be valid PNG");
    }
}

// =========================================================
// Native size constraint tests
// =========================================================

mod native_size_constraint {
    use super::*;

    fn make_png(w: u32, h: u32) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(w, h, |x, _| Rgb([(x % 256) as u8, 100, 200]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn test_image_already_within_bounds_not_re_encoded() {
        let data = make_png(64, 64);
        let original_len = data.len();
        let result = constrain_to_size(data, 128, 128).unwrap();
        assert_eq!(
            result.len(),
            original_len,
            "Small image should pass through unchanged"
        );
    }

    #[test]
    fn test_oversized_image_is_constrained() {
        let data = make_png(512, 512);
        let result = constrain_to_size(data, 128, 128).unwrap();
        let img = image::load_from_memory(&result).unwrap();
        assert!(img.width() <= 128, "Width {} exceeds 128", img.width());
        assert!(img.height() <= 128, "Height {} exceeds 128", img.height());
    }

    #[test]
    fn test_aspect_ratio_preserved_on_constrain() {
        let data = make_png(800, 400);
        let result = constrain_to_size(data, 128, 128).unwrap();
        let img = image::load_from_memory(&result).unwrap();
        assert!(img.width() <= 128);
        assert!(img.height() <= 128);
        let ratio = img.width() as f32 / img.height() as f32;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "Aspect ratio should be ~2:1, got {:.2}",
            ratio
        );
    }

    #[test]
    fn test_constrain_respects_exact_boundary() {
        let data = make_png(128, 128);
        let original_len = data.len();
        let result = constrain_to_size(data, 128, 128).unwrap();
        assert_eq!(result.len(), original_len);
    }
}

