// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {SP1UniswapCC} from "../src/Counter.sol";

contract CounterTest is Test {
    SP1UniswapCC public uniswapCC;

    function setUp() public {
        uniswapCC = new SP1UniswapCC(0x00308765c1988bfc493c6151db54c02172a76ecf0372acf912ef4e70a0a06e42,
            0x0000000000000000000000000000000000000000000000000000000000000000,
            "0xaeE21CeadF7A03b3034DAE4f190bFE5F861b6ebf"
        );

    }

    function test_Increment() public {
        counter.increment();
        assertEq(counter.number(), 1);
    }
}
