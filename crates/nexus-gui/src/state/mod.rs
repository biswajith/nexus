//! Plain state structs.

pub mod collections;
pub mod console;
pub mod dialogs;
pub mod environments;
pub mod forms;
pub mod gql_schema;
pub mod graphql;
pub mod history;
pub mod mcp;
pub mod response;
pub mod runner;
pub mod tabs;
pub mod ui_state;
pub mod ws;

pub use collections::CollectionsState;
pub use console::ConsoleState;
pub use environments::EnvState;
pub use graphql::GqlState;
pub use history::HistoryState;
pub use mcp::McpState;
pub use response::Resp;
pub use runner::RunnerState;
pub use tabs::{Origin, RequestTab, Tabs};
pub use ui_state::{Panel, SidebarTab, UiState};
pub use ws::WsState;

#[derive(PartialEq, Clone, Copy)]
pub enum ReqTab {
    Params,
    Headers,
    Body,
    Auth,
    Scripts,
}

#[derive(PartialEq, Clone, Copy)]
pub enum RespTab {
    Body,
    Headers,
    Tests,
    Timing,
    Visualize,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ScriptTab {
    Pre,
    Post,
}

/// Transient view state of the HTTP panel (not persisted).
pub struct ViewState {
    pub req_tab: ReqTab,
    pub resp_tab: RespTab,
    pub script_tab: ScriptTab,
    /// Generated cURL command shown in the popup.
    pub curl: Option<String>,
    /// Save popup: collection directory picked, and the request name typed.
    pub save_target: Option<String>,
    pub save_name: String,
    /// Secret auth fields currently shown in clear text.
    pub revealed: std::collections::HashSet<String>,
    /// Focus the URL bar / the environment switcher on the next frame.
    pub focus_url: bool,
    pub focus_env: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            req_tab: ReqTab::Body,
            resp_tab: RespTab::Body,
            script_tab: ScriptTab::Pre,
            curl: None,
            save_target: None,
            save_name: String::new(),
            revealed: Default::default(),
            focus_url: false,
            focus_env: false,
        }
    }
}
