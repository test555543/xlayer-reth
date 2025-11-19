//! XLayer custom genesis initialization module.
//!
//! This module provides a trait-based system for customizing genesis initialization
//! to handle non-zero genesis block numbers and proper static file initialization.

use alloy_consensus::{BlockHeader, Header, Sealable};
use alloy_eips::eip7840::BlobParams;
use alloy_primitives::B256;
use delegate::delegate;
use reth_chainspec::{Chain, EthChainSpec};
use reth_codecs::Compact;
use reth_db_common::init::InitStorageError;
use reth_network_peers::NodeRecord;
use reth_node_types::NodePrimitives;
use reth_provider::{
    providers::{StaticFileProvider, StaticFileWriter},
    BlockHashReader, BundleStateInit, ChainSpecProvider, DBProvider, DatabaseProviderFactory,
    ExecutionOutcome, HashingWriter, HeaderProvider, HistoryWriter, OriginalValuesKnown,
    ProviderError, StageCheckpointReader, StageCheckpointWriter, StateWriter,
    StaticFileProviderFactory, TrieWriter,
};
use reth_stages_types::StageCheckpoint;
use reth_static_file_types::{StaticFileSegment, DEFAULT_BLOCKS_PER_STATIC_FILE};
use reth_trie::{IntermediateStateRootState, StateRoot as StateRootComputer, StateRootProgress};
use reth_trie_db::DatabaseStateRoot;
use std::sync::Arc;
use tracing::{debug, info, trace};

/// Soft limit for the number of flushed updates after which to log progress summary.
const SOFT_LIMIT_COUNT_FLUSHED_UPDATES: usize = 1_000_000;

/// Wrapper around ChainSpec that overrides genesis_header with modified number and parent_hash.
/// This is needed because insert_genesis_header reads header from chain.genesis_header(),
/// and we need to override the number and parent_hash fields from genesis.number and genesis.parent_hash.
#[derive(Debug)]
struct XLayerChainSpec<CS: EthChainSpec> {
    inner: Arc<CS>,
    modified_header: CS::Header,
}

impl<CS> XLayerChainSpec<CS>
where
    CS: EthChainSpec,
    CS::Header: Clone + BlockHeader,
{
    fn new(chain_spec: &Arc<CS>) -> Self {
        let genesis = chain_spec.genesis();

        // Get genesis number from genesis.number, or override with legacyXLayerBlock if present
        let genesis_number = if let Some(legacy_block_value) =
            genesis.config.extra_fields.get("legacyXLayerBlock")
        {
            if let Some(legacy_block) = legacy_block_value.as_u64() {
                legacy_block
            } else {
                genesis.number.unwrap_or_default()
            }
        } else {
            genesis.number.unwrap_or_default()
        };

        let genesis_parent_hash = genesis.parent_hash.unwrap_or_default();

        // Clone the original header and modify number and parent_hash
        let original_header = chain_spec.genesis_header();
        let modified_header =
            create_modified_header(original_header, genesis_number, genesis_parent_hash);

        Self { inner: chain_spec.clone(), modified_header }
    }
}

/// Helper function to create a modified header with custom number and parent_hash.
/// Since Header is a trait, we need to work with the concrete type.
/// We'll use unsafe to modify the fields, which is safe because we're only modifying
/// standard fields (number and parent_hash) that exist in all Header implementations.
fn create_modified_header<H: BlockHeader + Clone>(
    original: &H,
    number: u64,
    parent_hash: B256,
) -> H {
    // Clone the header
    let mut modified = original.clone();

    // Modify number and parent_hash fields
    // Since H is a trait, we can't directly access fields. We need to use unsafe
    // to cast to the concrete Header type and modify the fields.
    // SAFETY: We know that H is Header (the concrete type from alloy_consensus) in practice.
    // We're only modifying number and parent_hash which are standard fields in all Header implementations.
    unsafe {
        // Cast to *mut Header to modify fields
        let header_ptr = &mut modified as *mut H as *mut Header;
        (*header_ptr).number = number;
        (*header_ptr).parent_hash = parent_hash;
    }

    modified
}

