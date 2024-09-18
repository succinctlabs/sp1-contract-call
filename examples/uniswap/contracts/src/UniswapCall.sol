// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// The public values returned by the contract call execution.
struct ContractPublicValues {
    bytes32 blockHash;
    address callerAddress;
    address contractAddress;
    bytes contractCalldata;
    bytes contractOutput;
}

/// @title SP1 UniswapCall.
/// @notice This contract implements a simple example of verifying the proof of call to a smart 
///         contract.
contract UniswapCall {
    /// @notice The address of the SP1 verifier contract.
    address public verifier;

    /// @notice The verification key for the uniswapCall program.
    bytes32 public uniswapCallProgramVKey;

    constructor(address _verifier, bytes32 _uniswapCallProgramVKey) {
        verifier = _verifier;
        uniswapCallProgramVKey = _uniswapCallProgramVKey;
    }

    /// @notice The entrypoint for verifying the proof of a uniswapCall number.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyUniswapCallProof(bytes calldata _publicValues, bytes calldata _proofBytes)
        public
        view
        returns (uint160)
    {
        ISP1Verifier(verifier).verifyProof(uniswapCallProgramVKey, _publicValues, _proofBytes);
        ContractPublicValues memory publicValues = abi.decode(_publicValues, (ContractPublicValues));
        uint160 sqrtPriceX96 = abi.decode(publicValues.contractOutput, (uint160));
        return sqrtPriceX96;
    }
}