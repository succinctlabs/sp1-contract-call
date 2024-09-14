// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";

contract SimpleStaking {
    using ECDSA for bytes32;

    mapping(address => uint256) public stakeWeight;

    function getStake(address addr) public view returns (uint256) {
        return stakeWeight[addr];
    }

    function update(address addr, uint256 weight) public {
        stakeWeight[addr] = weight;
    }

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
            // If the signature is invalid, we simply ignore it and move on
        }

        return totalStake;
    }
}