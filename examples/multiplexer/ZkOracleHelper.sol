// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

//Helper contract to fetch collateral asset prices

interface IsfrxETH {
    function pricePerShare() external view returns (uint256);
}

interface IrETH {
    function getExchangeRate() external view returns (uint256);
}

interface IWStETH {
    function stEthPerToken() external view returns (uint256);
}

interface IcbETH {
    function exchangeRate() external view returns (uint256);
}

interface IankrETH {
    function sharesToBonds(uint256) external view returns (uint256);
}

interface IswETH {
    function swETHToETHRate() external view returns (uint256);
}

interface IethxOracle {
    function exchangeRate() external view returns (uint256 reportingBlockNumber, uint256 totalETHBalance, uint256 totalETHXSupply);
}

interface IApxETH {
    function assetsPerShare() external view returns (uint256);
}

interface IPufETH {
    function previewRedeem(uint256 amount) external view returns (uint256);
}

interface IRsETH {
    function rsETHPrice() external view returns (uint256);
}

interface ISUSDe {
    function previewRedeem(uint256 amount) external view returns (uint256);
}

interface IWeETH {
    function getRate() external view returns (uint256);
}

interface IEzETH {
    //TODO
}


contract ZkOracleHelper {

    address public constant ankrETH = 0xE95A203B1a91a908F9B9CE46459d101078c2c3cb;
    address public constant apxEth = 0x9Ba021B0a9b958B5E75cE9f6dff97C7eE52cb3E6;
    address public constant cbETH = 0xBe9895146f7AF43049ca1c1AE358B0541Ea49704;

    address public constant ethx = 0xA35b1B31Ce002FBF2058D22F30f95D405200A15b;
    address public constant ethxOracle = 0xF64bAe65f6f2a5277571143A24FaaFDFC0C2a737;

    address public constant ezEth = 0xbf5495Efe5DB9ce00f80364C8B423567e58d2110;
    address public constant pufEth = 0xD9A442856C234a39a81a089C06451EBAa4306a72;

    address public constant rETH = 0xae78736Cd615f374D3085123A210448E74Fc6393;

    address public constant rsEth = 0xA1290d69c65A6Fe4DF752f95823fae25cB99e5A7;
    address public constant rsEthOracle = 0x349A73444b1a310BAe67ef67973022020d70020d;

    address public constant sfrxETH = 0xac3E018457B222d93114458476f3E3416Abbe38F;
    address public constant sUSDe = 0x9D39A5DE30e57443BfF2A8307A4256c8797A3497;

    address public constant swETH = 0xf951E335afb289353dc249e82926178EaC7DEd78;
    address public constant weEth = 0xCd5fE23C85820F7B72D0926FC9b05b43E359b7ee;
    address public constant wstETH = 0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0;


    function getRate(address collateral) public view returns (uint256) {
        if (collateral == ankrETH) return IankrETH(ankrETH).sharesToBonds(1e18);
        if (collateral == apxEth) return IApxETH(apxEth).assetsPerShare();
        if (collateral == wstETH) return IWStETH(wstETH).stEthPerToken();
        if (collateral == cbETH) return IcbETH(cbETH).exchangeRate();
        if (collateral == ethx) {
            (, uint256 totalETHBalance, uint256 totalETHXSupply) = IethxOracle(ethxOracle).exchangeRate();
            return (totalETHBalance * 1e18) / totalETHXSupply;
        }
        //        if (collateral == ezEth) return 0; //TODO
        if (collateral == pufEth) return IPufETH(pufEth).previewRedeem(1e18);
        if (collateral == rETH) return IrETH(rETH).getExchangeRate();
        if (collateral == rsEth) return IRsETH(rsEthOracle).rsETHPrice();
        if (collateral == sfrxETH) return IsfrxETH(sfrxETH).pricePerShare();
        if (collateral == sUSDe) return ISUSDe(sUSDe).previewRedeem(1e18);
        if (collateral == swETH) return IswETH(swETH).swETHToETHRate();
        if (collateral == weEth) return IWeETH(weEth).getRate();
        else revert();
    }

    function getRates(address[] memory collaterals) external view returns (uint256[] memory) {
        uint256[] memory rates = new uint256[](collaterals.length);
        for (uint256 i = 0; i < collaterals.length; i++) {
            rates[i] = getRate(collaterals[i]);
        }
        return rates;
    }
}