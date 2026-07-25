//! Entity domain — classification, canonicalization, and graph operations.
//!
//! Entity is a first-class intelligence domain, not an AI pipeline concern.
//! Sources of entities include AI extraction, RSS metadata, user input,
//! web search, and decision records.
//!
//! This crate is pure logic (no Worker dependency), so it can be tested
//! with standard Rust unit tests and used from any other crate.

pub mod canonicalizer;
pub mod classifier;
pub mod models;
