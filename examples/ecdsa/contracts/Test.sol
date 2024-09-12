// SPDX-License-Identifier: MIT 
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";

contract MyTest is Test {
    uint private amount = 20;

    function setUp() external {}

    function testIs20() public {
        assertEq(amount, 20);
    }
}
