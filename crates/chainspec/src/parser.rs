//! XLayer chain specification parser

use crate::{XLAYER_MAINNET, XLAYER_TESTNET};
use alloy_genesis::Genesis;
use reth_cli::chainspec::ChainSpecParser;
use reth_optimism_chainspec::OpChainSpec;
use std::sync::Arc;

/// XLayer chain specification parser
///
/// This parser extends the default OpChainSpecParser to support XLayer chains:
/// - xlayer-mainnet (chain id 196)
/// - xlayer-testnet (chain id 1952)
///
/// It also supports all standard Optimism chains through delegation to the
/// upstream OpChainSpecParser.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct XLayerChainSpecParser;

impl ChainSpecParser for XLayerChainSpecParser {
    type ChainSpec = OpChainSpec;

    const SUPPORTED_CHAINS: &'static [&'static str] = &[
        // Standard OP chains
        "dev",
        "optimism",
        "optimism_sepolia",
        "optimism-sepolia",
        "base",
        "base_sepolia",
        "base-sepolia",
        // XLayer chains
        "xlayer-mainnet",
        "xlayer-testnet",
    ];

    fn parse(s: &str) -> eyre::Result<Arc<Self::ChainSpec>> {
        xlayer_chain_value_parser(s)
    }
}

/// Parse genesis from file path or JSON string
fn parse_genesis(s: &str) -> eyre::Result<Genesis> {
    // Try to parse as file path first
    if let Ok(contents) = std::fs::read_to_string(s) {
        return serde_json::from_str(&contents)
            .map_err(|e| eyre::eyre!("Failed to parse genesis file: {}", e));
    }

    // Try to parse as JSON string
    serde_json::from_str(s).map_err(|e| eyre::eyre!("Failed to parse genesis JSON: {}", e))
}

/// XLayer chain value parser
///
/// Parses chain specifications with the following priority:
/// 1. XLayer named chains (xlayer-mainnet, xlayer-testnet)
/// 2. Standard Optimism named chains (via OpChainSpecParser)
/// 3. Genesis file path or JSON string
fn xlayer_chain_value_parser(s: &str) -> eyre::Result<Arc<OpChainSpec>> {
    match s {
        "xlayer-mainnet" => {
            // Support environment variable override for genesis path
            if let Ok(genesis_path) = std::env::var("XLAYER_MAINNET_GENESIS") {
                return Ok(Arc::new(parse_genesis(&genesis_path)?.into()));
            }
            Ok(XLAYER_MAINNET.clone())
        }
        "xlayer-testnet" => {
            // Support environment variable override for genesis path
            if let Ok(genesis_path) = std::env::var("XLAYER_TESTNET_GENESIS") {
                return Ok(Arc::new(parse_genesis(&genesis_path)?.into()));
            }
            Ok(XLAYER_TESTNET.clone())
        }
        // Delegate to upstream OpChainSpecParser for other chains
        _ => reth_optimism_cli::chainspec::chain_value_parser(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xlayer_mainnet() {
        let spec = XLayerChainSpecParser::parse("xlayer-mainnet").unwrap();
        assert_eq!(spec.chain().id(), 196);
    }

    #[test]
    fn test_parse_xlayer_testnet() {
        let spec = XLayerChainSpecParser::parse("xlayer-testnet").unwrap();
        assert_eq!(spec.chain().id(), 1952);
    }

    #[test]
    fn test_parse_optimism() {
        let spec = XLayerChainSpecParser::parse("optimism").unwrap();
        assert_eq!(spec.chain().id(), 10);
    }

    #[test]
    fn test_parse_base() {
        let spec = XLayerChainSpecParser::parse("base").unwrap();
        assert_eq!(spec.chain().id(), 8453);
    }

    #[test]
    fn test_supported_chains() {
        assert!(XLayerChainSpecParser::SUPPORTED_CHAINS.contains(&"xlayer-mainnet"));
        assert!(XLayerChainSpecParser::SUPPORTED_CHAINS.contains(&"xlayer-testnet"));
        assert!(XLayerChainSpecParser::SUPPORTED_CHAINS.contains(&"optimism"));
        assert!(XLayerChainSpecParser::SUPPORTED_CHAINS.contains(&"base"));
    }

    #[test]
    fn test_parse_all_supported_chains() {
        for &chain in XLayerChainSpecParser::SUPPORTED_CHAINS {
            assert!(XLayerChainSpecParser::parse(chain).is_ok(), "Failed to parse {chain}");
        }
    }
}
