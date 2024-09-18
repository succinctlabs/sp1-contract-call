// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";

/// @title SimpleStaking
/// @notice This contract models a voting scheme, where each address has some stake. 
///         Eventually, when a vote is called, signatures are collected and the total stake 
///         corresponding to those signatures is returned.
contract SimpleStaking {
    using ECDSA for bytes32;

    mapping(address => uint256) public stakeWeight;

    /// @notice Returns the total stake of an address.
    function getStake(address addr) public view returns (uint256) {
        return stakeWeight[addr];
    }

    /// @notice Updates the stake of an address.
    function update(address addr, uint256 weight) public {
        stakeWeight[addr] = weight;
    }

    /// @notice Collects signatures over many messages, and returns the total stake corresponding
    ///         to those signatures. 
    ///
    ///         Calling this function onchain could be expensive with a large
    ///         number of signatures -- in that case, it would be better to prove its execution
    ///         with SP1.
    function verifySigned(
        bytes32[] memory messageHashes,
        bytes[] memory signatures
    ) public view returns (uint256) {
        require(
            messageHashes.length == signatures.length,
            "Input arrays must have the same length"
        );

        uint256 totalStake = 0;

        for (uint i = 0; i < messageHashes.length; i++) {
            address recoveredSigner = messageHashes[i].recover(signatures[i]);
            totalStake += stakeWeight[recoveredSigner];
        }

        return totalStake;
    }
}