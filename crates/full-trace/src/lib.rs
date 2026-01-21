//! XLayer full trace support
//!
//! This crate provides tracing functionality for the XLayer Engine API and RPC calls.
//!
//! # Features
//!
//! - **Engine API Tracing**: Trace Engine API calls like `fork_choice_updated` and `new_payload`
//! - **RPC Transaction Tracing**: Trace transaction submissions via `eth_sendRawTransaction` and `eth_sendTransaction`
//! - **Blockchain Tracing**: Monitor canonical state changes (block commits, transaction commits)
//!
//! # Architecture
//!
//! The tracer system is designed around a shared `Tracer<Args>` that holds configuration
//! and event handlers. This config is shared across multiple tracer components:
//!
//! - `EngineApiTracer`: Middleware for Engine API calls (implements `EngineApiBuilder`)
//! - `RpcTracerLayer`: Tower layer for RPC middleware
//! - `initialize_blockchain_tracer`: Background task for canonical state monitoring
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use xlayer_full_trace::{Tracer, EngineApiTracer, RpcTracerLayer};
//!
//! // Create a shared tracer configuration (returns Arc<Tracer<Args>>)
//! let tracer = Tracer::new(xlayer_args.full_trace);
//!
//! // Create Engine API tracer with the shared tracer
//! let engine_tracer = EngineApiTracer::new(tracer.clone());
//!
//! // Add RPC tracing middleware and Engine API tracer
//! let add_ons = op_node
//!     .add_ons()
//!     .with_rpc_middleware(RpcTracerLayer::new(tracer.clone()))
//!     .with_engine_api(engine_tracer);
//!
//! // Later, in extend_rpc_modules, initialize blockchain tracer
//! tracer.initialize_blockchain_tracer(ctx.node());
//! ```
//!
//! # Implementing Custom Event Handlers
//!
//! To add custom tracing logic, modify the event handler methods in `Tracer`:
//! - `on_fork_choice_updated`: Called before fork choice updates
//! - `on_new_payload`: Called before new payload execution
//! - `on_recv_transaction`: Called when a transaction is received via RPC
//! - `on_block_commit`: Called when a block is committed to canonical chain
//! - `on_tx_commit`: Called when a transaction is committed to canonical chain

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod blockchain_tracer;
mod engine_api_tracer;
mod rpc_tracer;
mod tracer;

pub use blockchain_tracer::handle_canonical_state_stream;
pub use engine_api_tracer::EngineApiTracer;
pub use rpc_tracer::{RpcTracerLayer, RpcTracerService};
pub use tracer::{BlockInfo, Tracer};