impl<CS> EthChainSpec for XLayerChainSpec<CS>
where
    CS: EthChainSpec,
{
    type Header = CS::Header;

    // Delegate all methods to inner except genesis_header and genesis_hash
    delegate! {
        to self.inner {
            fn chain(&self) -> Chain;
            fn base_fee_params_at_timestamp(&self, timestamp: u64) -> reth_chainspec::BaseFeeParams;
            fn blob_params_at_timestamp(&self, timestamp: u64) -> Option<BlobParams>;
            fn deposit_contract(&self) -> Option<&reth_chainspec::DepositContract>;
            fn prune_delete_limit(&self) -> usize;
            fn display_hardforks(&self) -> Box<dyn std::fmt::Display>;
            fn bootnodes(&self) -> Option<Vec<NodeRecord>>;
            fn final_paris_total_difficulty(&self) -> Option<alloy_primitives::U256>;
            fn genesis(&self) -> &alloy_genesis::Genesis;
        }
    }

    // Override genesis_header to return the modified header
    fn genesis_header(&self) -> &Self::Header {
        &self.modified_header
    }

    // Override genesis_hash to return the hash of the modified header
    // This is necessary because genesis_hash is calculated from genesis_header.hash_slow(),
    // and we've modified the header's number and parent_hash fields
    fn genesis_hash(&self) -> B256 {
        self.modified_header.hash_slow()
    }
}

/// Trait for custom genesis initialization logic.
///
/// This trait allows implementing custom genesis initialization strategies
/// that can handle different chain configurations, including non-zero genesis blocks.
pub trait GenesisInitializer<PF>
where
    PF: DatabaseProviderFactory
        + StaticFileProviderFactory<Primitives: NodePrimitives<BlockHeader: Compact>>
        + ChainSpecProvider
        + StageCheckpointReader
        + BlockHashReader,
    PF::ProviderRW: StaticFileProviderFactory<Primitives = PF::Primitives>
        + StageCheckpointWriter
        + HistoryWriter
        + HeaderProvider
        + HashingWriter
        + StateWriter
        + TrieWriter
        + ChainSpecProvider
        + AsRef<PF::ProviderRW>,
    PF::ChainSpec: EthChainSpec<Header = <PF::Primitives as NodePrimitives>::BlockHeader>,
{
    /// Initialize genesis block with custom logic.
    ///
    /// Returns the genesis hash if successful.
    fn init_genesis(&self, factory: &PF) -> Result<B256, InitStorageError>;
}

/// Default XLayer genesis initializer implementation.
///
/// This implementation handles non-zero genesis block numbers correctly
/// by using the genesis block number from the chain spec instead of hardcoding 0.
#[derive(Debug, Clone, Default)]
pub struct XLayerGenesisInitializer;

