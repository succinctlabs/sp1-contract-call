use alloy_primitives::{address, Address};
use alloy_sol_macro::sol;
use alloy_sol_types::SolValue;
use sp1_cc_client_executor::{io::EvmSketchInput, ClientExecutor, ContractInput};

const CONTRACT: Address = address!("0x420000000000000000000000000000000000000F");

sol! {
    interface IGasPriceOracle {
        function gasPrice() external view returns (uint256);
    }
}

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let state_sketch = bincode::deserialize::<EvmSketchInput>(&state_sketch_bytes).unwrap();

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::optimism(&state_sketch).unwrap();

    // Execute the gasPrice call using the client executor.
    let call = ContractInput::new_call(CONTRACT, Address::default(), IGasPriceOracle::gasPriceCall);
    let public_vals = executor.execute(call).unwrap();

    // Commit the abi-encoded output.
    sp1_zkvm::io::commit_slice(&public_vals.abi_encode());
}
