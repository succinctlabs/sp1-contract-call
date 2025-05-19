use alloy_primitives::{address, Address};
use alloy_sol_types::sol;

pub const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

sol! {
    interface IERC20 {
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}
