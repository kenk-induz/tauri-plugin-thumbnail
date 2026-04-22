use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::Result;
use crate::ThumbnailExt;

/// Retrieves a thumbnail for the given request.
#[command]
pub async fn get_thumbnail<R: Runtime>(
  app: AppHandle<R>,
  payload: GetThumbnailRequest,
) -> Result<GetThumbnailResponse> {
  app.thumbnail().get_thumbnail(payload)
}
