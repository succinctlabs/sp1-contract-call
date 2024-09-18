#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_primitives::{address, Address, Bytes, B256};
use alloy_sol_macro::sol;
use alloy_sol_types::SolValue;
use bincode;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use secp256k1::{generate_keypair, Message, SECP256K1};
use sp1_cc_client_executor::{io::EVMStateSketch, ClientExecutor, ContractInput};

sol! {
    /// Part of the SimpleStaking interface
    interface SimpleStaking {
        function getStake(address addr) public view returns (uint256);
        function update(address addr, uint256 weight) public;
        function verifySigned(bytes32[] memory messageHashes, bytes[] memory signatures) public view returns (uint256);
    }
}

/// Address of the SimpleStaking contract on Ethereum Sepolia.
const CONTRACT: Address = address!("C82bbB1719271318282fe332795935f39B89b5cf");

/// Address of the caller.
const CALLER: Address = address!("0000000000000000000000000000000000000000");

/// The number of stakers.
const NUM_STAKERS: usize = 3;

/// The seed for the RNG. Needs to be set in order for client and host to be consistent.
const SEED: u64 = 12;

pub fn main() {
    // Read the state sketch from stdin. Use this during the execution in order to
    // access Ethereum state.
    let state_sketch_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let state_sketch = bincode::deserialize::<EVMStateSketch>(&state_sketch_bytes).unwrap();

    // Read messages and signatures from stdin.
    let mut messages = sp1_zkvm::io::read::<Vec<B256>>();
    let mut signatures = sp1_zkvm::io::read::<Vec<Bytes>>();

    // Initialize the client executor with the state sketch.
    // This step also validates all of the storage against the provided state root.
    let executor = ClientExecutor::new(state_sketch).unwrap();

    // Set up the call to `verifySigned`.
    let verify_signed_call = ContractInput {
        contract_address: CONTRACT,
        caller_address: CALLER,
        calldata: SimpleStaking::verifySignedCall { messageHashes: messages, signatures },
    };

    // Execute the call.
    let total_stake = executor.execute(verify_signed_call).unwrap();

    // Commit the result.
    sp1_zkvm::io::commit(&total_stake.abi_encode());
}
