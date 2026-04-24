package com.tauri.thumbnail

import android.app.Activity
import android.graphics.Bitmap
import android.media.ThumbnailUtils
import android.util.Size
import android.webkit.MimeTypeMap
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke
import java.io.ByteArrayOutputStream
import java.io.File
import android.util.Base64

@TauriPlugin
class ThumbnailPlugin(private val activity: Activity) : app.tauri.plugin.Plugin(activity) {

    @Command
    fun getThumbnail(invoke: Invoke) {
        val args = invoke.getArgs()
        val path = args.getString("path", null)
        val width = args.getInteger("width", 200)
        val height = args.getInteger("height", 200)

        if (path == null) {
            invoke.reject("Path is required")
            return
        }

        val file = File(path)
        if (!file.exists()) {
            invoke.reject("File does not exist: $path")
            return
        }

        try {
            val size = Size(width, height)
            val bitmap = if (isVideo(path)) {
                ThumbnailUtils.createVideoThumbnail(file, size, null)
            } else {
                ThumbnailUtils.createImageThumbnail(file, size, null)
            }

            if (bitmap == null) {
                invoke.reject("Failed to generate thumbnail for $path")
                return
            }

            val outputStream = ByteArrayOutputStream()
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, outputStream)
            val byteArray = outputStream.toByteArray()
            val base64 = Base64.encodeToString(byteArray, Base64.NO_WRAP)

            val response = JSObject()
            response.put("thumbnail", "data:image/png;base64,$base64")
            invoke.resolve(response)
        } catch (e: Exception) {
            invoke.reject("Error generating thumbnail: ${e.message}")
        }
    }

    private fun isVideo(path: String): Boolean {
        val extension = MimeTypeMap.getFileExtensionFromUrl(path)
        val mimeType = MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension.lowercase())
        return mimeType?.startsWith("video/") == true
    }
}
