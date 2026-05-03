use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<TauriPluginNostr<R>> {
    Ok(TauriPluginNostr(app.clone()))
}

/// Access to the tauri-plugin-nostr APIs.
pub struct TauriPluginNostr<R: Runtime>(AppHandle<R>);

impl<R: Runtime> TauriPluginNostr<R> {
    // Commands will be added in Phase 2.
}