impl<PF> GenesisInitializer<PF> for XLayerGenesisInitializer
where
    PF: DatabaseProviderFactory
        + StaticFileProviderFactory<Primitives: NodePrimitives<BlockHeader: Compact>>
        + ChainSpecProvider
        + StageCheckpointReader
        + BlockHashReader,
    PF::ProviderRW: StaticFileProviderFactory<Primitives = PF::Primitives>
        + StageCheckpointWriter
        + HistoryWriter
        + HeaderProvider
        + HashingWriter
        + StateWriter
        + TrieWriter
        + ChainSpecProvider
        + AsRef<PF::ProviderRW>,
    PF::ChainSpec: EthChainSpec<Header = <PF::Primitives as NodePrimitives>::BlockHeader>,
{
    fn init_genesis(&self, factory: &PF) -> Result<B256, InitStorageError> {
        use reth_db_common::init::insert_genesis_hashes;
        use reth_stages_types::StageId;
        use reth_tracing::tracing::error;

        let chain_spec = factory.chain_spec();

        // Insert header with modified number and parent_hash from genesis
        // Create a modified chain spec wrapper that overrides genesis_header
        // If legacyXLayerBlock is specified in config, it will override genesis.number
        let chain = XLayerChainSpec::new(&chain_spec);

        let genesis = chain.genesis();
        let hash = chain.genesis_hash();

        // Get the genesis block number from the chain spec
        let genesis_block_number = chain.genesis_header().number();

        // Check if we already have the genesis header or if we have the wrong one.
        match factory.block_hash(genesis_block_number) {
            Ok(None)
            | Err(ProviderError::MissingStaticFileBlock(StaticFileSegment::Headers, _)) => {}
            Ok(Some(block_hash)) => {
                if block_hash == hash {
                    // Some users will at times attempt to re-sync from scratch by just deleting the
                    // database. Since `factory.block_hash` will only query the static files, we need to
                    // make sure that our database has been written to, and throw error if it's empty.
                    if factory.get_stage_checkpoint(StageId::Headers)?.is_none() {
                        error!(target: "reth::storage", "Genesis header found on static files, but database is uninitialized.");
                        return Err(InitStorageError::UninitializedDatabase);
                    }

                    debug!("Genesis already written, skipping.");
                    return Ok(hash);
                }

                return Err(InitStorageError::GenesisHashMismatch {
                    chainspec_hash: hash,
                    storage_hash: block_hash,
                });
            }
            Err(e) => {
                debug!(?e);
                return Err(e.into());
            }
        }

        debug!("Writing genesis block in custom block number: {}", genesis_block_number);

        let alloc = &genesis.alloc;

        // use transaction to insert genesis header
        let provider_rw = factory.database_provider_rw()?;
        insert_genesis_hashes(&provider_rw, alloc.iter())?;
        // Use custom insert_genesis_history that supports non-zero genesis block numbers
        insert_genesis_history_custom(&provider_rw, alloc.iter(), genesis_block_number)?;

        // Custom insert_genesis_header that supports non-zero genesis block numbers
        insert_genesis_header_custom(&provider_rw, &chain)?;

        // Use custom insert_genesis_state that supports non-zero genesis block numbers
        insert_genesis_state_custom(&provider_rw, alloc.iter(), genesis_block_number)?;

        // compute state root to populate trie tables
        compute_state_root(&provider_rw, None)?;

        // Set stage checkpoint to genesis block number for all stages
        let checkpoint =
            StageCheckpoint { block_number: genesis_block_number, ..Default::default() };
        for stage in StageId::ALL {
            provider_rw.save_stage_checkpoint(stage, checkpoint)?;
        }

        // Static file segments start empty, so we need to initialize the genesis block.
        let static_file_provider = provider_rw.static_file_provider();

        // Initialize Transactions and Receipts static files
        initialize_transactions_and_receipts_static_files(
            &static_file_provider,
            genesis_block_number,
        )?;

        // `commit_unwind`` will first commit the DB and then the static file provider, which is
        // necessary on `init_genesis`.
        provider_rw.commit()?;

        Ok(hash)
    }
}

/// Custom insert_genesis_header that supports non-zero genesis block numbers.
/// For non-zero genesis blocks, this function inserts empty headers from block 0 to genesis_block_number - 1,
/// then inserts the actual genesis header at genesis_block_number.
fn insert_genesis_header_custom<Provider, Spec>(
    provider: &Provider,
    chain: &Spec,
) -> Result<(), InitStorageError>
where
    Provider: StaticFileProviderFactory<Primitives: NodePrimitives<BlockHeader: Compact>>
        + DBProvider<Tx: reth_db_api::transaction::DbTxMut>,
    Spec: EthChainSpec<Header = <Provider::Primitives as NodePrimitives>::BlockHeader>,
{
    use reth_db_api::{tables, transaction::DbTxMut};

    let (header, block_hash) = (chain.genesis_header(), chain.genesis_hash());
    let static_file_provider = provider.static_file_provider();

    // Get the actual genesis block number from the header
    let genesis_block_number = header.number();

    match static_file_provider.block_hash(genesis_block_number) {
        Ok(None) | Err(ProviderError::MissingStaticFileBlock(StaticFileSegment::Headers, _)) => {
            let difficulty = header.difficulty();

            // For non-zero genesis blocks, initialize header static files
            if genesis_block_number > 0 {
                initialize_header_static_files(
                    provider,
                    &static_file_provider,
                    header,
                    &block_hash,
                    alloy_primitives::U256::from(difficulty),
                    genesis_block_number,
                )?;
            } else {
                // For zero genesis blocks, use normal append_header
                let mut writer = static_file_provider.latest_writer(StaticFileSegment::Headers)?;
                writer.append_header(header, &block_hash)?;
            }
        }
        Ok(Some(_)) => {}
        Err(e) => return Err(e.into()),
    }

    // Store the genesis header number mapping
    provider.tx_ref().put::<tables::HeaderNumbers>(block_hash, genesis_block_number)?;
    // Store genesis block body indices
    provider.tx_ref().put::<tables::BlockBodyIndices>(genesis_block_number, Default::default())?;

    Ok(())
}

