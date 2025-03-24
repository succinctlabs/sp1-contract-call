use alloy_primitives::{address, keccak256, Address, Bytes, B256};
use alloy_provider::RootProvider;
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use secp256k1::{generate_keypair, Message, PublicKey, SECP256K1};
use sp1_cc_client_executor::{ContractInput, ContractPublicValues};
use sp1_cc_host_executor::HostExecutor;
use sp1_sdk::{include_elf, utils, ProverClient, SP1Stdin};
use url::Url;
use SimpleStaking::verifySignedCall;

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

/// The ELF we want to execute inside the zkVM.
const ELF: &[u8] = include_elf!("verify-quorum-client");

/// The number of stakers.
const NUM_STAKERS: usize = 3;

/// The seed for the RNG.
///
/// Addresses corresponding to those generated by this seed were prepopulated with some stake on
/// Ethereum Sepolia. Changing this seed will result in different addresses, and almost certainly
/// 0 total stake.
const SEED: u64 = 12;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    // Setup logging.
    utils::setup_logger();

    // Which block transactions are executed on.
    let block_number = BlockNumberOrTag::Latest;

    // The testing rng we use to generate messages and secret keys.
    //
    // Note: this is deterministic based on the `SEED`.
    let mut test_rng = ChaCha20Rng::seed_from_u64(SEED);

    // Prepare the host executor.
    //
    // Use `ETH_SEPOLIA_RPC_URL` to get all of the necessary state for the smart contract call.
    let rpc_url = std::env::var("ETH_SEPOLIA_RPC_URL")
        .unwrap_or_else(|_| panic!("Missing ETH_SEPOLIA_RPC_URL in env"));
    let provider = RootProvider::new_http(Url::parse(&rpc_url)?);
    let mut host_executor = HostExecutor::new(provider.clone(), block_number).await?;

    // Keep track of the block hash. Later, validate the client's execution against this.
    let block_hash = host_executor.header.hash_slow();

    // Generate messages and signatures, with random (but deterministic) signing keys.
    let mut addresses = Vec::with_capacity(NUM_STAKERS);
    let mut signatures = Vec::with_capacity(NUM_STAKERS);
    let mut messages = Vec::with_capacity(NUM_STAKERS);

    for _ in 0..NUM_STAKERS {
        // Generate a random signing key and message, and sign the message with the key.
        let (sk, pk) = generate_keypair(&mut test_rng);
        let message = B256::random_with(&mut test_rng);
        let message_hash = alloy_primitives::keccak256(message);
        let signature = SECP256K1.sign_ecdsa_recoverable(&Message::from_digest(*message_hash), &sk);

        // Manually serialize the signature to match the EVM-compatible format
        let (id, r_and_s) = signature.serialize_compact();
        let mut signature_bytes = r_and_s.to_vec();
        signature_bytes.push((id.to_i32() as u8) + 27);
        let signature_bytes = Bytes::from(signature_bytes);

        // For transparency, print out the address corresponding to the public key of the signing
        // key.
        let address = public_key_to_address(pk);
        println!(
            "address: {}\nsignature: {}\nmessage: {}\n",
            address, signature_bytes, message_hash
        );

        messages.push(message_hash);
        signatures.push(signature_bytes);
        addresses.push(address);
    }

    // Set up the call to `verifySigned`.
    let verify_signed_call = ContractInput::new_call(
        CONTRACT,
        Address::default(),
        SimpleStaking::verifySignedCall {
            messageHashes: messages.clone(),
            signatures: signatures.clone(),
        },
    );

    // The host executes the call to `verifySigned`.
    let total_stake_bytes = host_executor.execute(verify_signed_call).await?;
    let total_stake = verifySignedCall::abi_decode_returns(&total_stake_bytes, true)?._0;
    println!("total_stake: {}", total_stake);

    // Now that we've executed the call, get the `EVMStateSketch` from the host executor.
    let input = host_executor.finalize().await?;

    // Feed the sketch into the client.
    let input_bytes = bincode::serialize(&input)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&input_bytes);

    // Additionally write the messages and signatures to stdin.
    stdin.write(&messages);
    stdin.write(&signatures);

    // Create a `ProverClient`.
    let client = ProverClient::from_env();

    // Execute the program using the `ProverClient.execute` method, without generating a proof.
    let (_, report) = client.execute(ELF, &stdin).run().unwrap();
    println!("executed program with {} cycles", report.total_instruction_count());

    // Generate the proof for the given program and input.
    let (pk, vk) = client.setup(ELF);
    let proof = client.prove(&pk, &stdin).run().unwrap();
    println!("generated proof");

    // Read the public values, and deserialize them.
    let public_vals = ContractPublicValues::abi_decode(proof.public_values.as_slice(), true)?;

    // Check that the provided block hash matches the one in the proof.
    assert_eq!(public_vals.blockHash, block_hash);
    println!("verified block hash");

    // Read the output, and then calculate the total stake associated with valid signatures.
    //
    // Note that this output is read from values commited to in the program using
    // `sp1_zkvm::io::commit`.
    let client_total_stake =
        verifySignedCall::abi_decode_returns(&public_vals.contractOutput, true)?._0;
    assert_eq!(client_total_stake, total_stake);
    println!("verified total stake calculation");

    // Verify proof and public values.
    client.verify(&proof, &vk).expect("verification failed");
    println!("successfully generated and verified proof for the program!");
    Ok(())
}

// Can't use `public_key_to_address()` from `reth_primitives` because Reth depends on
// `secp256k1` 0.30 while this crate is still on 0.29.
pub fn public_key_to_address(public: PublicKey) -> Address {
    // Strip out the first byte because that should be the SECP256K1_TAG_PUBKEY_UNCOMPRESSED
    // tag returned by libsecp's uncompressed pubkey serialization.
    let hash = keccak256(&public.serialize_uncompressed()[1..]);
    Address::from_slice(&hash[12..])
}
