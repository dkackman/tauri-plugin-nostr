use std::sync::Arc;
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
mod state;
pub(crate) mod outbox;

pub use error::{Error, Result};
pub use state::NostrSyncState;

#[cfg(desktop)]
use desktop::TauriPluginNostr as TauriPluginNostrSync;
#[cfg(mobile)]
use mobile::TauriPluginNostr as TauriPluginNostrSync;

pub trait TauriPluginNostrSyncExt<R: Runtime> {
    fn nostr_sync(&self) -> &TauriPluginNostrSync<R>;
}

impl<R: Runtime, T: Manager<R>> TauriPluginNostrSyncExt<R> for T {
    fn nostr_sync(&self) -> &TauriPluginNostrSync<R> {
        self.state::<TauriPluginNostrSync<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tauri-plugin-nostr-sync")
        .invoke_handler(tauri::generate_handler![])
        .setup(|app, api| {
            #[cfg(mobile)]
            {
                let plugin = mobile::init(app, api)?;
                app.manage(plugin);
            }
            #[cfg(desktop)]
            {
                let plugin = desktop::init(app, api)?;
                app.manage(plugin);
            }
            Ok(())
        })
        .build()
}
