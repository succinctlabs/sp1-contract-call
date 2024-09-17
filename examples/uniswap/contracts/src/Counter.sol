// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

// The public values outputted by
struct ContractPublicValues {
    address contractAddress;
    address callerAddress;
    bytes contractCallData;
    bytes contractOutput;
    bytes32 blockHash;
}

// @title SP1UniswapCC
// @notice An example application of the SP1 Contract Call library.
contract SP1UniswapCC {
    // @notice The SP1 verification key hash for the Uniswap contract call program.
    bytes32 public uniswapVkeyHash;
    // @notice The block hash we run the query at. 
    bytes32 public targetBlockHash;
    // @notice The SP1 verifier contract.
    ISP1Verifier public verifier;

    // @notice The constructor sets the program verification key, the initial block hash, the initial height, and the SP1 verifier.
    // @param _uniswapVkeyHash The verification key for the Uniswap Contract Call program.
    // @param _initialBlockHash The initial block hash.
    // @param _initialHeight The initial height.
    // @param _verifier The address of the SP1 verifier contract.
    constructor(
        bytes32 _uniswapVkeyHash,
        bytes32 _targetBlockHash,
        address _verifier
    ) {
        uniswapVkeyHash = _uniswapVkeyHash;
        targetBlockHash = _initialBlockHash;
        verifier = ISP1Verifier(_verifier);
    }

    // @notice Verify an SP1 Uniswap call proof.
    // @param proof The proof to verified. Should correspond to the supplied `publicValues`.
    // @param publicValues The public values to verify the proof against. The `publicValues` is the
    // ABI-encoded ContractPublicValues
    function verifyUniswapCallProof(
        bytes calldata proof,
        bytes calldata publicValues
    ) public {
        ContractPublicValues contractPublicValues = abi.decode(publicValues, ContractPublicValues);

        // Require that the block hash from the public values matches the target block hash. 
        require(contractPublicValues.blockHash == targetBlockHash);

        // Verify the proof with the associated public values.
        verifier.verifyProof(uniswapVkeyHash, publicValues, proof);

        // Now, you can do something with the contractOutput -- an abi encoded exchange rate. 

    }
}
