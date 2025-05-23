pub mod io;
use std::sync::Arc;

use alloy_eips::Encodable2718;
use alloy_evm::{Database, Evm, IntoTxEnv};
use alloy_primitives::Log;
use alloy_rpc_types::{Filter, FilteredParams};
use alloy_sol_types::{sol, SolCall, SolEvent};
use alloy_trie::root::ordered_trie_root_with_encoder;
use eyre::OptionExt;
use io::EvmSketchInput;
use reth_chainspec::ChainSpec;
use reth_consensus::HeaderValidator;
use reth_ethereum_consensus::EthBeaconConsensus;
use reth_evm::{ConfigureEvm, EvmEnv};
use reth_evm_ethereum::{EthEvm, EthEvmConfig};
use reth_primitives::{Header, SealedHeader};
use revm::{
    context::TxEnv, database::CacheDB, inspector::NoOpInspector, Context, MainBuilder, MainContext,
};
use revm_primitives::{Address, Bytes, TxKind, B256, U256};
use rsp_client_executor::io::{TrieDB, WitnessInput};

mod anchor;
pub use anchor::{
    get_beacon_root_from_state, rebuild_merkle_root, Anchor, BeaconAnchor, BeaconAnchorId,
    BeaconBlockField, BeaconStateAnchor, BeaconWithHeaderAnchor, ChainedBeaconAnchor, HeaderAnchor,
    HISTORY_BUFFER_LENGTH,
};

mod errors;
pub use errors::ClientError;

pub use rsp_primitives::genesis::Genesis;

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
    ) -> Self {
        Self {
            id,
            anchorHash: anchor,
            anchorType: anchor_type,
            contractAddress: call.contract_address,
            callerAddress: call.caller_address,
            contractCalldata: call.calldata.to_bytes(),
            contractOutput: output,
        }
    }
}

/// An executor that executes smart contract calls inside a zkVM.
#[derive(Debug)]
pub struct ClientExecutor<'a> {
    /// The block anchor.
    pub anchor: &'a Anchor,
    /// The genesis block specification.
    pub chain_spec: Arc<ChainSpec>,
    /// The database that the executor uses to access state.
    pub witness_db: TrieDB<'a>,
    /// All logs in the block.
    pub logs: Vec<Log>,
}

impl<'a> ClientExecutor<'a> {
    /// Instantiates a new [`ClientExecutor`]
    pub fn new(state_sketch: &'a EvmSketchInput) -> Result<Self, ClientError> {
        let chain_spec = Arc::new(ChainSpec::try_from(&state_sketch.genesis).unwrap());
        let validator = EthBeaconConsensus::new(chain_spec.clone());
        let header = state_sketch.anchor.header();

        validator
            .validate_header(&SealedHeader::new_unhashed(header.clone()))
            .expect("the header in not valid");

        assert_eq!(header.state_root, state_sketch.state.state_root(), "State root mismatch");

        // verify that ancestors form a valid chain
        let mut previous_header = header;
        for ancestor in &state_sketch.ancestor_headers {
            let ancestor_hash = ancestor.hash_slow();

            validator
                .validate_header(&SealedHeader::new_unhashed(ancestor.clone()))
                .unwrap_or_else(|_| panic!("the ancestor {} header in not valid", ancestor.number));
            assert_eq!(
                previous_header.parent_hash, ancestor_hash,
                "block {} is not the parent of {}",
                ancestor.number, previous_header.number
            );
            previous_header = ancestor;
        }

        if let Some(receipts) = &state_sketch.receipts {
            // verify the receipts root hash
            let root = ordered_trie_root_with_encoder(receipts, |r, out| r.encode_2718(out));
            assert_eq!(state_sketch.anchor.header().receipts_root, root, "Receipts root mismatch");
        }

        let logs = state_sketch
            .receipts
            .as_ref()
            .unwrap_or(&vec![])
            .iter()
            .flat_map(|r| r.logs().to_vec())
            .collect();

        Ok(Self {
            anchor: &state_sketch.anchor,
            chain_spec,
            witness_db: state_sketch.witness_db()?,
            logs,
        })
    }

    /// Executes the smart contract call with the given [`ContractInput`] in SP1.
    ///
    /// Storage accesses are already validated against the `witness_db`'s state root.
    pub fn execute(&self, call: ContractInput) -> eyre::Result<ContractPublicValues> {
        let cache_db = CacheDB::new(&self.witness_db);
        let mut evm = new_evm(cache_db, self.anchor.header(), U256::ZERO, self.chain_spec.clone());
        let tx_output = evm.transact(&call)?;
        let tx_output_bytes = tx_output.result.output().ok_or_eyre("Error decoding result")?;
        let resolved = self.anchor.resolve();

        let public_values = ContractPublicValues::new(
            call,
            tx_output_bytes.clone(),
            resolved.id,
            resolved.hash,
            self.anchor.ty(),
        );

        Ok(public_values)
    }

    /// Returns the decoded logs matching the provided `filter`.
    ///
    /// To be avaliable in the client, the logs need to be prefetched in the host first.
    pub fn get_logs<E: SolEvent>(&self, filter: Filter) -> Result<Vec<Log<E>>, ClientError> {
        let params = FilteredParams::new(Some(filter));

        self.logs
            .iter()
            .filter(|log| params.filter_address(&log.address) && params.filter_topics(log.topics()))
            .map(|log| E::decode_log(log))
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }
}

/// Instantiates a new EVM, which is ready to run `call`.
pub fn new_evm<DB>(
    db: DB,
    header: &Header,
    difficulty: U256,
    chain_spec: Arc<ChainSpec>,
) -> EthEvm<DB, NoOpInspector>
where
    DB: Database,
{
    let EvmEnv { cfg_env, mut block_env, .. } = EthEvmConfig::new(chain_spec).evm_env(header);

    // Set the base fee to 0 to enable 0 gas price transactions.
    block_env.basefee = 0;
    block_env.difficulty = difficulty;

    let evm = Context::mainnet()
        .with_db(db)
        .with_cfg(cfg_env)
        .with_block(block_env)
        .modify_tx_chained(|tx_env| {
            tx_env.gas_limit = header.gas_limit;
        })
        .build_mainnet_with_inspector(NoOpInspector {});

    EthEvm::new(evm, false)
}
