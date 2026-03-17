// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/FAXAnchor.sol";
import "../src/FAXEscrow.sol";
import "../src/FAXReputation.sol";

contract DeployFAX is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        address arbitrator = vm.envAddress("ARBITRATOR_ADDRESS");

        vm.startBroadcast(deployerPrivateKey);

        FAXAnchor anchor = new FAXAnchor();
        FAXReputation reputation = new FAXReputation();
        FAXEscrow escrow = new FAXEscrow(arbitrator);

        escrow.setReputationRegistry(address(reputation));
        reputation.setEscrowContract(address(escrow));

        vm.stopBroadcast();

        console.log("FAXAnchor:", address(anchor));
        console.log("FAXEscrow:", address(escrow));
        console.log("FAXReputation:", address(reputation));
    }
}
