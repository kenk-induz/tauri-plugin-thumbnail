# Tauri Plugin Thumbnail

A powerful and cross-platform Tauri plugin for generating and retrieving file thumbnails. It prioritizes OS-native thumbnail extraction and falls back to manual generation for various file types including images, audio (cover art), videos, and PDFs.

## Features

- **OS-Native Support**: 
  - **Linux**: Supports the Freedesktop Thumbnail Specification (XDG).
  - **macOS**: Uses `qlmanage` (Quick Look) and `sips`.
  - **Windows**: (Planned/Fallback) Supports manual generation.
- **Rich Fallbacks**:
  - **Images**: Generates thumbnails for common image formats (PNG, JPEG, WebP).
  - **Audio**: Extracts album art from ID3 tags.
  - **Video**: Generates thumbnails using `ffmpeg`.
  - **PDF**: Generates thumbnails using `pdftoppm`.
- **Smart Sizing**: Automatically resizes and optimizes thumbnails to stay under a reasonable size (200KB) for IPC performance.
- **Efficient IPC**: Uses binary data transfers for high performance.

## Installation

### 1. Add Rust Dependency

Add the plugin to your `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-thumbnail = { git = "https://github.com/kenk-induz/tauri-plugin-thumbnail" }
```

### 2. Register Plugin

Initialize the plugin in your `src-tauri/src/lib.rs`:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_thumbnail::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. Add JavaScript Guest

Install the guest-js package:

```bash
npm install https://github.com/kenk-induz/tauri-plugin-thumbnail
# or
yarn add https://github.com/kenk-induz/tauri-plugin-thumbnail
```

## Usage

### JavaScript / TypeScript

```typescript
import { getThumbnail } from 'tauri-plugin-thumbnail';

async function displayThumbnail(filePath: string) {
  try {
    const response = await getThumbnail(filePath, 256, 256);
    
    // The thumbnail is returned as a byte array
    const blob = new Blob([new Uint8Array(response.thumbnail)], { type: response.mimeType });
    const url = URL.createObjectURL(blob);
    
    const img = document.getElementById('my-image') as HTMLImageElement;
    img.src = url;
  } catch (error) {
    console.error('Failed to get thumbnail:', error);
  }
}
```

## Requirements

For advanced file type support (Video/PDF), ensure the following tools are installed on the system:

- **Video**: `ffmpeg`
- **PDF**: `pdftoppm` (usually part of `poppler-utils`)

## Permissions

By default, the plugin requires the `allow-get-thumbnail` permission. Add it to your `capabilities` in `src-tauri/capabilities/default.json`:

```json
{
  "permissions": [
    "thumbnail:default"
  ]
}
```

## License

MIT © [kenk-induz](https://github.com/kenk-induz)
