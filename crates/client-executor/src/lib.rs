pub mod io;
use alloy_sol_types::{sol, SolCall};
use eyre::OptionExt;
use io::EVMStateSketch;
use reth_evm::ConfigureEvmEnv;
use reth_evm_ethereum::EthEvmConfig;
use reth_primitives::Header;
use revm::{db::CacheDB, Database, Evm, EvmBuilder, State};
use revm_primitives::{Address, BlockEnv, Bytes, CfgEnvWithHandlerCfg, SpecId, TxKind, B256, U256};
use rsp_client_executor::io::WitnessInput;
use rsp_witness_db::WitnessDb;

/// Input to a contract call.
#[derive(Debug, Clone)]
pub struct ContractInput<C: SolCall> {
    /// The address of the contract to call.
    pub contract_address: Address,
    /// The address of the caller.
    pub caller_address: Address,
    /// The calldata to pass to the contract.
    pub calldata: C,
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
    pub fn new<C: SolCall>(call: ContractInput<C>, output: Bytes, block_hash: B256) -> Self {
        Self {
            contractAddress: call.contract_address,
            callerAddress: call.caller_address,
            contractCalldata: call.calldata.abi_encode().into(),
            contractOutput: output,
            blockHash: block_hash,
        }
    }
}

/// An executor that executes smart contract calls inside a zkVM.
#[derive(Debug)]
pub struct ClientExecutor {
    /// The database that the executor uses to access state.
    pub witness_db: WitnessDb,
    /// The block header.
    pub header: Header,
}

impl ClientExecutor {
    /// Instantiates a new [`ClientExecutor`]
    pub fn new(state_sketch: EVMStateSketch) -> eyre::Result<Self> {
        // let header = state_sketch.header.clone();
        Ok(Self { witness_db: state_sketch.witness_db().unwrap(), header: state_sketch.header })
    }

    /// Executes the smart contract call with the given [`ContractInput`] in SP1.
    ///
    /// Storage accesses are already validated against the `witness_db`'s state root.
    pub fn execute<C: SolCall>(
        &self,
        call: ContractInput<C>,
    ) -> eyre::Result<ContractPublicValues> {
        let cache_db = CacheDB::new(&self.witness_db);
        let mut evm = new_evm(cache_db, &self.header, U256::ZERO, &call);
        let tx_output = evm.transact()?;
        let tx_output_bytes = tx_output.result.output().ok_or_eyre("Error decoding result")?;
        Ok(ContractPublicValues::new::<C>(call, tx_output_bytes.clone(), self.header.hash_slow()))
    }
}

/// TODO Add support for other chains besides Ethereum Mainnet.
/// Instantiates a new EVM, which is ready to run `call`.
pub fn new_evm<'a, D, C>(
    db: D,
    header: &Header,
    total_difficulty: U256,
    call: &ContractInput<C>,
) -> Evm<'a, (), State<D>>
where
    D: Database,
    C: SolCall,
{
    let mut cfg_env = CfgEnvWithHandlerCfg::new_with_spec_id(Default::default(), SpecId::LATEST);
    let mut block_env = BlockEnv::default();

    EthEvmConfig::default().fill_cfg_and_block_env(
        &mut cfg_env,
        &mut block_env,
        &rsp_primitives::chain_spec::mainnet(),
        header,
        total_difficulty,
    );
    // Set the base fee to 0 to enable 0 gas price transactions.
    block_env.basefee = U256::from(0);

    let state = State::builder().with_database(db).build();

    let mut evm = EvmBuilder::default()
        .with_db(state)
        .with_cfg_env_with_handler_cfg(cfg_env)
        .modify_block_env(|evm_block_env| *evm_block_env = block_env)
        .build();

    let tx_env = evm.tx_mut();
    tx_env.caller = call.caller_address;
    tx_env.data = call.calldata.abi_encode().into();
    tx_env.gas_limit = header.gas_limit;
    // Set the gas price to 0 to avoid lack of funds (0) error.
    tx_env.gas_price = U256::from(0);
    tx_env.transact_to = TxKind::Call(call.contract_address);

    evm
}
