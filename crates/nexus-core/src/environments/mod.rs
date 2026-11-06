//! Port of `packages/core/src/environments`: layered variable scopes and `{{var}}` interpolation.

mod dynamic;
mod store;
mod variable_store;

pub use dynamic::DYNAMIC_KEYS;
pub use store::EnvironmentStore;
pub use variable_store::{VariableScope, VariableScopes, VariableStore};
