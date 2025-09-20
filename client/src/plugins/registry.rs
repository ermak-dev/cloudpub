use crate::plugins::minecraft::MinecraftPlugin;
use crate::plugins::onec::OneCPlugin;
use crate::plugins::webdav::WebdavPlugin;
use crate::plugins::Plugin;
use cloudpub_common::protocol::Protocol;
use std::collections::HashMap;
use std::sync::Arc;

pub struct PluginRegistry {
    plugins: HashMap<Protocol, Arc<dyn Plugin>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            plugins: HashMap::new(),
        };

        // Register all available plugins
        registry.register(Protocol::Webdav, Arc::new(WebdavPlugin));
        registry.register(Protocol::OneC, Arc::new(OneCPlugin));
        registry.register(Protocol::Minecraft, Arc::new(MinecraftPlugin));

        registry
    }

    pub fn register(&mut self, protocol: Protocol, plugin: Arc<dyn Plugin>) {
        self.plugins.insert(protocol, plugin);
    }

    pub fn get(&self, protocol: Protocol) -> Option<Arc<dyn Plugin>> {
        self.plugins.get(&protocol).cloned()
    }
}