/// Creates an empty header for a given block number.
/// This is used to fill in blocks before the actual genesis block.
fn create_empty_header<H>(block_number: u64) -> H
where
    H: alloy_consensus::BlockHeader + Default,
{
    let mut header = H::default();
    // Set the block number using unsafe pointer cast
    // SAFETY: We're only setting the number field which exists in all BlockHeader implementations
    unsafe {
        use alloy_consensus::Header;
        let header_ptr = &mut header as *mut H as *mut Header;
        (*header_ptr).number = block_number;
    }
    header
}

/// Generic function to create static file ranges with custom logic per range.
/// Uses a closure to allow different implementations for headers vs transactions/receipts.
fn create_static_file_ranges<F, T>(
    num_ranges_to_create: u64,
    genesis_range_idx: u64,
    segment_name: &str,
    mut range_handler: F,
) -> Result<(), InitStorageError>
where
    F: FnMut(u64, u64, u64) -> Result<T, InitStorageError>,
    T: std::fmt::Debug,
{
    use reth_tracing::tracing::info;

    for range_idx in 0..num_ranges_to_create {
        let range_start = range_idx * DEFAULT_BLOCKS_PER_STATIC_FILE;
        let range_end = (range_idx + 1) * DEFAULT_BLOCKS_PER_STATIC_FILE - 1;

        // Call the custom handler for this range
        range_handler(range_idx, range_start, range_end)?;

        info!(
            target: "reth::cli",
            range_idx,
            range_start,
            range_end,
            segment = segment_name,
            progress = format!("{:.1}%", ((range_idx + 1) as f64 / (genesis_range_idx + 1) as f64) * 100.0),
            "Created static file range"
        );
    }

    Ok(())
}

/// Initializes header static files for non-zero genesis blocks.
/// Creates all necessary static file ranges and fills them with empty headers up to and including the genesis block.
fn initialize_header_static_files<Provider, H>(
    provider: &Provider,
    static_file_provider: &StaticFileProvider<impl NodePrimitives<BlockHeader = H>>,
    genesis_header: &H,
    genesis_hash: &B256,
    difficulty: alloy_primitives::U256,
    genesis_block_number: u64,
) -> Result<(), InitStorageError>
where
    Provider: DBProvider<Tx: reth_db_api::transaction::DbTxMut>,
    H: alloy_consensus::BlockHeader + Default + Sealable + Compact,
{
    // Calculate which range the genesis block is in
    let genesis_range_idx = genesis_block_number / DEFAULT_BLOCKS_PER_STATIC_FILE;

    // We need to create files for all ranges up to and including the genesis range
    let num_ranges_to_create = genesis_range_idx + 1;

    info!(
        target: "reth::cli",
        genesis_block_number,
        genesis_range_idx,
        num_ranges_to_create,
        blocks_per_file = DEFAULT_BLOCKS_PER_STATIC_FILE,
        "Creating static file ranges for headers from block 0 to genesis block"
    );

    // Create all header static file ranges and fill genesis range
    create_and_fill_header_static_files(
        provider,
        static_file_provider,
        genesis_header,
        genesis_hash,
        difficulty,
        num_ranges_to_create,
        genesis_range_idx,
        genesis_block_number,
    )?;

    Ok(())
}

