//! XLayer chain specifications
//!
//! This crate provides chain specifications for XLayer mainnet and testnet networks.

mod parser;
mod xlayer_mainnet;
mod xlayer_testnet;

pub use parser::XLayerChainSpecParser;
pub use xlayer_mainnet::XLAYER_MAINNET;
pub use xlayer_testnet::XLAYER_TESTNET;

// Re-export OpChainSpec for convenience
pub use reth_optimism_chainspec::OpChainSpec;
