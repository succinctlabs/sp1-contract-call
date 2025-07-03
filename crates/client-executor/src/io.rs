//! EVM sketch input structures and implementations.
//!
//! This module provides the [`EvmSketchInput`] struct, which contains all the necessary
//! information for executing Ethereum Virtual Machine (EVM) contracts in SP1. Instead of
//! passing the entire blockchain state, it includes only the required state roots, merkle
//! proofs, and specific storage slots that were accessed or modified during execution.
//!
//! The main purpose is to optimize contract execution by providing a minimal witness
//! that contains just the data needed to prove correct execution.

use std::{fmt::Debug, iter::once, sync::Arc};

use alloy_consensus::ReceiptEnvelope;
use alloy_evm::{Database, Evm};
use reth_chainspec::{ChainSpec, EthChainSpec};
use reth_consensus::{ConsensusError, HeaderValidator};
use reth_ethereum_consensus::EthBeaconConsensus;
use reth_evm::{ConfigureEvm, EthEvm, EvmEnv};
use reth_evm_ethereum::EthEvmConfig;
use reth_primitives::{EthPrimitives, Header, NodePrimitives, SealedHeader};
use revm::{
    context::result::{HaltReason, ResultAndState},
    inspector::NoOpInspector,
    state::Bytecode,
    Context, MainBuilder, MainContext,
};
use revm_primitives::{Address, B256, U256};
use rsp_client_executor::{error::ClientError, io::WitnessInput};
use rsp_mpt::EthereumState;
use rsp_primitives::genesis::Genesis;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{Anchor, ContractInput};

/// Information about how the contract executions accessed state, which is needed to execute the
/// contract in SP1.
///
/// Instead of passing in the entire state, only the state roots and merkle proofs
/// for the storage slots that were modified and accessed are passed in.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvmSketchInput {
    /// The current block anchor.
    pub anchor: Anchor,
    /// The genesis block specification.
    pub genesis: Genesis,
    /// The previous block headers starting from the most recent. These are used for calls to the
    /// blockhash opcode.
    #[serde_as(as = "Vec<alloy_consensus::serde_bincode_compat::Header>")]
    pub ancestor_headers: Vec<Header>,
    /// Current block's Ethereum state.
    pub state: EthereumState,
    /// Account bytecodes.
    pub bytecodes: Vec<Bytecode>,
    /// Receipts.
    #[serde_as(as = "Option<Vec<alloy_consensus::serde_bincode_compat::ReceiptEnvelope>>")]
    pub receipts: Option<Vec<ReceiptEnvelope>>,
}

impl WitnessInput for EvmSketchInput {
    #[inline(always)]
    fn state(&self) -> &EthereumState {
        &self.state
    }

    #[inline(always)]
    fn state_anchor(&self) -> B256 {
        self.anchor.header().state_root
    }

    #[inline(always)]
    fn state_requests(&self) -> impl Iterator<Item = (&Address, &Vec<U256>)> {
        // Workaround for https://github.com/rust-lang/rust/issues/36375
        if true {
            unimplemented!()
        } else {
            std::iter::empty()
        }
    }

    #[inline(always)]
    fn bytecodes(&self) -> impl Iterator<Item = &Bytecode> {
        self.bytecodes.iter()
    }

    #[inline(always)]
    fn sealed_headers(&self) -> impl Iterator<Item = SealedHeader> {
        once(SealedHeader::seal_slow(self.anchor.header().clone()))
            .chain(self.ancestor_headers.iter().map(|h| SealedHeader::seal_slow(h.clone())))
    }
}

pub trait Primitives: NodePrimitives {
    type ChainSpec: EthChainSpec + Debug;
    type HaltReason: Debug;

    fn build_spec(genesis: &Genesis) -> Result<Arc<Self::ChainSpec>, ClientError>;

    fn validate_header(
        header: &SealedHeader,
        chain_spec: Arc<Self::ChainSpec>,
    ) -> Result<(), ConsensusError>;

    fn transact<DB>(
        input: &ContractInput,
        db: DB,
        header: &Header,
        difficulty: U256,
        chain_spec: Arc<Self::ChainSpec>,
    ) -> Result<ResultAndState<Self::HaltReason>, String>
    where
        DB: Database;

