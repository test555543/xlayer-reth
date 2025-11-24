# XLayer ChainSpec

XLayer chain specifications for xlayer-reth.

## Overview

This crate provides chain specifications for XLayer networks:
- **xlayer-mainnet** (Chain ID: 196)
- **xlayer-testnet** (Chain ID: 195)

It extends the Optimism chain specification parser to support XLayer chains while maintaining compatibility with all standard Optimism chains (optimism, base, etc.).

## Structure

```
crates/chainspec/
├── Cargo.toml
├── README.md
├── res/
│   └── genesis/
│       ├── xlayer-mainnet.json    # XLayer mainnet genesis
│       └── xlayer-testnet.json    # XLayer testnet genesis
└── src/
    ├── lib.rs                     # Main library exports
    ├── parser.rs                  # XLayerChainSpecParser implementation
    ├── xlayer_mainnet.rs          # XLayer mainnet chain spec
    └── xlayer_testnet.rs          # XLayer testnet chain spec
```

## Usage

### In Code

```rust
use xlayer_chainspec::{XLayerChainSpecParser, XLAYER_MAINNET, XLAYER_TESTNET};

// Use the parser with reth CLI
Cli::<XLayerChainSpecParser, Args>::parse()
    .run(|builder, args| async move {
        // Your node logic here
    })
    .unwrap();

// Or access chain specs directly
let mainnet_spec = &*XLAYER_MAINNET;
assert_eq!(mainnet_spec.chain().id(), 196);

let testnet_spec = &*XLAYER_TESTNET;
assert_eq!(testnet_spec.chain().id(), 1952);
```

### Command Line

```bash
# Start XLayer mainnet node
xlayer-reth-node node --chain xlayer-mainnet

# Start XLayer testnet node
xlayer-reth-node node --chain xlayer-testnet

# Use custom genesis file (via environment variable)
XLAYER_MAINNET_GENESIS=/path/to/custom/genesis.json xlayer-reth-node node --chain xlayer-mainnet

# Standard Optimism chains still work
xlayer-reth-node node --chain optimism
xlayer-reth-node node --chain base
```

## Supported Chains

The `XLayerChainSpecParser` supports the following chains:

### XLayer Chains
- `xlayer-mainnet` - XLayer Mainnet (Chain ID: 196)
- `xlayer-testnet` - XLayer Testnet (Chain ID: 195)

### Standard Optimism Chains
- `dev` - Development chain
- `optimism` / `optimism_sepolia` / `optimism-sepolia` - Optimism networks
- `base` / `base_sepolia` / `base-sepolia` - Base networks

## Genesis Configuration

The genesis files in `res/genesis/` contain the initial state and configuration for each network:

- **Chain ID**: Network identifier
- **Hardfork Times**: Activation timestamps for protocol upgrades
- **Optimism Config**: OP stack specific parameters (EIP-1559, etc.)
- **Alloc**: Initial account balances and contract deployments

### Customizing Genesis

You can override the built-in genesis files using environment variables:

```bash
# Use custom mainnet genesis
export XLAYER_MAINNET_GENESIS=/path/to/custom/mainnet-genesis.json
xlayer-reth node --chain xlayer-mainnet

# Use custom testnet genesis
export XLAYER_TESTNET_GENESIS=/path/to/custom/testnet-genesis.json
xlayer-reth node --chain xlayer-testnet
```

## Testing

Run the tests to verify chain specifications:

```bash
cargo test --package xlayer-chainspec
```

Tests include:
- Genesis file parsing
- Chain ID verification
- Optimism compatibility checks
- Parser functionality for all supported chains

## Integration

This crate is integrated into `xlayer-reth-node`:

```toml
# crates/node/Cargo.toml
[dependencies]
xlayer-chainspec = { workspace = true }
```

```rust
// crates/node/src/main.rs
use xlayer_chainspec::XLayerChainSpecParser;

Cli::<XLayerChainSpecParser, Args>::parse()
    .run(|builder, args| async move {
        // Node implementation
    })
    .unwrap();
```

## Architecture

The parser follows a delegation pattern:

1. **XLayer chains** (`xlayer-mainnet`, `xlayer-testnet`) → Loaded from built-in genesis files
2. **Standard OP chains** → Delegated to `reth_optimism_cli::chainspec::chain_value_parser`
3. **Custom paths/JSON** → Parsed as genesis files

This ensures backward compatibility with all Optimism chains while adding XLayer support.

## Dependencies

- `reth-optimism-chainspec` - Base Optimism chain specification
- `reth-optimism-cli` - Optimism CLI utilities
- `alloy-genesis` - Genesis file parsing
- `once_cell` - Lazy static initialization

## License

MIT

