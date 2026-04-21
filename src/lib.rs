use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Thumbnail;
#[cfg(mobile)]
use mobile::Thumbnail;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the thumbnail APIs.
pub trait ThumbnailExt<R: Runtime> {
  fn thumbnail(&self) -> &Thumbnail<R>;
}

impl<R: Runtime, T: Manager<R>> crate::ThumbnailExt<R> for T {
  fn thumbnail(&self) -> &Thumbnail<R> {
    self.state::<Thumbnail<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("thumbnail")
    .invoke_handler(tauri::generate_handler![commands::ping, commands::get_thumbnail])
    .setup(|app, api| {
      #[cfg(mobile)]
      let thumbnail = mobile::init(app, api)?;
      #[cfg(desktop)]
      let thumbnail = desktop::init(app, api)?;
      app.manage(thumbnail);
      Ok(())
    })
    .build()
}
