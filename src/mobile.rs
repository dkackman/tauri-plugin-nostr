use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_tauri_plugin_nostr_sync);

// initializes the Kotlin or Swift plugin classes
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<TauriPluginNostr<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("tauri-plugin-nostr-sync", "ExamplePlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_tauri_plugin_nostr_sync)?;
    Ok(TauriPluginNostr(handle))
}

/// Access to the tauri-plugin-nostr-sync APIs.
pub struct TauriPluginNostr<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> TauriPluginNostr<R> {
    // Commands will be added in Phase 2.
}
