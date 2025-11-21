//! XLayer Testnet chain specification

use alloy_genesis::Genesis;
use once_cell::sync::Lazy;
use reth_optimism_chainspec::OpChainSpec;
use std::sync::Arc;

/// XLayer Testnet genesis
const XLAYER_TESTNET_GENESIS: &str = include_str!("../res/genesis/xlayer-testnet.json");

/// XLayer Testnet chain spec
pub static XLAYER_TESTNET: Lazy<Arc<OpChainSpec>> = Lazy::new(|| {
    let genesis: Genesis = serde_json::from_str(XLAYER_TESTNET_GENESIS)
        .expect("Failed to parse XLayer testnet genesis");
    Arc::new(genesis.into())
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xlayer_testnet_genesis() {
        let spec = &*XLAYER_TESTNET;
        assert_eq!(spec.chain().id(), 195);
    }

    #[test]
    fn test_xlayer_testnet_is_optimism() {
        use reth_chainspec::EthChainSpec;
        assert!(XLAYER_TESTNET.is_optimism());
    }
}

