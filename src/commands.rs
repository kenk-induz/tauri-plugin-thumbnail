use tauri::{AppHandle, command, Runtime};

use crate::models::*;
use crate::Result;
use crate::ThumbnailExt;

#[command]
pub(crate) async fn ping<R: Runtime>(
    app: AppHandle<R>,
    payload: PingRequest,
) -> Result<PingResponse> {
    app.thumbnail().ping(payload)
}

#[command]
pub(crate) async fn get_thumbnail<R: Runtime>(
    app: AppHandle<R>,
    payload: GetThumbnailRequest,
) -> Result<GetThumbnailResponse> {
    app.thumbnail().get_thumbnail(payload)
}