/// Creates all header static file ranges and fills the genesis range with empty headers.
fn create_and_fill_header_static_files<Provider, H>(
    provider: &Provider,
    static_file_provider: &StaticFileProvider<impl NodePrimitives<BlockHeader = H>>,
    genesis_header: &H,
    genesis_hash: &B256,
    difficulty: alloy_primitives::U256,
    num_ranges_to_create: u64,
    genesis_range_idx: u64,
    genesis_block_number: u64,
) -> Result<(), InitStorageError>
where
    Provider: DBProvider<Tx: reth_db_api::transaction::DbTxMut>,
    H: alloy_consensus::BlockHeader + Default + Sealable + Compact,
{
    use alloy_primitives::U256;
    use reth_db_api::{tables, transaction::DbTxMut};

    // Create all header static file ranges
    create_static_file_ranges(
        num_ranges_to_create,
        genesis_range_idx,
        "headers",
        |range_idx, range_start, range_end| {
            // Get writer for this specific range
            let mut writer =
                static_file_provider.get_writer(range_start, StaticFileSegment::Headers)?;

            // Insert one empty header at the start of this range to create the file
            let empty_header = create_empty_header::<H>(range_start);
            let empty_hash = empty_header.hash_slow();

            // Set block range: block_end = range_start - 1, so next_block_number() = range_start
            // This allows append_header_with_td to insert at range_start
            if range_start > 0 {
                writer.user_header_mut().set_block_range(range_start, range_start - 1);
            }
            writer.append_header_with_td(&empty_header, U256::ZERO, &empty_hash)?;

            // Store the header number mapping in the database
            provider.tx_ref().put::<tables::HeaderNumbers>(empty_hash, range_start)?;
            // Store empty block body indices
            provider.tx_ref().put::<tables::BlockBodyIndices>(range_start, Default::default())?;

            Ok((range_idx, range_start, range_end))
        },
    )?;

    info!(
        target: "reth::cli",
        total_ranges_created = num_ranges_to_create,
        "Finished creating header static file ranges, now filling genesis range and inserting genesis header"
    );

    // Fill the genesis range and insert the actual genesis header
    fill_header_genesis_range(
        provider,
        static_file_provider,
        genesis_header,
        genesis_hash,
        difficulty,
        genesis_range_idx,
        genesis_block_number,
    )?;

    Ok(())
}

/// Fills the genesis range with empty headers and inserts the actual genesis header.
fn fill_header_genesis_range<Provider, H>(
    provider: &Provider,
    static_file_provider: &StaticFileProvider<impl NodePrimitives<BlockHeader = H>>,
    genesis_header: &H,
    genesis_hash: &B256,
    difficulty: alloy_primitives::U256,
    genesis_range_idx: u64,
    genesis_block_number: u64,
) -> Result<(), InitStorageError>
where
    Provider: DBProvider<Tx: reth_db_api::transaction::DbTxMut>,
    H: alloy_consensus::BlockHeader + Default + Sealable + Compact,
{
    use alloy_primitives::U256;
    use reth_db_api::{tables, transaction::DbTxMut};

    let genesis_range_start = genesis_range_idx * DEFAULT_BLOCKS_PER_STATIC_FILE;
    let mut genesis_writer =
        static_file_provider.get_writer(genesis_range_start, StaticFileSegment::Headers)?;

    // Fill in all empty headers from genesis_range_start+1 to genesis_block_number-1
    for block_num in (genesis_range_start + 1)..genesis_block_number {
        let empty_header = create_empty_header::<H>(block_num);
        let empty_hash = empty_header.hash_slow();

        genesis_writer.append_header_with_td(&empty_header, U256::ZERO, &empty_hash)?;

        // Store the header number mapping in the database
        provider.tx_ref().put::<tables::HeaderNumbers>(empty_hash, block_num)?;
        // Store empty block body indices
        provider.tx_ref().put::<tables::BlockBodyIndices>(block_num, Default::default())?;

        // Log progress every 100,000 blocks
        if block_num % 100_000 == 0 || block_num == genesis_block_number - 1 {
            info!(
                target: "reth::cli",
                block_num,
                progress = format!("{:.1}%", ((block_num - genesis_range_start) as f64 / (genesis_block_number - genesis_range_start) as f64) * 100.0),
                "Filling genesis range with empty headers"
            );
        }
    }

    // Now insert the actual genesis header
    genesis_writer.append_header_with_td(genesis_header, U256::from(difficulty), genesis_hash)?;

    Ok(())
}

