pub mod types;
pub mod error;
pub mod event;

pub mod fetcher;
pub mod installer;
pub mod activator;
pub mod deleter;

mod manager;
pub use manager::{VersionCommand, VersionManager, ExecuteOutput};

#[cfg(test)]
mod tests;
