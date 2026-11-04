//! Threshold alerts over live monitoring samples. Rules and history are stored in SQLite; the
//! firing state is kept in memory and ends when monitoring stops.

pub mod engine;
pub mod model;
pub mod repo;
pub mod service;
