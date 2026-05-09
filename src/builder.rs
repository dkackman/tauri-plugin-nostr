use tauri::{plugin::TauriPlugin, Manager, Runtime};

pub struct PluginBuilder {
    pub(crate) relays: Vec<String>,
    pub(crate) namespace: String,
    pub(crate) network: String,
    pub(crate) device_id: String,
    pub(crate) max_payload_size: usize,
}

impl PluginBuilder {
    pub fn new() -> Self {
        Self {
            relays: Vec::new(),
            namespace: "default".to_string(),
            network: "mainnet".to_string(),
            device_id: uuid::Uuid::new_v4().to_string(),
            max_payload_size: crate::state::DEFAULT_PAYLOAD_LIMIT,
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

    pub fn network(mut self, network: impl Into<String>) -> Self {
        self.network = network.into();
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

    /// Override the maximum payload size in bytes. Defaults to 64KB. Must not exceed 400KB.
    ///
    /// Payloads exceeding this limit return `Error::PayloadTooLarge` from `publish`.
    /// Values above 400KB surface as `Error::InvalidPayloadLimit` at plugin startup.
    pub fn max_payload_size(mut self, bytes: usize) -> Self {
        self.max_payload_size = bytes;
        self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        // Panic on invalid namespace — consistent with Tauri builder conventions.
        crate::state::validate_namespace(&self.namespace)
            .unwrap_or_else(|e| panic!("invalid app_namespace: {e}"));

        let relays = self.relays;
        let namespace = self.namespace;
        let network = self.network;
        let device_id = self.device_id;
        let max_payload_size = self.max_payload_size;

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
                    let plugin = crate::mobile::init(
                        app,
                        api,
                        relays,
                        &namespace,
                        &network,
                        &device_id,
                        max_payload_size,
                    )?;
                    app.manage(plugin);
                }
                #[cfg(desktop)]
                {
                    let _ = &api;
                    let plugin = crate::desktop::init(
                        app,
                        relays,
                        &namespace,
                        &network,
                        &device_id,
                        max_payload_size,
                    )?;
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

    #[test]
    fn max_payload_size_defaults_to_64kb() {
        let b = PluginBuilder::new();
        assert_eq!(b.max_payload_size, crate::state::DEFAULT_PAYLOAD_LIMIT);
    }

    #[test]
    fn max_payload_size_setter_stores_value() {
        let b = PluginBuilder::new().max_payload_size(128 * 1024);
        assert_eq!(b.max_payload_size, 128 * 1024);
    }
}