/// Initializes Transactions and Receipts static files.
/// For non-zero genesis blocks, creates all necessary static file ranges and fills them with empty blocks.
/// For zero genesis blocks, just sets the block range.
fn initialize_transactions_and_receipts_static_files(
    static_file_provider: &StaticFileProvider<impl NodePrimitives>,
    genesis_block_number: u64,
) -> Result<(), InitStorageError> {
    use reth_tracing::tracing::info;

    // For zero genesis blocks, just set the block range
    if genesis_block_number == 0 {
        let segment = StaticFileSegment::Receipts;
        static_file_provider
            .get_writer(genesis_block_number, segment)?
            .user_header_mut()
            .set_block_range(genesis_block_number, genesis_block_number);

        let segment = StaticFileSegment::Transactions;
        static_file_provider
            .get_writer(genesis_block_number, segment)?
            .user_header_mut()
            .set_block_range(genesis_block_number, genesis_block_number);

        return Ok(());
    }

    // For non-zero genesis blocks, create all necessary ranges
    // Calculate which range the genesis block is in
    let genesis_range_idx = genesis_block_number / DEFAULT_BLOCKS_PER_STATIC_FILE;

    // We need to create files for all ranges up to and including the genesis range
    let num_ranges_to_create = genesis_range_idx + 1;

    info!(
        target: "reth::cli",
        genesis_block_number,
        genesis_range_idx,
        num_ranges_to_create,
        "Creating static file ranges for Transactions and Receipts"
    );

    // Initialize Transactions segment
    initialize_segment_static_files(
        static_file_provider,
        StaticFileSegment::Transactions,
        num_ranges_to_create,
        genesis_range_idx,
        genesis_block_number,
    )?;

    // Initialize Receipts segment
    initialize_segment_static_files(
        static_file_provider,
        StaticFileSegment::Receipts,
        num_ranges_to_create,
        genesis_range_idx,
        genesis_block_number,
    )?;

    Ok(())
}

/// Initializes a single static file segment (Transactions or Receipts).
/// Creates all necessary static file ranges and fills the genesis range with empty blocks.
fn initialize_segment_static_files(
    static_file_provider: &StaticFileProvider<impl NodePrimitives>,
    segment: StaticFileSegment,
    num_ranges_to_create: u64,
    genesis_range_idx: u64,
    genesis_block_number: u64,
) -> Result<(), InitStorageError> {
    // Get segment name for logging
    let segment_name = match segment {
        StaticFileSegment::Transactions => "transactions",
        StaticFileSegment::Receipts => "receipts",
        _ => "segment",
    };

    // Initialize segment - one increment per range
    create_static_file_ranges(
        num_ranges_to_create,
        num_ranges_to_create - 1,
        segment_name,
        |range_idx, range_start, _range_end| {
            let mut writer = static_file_provider.get_writer(range_start, segment)?;

            // Set block range: block_end = range_start - 1, so next_block_number() = range_start
            if range_start > 0 {
                writer.user_header_mut().set_block_range(range_start, range_start - 1);
            }
            writer.increment_block(range_start)?;

            Ok((range_idx, range_start, segment))
        },
    )?;

    // Fill the genesis range with all missing blocks
    let genesis_range_start = genesis_range_idx * DEFAULT_BLOCKS_PER_STATIC_FILE;
    fill_static_file_range_with_empty_blocks(
        static_file_provider,
        segment,
        genesis_range_start,
        genesis_block_number,
    )?;

    info!(
        target: "reth::cli",
        segment = ?segment,
        total_ranges_created = num_ranges_to_create,
        "Finished creating static file ranges"
    );

    Ok(())
}

