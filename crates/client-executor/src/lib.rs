//! # RSP Client Executor Lib
//!
//! This library provides the core functionality for executing smart contract calls within a
//! zero-knowledge virtual machine (zkVM) environment. It includes utilities for blockchain
//! state validation, EVM execution, and proof generation.
//!
//! ## Main Components
//!
//! - [`ClientExecutor`]: The primary executor for smart contract calls in zkVM
//! - [`ContractInput`]: Input specification for contract calls and creations
//! - [`ContractPublicValues`]: Public outputs that can be verified on-chain
//! - [`Anchor`]: Various blockchain anchoring mechanisms for state validation
//!
//! ## Features
//!
//! - Execute smart contracts with full EVM compatibility
//! - Validate blockchain state against Merkle proofs
//! - Support for multiple anchor types (block hash, EIP-4788, consensus)
//! - Log filtering and event decoding
//! - Zero-knowledge proof generation for contract execution

pub mod io;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use alloy_consensus::Header;
use alloy_eips::Encodable2718;
use alloy_evm::IntoTxEnv;
use alloy_primitives::{keccak256, Log};
use alloy_rpc_types::{Filter, FilteredParams};
use alloy_sol_types::{sol, SolCall, SolEvent, SolValue};
use alloy_trie::root::ordered_trie_root_with_encoder;
use eyre::bail;
use io::EvmSketchInput;
use reth_primitives::EthPrimitives;
use revm::{
    context::{result::ExecutionResult, TxEnv},
    database::CacheDB,
};
use revm_primitives::{Address, Bytes, TxKind, B256, U256};
use rsp_client_executor::io::{TrieDB, WitnessInput};

mod anchor;
pub use anchor::{
    get_beacon_root_from_state, rebuild_merkle_root, Anchor, BeaconAnchor, BeaconAnchorId,
    BeaconStateAnchor, BeaconWithHeaderAnchor, ChainedBeaconAnchor, HeaderAnchor,
    BLOCK_HASH_LEAF_INDEX, HISTORY_BUFFER_LENGTH, STATE_ROOT_LEAF_INDEX,
};

mod errors;
pub use errors::ClientError;

pub use rsp_primitives::genesis::Genesis;

use crate::{anchor::ResolvedAnchor, io::Primitives};

/// Input to a contract call.
///
/// Can be used to call an existing contract or create a new one. If used to create a new one,
#[derive(Debug, Clone)]
pub struct ContractInput {
    /// The address of the contract to call.
    pub contract_address: Address,
    /// The address of the caller.
    pub caller_address: Address,
    /// The calldata to pass to the contract.
    pub calldata: ContractCalldata,
}

/// The type of calldata to pass to a contract.
///
/// This enum is used to distinguish between contract calls and contract creations.
#[derive(Debug, Clone)]
pub enum ContractCalldata {
    Call(Bytes),
    Create(Bytes),
}

impl ContractCalldata {
    /// Encode the calldata as a bytes.
    pub fn to_bytes(&self) -> Bytes {
        match self {
            Self::Call(calldata) => calldata.clone(),
            Self::Create(calldata) => calldata.clone(),
        }
    }
}

impl ContractInput {
    /// Create a new contract call input.
    pub fn new_call<C: SolCall>(
        contract_address: Address,
        caller_address: Address,
        calldata: C,
    ) -> Self {
        Self {
            contract_address,
            caller_address,
            calldata: ContractCalldata::Call(calldata.abi_encode().into()),
        }
    }

    /// Creates a new contract creation input.
    ///
    /// To create a new contract, we send a transaction with TxKind Create to the
    /// zero address. As such, the contract address will be set to the zero address.
    pub fn new_create(caller_address: Address, calldata: Bytes) -> Self {
        Self {
            contract_address: Address::ZERO,
            caller_address,
            calldata: ContractCalldata::Create(calldata),
        }
    }
}

