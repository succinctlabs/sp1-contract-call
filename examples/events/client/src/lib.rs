use alloy_sol_types::sol;

sol! {
    interface IERC20 {
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}
