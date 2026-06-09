pub mod types;
pub mod error;
pub mod event;

mod fetcher;
mod installer;
mod activator;
mod deleter;

mod manager;
pub use manager::{VersionCommand, VersionManager, ExecuteOutput};
