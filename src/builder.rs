use tauri::{plugin::TauriPlugin, Manager, Runtime};

pub struct PluginBuilder {
    pub(crate) relays: Vec<String>,
    pub(crate) namespace: String,
    pub(crate) device_id: String,
}

impl PluginBuilder {
    pub fn new() -> Self {
        Self {
            relays: Vec::new(),
            namespace: "default".to_string(),
            device_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn relays(mut self, urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.relays = urls.into_iter().map(|u| u.into()).collect();
        self
    }

    pub fn app_namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = ns.into();
        self
    }

    /// Override the device identifier stamped on published events.
    ///
    /// Defaults to a random UUID generated at process start (ephemeral). Supply a
    /// stable, per-device value — e.g. a UUID persisted in the app's data directory,
    /// a hardware serial, or a hash of the machine name — so that consumers can
    /// reliably attribute events to a specific device across restarts.
    pub fn device_id(mut self, id: impl Into<String>) -> Self {
        self.device_id = id.into();
        self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        // Panic on invalid namespace — consistent with Tauri builder conventions.
        crate::state::validate_namespace(&self.namespace)
            .unwrap_or_else(|e| panic!("invalid app_namespace: {e}"));

        let relays = self.relays;
        let namespace = self.namespace;
        let device_id = self.device_id;

        tauri::plugin::Builder::<R>::new("nostr-sync")
            .invoke_handler(tauri::generate_handler![
                crate::commands::publish,
                crate::commands::fetch,
                crate::commands::sync_all,
                crate::commands::add_relay,
                crate::commands::remove_relay,
                crate::commands::get_relays,
                crate::commands::get_pubkey,
                crate::commands::get_status,
                crate::commands::poll,
            ])
            .setup(move |app, api| {
                #[cfg(mobile)]
                {
                    let plugin = crate::mobile::init(app, api)?;
                    app.manage(plugin);
                }
                #[cfg(desktop)]
                {
                    let _ = &api;
                    let plugin = crate::desktop::init(app, relays, &namespace, &device_id)?;
                    app.manage(plugin);
                }
                Ok(())
            })
            .build()
    }
}

impl Default for PluginBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_default_namespace() {
        let b = PluginBuilder::new();
        assert_eq!(b.namespace, "default");
    }

    #[test]
    fn defaults_to_empty_relays() {
        let b = PluginBuilder::new();
        assert!(b.relays.is_empty());
    }

    #[test]
    fn defaults_to_ephemeral_device_id() {
        let b = PluginBuilder::new();
        assert!(!b.device_id.is_empty());
        // Two builders get different ephemeral IDs.
        assert_ne!(b.device_id, PluginBuilder::new().device_id);
    }

    #[test]
    fn device_id_overrides_ephemeral() {
        let b = PluginBuilder::new().device_id("my-stable-device");
        assert_eq!(b.device_id, "my-stable-device");
    }

    #[test]
    fn app_namespace_overrides_default() {
        let b = PluginBuilder::new().app_namespace("sage");
        assert_eq!(b.namespace, "sage");
    }

    #[test]
    fn relays_stores_provided_urls() {
        let b = PluginBuilder::new().relays(vec!["wss://relay.damus.io", "wss://nos.lol"]);
        assert_eq!(b.relays, vec!["wss://relay.damus.io", "wss://nos.lol"]);
    }
}
