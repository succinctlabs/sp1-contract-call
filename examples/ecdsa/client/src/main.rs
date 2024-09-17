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

    // The testing rng we use to generate messages and secret keys.
    //
    // Note: this is deterministic based on the `SEED`, so the host and the client have the same
    // behavior.
    let mut test_rng = ChaCha20Rng::seed_from_u64(SEED);

    // Commit the sketch's state root.
    let state_root = state_sketch.header.state_root;
    sp1_zkvm::io::commit(&state_root);

    // Generate messages and signatures, with random (but deterministic) signing keys.
    let mut signatures = Vec::with_capacity(NUM_STAKERS);
    let mut messages = Vec::with_capacity(NUM_STAKERS);

    for _ in 0..NUM_STAKERS {
        // Generate a random signing key and message, and sign the message with the key.
        let (sk, _pk) = generate_keypair(&mut test_rng);
        let message = B256::random_with(&mut test_rng);
        let message_hash = alloy_primitives::keccak256(message);
        let signature = SECP256K1.sign_ecdsa_recoverable(&Message::from_digest(*message_hash), &sk);

        // Manually serialize the signature to match the EVM-compatible format
        let (id, r_and_s) = signature.serialize_compact();
        let mut signature_bytes = r_and_s.to_vec();
        signature_bytes.push((id.to_i32() as u8) + 27);

        let signature_bytes = Bytes::from(signature_bytes);

        messages.push(message_hash);
        signatures.push(signature_bytes);
    }

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
