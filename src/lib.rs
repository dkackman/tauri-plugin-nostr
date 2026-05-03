use tauri::{plugin::TauriPlugin, Manager, Runtime};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod builder;
mod commands;
mod error;
mod models;
mod state;

pub use builder::PluginBuilder;
pub use error::{Error, Result};
pub use state::NostrSyncState;

#[cfg(desktop)]
use desktop::TauriPluginNostrSync;
#[cfg(mobile)]
use mobile::TauriPluginNostrSync;

pub trait TauriPluginNostrSyncExt<R: Runtime> {
    fn nostr_sync(&self) -> &TauriPluginNostrSync<R>;
}

impl<R: Runtime, T: Manager<R>> TauriPluginNostrSyncExt<R> for T {
    fn nostr_sync(&self) -> &TauriPluginNostrSync<R> {
        self.state::<TauriPluginNostrSync<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    PluginBuilder::new().build()
}