    fn active_fork_name(chain_spec: &Self::ChainSpec, header: &Header) -> String;
}

impl Primitives for EthPrimitives {
    type ChainSpec = ChainSpec;
    type HaltReason = HaltReason;

    fn build_spec(genesis: &Genesis) -> Result<Arc<Self::ChainSpec>, ClientError> {
        Ok(Arc::new(ChainSpec::try_from(genesis).unwrap()))
    }

    fn validate_header(
        header: &SealedHeader,
        chain_spec: Arc<Self::ChainSpec>,
    ) -> Result<(), ConsensusError> {
        let validator = EthBeaconConsensus::new(chain_spec);
        validator.validate_header(header)
    }

    fn transact<DB: Database>(
        input: &ContractInput,
        db: DB,
        header: &Header,
        difficulty: U256,
        chain_spec: Arc<Self::ChainSpec>,
    ) -> Result<ResultAndState<Self::HaltReason>, String> {
        let EvmEnv { mut cfg_env, mut block_env, .. } =
            EthEvmConfig::new(chain_spec).evm_env(header);

        // Set the base fee to 0 to enable 0 gas price transactions.
        block_env.basefee = 0;
        block_env.difficulty = difficulty;
        cfg_env.disable_nonce_check = true;
        cfg_env.disable_balance_check = true;

        let evm = Context::mainnet()
            .with_db(db)
            .with_cfg(cfg_env)
            .with_block(block_env)
            .modify_tx_chained(|tx_env| {
                tx_env.gas_limit = header.gas_limit;
            })
            .build_mainnet_with_inspector(NoOpInspector {});

        let mut evm = EthEvm::new(evm, false);

        evm.transact(input).map_err(|err| err.to_string())
    }

    fn active_fork_name(chain_spec: &Self::ChainSpec, header: &Header) -> String {
        let spec = reth_evm_ethereum::revm_spec(chain_spec, header);

        spec.to_string()
    }
}

#[cfg(feature = "optimism")]
impl Primitives for reth_optimism_primitives::OpPrimitives {
    type ChainSpec = reth_optimism_chainspec::OpChainSpec;
    type HaltReason = op_revm::OpHaltReason;

    fn build_spec(genesis: &Genesis) -> Result<Arc<Self::ChainSpec>, ClientError> {
        Ok(Arc::new(reth_optimism_chainspec::OpChainSpec::try_from(genesis).unwrap()))
    }

    fn validate_header(
        header: &SealedHeader,
        chain_spec: Arc<Self::ChainSpec>,
    ) -> Result<(), ConsensusError> {
        let validator = reth_optimism_consensus::OpBeaconConsensus::new(chain_spec);
        validator.validate_header(header)
    }

    fn transact<DB: Database>(
        input: &ContractInput,
        db: DB,
        header: &Header,
        difficulty: U256,
        chain_spec: Arc<Self::ChainSpec>,
    ) -> Result<ResultAndState<Self::HaltReason>, String> {
        use op_revm::{DefaultOp, OpBuilder};

        let EvmEnv { mut cfg_env, mut block_env, .. } =
            reth_optimism_evm::OpEvmConfig::optimism(chain_spec).evm_env(header);

        // Set the base fee to 0 to enable 0 gas price transactions.
        block_env.basefee = 0;
        block_env.difficulty = difficulty;
        cfg_env.disable_nonce_check = true;
        cfg_env.disable_balance_check = true;

        let evm = op_revm::OpContext::op()
            .with_db(db)
            .with_cfg(cfg_env)
            .with_block(block_env)
            .modify_tx_chained(|tx_env| {
                tx_env.base.gas_limit = header.gas_limit;
            })
            .build_op_with_inspector(NoOpInspector {});

        let mut evm = alloy_op_evm::OpEvm::new(evm, false);

        evm.transact(input).map_err(|err| err.to_string())
    }

    fn active_fork_name(chain_spec: &Self::ChainSpec, header: &Header) -> String {
        let spec = reth_optimism_evm::revm_spec(chain_spec, header);
        let spec: &'static str = spec.into();

        spec.to_string()
    }
}
