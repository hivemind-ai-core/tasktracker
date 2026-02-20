//! Database layer for tt
//!
//! Handles all SQLite operations including schema, connection management,
//! and CRUD operations for tasks, dependencies, artifacts, and config.

pub mod artifacts;
pub mod config;
pub mod connection;
pub mod dependencies;
pub mod schema;
pub mod tasks;

pub use connection::{init_schema, is_initialized, open_db, open_memory_db};
