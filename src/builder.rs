use tauri::{plugin::TauriPlugin, Manager, Runtime};

pub struct PluginBuilder {
    pub(crate) relays: Vec<String>,
    pub(crate) namespace: String,
}

impl PluginBuilder {
    pub fn new() -> Self {
        Self {
            relays: Vec::new(),
            namespace: "default".to_string(),
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

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        // Panic on invalid namespace — consistent with Tauri builder conventions.
        crate::state::validate_namespace(&self.namespace)
            .unwrap_or_else(|e| panic!("invalid app_namespace: {e}"));

        let relays = self.relays;
        let namespace = self.namespace;

        tauri::plugin::Builder::<R>::new("tauri-plugin-nostr-sync")
            .invoke_handler(tauri::generate_handler![])
            .setup(move |app, api| {
                #[cfg(mobile)]
                {
                    let plugin = crate::mobile::init(app, api)?;
                    app.manage(plugin);
                }
                #[cfg(desktop)]
                {
                    let _ = &api;
                    let plugin = crate::desktop::init(app, relays, &namespace)?;
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
    fn app_namespace_overrides_default() {
        let b = PluginBuilder::new().app_namespace("sage");
        assert_eq!(b.namespace, "sage");
    }

    #[test]
    fn relays_stores_provided_urls() {
        let b = PluginBuilder::new()
            .relays(vec!["wss://relay.damus.io", "wss://nos.lol"]);
        assert_eq!(b.relays, vec!["wss://relay.damus.io", "wss://nos.lol"]);
    }
}
