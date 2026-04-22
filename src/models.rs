use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetThumbnailRequest {
  pub path: String,
  pub width: Option<u32>,
  pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetThumbnailResponse {
  pub thumbnail: Vec<u8>,
  pub mime_type: String,
}
