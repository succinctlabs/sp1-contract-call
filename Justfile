deploy-uniswap-example-contract rpc_url private_key:
  #!/usr/bin/env bash

  cd examples/uniswap/contracts
  forge create src/UniswapCall.sol:UniswapCall \
    --rpc-url {{rpc_url}} \
    --broadcast \
    --private-key {{private_key}} \
    --constructor-args "0x397A5f7f3dBd538f23DE225B51f532c34448dA9B" "0x00607e4512c634d557bb1f8f631d1299715483ec00fb8c29e633f78a97cb6763"
