import { invoke } from '@tauri-apps/api/core'

export interface GetThumbnailRequest {
  path: string;
  width?: number;
  height?: number;
}

export interface GetThumbnailResponse {
  thumbnail: number[];
  mimeType: string;
}

/**
 * Generates or retrieves a thumbnail for the specified file path.
 * 
 * @param path The absolute path to the file.
 * @param width Optional width of the thumbnail (default 64).
 * @param height Optional height of the thumbnail (default 64).
 * @returns A promise that resolves to the thumbnail data and its MIME type.
 */
export async function getThumbnail(path: string, width?: number, height?: number): Promise<GetThumbnailResponse> {
  return await invoke<GetThumbnailResponse>('plugin:thumbnail|get_thumbnail', {
    payload: {
      path,
      width,
      height,
    },
  });
}
