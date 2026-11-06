//! Nexus engine library (no UI dependency): one module per concern.

pub mod auth;
pub mod codegen;
pub mod collections;
pub mod console;
pub mod cookies;
pub mod docs;
pub mod environments;
pub mod error;
pub mod fs;

pub mod graphql;
pub mod history;
pub mod http;
pub mod import;
pub mod mcp;
pub mod response;
pub mod runner;
pub mod scripts;
pub mod service;
pub mod types;
pub mod visualizer;
pub mod websocket;

pub use error::{Error, Result};