/// Fills a static file segment range with empty blocks using increment_block.
/// This is used for Transactions and Receipts segments to fill gaps before and including the genesis block.
fn fill_static_file_range_with_empty_blocks(
    static_file_provider: &StaticFileProvider<impl NodePrimitives>,
    segment: StaticFileSegment,
    genesis_range_start: u64,
    genesis_block_number: u64,
) -> Result<(), InitStorageError> {
    use reth_tracing::tracing::info;

    let mut writer = static_file_provider.get_writer(genesis_range_start, segment)?;

    // Increment for each block from genesis_range_start+1 to genesis_block_number (inclusive)
    for block_num in (genesis_range_start + 1)..=genesis_block_number {
        writer.increment_block(block_num)?;

        // Log progress every 100,000 blocks
        if block_num % 100_000 == 0 || block_num == genesis_block_number {
            info!(
                target: "reth::cli",
                block_num,
                segment = ?segment,
                progress = format!("{:.1}%", ((block_num - genesis_range_start) as f64 / (genesis_block_number - genesis_range_start) as f64) * 100.0),
                "Filling genesis range with empty blocks"
            );
        }
    }

    // Set the final block range to include the genesis block
    writer.user_header_mut().set_block_range(genesis_range_start, genesis_block_number);

    Ok(())
}

/// Custom insert_genesis_history that supports non-zero genesis block numbers.
/// This is copied from xlayer-old-reth's implementation which uses the actual genesis block number
/// instead of hardcoding 0.
fn insert_genesis_history_custom<'a, 'b, Provider>(
    provider: &Provider,
    alloc: impl Iterator<Item = (&'a alloy_primitives::Address, &'b alloy_genesis::GenesisAccount)>
        + Clone,
    genesis_block_number: u64,
) -> Result<(), InitStorageError>
where
    Provider: DBProvider<Tx: reth_db_api::transaction::DbTxMut> + HistoryWriter,
{
    use alloy_genesis::GenesisAccount;
    use alloy_primitives::Address;

    // Insert account history indices with the actual genesis block number
    let account_transitions =
        alloc.clone().map(|(addr, _): (&Address, &GenesisAccount)| (*addr, [genesis_block_number]));
    provider.insert_account_history_index(account_transitions)?;

    trace!(target: "reth::cli", "Inserted account history");

    // Insert storage history indices with the actual genesis block number
    let storage_transitions = alloc
        .filter_map(|(addr, account): (&Address, &GenesisAccount)| {
            account.storage.as_ref().map(|storage| (addr, storage))
        })
        .flat_map(|(addr, storage)| {
            storage.keys().map(move |key| ((*addr, *key), [genesis_block_number]))
        });
    provider.insert_storage_history_index(storage_transitions)?;

    trace!(target: "reth::cli", "Inserted storage history");

    Ok(())
}

