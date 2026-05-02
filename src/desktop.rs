use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<TauriPluginNostr<R>> {
    Ok(TauriPluginNostr(app.clone()))
}

/// Access to the tauri-plugin-nostr APIs.
pub struct TauriPluginNostr<R: Runtime>(AppHandle<R>);

impl<R: Runtime> TauriPluginNostr<R> {
    pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
        Ok(PingResponse {
            value: payload.value,
        })
    }
}
