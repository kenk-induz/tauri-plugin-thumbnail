import UIKit
import WebKit
import QuickLookThumbnailing
import Tauri

class ThumbnailPlugin: Plugin {
    @objc public func getThumbnail(_ invoke: Invoke) {
        let path = invoke.getString("path") ?? ""
        let width = invoke.getInt("width") ?? 200
        let height = invoke.getInt("height") ?? 200
        
        if path.isEmpty {
            invoke.reject("Path is required")
            return
        }
        
        let url = URL(fileURLWithPath: path)
        let size = CGSize(width: CGFloat(width), height: CGFloat(height))
        let scale = UIScreen.main.scale
        
        let request = QLThumbnailGenerator.Request(
            fileAt: url,
            size: size,
            scale: scale,
            representationTypes: .thumbnail
        )
        
        QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { thumbnail, error in
            if let error = error {
                invoke.reject("Error generating thumbnail: \(error.localizedDescription)")
                return
            }
            
            guard let thumbnail = thumbnail else {
                invoke.reject("Failed to generate thumbnail")
                return
            }
            
            let image = thumbnail.uiImage
            if let data = image.pngData() {
                invoke.resolve([
                    "thumbnail": data,
                    "mimeType": "image/png"
                ])
            } else {
                invoke.reject("Failed to encode thumbnail to PNG")
            }
        }
    }
}

@objc public init_plugin_thumbnail() -> Plugin {
  return ThumbnailPlugin()
}