impl IntoTxEnv<TxEnv> for &ContractInput {
    fn into_tx_env(self) -> TxEnv {
        TxEnv {
            caller: self.caller_address,
            data: self.calldata.to_bytes(),
            // Set the gas price to 0 to avoid lack of funds (0) error.
            gas_price: 0,
            kind: match self.calldata {
                ContractCalldata::Create(_) => TxKind::Create,
                ContractCalldata::Call(_) => TxKind::Call(self.contract_address),
            },
            chain_id: None,
            ..Default::default()
        }
    }
}

#[cfg(feature = "optimism")]
impl IntoTxEnv<op_revm::OpTransaction<TxEnv>> for &ContractInput {
    fn into_tx_env(self) -> op_revm::OpTransaction<TxEnv> {
        op_revm::OpTransaction { base: self.into_tx_env(), ..Default::default() }
    }
}

sol! {
    #[derive(Debug)]
    enum AnchorType { BlockHash, Eip4788, Consensus }

    /// Public values of a contract call.
    ///
    /// These outputs can easily be abi-encoded, for use on-chain.
    #[derive(Debug)]
    struct ContractPublicValues {
        uint256 id;
        bytes32 anchorHash;
        AnchorType anchorType;
        bytes32 genesisHash;
        address callerAddress;
        address contractAddress;
        bytes contractCalldata;
        bytes contractOutput;
    }
}

impl ContractPublicValues {
    /// Construct a new [`ContractPublicValues`]
    ///
    /// By default, commit the contract input, the output, and the block hash to public values of
    /// the proof. More can be committed if necessary.
    pub fn new(
        call: ContractInput,
        output: Bytes,
        id: U256,
        anchor: B256,
        anchor_type: AnchorType,
        genesis_hash: B256,
    ) -> Self {
        Self {
            id,
            anchorHash: anchor,
            anchorType: anchor_type,
            genesisHash: genesis_hash,
            contractAddress: call.contract_address,
            callerAddress: call.caller_address,
            contractCalldata: call.calldata.to_bytes(),
            contractOutput: output,
        }
    }
}

/// An executor that executes smart contract calls inside a zkVM.
#[derive(Debug)]
pub struct ClientExecutor<'a, P: Primitives> {
    // The execution block header
    pub header: &'a Header,
    /// The block anchor.
    pub anchor: ResolvedAnchor,
    /// The chain specification.
    pub chain_spec: Arc<P::ChainSpec>,
    /// The database that the executor uses to access state.
    pub witness_db: TrieDB<'a>,
    /// All logs in the block.
    pub logs: Option<Vec<Log>>,
    /// The hashed genesis block specification.
    pub genesis_hash: B256,
}

impl<'a> ClientExecutor<'a, EthPrimitives> {
    /// Instantiates a new [`ClientExecutor`]
    pub fn eth(state_sketch: &'a EvmSketchInput) -> Result<Self, ClientError> {
        Self::new(state_sketch)
    }
}

#[cfg(feature = "optimism")]
impl<'a> ClientExecutor<'a, reth_optimism_primitives::OpPrimitives> {
    /// Instantiates a new [`ClientExecutor`]
    pub fn optimism(state_sketch: &'a EvmSketchInput) -> Result<Self, ClientError> {
        Self::new(state_sketch)
    }
}

