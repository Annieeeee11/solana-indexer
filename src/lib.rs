//! Real-time Solana blockchain indexer with multi-tier caching and optional HTTP API.
//!
//! # Quick start (CLI)
//!
//! ```bash
//! cargo install solana-indexer
//! cp .env.example .env   # set SOLANA_RPC_URL and optionally DATABASE_URL
//! solana-indexer start
//! ```
//!
//! # Library
//!
//! Use [`context::AppContext`] to bootstrap config, storage, cache, and data sources.
//! See the repository README for Supabase setup and live demo deployment.

pub mod context;
pub mod core;
pub mod data_sources;
pub mod storage;
pub mod utils;

pub mod api;

#[cfg(test)]
pub mod testing;
