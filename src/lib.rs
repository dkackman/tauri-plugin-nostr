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
use desktop::TauriPluginNostr;
#[cfg(mobile)]
use mobile::TauriPluginNostr;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the tauri-plugin-nostr APIs.
pub trait TauriPluginNostrExt<R: Runtime> {
    fn tauri_plugin_nostr(&self) -> &TauriPluginNostr<R>;
}

impl<R: Runtime, T: Manager<R>> crate::TauriPluginNostrExt<R> for T {
    fn tauri_plugin_nostr(&self) -> &TauriPluginNostr<R> {
        self.state::<TauriPluginNostr<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tauri-plugin-nostr")
        .invoke_handler(tauri::generate_handler![commands::ping])
        .setup(|app, api| {
            #[cfg(mobile)]
            let tauri_plugin_nostr = mobile::init(app, api)?;
            #[cfg(desktop)]
            let tauri_plugin_nostr = desktop::init(app, api)?;
            app.manage(tauri_plugin_nostr);
            Ok(())
        })
        .build()
}
