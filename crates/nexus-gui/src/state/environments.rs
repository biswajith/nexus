//! Environments and the selected one (replaces `environment-store.ts`).

use nexus_core::types::{NexusEnvironment, Variable};

#[derive(Default)]
pub struct EnvState {
    pub environments: Vec<NexusEnvironment>,
}

impl EnvState {
    pub fn get(&self, id: Option<&str>) -> Option<&NexusEnvironment> {
        let id = id?;
        self.environments.iter().find(|e| e.id == id)
    }

    pub fn name(&self, id: Option<&str>) -> &str {
        self.get(id).map_or("No Environment", |e| e.name.as_str())
    }

    pub fn variables(&self, id: Option<&str>) -> Vec<Variable> {
        self.get(id)
            .map(|e| e.variables.clone())
            .unwrap_or_default()
    }
}