impl<'a, P: Primitives> ClientExecutor<'a, P> {
    /// Instantiates a new [`ClientExecutor`]
    fn new(sketch_input: &'a EvmSketchInput) -> Result<Self, ClientError> {
        let chain_spec = P::build_spec(&sketch_input.genesis)?;
        let genesis_hash = hash_genesis(&sketch_input.genesis);
        let header = sketch_input.anchor.header();
        let sealed_headers = sketch_input.sealed_headers().collect::<Vec<_>>();

        P::validate_header(&sealed_headers[0], chain_spec.clone())
            .expect("the header is not valid");

        // Verify the state root
        assert_eq!(header.state_root, sketch_input.state.state_root(), "State root mismatch");

        // Verify that ancestors form a valid chain
        let mut previous_header = header;
        for ancestor in sealed_headers.iter().skip(1) {
            let ancestor_hash = ancestor.hash();

            P::validate_header(ancestor, chain_spec.clone())
                .unwrap_or_else(|_| panic!("the ancestor {} header in not valid", ancestor.number));
            assert_eq!(
                previous_header.parent_hash, ancestor_hash,
                "block {} is not the parent of {}",
                ancestor.number, previous_header.number
            );
            previous_header = ancestor;
        }

        let header = sketch_input.anchor.header();
        let anchor = sketch_input.anchor.resolve();

        if let Some(receipts) = &sketch_input.receipts {
            // verify the receipts root hash
            let root = ordered_trie_root_with_encoder(receipts, |r, out| r.encode_2718(out));
            assert_eq!(sketch_input.anchor.header().receipts_root, root, "Receipts root mismatch");
        }

        let logs = sketch_input
            .receipts
            .as_ref()
            .map(|receipts| receipts.iter().flat_map(|r| r.logs().to_vec()).collect());

        Ok(Self {
            header,
            anchor,
            chain_spec,
            witness_db: sketch_input.witness_db(&sealed_headers)?,
            logs,
            genesis_hash,
        })
    }

    /// Executes the smart contract call with the given [`ContractInput`] in SP1.
    ///
    /// Storage accesses are already validated against the `witness_db`'s state root.
    ///
    /// Note: It's the caller's responsability to commit the pubic values returned by
    /// this function. [`execute_and_commit`] can be used instead of this function
    /// to automatically commit if the execution is successful.
    ///
    /// [`execute_and_commit`]: ClientExecutor::execute_and_commit
    pub fn execute(&self, call: ContractInput) -> eyre::Result<ContractPublicValues> {
        let cache_db = CacheDB::new(&self.witness_db);
        let tx_output =
            P::transact(&call, cache_db, self.header, U256::ZERO, self.chain_spec.clone()).unwrap();

        let tx_output_bytes = match tx_output.result {
            ExecutionResult::Success { output, .. } => output.data().clone(),
            ExecutionResult::Revert { output, .. } => bail!("Execution reverted: {output}"),
            ExecutionResult::Halt { reason, .. } => bail!("Execution halted : {reason:?}"),
        };

        let public_values = ContractPublicValues::new(
            call,
            tx_output_bytes,
            self.anchor.id,
            self.anchor.hash,
            self.anchor.ty,
            self.genesis_hash,
        );

        Ok(public_values)
    }

    /// Executes the smart contract call with the given [`ContractInput`] in SP1
    /// and commit the result to the public values stream.
    ///
    /// Storage accesses are already validated against the `witness_db`'s state root.
    pub fn execute_and_commit(&self, call: ContractInput) -> eyre::Result<()> {
        let public_values = self.execute(call)?;
        sp1_zkvm::io::commit_slice(&public_values.abi_encode());

        Ok(())
    }

    /// Returns the decoded logs matching the provided `filter`.
    ///
    /// To be available in the client, the logs need to be prefetched in the host first.
    pub fn get_logs<E: SolEvent>(&self, filter: Filter) -> Result<Vec<Log<E>>, ClientError> {
        if let Some(logs) = &self.logs {
            let params = FilteredParams::new(Some(filter));

            logs.iter()
                .filter(|log| {
                    params.filter_address(&log.address) && params.filter_topics(log.topics())
                })
                .map(|log| E::decode_log(log))
                .collect::<Result<_, _>>()
                .map_err(Into::into)
        } else {
            Err(ClientError::LogsNotPrefetched)
        }
    }
}

pub fn hash_genesis(genesis: &Genesis) -> B256 {
    let mut s = DefaultHasher::new();
    genesis.hash(&mut s);
    let hash = s.finish();

    keccak256(hash.to_be_bytes())
}
