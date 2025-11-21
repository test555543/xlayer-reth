//! XLayer Mainnet chain specification

use alloy_genesis::Genesis;
use once_cell::sync::Lazy;
use reth_optimism_chainspec::OpChainSpec;
use std::sync::Arc;

/// XLayer Mainnet genesis
const XLAYER_MAINNET_GENESIS: &str = include_str!("../res/genesis/xlayer-mainnet.json");

/// XLayer Mainnet chain spec
pub static XLAYER_MAINNET: Lazy<Arc<OpChainSpec>> = Lazy::new(|| {
    let genesis: Genesis = serde_json::from_str(XLAYER_MAINNET_GENESIS)
        .expect("Failed to parse XLayer mainnet genesis");
    Arc::new(genesis.into())
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xlayer_mainnet_genesis() {
        let spec = &*XLAYER_MAINNET;
        assert_eq!(spec.chain().id(), 196);
    }

    #[test]
    fn test_xlayer_mainnet_is_optimism() {
        use reth_chainspec::EthChainSpec;
        assert!(XLAYER_MAINNET.is_optimism());
    }
}