/// Custom insert_genesis_state that supports non-zero genesis block numbers.
/// This is copied from xlayer-old-reth's implementation which uses the actual genesis block number
/// instead of hardcoding 0.
fn insert_genesis_state_custom<'a, 'b, Provider>(
    provider: &Provider,
    alloc: impl Iterator<Item = (&'a alloy_primitives::Address, &'b alloy_genesis::GenesisAccount)>,
    genesis_block_number: u64,
) -> Result<(), InitStorageError>
where
    Provider: StaticFileProviderFactory
        + DBProvider<Tx: reth_db_api::transaction::DbTxMut>
        + HeaderProvider
        + StateWriter
        + AsRef<Provider>,
{
    use alloy_primitives::{map::HashMap, U256};
    use reth_db_api::DatabaseError;
    use reth_primitives::{Account, Bytecode, StorageEntry};

    let capacity = alloc.size_hint().1.unwrap_or(0);
    let mut state_init: BundleStateInit =
        HashMap::with_capacity_and_hasher(capacity, Default::default());
    let mut reverts_init = HashMap::with_capacity_and_hasher(capacity, Default::default());
    let mut contracts: HashMap<B256, Bytecode> =
        HashMap::with_capacity_and_hasher(capacity, Default::default());

    for (address, account) in alloc {
        let bytecode_hash = if let Some(code) = &account.code {
            match Bytecode::new_raw_checked(code.clone()) {
                Ok(bytecode) => {
                    let hash = bytecode.hash_slow();
                    contracts.insert(hash, bytecode);
                    Some(hash)
                }
                Err(err) => {
                    tracing::error!(%address, %err, "Failed to decode genesis bytecode.");
                    return Err(DatabaseError::Other(err.to_string()).into());
                }
            }
        } else {
            None
        };

        // get state
        let storage = account
            .storage
            .as_ref()
            .map(|m| {
                m.iter()
                    .map(|(key, value)| {
                        let value = U256::from_be_bytes(value.0);
                        (*key, (U256::ZERO, value))
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        reverts_init.insert(
            *address,
            (Some(None), storage.keys().map(|k| StorageEntry::new(*k, U256::ZERO)).collect()),
        );

        state_init.insert(
            *address,
            (
                None,
                Some(Account {
                    nonce: account.nonce.unwrap_or_default(),
                    balance: account.balance,
                    bytecode_hash,
                }),
                storage,
            ),
        );
    }

    let all_reverts_init = HashMap::from_iter([(genesis_block_number, reverts_init)]);

    let execution_outcome = ExecutionOutcome::new_init(
        state_init,
        all_reverts_init,
        contracts,
        Vec::default(),
        genesis_block_number,
        Vec::new(),
    );

    provider.write_state(&execution_outcome, OriginalValuesKnown::Yes)?;

    trace!(target: "reth::cli", "Inserted state");

    Ok(())
}

/// Computes the state root (from scratch) based on the accounts and storages present in the
/// database.
///
/// This function is copied from reth_db_common::init::compute_state_root which is private.
/// It's needed to populate trie tables during genesis initialization.
fn compute_state_root<Provider>(
    provider: &Provider,
    prefix_sets: Option<reth_trie::prefix_set::TriePrefixSets>,
) -> Result<B256, InitStorageError>
where
    Provider: DBProvider + TrieWriter,
    <Provider as DBProvider>::Tx: reth_db_api::transaction::DbTxMut,
{
    trace!(target: "reth::cli", "Computing state root");

    let tx = provider.tx_ref();
    let mut intermediate_state: Option<IntermediateStateRootState> = None;
    let mut total_flushed_updates = 0;

    loop {
        let mut state_root =
            StateRootComputer::from_tx(tx).with_intermediate_state(intermediate_state);

        if let Some(sets) = prefix_sets.clone() {
            state_root = state_root.with_prefix_sets(sets);
        }

        match state_root.root_with_progress()? {
            StateRootProgress::Progress(state, _, updates) => {
                let updated_len = provider.write_trie_updates(updates)?;
                total_flushed_updates += updated_len;

                trace!(target: "reth::cli",
                    last_account_key = %state.account_root_state.last_hashed_key,
                    updated_len,
                    total_flushed_updates,
                    "Flushing trie updates"
                );

                intermediate_state = Some(*state);

                if total_flushed_updates.is_multiple_of(SOFT_LIMIT_COUNT_FLUSHED_UPDATES) {
                    info!(target: "reth::cli",
                        total_flushed_updates,
                        "Flushing trie updates"
                    );
                }
            }
            StateRootProgress::Complete(root, _, updates) => {
                let updated_len = provider.write_trie_updates(updates)?;
                total_flushed_updates += updated_len;

                trace!(target: "reth::cli",
                    %root,
                    updated_len,
                    total_flushed_updates,
                    "State root has been computed"
                );

                return Ok(root);
            }
        }
    }
}
