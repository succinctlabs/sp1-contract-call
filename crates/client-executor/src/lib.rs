pub mod io;
use std::sync::Arc;

use alloy_sol_types::{sol, SolCall};
use eyre::OptionExt;
use io::EVMStateSketch;
use reth_evm::{ConfigureEvmEnv, EvmEnv};
use reth_evm_ethereum::EthEvmConfig;
use reth_primitives::Header;
use revm::{db::CacheDB, Database, Evm, EvmBuilder, State};
use revm_primitives::{Address, Bytes, CfgEnvWithHandlerCfg, SpecId, TxKind, B256, U256};
use rsp_client_executor::io::{TrieDB, WitnessInput};

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

sol! {
    /// Public values of a contract call.
    ///
    /// These outputs can easily be abi-encoded, for use on-chain.
    struct ContractPublicValues {
        bytes32 blockHash;
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
    pub fn new(call: ContractInput, output: Bytes, block_hash: B256) -> Self {
        Self {
            contractAddress: call.contract_address,
            callerAddress: call.caller_address,
            contractCalldata: call.calldata.to_bytes(),
            contractOutput: output,
            blockHash: block_hash,
        }
    }
}

/// An executor that executes smart contract calls inside a zkVM.
#[derive(Debug)]
pub struct ClientExecutor<'a> {
    /// The database that the executor uses to access state.
    pub witness_db: TrieDB<'a>,
    /// The block header.
    pub header: &'a Header,
}

impl<'a> ClientExecutor<'a> {
    /// Instantiates a new [`ClientExecutor`]
    pub fn new(state_sketch: &'a EVMStateSketch) -> eyre::Result<Self> {
        // let header = state_sketch.header.clone();
        Ok(Self { witness_db: state_sketch.witness_db().unwrap(), header: &state_sketch.header })
    }

    /// Executes the smart contract call with the given [`ContractInput`] in SP1.
    ///
    /// Storage accesses are already validated against the `witness_db`'s state root.
    pub fn execute(&self, call: ContractInput) -> eyre::Result<ContractPublicValues> {
        let cache_db = CacheDB::new(&self.witness_db);
        let mut evm = new_evm(cache_db, self.header, U256::ZERO, &call);
        let tx_output = evm.transact()?;
        let tx_output_bytes = tx_output.result.output().ok_or_eyre("Error decoding result")?;
        Ok(ContractPublicValues::new(call, tx_output_bytes.clone(), self.header.hash_slow()))
    }
}

/// TODO Add support for other chains besides Ethereum Mainnet.
/// Instantiates a new EVM, which is ready to run `call`.
pub fn new_evm<'a, D>(
    db: D,
    header: &Header,
    total_difficulty: U256,
    call: &ContractInput,
) -> Evm<'a, (), State<D>>
where
    D: Database,
{
    let chain_spec = Arc::new(rsp_primitives::chain_spec::mainnet().unwrap());

    let EvmEnv { cfg_env, mut block_env, .. } = EthEvmConfig::new(chain_spec).evm_env(header);

    // Set the base fee to 0 to enable 0 gas price transactions.
    block_env.basefee = U256::from(0);
    block_env.difficulty = total_difficulty;

    let state = State::builder().with_database(db).build();

    let mut evm = EvmBuilder::default()
        .with_db(state)
        .with_cfg_env_with_handler_cfg(CfgEnvWithHandlerCfg::new_with_spec_id(
            cfg_env,
            SpecId::LATEST,
        ))
        .modify_block_env(|evm_block_env| *evm_block_env = block_env)
        .build();

    let tx_env = evm.tx_mut();
    tx_env.caller = call.caller_address;
    tx_env.data = call.calldata.to_bytes();
    tx_env.gas_limit = header.gas_limit;
    // Set the gas price to 0 to avoid lack of funds (0) error.
    tx_env.gas_price = U256::from(0);
    tx_env.transact_to = match call.calldata {
        ContractCalldata::Create(_) => TxKind::Create,
        ContractCalldata::Call(_) => TxKind::Call(call.contract_address),
    };
    evm
}
