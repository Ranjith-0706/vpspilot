//! Persistent monitoring history. Successful samples from the monitor are stored in SQLite so
//! charts survive a restart. Samples are thinned to one per minute per server and trimmed by a
//! configurable retention period, so storage stays bounded. Nothing here talks to a server.

pub mod model;
pub mod repo;
pub mod service;

#[cfg(test)]
mod tests;
